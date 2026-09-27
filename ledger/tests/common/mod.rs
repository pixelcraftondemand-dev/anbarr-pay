//! Shared integration-test harness. Each test binary that declares
//! `mod common;` gets its own copy, so per-process state (like the reset
//! OnceCell) is independent per binary — important because `cargo test` runs
//! test binaries in parallel against the same database.
//!
//! Requires the dev database (docker-compose.yml) —
//! `postgres://anbarr:anbarr_dev@localhost:5433/anbarr`. Override with DATABASE_URL.

use anbarr_ledger::db;
use anbarr_ledger::engine::LedgerEngine;
use anbarr_ledger::money::Currency;
use anbarr_ledger::types::{Direction, JournalSpec, JournalType, Leg, Origin};
use sqlx::postgres::PgPoolOptions;
use sqlx::PgPool;
use uuid::Uuid;

fn ensure_test_token() {
    // Intentionally no implicit default token. The gRPC boundary should fail
    // closed unless an explicit dev/test mode is enabled by the test itself.
}

pub async fn pool() -> PgPool {
    ensure_test_token();
    let url = db::database_url_from_env();
    let schema = test_schema_name();
    ensure_schema(&url, schema).await;
    let url = with_search_path(&url, schema);
    let p = PgPoolOptions::new()
        .max_connections(20)
        .connect(&url)
        .await
        .expect("connect to test postgres (run: docker compose up -d)");
    ensure_migrated(&p).await;
    reset_db(&p).await;
    p
}

/// Per-binary schema name, e.g. `test_engine` for the `engine` test binary.
/// Test binaries run in parallel against one Postgres server and share
/// singleton accounts (`fee_revenue`, `hold_escrow`, `rail_bridge`) whose
/// absolute balances are asserted — cross-binary truncation would corrupt
/// each other's data. Each binary therefore gets its own schema via
/// `search_path`, and every table reference in this crate's queries resolves
/// into it.
fn test_schema_name() -> &'static str {
    // Cargo sets CARGO_CRATE_NAME for the test target being compiled, e.g.
    // "engine", "fees", "reconcile", "accounts", "reversals", "properties".
    concat!("test_", env!("CARGO_CRATE_NAME"))
}

async fn ensure_schema(base_url: &str, schema: &str) {
    let admin = PgPoolOptions::new()
        .max_connections(1)
        .connect(base_url)
        .await
        .expect("connect to test postgres (run: docker compose up -d)");
    sqlx::query(&format!("CREATE SCHEMA IF NOT EXISTS {schema}"))
        .execute(&admin)
        .await
        .expect("create per-binary schema");
    admin.close().await;
}

/// Append `options=-csearch_path=<schema>` to the connection URL so every
/// query on the pool resolves unqualified table names into the per-binary
/// schema (`%3D` is the URL-encoded `=`).
fn with_search_path(url: &str, schema: &str) -> String {
    let sep = if url.contains('?') { "&" } else { "?" };
    format!("{url}{sep}options=-csearch_path%3D{schema}")
}

/// Apply migrations exactly once per database, even when many tests run
/// concurrently. Serialized with a Postgres advisory lock on a dedicated
/// connection (a `Once` can't be used here: tests already run inside a tokio
/// runtime, so blocking inside it panics). sqlx tracks applied migrations, so
/// running them on every `pool()` is cheap and picks up new ones.
pub async fn ensure_migrated(p: &PgPool) {
    let mut conn = p.acquire().await.expect("acquire connection");
    sqlx::query("SELECT pg_advisory_lock(hashtext('anbarr_migrations'))")
        .execute(&mut *conn)
        .await
        .expect("advisory lock");
    db::run_migrations(p).await.expect("run migrations");
    sqlx::query("SELECT pg_advisory_unlock(hashtext('anbarr_migrations'))")
        .execute(&mut *conn)
        .await
        .expect("advisory unlock");
}

/// Tests share one database across runs and across processes, but idempotency
/// keys must never collide with a previous run (a stale key would make the
/// engine replay an old journal instead of posting a fresh one). Truncate the
/// app tables exactly once per test-binary process, before any test touches the
/// DB. The sqlx migration-tracking table (`_sqlx_migrations`) is preserved.
static RESET_DB: tokio::sync::OnceCell<()> = tokio::sync::OnceCell::const_new();

pub async fn reset_db(p: &PgPool) {
    RESET_DB.get_or_init(|| truncate_all(p)).await;
}

/// Wipe all test data but keep the seeded platform accounts
/// (0002_platform_accounts.sql): the engine resolves escrow / fee / bridge
/// singletons by (type, currency), and tests must all share the same ones.
/// Callable per test where the OnceCell-once semantics aren't enough (e.g.
/// tests that must not see each other's journals) — only from inside a lock
/// that also covers the test body.
pub async fn truncate_all(p: &PgPool) {
    sqlx::query(
        "TRUNCATE ledger_events, wallet_snapshots, holds, idempotency_keys, journals, \
         reconciliation_runs CASCADE",
    )
    .execute(p)
    .await
    .expect("truncate journals/entries for clean test run");
    sqlx::query("DELETE FROM accounts WHERE owner_type <> 'platform'")
        .execute(p)
        .await
        .expect("remove test wallets for clean test run");
}

// Each test binary compiles its own copy of this module and uses a different
// subset of the helpers, so unused-in-one-binary is fine.
#[allow(dead_code)]
/// Create a user wallet account. Returns the account id.
pub async fn create_wallet(p: &PgPool, currency: Currency, initial: i64) -> Uuid {
    let id = Uuid::new_v4();
    sqlx::query(
        "INSERT INTO accounts (id, owner_type, owner_id, type, currency, name, status)
         VALUES ($1, 'user', $2, 'wallet', $3, $4, 'active')",
    )
    .bind(id)
    .bind(id) // owner_id = account id for tests
    .bind(currency.as_str())
    .bind("test wallet")
    .execute(p)
    .await
    .expect("insert wallet");

    if initial > 0 {
        seed_funds(p, id, currency, initial).await;
    }
    id
}

/// Resolve a seeded singleton platform account (0002_platform_accounts.sql).
/// The engine finds the same one, so tests assert against the real account.
#[allow(dead_code)]
pub async fn platform_account(p: &PgPool, account_type: &str, currency: Currency) -> Uuid {
    sqlx::query_scalar(
        "SELECT id FROM accounts WHERE type = $1 AND currency = $2 AND owner_id IS NULL",
    )
    .bind(account_type)
    .bind(currency.as_str())
    .fetch_one(p)
    .await
    .expect("seeded platform account")
}

/// Post a funding journal (debit rail_bridge, credit wallet) so a wallet has
/// funds before being used in tests.
#[allow(dead_code)]
pub async fn seed_funds(p: &PgPool, wallet: Uuid, currency: Currency, amount: i64) {
    let engine = LedgerEngine::new(p.clone());
    let bridge = platform_account(p, "rail_bridge", currency).await;
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
}
