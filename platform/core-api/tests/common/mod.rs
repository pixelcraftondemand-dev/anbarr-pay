//! Shared harness for core-api integration tests: a real ledger gRPC server
//! runs in-process (real engine, real Postgres in a per-binary schema) and
//! the core-api router is exercised over axum's test transport — the same
//! contract the deployed services use, with no mocks on the money path.
//!
//! Each test binary passes its own ledger schema + core database name
//! (`harness("anbarr_core_test_<binary>")`), so parallel binaries never step
//! on each other's state — the core-side analogue of the ledger tests'
//! per-binary schemas.
//!
//! Requires the dev database (docker-compose.yml).

use anbarr_core_api::ledger::pb::ledger_server::LedgerServer;
use anbarr_core_api::{create_app, AppState, LedgerClient};
use anbarr_ledger::db;
use anbarr_ledger::engine::LedgerEngine;
use anbarr_ledger::grpc::LedgerGrpc;
use axum::body::Body;
use axum::http::{Request, StatusCode};
use serde_json::{json, Value};
use sqlx::postgres::PgPoolOptions;
use sqlx::PgPool;
use tower::ServiceExt; // for `oneshot`
use uuid::Uuid;

pub struct TestHarness {
    pub app: axum::Router,
    /// The ledger-side pool (for seeding accounts + funds directly).
    /// Not every test binary seeds the ledger directly — hence the allow.
    #[allow(dead_code)]
    pub ledger_pool: PgPool,
    /// The core-side pool (users, sessions, goals, wallet links).
    #[allow(dead_code)]
    pub core_pool: PgPool,
}

/// Bind an ephemeral port and hand it to the caller BEFORE the async server
/// binds it, so tests know the address without tonic exposing it. The std
/// listener is dropped immediately; a tiny race remains, so the readiness
/// loop below retries.
fn ephemeral_addr() -> std::net::SocketAddr {
    let listener = std::net::TcpListener::bind("127.0.0.1:0").expect("bind probe");
    let addr = listener.local_addr().expect("addr");
    drop(listener);
    addr
}

/// Ledger-side harness: per-binary schema, migrations, fresh tables,
/// in-process gRPC server, authenticated client.
async fn ledger_stack(schema: &str) -> (PgPool, LedgerClient) {
    // Hermetic auth: the ledger's interceptor reads LEDGER_GRPC_TOKEN from
    // the environment (deployment: secrets manager). Pin it to the value the
    // test client presents so tests never depend on the developer's shell.
    std::env::set_var("LEDGER_GRPC_TOKEN", "test-token");

    let url = db::database_url_from_env();
    let admin = PgPoolOptions::new()
        .max_connections(1)
        .connect(&url)
        .await
        .expect("connect to test postgres (run: docker compose up -d)");
    // Fresh schema every run: leftover tables would leak state across
    // cargo test invocations (same for the core DB below).
    sqlx::query(&format!("DROP SCHEMA IF EXISTS {schema} CASCADE"))
        .execute(&admin)
        .await
        .expect("drop schema");
    sqlx::query(&format!("CREATE SCHEMA {schema}"))
        .execute(&admin)
        .await
        .expect("create schema");
    admin.close().await;

    let sep = if url.contains('?') { "&" } else { "?" };
    let schema_url = format!("{url}{sep}options=-csearch_path%3D{schema}");
    let pool = PgPoolOptions::new()
        .max_connections(10)
        .connect(&schema_url)
        .await
        .expect("connect ledger pool");
    anbarr_ledger::db::run_migrations(&pool)
        .await
        .expect("ledger migrations");

    // Fresh ledger state (platform accounts are preserved, as in ledger tests).
    sqlx::query(
        "TRUNCATE ledger_events, wallet_snapshots, holds, idempotency_keys, journals, reconciliation_runs CASCADE",
    )
    .execute(&pool)
    .await
    .expect("truncate ledger tables");
    sqlx::query("DELETE FROM accounts WHERE owner_type <> 'platform'")
        .execute(&pool)
        .await
        .expect("clear wallets");

    // In-process ledger gRPC server on an ephemeral port. The JoinHandle
    // stays alive (unnamed) for the test's lifetime; the task ends when the
    // runtime shuts down.
    let addr = ephemeral_addr();
    let engine = LedgerEngine::new(pool.clone());
    let _server = tokio::spawn(async move {
        tonic::transport::Server::builder()
            .add_service(LedgerServer::new(LedgerGrpc::new(engine)))
            .serve(addr)
            .await
            .expect("ledger server runs");
    });

    let addr_uri = format!("http://{addr}");
    // Readiness loop: retry until the server accepts (or fail loudly with
    // the last health-check error for debuggability).
    let client = {
        let mut last_err = String::from("never probed");
        let mut found = None;
        for _ in 0..150 {
            let channel = tonic::transport::Channel::from_shared(addr_uri.clone())
                .expect("uri")
                .connect_lazy();
            let inner = anbarr_ledger::grpc_proto::ledger_client::LedgerClient::new(channel);
            let candidate = LedgerClient::new(inner, "test-token".to_string());
            match candidate
                .health_check(std::time::Duration::from_millis(100))
                .await
            {
                Ok(()) => {
                    found = Some(candidate);
                    break;
                }
                Err(e) => {
                    last_err = e;
                    tokio::time::sleep(std::time::Duration::from_millis(20)).await;
                }
            }
        }
        found.unwrap_or_else(|| panic!("ledger gRPC never became ready: {last_err}"))
    };

    (pool, client)
    // `server` task keeps running for the test's lifetime; dropped at end.
}

/// Core-side harness: a dedicated core database per binary (the real
/// boundary — this is the database the Core API owns).
async fn core_pool(db_name: &str) -> PgPool {
    let admin = PgPoolOptions::new()
        .max_connections(1)
        .connect("postgres://anbarr:anbarr_dev@localhost:5433/postgres?sslmode=disable")
        .await
        .expect("connect admin postgres");
    // Postgres has no CREATE DATABASE IF NOT EXISTS; ignore the duplicate
    // error (42P04) — the database existing is the desired state.
    // Fresh database every run (drop first — stale rows would otherwise
    // leak into assertions across `cargo test` invocations).
    let _ = sqlx::query(&format!("DROP DATABASE IF EXISTS {db_name} WITH (FORCE)"))
        .execute(&admin)
        .await;
    sqlx::query(&format!("CREATE DATABASE {db_name}"))
        .execute(&admin)
        .await
        .expect("create core test database");
    admin.close().await;

    let url = format!("postgres://anbarr:anbarr_dev@localhost:5433/{db_name}?sslmode=disable");
    let pool = PgPoolOptions::new()
        .max_connections(5)
        .connect(&url)
        .await
        .expect("connect core test db");
    anbarr_core_api::db::run_migrations(&pool)
        .await
        .expect("core migrations");
    sqlx::query(
        "TRUNCATE users, sessions, otp_codes, wallet_links, vault_goals, vault_goal_locks CASCADE",
    )
    .execute(&pool)
    .await
    .expect("truncate core tables");
    pool
}

/// Full stack: ledger gRPC + core DB + router, **fully isolated per test**:
/// each test passes a unique `tag` and gets its own core database
/// (`anbarr_core_test_<tag>`) and ledger schema (`test_core_api_<tag>`), so
/// parallel tests never share or truncate each other's state — the same
/// isolation philosophy as the ledger tests' per-binary schemas.
///
/// `ANBARR_DEV_OTP_ECHO=1` is set process-wide so `sign_in` can read the code
/// from the response (dev-only channel — production must never echo codes).
pub async fn harness(tag: &str) -> TestHarness {
    use std::sync::atomic::{AtomicU32, Ordering};
    static N: AtomicU32 = AtomicU32::new(0);
    let n = N.fetch_add(1, Ordering::SeqCst);
    // Unique-per-call names: parallel tests never share a database or schema.
    let unique = format!("{tag}_{n}");
    std::env::set_var("ANBARR_DEV_OTP_ECHO", "1");
    let (ledger_pool, ledger) = ledger_stack(&format!("test_core_api_{unique}")).await;
    let core = core_pool(&format!("anbarr_core_test_{unique}")).await;
    let state = AppState::new(
        core.clone(),
        ledger,
        reqwest::Client::new(),
        "http://127.0.0.1:11434".to_string(),
        "llama3.2:latest".to_string(),
    );
    TestHarness {
        app: create_app(state),
        ledger_pool,
        core_pool: core,
    }
}

/// Issue an access token by walking the real flow: OTP request (code echoed
/// in dev) → OTP verify → token pair.
pub async fn sign_in(app: &axum::Router, phone: &str, display_name: &str) -> (String, String) {
    let (status, body) = request_json(
        app,
        "POST",
        "/v1/auth/otp/request",
        None,
        Some(json!({ "phone": phone, "purpose": "sign_in" })),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let code = body["dev_code"]
        .as_str()
        .expect("dev_code present (ANBARR_DEV_OTP_ECHO=1)")
        .to_string();

    let (status, body) = request_json(
        app,
        "POST",
        "/v1/auth/otp/verify",
        None,
        Some(json!({ "phone": phone, "purpose": "sign_in", "code": code, "display_name": display_name })),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    (
        body["access_token"]
            .as_str()
            .expect("access_token")
            .to_string(),
        body["refresh_token"]
            .as_str()
            .expect("refresh_token")
            .to_string(),
    )
}

/// Unique SL phone per call: tests must never share a phone — OTP codes are
/// keyed per (phone, purpose), and the newest code would win across tests.
fn unique_phone() -> String {
    use std::sync::atomic::{AtomicU32, Ordering};
    static N: AtomicU32 = AtomicU32::new(1);
    let n = N.fetch_add(1, Ordering::SeqCst);
    format!("076{n:06}") // 9 digits starting with 0 → +23276xxxxx
}

/// A signed-in caller: phone + access token.
pub async fn new_caller(app: &axum::Router, display_name: &str) -> (String, String) {
    let phone = unique_phone();
    let token = sign_in(app, &phone, display_name).await.0;
    (phone, token)
}

pub async fn request_json(
    app: &axum::Router,
    method: &str,
    uri: &str,
    token: Option<&str>,
    body: Option<Value>,
) -> (StatusCode, Value) {
    request_json_with_headers(app, method, uri, token, body, &[]).await
}

/// `request_json` plus arbitrary extra headers (Pin-Token, Idempotency-Key) —
/// money-movement endpoints require them (docs/api.md §1).
pub async fn request_json_with_headers(
    app: &axum::Router,
    method: &str,
    uri: &str,
    token: Option<&str>,
    body: Option<Value>,
    extra: &[(&str, &str)],
) -> (StatusCode, Value) {
    let mut builder = Request::builder().method(method).uri(uri);
    if let Some(token) = token {
        builder = builder.header("authorization", format!("Bearer {token}"));
    }
    for (name, value) in extra {
        builder = builder.header(*name, *value);
    }
    let req = match body {
        Some(v) => builder
            .header("content-type", "application/json")
            .body(Body::from(serde_json::to_vec(&v).unwrap()))
            .unwrap(),
        None => builder.body(Body::empty()).unwrap(),
    };
    let resp = app.clone().oneshot(req).await.unwrap();
    let status = resp.status();
    let bytes = axum::body::to_bytes(resp.into_body(), 1 << 20)
        .await
        .unwrap();
    let value = if bytes.is_empty() {
        Value::Null
    } else {
        serde_json::from_slice(&bytes).unwrap_or(Value::Null)
    };
    (status, value)
}

/// Fund a wallet via the ledger's topup journal path (equivalent of the
/// ledger tests' common harness, local to this crate). Only the binaries
/// that move real money use it; the allow keeps the others clippy-clean.
#[allow(dead_code)]
pub async fn create_funded_wallet(
    pool: &PgPool,
    currency: anbarr_ledger::money::Currency,
    amount: i64,
) -> Uuid {
    use anbarr_ledger::types::{Direction, JournalSpec, JournalType, Leg, Origin};
    let wallet = Uuid::new_v4();
    sqlx::query(
        "INSERT INTO accounts (id, owner_type, owner_id, type, currency, name, status)
         VALUES ($1, 'user', $1, 'wallet', $2, 'test wallet', 'active')",
    )
    .bind(wallet)
    .bind(currency.as_str())
    .execute(pool)
    .await
    .expect("insert wallet");

    let bridge: Uuid = sqlx::query_scalar(
        "SELECT id FROM accounts WHERE type = 'rail_bridge' AND currency = $1 AND owner_id IS NULL",
    )
    .bind(currency.as_str())
    .fetch_one(pool)
    .await
    .expect("seeded rail_bridge");

    let engine = LedgerEngine::new(pool.clone());
    let spec = JournalSpec {
        journal_type: JournalType::Topup,
        currency,
        origin: Origin {
            channel: "test".into(),
            ..Default::default()
        },
        legs: vec![
            Leg {
                account_id: bridge,
                direction: Direction::Debit,
                amount_minor: amount,
            },
            Leg {
                account_id: wallet,
                direction: Direction::Credit,
                amount_minor: amount,
            },
        ],
    };
    engine
        .post_journal("test", &format!("seed-{}", Uuid::new_v4()), spec)
        .await
        .expect("seed funds");
    wallet
}
