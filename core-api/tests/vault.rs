//! Core API integration tests. A real ledger gRPC server runs in-process
//! (real engine, real Postgres in a per-binary schema) and the core-api
//! router is exercised over axum's test transport — the same contract the
//! deployed services use, with no mocks on the money path.
//!
//! Requires the dev database (docker-compose.yml).

use anbarr_core_api::ledger::pb::ledger_server::LedgerServer;
use anbarr_core_api::{create_app, AppState, LedgerClient};
use amber_ledger::db;
use amber_ledger::engine::LedgerEngine;
use amber_ledger::grpc::LedgerGrpc;
use axum::body::Body;
use axum::http::{Request, StatusCode};
use serde_json::{json, Value};
use sqlx::postgres::PgPoolOptions;
use sqlx::PgPool;
use tower::ServiceExt; // for `oneshot`
use uuid::Uuid;

const CALLER: &str = "caller-vault-tests";

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
async fn ledger_stack() -> (PgPool, LedgerClient) {
    let url = db::database_url_from_env();
    let schema = "test_core_api";
    let admin = PgPoolOptions::new()
        .max_connections(1)
        .connect(&url)
        .await
        .expect("connect to test postgres (run: docker compose up -d)");
    sqlx::query(&format!("CREATE SCHEMA IF NOT EXISTS {schema}"))
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
    amber_ledger::db::run_migrations(&pool).await.expect("ledger migrations");

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

    // In-process ledger gRPC server on an ephemeral port.
    let addr = ephemeral_addr();
    let engine = LedgerEngine::new(pool.clone());
    let server = tokio::spawn(async move {
        tonic::transport::Server::builder()
            .add_service(LedgerServer::new(LedgerGrpc::new(engine)))
            .serve(addr)
            .await
            .expect("ledger server runs");
    });

    let addr_uri = format!("http://{addr}");
    // Readiness loop: retry until the server accepts (or fail loudly).
    let client = {
        let mut last_err = String::new();
        let mut found = None;
        for _ in 0..150 {
            let channel = tonic::transport::Channel::from_shared(addr_uri.clone())
                .expect("uri")
                .connect_lazy();
            let inner = amber_ledger::grpc_proto::ledger_client::LedgerClient::new(channel);
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
        found.expect("ledger gRPC became ready")
    };

    (pool, client)
    // `server` task keeps running for the test's lifetime; dropped at end.
}

/// Core-side harness: a dedicated core database (the real boundary — this is
/// the database the Core API owns).
async fn core_pool() -> PgPool {
    let admin = PgPoolOptions::new()
        .max_connections(1)
        .connect("postgres://amber:amber_dev@localhost:5433/postgres?sslmode=disable")
        .await
        .expect("connect admin postgres");
    // Postgres has no CREATE DATABASE IF NOT EXISTS; ignore the duplicate
    // error (42P04) — the database existing is the desired state.
    let _ = sqlx::query("CREATE DATABASE amber_core_test")
        .execute(&admin)
        .await;
    admin.close().await;

    let pool = PgPoolOptions::new()
        .max_connections(5)
        .connect("postgres://amber:amber_dev@localhost:5433/amber_core_test?sslmode=disable")
        .await
        .expect("connect core test db");
    anbarr_core_api::db::run_migrations(&pool).await.expect("core migrations");
    sqlx::query("TRUNCATE wallet_links, vault_goals, vault_goal_locks CASCADE")
        .execute(&pool)
        .await
        .expect("truncate core tables");
    pool
}

fn app(pool: PgPool, ledger: LedgerClient) -> axum::Router {
    create_app(AppState::new(pool, ledger))
}

async fn request_json(
    app: &axum::Router,
    method: &str,
    uri: &str,
    caller: &str,
    body: Option<Value>,
) -> (StatusCode, Value) {
    let mut builder = Request::builder()
        .method(method)
        .uri(uri)
        .header("x-amber-caller", caller);
    let req = match body {
        Some(v) => builder
            .header("content-type", "application/json")
            .body(Body::from(serde_json::to_vec(&v).unwrap()))
            .unwrap(),
        None => builder.body(Body::empty()).unwrap(),
    };
    let resp = app.clone().oneshot(req).await.unwrap();
    let status = resp.status();
    let bytes = axum::body::to_bytes(resp.into_body(), 1 << 20).await.unwrap();
    let value = if bytes.is_empty() {
        Value::Null
    } else {
        serde_json::from_slice(&bytes).unwrap_or(Value::Null)
    };
    (status, value)
}

/// Fund a wallet via the ledger's topup journal path (equivalent of the
/// ledger tests' common harness, local to this binary).
async fn create_funded_wallet(
    pool: &PgPool,
    currency: amber_ledger::money::Currency,
    amount: i64,
) -> Uuid {
    use amber_ledger::types::{Direction, JournalSpec, JournalType, Leg, Origin};
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

#[tokio::test]
async fn vault_goal_lifecycle_end_to_end() {
    let (pool, ledger) = ledger_stack().await;
    let core = core_pool().await;
    let app = app(core, ledger.clone());

    let wallet = create_funded_wallet(&pool, amber_ledger::money::Currency::SLE, 500_000).await;

    // 1. Link the wallet (verified against the ledger before storing).
    let (status, body) =
        request_json(&app, "POST", "/v1/wallets", CALLER, Some(json!({ "account_id": wallet, "label": "Main", "primary": true }))).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["available_minor"], 500_000);

    // 2. GET /v1/wallets lists it with live balances.
    let (status, body) = request_json(&app, "GET", "/v1/wallets", CALLER, None).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body[0]["currency"], "SLE");
    assert_eq!(body[0]["available_minor"], 500_000);

    // 3. Create a goal — the first lock holds 200_000.
    let (status, body) = request_json(
        &app,
        "POST",
        "/v1/vault/goals",
        CALLER,
        Some(json!({ "name": "School fees", "currency": "SLE", "amount_minor": 200_000, "target_minor": 400_000 })),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let goal_id = body["id"].as_str().unwrap().to_string();

    // Balances moved to held on the ledger.
    let (_, wallets) = request_json(&app, "GET", "/v1/wallets", CALLER, None).await;
    assert_eq!(wallets[0]["available_minor"], 300_000);
    assert_eq!(wallets[0]["held_minor"], 200_000);

    // 4. Add a second lock.
    let (status, body) = request_json(
        &app,
        "POST",
        &format!("/v1/vault/goals/{goal_id}/locks"),
        CALLER,
        Some(json!({ "amount_minor": 100_000 })),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["locked_minor"], 300_000);

    // 5. List goals: progress is derived from open locks.
    let (_, goals) = request_json(&app, "GET", "/v1/vault/goals", CALLER, None).await;
    assert_eq!(goals[0]["locked_minor"], 300_000);
    assert_eq!(goals[0]["locks"].as_array().unwrap().len(), 2);

    // 6. Release everything — whole holds return to available.
    let (status, body) = request_json(
        &app,
        "POST",
        &format!("/v1/vault/goals/{goal_id}/release"),
        CALLER,
        Some(json!({})),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["released_minor"], 300_000);
    assert_eq!(body["released_holds"], 2);

    let (_, wallets) = request_json(&app, "GET", "/v1/wallets", CALLER, None).await;
    assert_eq!(wallets[0]["available_minor"], 500_000);
    assert_eq!(wallets[0]["held_minor"], 0);

    // 7. The goal is closed out.
    let (_, goals) = request_json(&app, "GET", "/v1/vault/goals", CALLER, None).await;
    assert_eq!(goals[0]["status"], "released");
    assert_eq!(goals[0]["locked_minor"], 0);
}

#[tokio::test]
async fn vault_goal_insufficient_funds_is_422_and_no_goal_row() {
    let (pool, ledger) = ledger_stack().await;
    let core = core_pool().await;
    let app = app(core, ledger.clone());

    let wallet = create_funded_wallet(&pool, amber_ledger::money::Currency::SLE, 100_000).await;

    let (status, _) =
        request_json(&app, "POST", "/v1/wallets", CALLER, Some(json!({ "account_id": wallet, "primary": true }))).await;
    assert_eq!(status, StatusCode::OK);

    // Ask for more than the wallet holds: the ledger's funds rule fires.
    let (status, body) = request_json(
        &app,
        "POST",
        "/v1/vault/goals",
        CALLER,
        Some(json!({ "name": "Too rich", "currency": "SLE", "amount_minor": 900_000 })),
    )
    .await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "{body}");

    // No phantom goal row was persisted (money-first ordering).
    let (_, goals) = request_json(&app, "GET", "/v1/vault/goals", CALLER, None).await;
    assert_eq!(goals.as_array().unwrap().len(), 0);
}

#[tokio::test]
async fn vault_goal_currency_mismatch_is_400() {
    let (pool, ledger) = ledger_stack().await;
    let core = core_pool().await;
    let app = app(core, ledger.clone());

    let wallet = create_funded_wallet(&pool, amber_ledger::money::Currency::SLE, 100_000).await;

    let (status, _) =
        request_json(&app, "POST", "/v1/wallets", CALLER, Some(json!({ "account_id": wallet, "primary": true }))).await;
    assert_eq!(status, StatusCode::OK);

    let (status, body) = request_json(
        &app,
        "POST",
        "/v1/vault/goals",
        CALLER,
        Some(json!({ "name": "USD goal", "currency": "USD", "amount_minor": 10_000 })),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "{body}");
}

#[tokio::test]
async fn vault_goal_ownership_is_enforced() {
    let (pool, ledger) = ledger_stack().await;
    let core = core_pool().await;
    let app = app(core, ledger.clone());

    let wallet = create_funded_wallet(&pool, amber_ledger::money::Currency::SLE, 400_000).await;

    let (status, _) =
        request_json(&app, "POST", "/v1/wallets", CALLER, Some(json!({ "account_id": wallet, "primary": true }))).await;
    assert_eq!(status, StatusCode::OK);

    let (_, body) = request_json(
        &app,
        "POST",
        "/v1/vault/goals",
        CALLER,
        Some(json!({ "name": "Mine", "currency": "SLE", "amount_minor": 50_000 })),
    )
    .await;
    assert!(
        body["id"].is_string(),
        "goal creation failed: {body}"
    );
    let goal_id = body["id"].as_str().unwrap().to_string();

    // A foreign caller sees no goals.
    let (_, goals) = request_json(&app, "GET", "/v1/vault/goals", "someone-else", None).await;
    assert_eq!(goals.as_array().unwrap().len(), 0);

    // A foreign caller cannot release them either.
    let (status, _) = request_json(
        &app,
        "POST",
        &format!("/v1/vault/goals/{goal_id}/release"),
        "someone-else",
        Some(json!({})),
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND);

    // The owner still can.
    let (status, _) = request_json(
        &app,
        "POST",
        &format!("/v1/vault/goals/{goal_id}/release"),
        CALLER,
        Some(json!({})),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
}
