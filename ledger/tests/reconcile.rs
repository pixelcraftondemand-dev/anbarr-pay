//! Tests for the reconciliation module.
//!
//! Pure tests exercise `reconcile()` directly; the DB-backed tests run the
//! `ReconcileService` against Postgres. The two DB tests are serialized on a
//! process-local mutex because the rail_bridge account is a shared singleton:
//! exact position assertions are only valid when one test observes the bridge
//! at a time (same reasoning as the HOLD_LOCK in engine.rs).

mod common;

use anbarr_ledger::engine::LedgerEngine;
use anbarr_ledger::money::Currency;
use anbarr_ledger::reconcile::{
    reconcile, AlertSink, Rail, RailStatementSource, ReconcileReport, ReconcileScheduler,
    ReconcileService, ReconcileStatus, ReconcileTask, StatementLine,
};
use anbarr_ledger::types::{Direction, JournalSpec, JournalType, Leg, Origin};
use chrono::{Duration, Utc};
use uuid::Uuid;

use common::{create_wallet, platform_account, pool, truncate_all};

fn window() -> (chrono::DateTime<chrono::Utc>, chrono::DateTime<chrono::Utc>) {
    (
        Utc::now() - Duration::hours(1),
        Utc::now() + Duration::minutes(1),
    )
}

fn line(reference: &str, amount: i64) -> StatementLine {
    StatementLine {
        external_reference: reference.into(),
        amount_minor: amount,
        currency: Currency::Sle,
        occurred_at: Utc::now(),
    }
}

fn ledger_ref(reference: &str, amount: i64) -> anbarr_ledger::reconcile::LedgerRef {
    anbarr_ledger::reconcile::LedgerRef {
        journal_id: Uuid::new_v4(),
        payment_code: reference.into(),
        journal_type: "topup".into(),
        amount_minor: amount,
        occurred_at: Utc::now(),
    }
}

// ----------------------------------------------------------------------
// Pure matching / position logic
// ----------------------------------------------------------------------

#[test]
fn all_lines_match_and_position_balances() {
    let (start, end) = window();
    let report = reconcile(
        Rail::OrangeMoney,
        Currency::Sle,
        start,
        end,
        &[line("R1", -5_000), line("R2", -7_000)],
        &[ledger_ref("R1", -5_000), ledger_ref("R2", -7_000)],
        Some(-12_000),
    );
    assert_eq!(report.status, ReconcileStatus::Balanced);
    assert_eq!(report.matched, 2);
    assert_eq!(report.statement_count, 2);
    assert_eq!(report.ledger_count, 2);
    assert_eq!(report.statement_total_minor, -12_000);
    assert_eq!(report.ledger_total_minor, -12_000);
    assert!(report.unmatched_on_ledger.is_empty());
    assert!(report.unmatched_on_rail.is_empty());
    assert!(report.amount_mismatches.is_empty());
    assert!(report.duplicate_references.is_empty());
}

#[test]
fn statement_line_without_journal_is_drift() {
    let (start, end) = window();
    let report = reconcile(
        Rail::OrangeMoney,
        Currency::Sle,
        start,
        end,
        &[line("R1", -5_000), line("R9", -100)],
        &[ledger_ref("R1", -5_000)],
        Some(-5_100),
    );
    assert!(matches!(report.status, ReconcileStatus::Drift { .. }));
    assert_eq!(report.unmatched_on_rail.len(), 1);
    assert_eq!(report.unmatched_on_rail[0].external_reference, "R9");
    assert!(report.unmatched_on_ledger.is_empty());
}

#[test]
fn journal_without_statement_line_is_drift() {
    let (start, end) = window();
    let report = reconcile(
        Rail::OrangeMoney,
        Currency::Sle,
        start,
        end,
        &[line("R1", -5_000)],
        &[ledger_ref("R1", -5_000), ledger_ref("R2", -7_000)],
        Some(-12_000),
    );
    assert!(matches!(report.status, ReconcileStatus::Drift { .. }));
    assert_eq!(report.unmatched_on_ledger.len(), 1);
    assert_eq!(report.unmatched_on_ledger[0].payment_code, "R2");
    assert!(report.unmatched_on_rail.is_empty());
}

#[test]
fn amount_mismatch_is_grouped_not_unmatched() {
    let (start, end) = window();
    let report = reconcile(
        Rail::OrangeMoney,
        Currency::Sle,
        start,
        end,
        &[line("R1", -5_000)],
        &[ledger_ref("R1", -5_500)], // same ref, different amount
        Some(-5_500),
    );
    assert!(matches!(report.status, ReconcileStatus::Drift { .. }));
    assert_eq!(report.amount_mismatches.len(), 1);
    assert_eq!(report.amount_mismatches[0].reference, "R1");
    assert_eq!(report.amount_mismatches[0].statement_minor, -5_000);
    assert_eq!(report.amount_mismatches[0].ledger_minor, -5_500);
    assert_eq!(report.matched, 0);
    assert!(
        report.unmatched_on_rail.is_empty(),
        "mismatch is not unmatched"
    );
    assert!(
        report.unmatched_on_ledger.is_empty(),
        "mismatch is not unmatched"
    );
}

#[test]
fn duplicate_reference_is_flagged() {
    let (start, end) = window();
    let report = reconcile(
        Rail::OrangeMoney,
        Currency::Sle,
        start,
        end,
        &[line("R1", -5_000)],
        &[ledger_ref("R1", -5_000), ledger_ref("R1", -2_000)], // same code twice
        Some(-7_000),
    );
    assert!(matches!(report.status, ReconcileStatus::Drift { .. }));
    assert_eq!(report.duplicate_references, vec!["R1".to_string()]);
    // A duplicated reference is ambiguous: neither journal matches the single
    // statement line cleanly, so both sides surface as unmatched detail while
    // `duplicate_references` carries the primary signal.
    assert_eq!(report.unmatched_on_ledger.len(), 2);
    assert_eq!(report.unmatched_on_rail.len(), 1);
    assert_eq!(report.matched, 0);
}

#[test]
fn bridge_position_mismatch_is_drift() {
    let (start, end) = window();
    // All lines match, but the bridge moved differently than the statement says.
    let report = reconcile(
        Rail::OrangeMoney,
        Currency::Sle,
        start,
        end,
        &[line("R1", -5_000)],
        &[ledger_ref("R1", -5_000)],
        Some(-5_500),
    );
    assert!(matches!(
        report.status,
        ReconcileStatus::Drift {
            difference_minor: 0,
            bridge_difference_minor: Some(500),
        }
    ));
}

#[test]
fn no_bridge_account_reports_none_and_still_balances_lines() {
    let (start, end) = window();
    let report = reconcile(
        Rail::OrangeMoney,
        Currency::Sle,
        start,
        end,
        &[line("R1", -5_000)],
        &[ledger_ref("R1", -5_000)],
        None,
    );
    assert_eq!(report.bridge_movement_minor, None);
    // Line matching is the check that ran; the missing bridge is surfaced as
    // `bridge_movement_minor: None` for ops to notice, not as line drift.
    assert_eq!(report.status, ReconcileStatus::Balanced);
}

// ----------------------------------------------------------------------
// End-to-end against Postgres
// ----------------------------------------------------------------------

static RECONCILE_LOCK: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());

/// Post a rail-tied topup journal: debit bridge, credit wallet, with a
/// payment_code that the rail's statement would reference.
async fn post_rail_tied(
    engine: &LedgerEngine,
    bridge: Uuid,
    wallet: Uuid,
    key: &str,
    reference: &str,
    amount: i64,
) {
    let spec = JournalSpec {
        journal_type: JournalType::Topup,
        currency: Currency::Sle,
        origin: Origin {
            channel: "reconcile-test".into(),
            payment_code: Some(reference.into()),
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
        .post_journal("reconcile", key, spec)
        .await
        .expect("post rail-tied journal");
}

#[tokio::test]
async fn service_balanced_end_to_end() {
    let _guard = RECONCILE_LOCK.lock().await;
    let p = pool().await;
    let engine = LedgerEngine::new(p.clone());
    // Fresh slate: tests in this binary must not see each other's journals
    // (the process-level reset only runs once). Safe because the lock covers
    // the whole test body.
    truncate_all(&p).await;
    let bridge = platform_account(&p, "rail_bridge", Currency::Sle).await;
    let wallet = create_wallet(&p, Currency::Sle, 0).await;

    post_rail_tied(&engine, bridge, wallet, "rc-e2e-1", "OM-REF-1", 5_000).await;
    post_rail_tied(&engine, bridge, wallet, "rc-e2e-2", "OM-REF-2", 7_000).await;

    let source = RailStatementSource::Stub {
        lines: vec![line("OM-REF-1", -5_000), line("OM-REF-2", -7_000)],
    };
    let service = ReconcileService::new(p.clone());
    let (start, end) = window();
    let report: ReconcileReport = service
        .run(Rail::OrangeMoney, Currency::Sle, start, end, &source)
        .await
        .expect("reconcile run");

    assert_eq!(report.status, ReconcileStatus::Balanced);
    assert_eq!(report.matched, 2);
    assert_eq!(report.statement_count, 2);
    assert_eq!(report.ledger_count, 2);
    assert_eq!(report.bridge_movement_minor, Some(-12_000));
    assert!(report.unmatched_on_rail.is_empty());
    assert!(report.unmatched_on_ledger.is_empty());
}

#[tokio::test]
async fn service_detects_statement_line_without_journal() {
    let _guard = RECONCILE_LOCK.lock().await;
    let p = pool().await;
    let engine = LedgerEngine::new(p.clone());
    truncate_all(&p).await;
    let bridge = platform_account(&p, "rail_bridge", Currency::Sle).await;
    let wallet = create_wallet(&p, Currency::Sle, 0).await;

    post_rail_tied(&engine, bridge, wallet, "rc-drift-1", "OM-REF-1", 5_000).await;

    // The rail reports an extra 100 that the ledger never saw.
    let source = RailStatementSource::Stub {
        lines: vec![line("OM-REF-1", -5_000), line("OM-REF-9", -100)],
    };
    let service = ReconcileService::new(p.clone());
    let (start, end) = window();
    let report = service
        .run(Rail::OrangeMoney, Currency::Sle, start, end, &source)
        .await
        .expect("reconcile run");

    assert!(matches!(report.status, ReconcileStatus::Drift { .. }));
    assert_eq!(report.unmatched_on_rail.len(), 1);
    assert_eq!(report.unmatched_on_rail[0].external_reference, "OM-REF-9");
    assert_eq!(report.matched, 1);
    assert!(report.unmatched_on_ledger.is_empty());
}

// ----------------------------------------------------------------------
// Scheduler + drift alerts
// ----------------------------------------------------------------------

async fn scheduler_with_source(
    p: &sqlx::PgPool,
    source: RailStatementSource,
) -> ReconcileScheduler {
    ReconcileScheduler::new(
        ReconcileService::new(p.clone()),
        source,
        AlertSink::Db { pool: p.clone() },
        vec![ReconcileTask {
            rail: Rail::OrangeMoney,
            currency: Currency::Sle,
        }],
        Duration::hours(1),
        std::time::Duration::from_secs(60), // only run_once is exercised in tests
    )
}

#[tokio::test]
async fn scheduler_records_balanced_run_in_db() {
    let _guard = RECONCILE_LOCK.lock().await;
    let p = pool().await;
    truncate_all(&p).await;
    let engine = LedgerEngine::new(p.clone());
    let bridge = platform_account(&p, "rail_bridge", Currency::Sle).await;
    let wallet = create_wallet(&p, Currency::Sle, 0).await;
    post_rail_tied(&engine, bridge, wallet, "rc-sched-1", "OM-REF-1", 5_000).await;

    let scheduler = scheduler_with_source(
        &p,
        RailStatementSource::Stub {
            lines: vec![line("OM-REF-1", -5_000)],
        },
    )
    .await;
    let reports = scheduler.run_once().await;
    assert_eq!(reports.len(), 1);
    assert_eq!(reports[0].status, ReconcileStatus::Balanced);

    let (status, matched, statement_count, diff): (String, i32, i32, i64) = sqlx::query_as(
        "SELECT status, matched, statement_count, difference_minor \
         FROM reconciliation_runs ORDER BY id DESC LIMIT 1",
    )
    .fetch_one(&p)
    .await
    .unwrap();
    assert_eq!(status, "balanced");
    assert_eq!(matched, 1);
    assert_eq!(statement_count, 1);
    assert_eq!(diff, 0);
}

#[tokio::test]
async fn scheduler_records_drift_run_and_alerts() {
    let _guard = RECONCILE_LOCK.lock().await;
    let p = pool().await;
    truncate_all(&p).await;
    let engine = LedgerEngine::new(p.clone());
    let bridge = platform_account(&p, "rail_bridge", Currency::Sle).await;
    let wallet = create_wallet(&p, Currency::Sle, 0).await;
    post_rail_tied(&engine, bridge, wallet, "rc-sched-2", "OM-REF-1", 5_000).await;

    // The rail saw money the ledger never recorded -> drift alert.
    let scheduler = scheduler_with_source(
        &p,
        RailStatementSource::Stub {
            lines: vec![line("OM-REF-1", -5_000), line("OM-REF-9", -100)],
        },
    )
    .await;
    let reports = scheduler.run_once().await;
    assert_eq!(reports.len(), 1);
    assert!(matches!(reports[0].status, ReconcileStatus::Drift { .. }));

    let (status, unmatched_rail, detail): (String, i32, serde_json::Value) = sqlx::query_as(
        "SELECT status, unmatched_on_rail, detail FROM reconciliation_runs ORDER BY id DESC LIMIT 1",
    )
    .fetch_one(&p)
    .await
    .unwrap();
    assert_eq!(status, "drift");
    assert_eq!(unmatched_rail, 1);
    assert_eq!(
        detail["unmatched_on_rail"][0]["external_reference"],
        "OM-REF-9"
    );
}

#[tokio::test]
async fn log_sink_accepts_reports_without_error() {
    let sink = AlertSink::Log;
    let (start, end) = window();
    let balanced = reconcile(
        Rail::OrangeMoney,
        Currency::Sle,
        start,
        end,
        &[line("R1", -5_000)],
        &[ledger_ref("R1", -5_000)],
        Some(-5_000),
    );
    sink.report(&balanced)
        .await
        .expect("log sink accepts balanced");

    let drift = reconcile(
        Rail::OrangeMoney,
        Currency::Sle,
        start,
        end,
        &[line("R1", -5_000), line("R9", -100)],
        &[ledger_ref("R1", -5_000)],
        Some(-5_100),
    );
    sink.report(&drift).await.expect("log sink accepts drift");
}

// ----------------------------------------------------------------------
// Failure paths: missing bridge, sink errors, run errors, and the spawn loop
// ----------------------------------------------------------------------

#[test]
fn rail_as_str_covers_all_variants() {
    assert_eq!(Rail::OrangeMoney.as_str(), "orange_money");
    assert_eq!(Rail::Afrimoney.as_str(), "afrimoney");
    assert_eq!(Rail::BankTransfer.as_str(), "bank_transfer");
    assert_eq!(Rail::Card.as_str(), "card");
}

/// A pool that is connected then closed, so every query on it fails.
async fn closed_pool() -> sqlx::PgPool {
    let closed = sqlx::postgres::PgPoolOptions::new()
        .connect(&anbarr_ledger::db::database_url_from_env())
        .await
        .expect("connect to dev postgres");
    closed.close().await;
    closed
}

#[tokio::test]
async fn service_without_bridge_account_reports_none() {
    let _guard = RECONCILE_LOCK.lock().await;
    let p = pool().await;
    truncate_all(&p).await;

    // No test uses the USD bridge: remove the seeded row so the service's
    // position lookup takes the "no bridge account" branch, then restore it.
    sqlx::query("DELETE FROM accounts WHERE type = 'rail_bridge' AND currency = 'USD'")
        .execute(&p)
        .await
        .unwrap();

    let service = ReconcileService::new(p.clone());
    let (start, end) = window();
    let report = service
        .run(
            Rail::BankTransfer,
            Currency::Usd,
            start,
            end,
            &RailStatementSource::Stub { lines: vec![] },
        )
        .await
        .expect("reconcile run without a bridge");

    // Restore the seeded platform account before asserting.
    sqlx::query(
        "INSERT INTO accounts (id, owner_type, owner_id, type, currency, name, status)
         VALUES ('11111111-1111-4111-8111-111111111402', 'platform', NULL, 'rail_bridge',
                 'USD', 'Rail bridge USD', 'active')",
    )
    .execute(&p)
    .await
    .unwrap();

    assert_eq!(report.bridge_movement_minor, None);
    assert_eq!(report.status, ReconcileStatus::Balanced);
}

#[tokio::test]
async fn scheduler_keeps_running_when_sink_fails() {
    let _guard = RECONCILE_LOCK.lock().await;
    let p = pool().await;
    truncate_all(&p).await;
    let engine = LedgerEngine::new(p.clone());
    let bridge = platform_account(&p, "rail_bridge", Currency::Sle).await;
    let wallet = create_wallet(&p, Currency::Sle, 0).await;
    post_rail_tied(&engine, bridge, wallet, "rc-sink-1", "OM-REF-1", 5_000).await;

    // A closed pool makes the Db sink's INSERT fail; the scheduler logs the
    // sink error and still surfaces the report.
    let scheduler = ReconcileScheduler::new(
        ReconcileService::new(p.clone()),
        RailStatementSource::Stub {
            lines: vec![line("OM-REF-1", -5_000)],
        },
        AlertSink::Db {
            pool: closed_pool().await,
        },
        vec![ReconcileTask {
            rail: Rail::OrangeMoney,
            currency: Currency::Sle,
        }],
        Duration::hours(1),
        std::time::Duration::from_secs(60),
    );
    let reports = scheduler.run_once().await;
    assert_eq!(
        reports.len(),
        1,
        "successful run is returned despite sink failure"
    );
    assert_eq!(reports[0].status, ReconcileStatus::Balanced);
}

#[tokio::test]
async fn scheduler_skips_run_when_service_fails() {
    let _guard = RECONCILE_LOCK.lock().await;
    let p = pool().await;
    truncate_all(&p).await;

    // A closed pool makes every service query fail; the scheduler logs the
    // error and continues with no report instead of panicking.
    let scheduler = ReconcileScheduler::new(
        ReconcileService::new(closed_pool().await),
        RailStatementSource::Stub { lines: vec![] },
        AlertSink::Log,
        vec![ReconcileTask {
            rail: Rail::OrangeMoney,
            currency: Currency::Sle,
        }],
        Duration::hours(1),
        std::time::Duration::from_secs(60),
    );
    let reports = scheduler.run_once().await;
    assert!(reports.is_empty(), "failed run is logged, not returned");
}

#[tokio::test]
async fn scheduler_spawn_ticks_and_shuts_down_on_signal() {
    let _guard = RECONCILE_LOCK.lock().await;
    let p = pool().await;
    truncate_all(&p).await;
    let engine = LedgerEngine::new(p.clone());
    let bridge = platform_account(&p, "rail_bridge", Currency::Sle).await;
    let wallet = create_wallet(&p, Currency::Sle, 0).await;
    post_rail_tied(&engine, bridge, wallet, "rc-spawn-1", "OM-REF-1", 5_000).await;

    let scheduler = ReconcileScheduler::new(
        ReconcileService::new(p.clone()),
        RailStatementSource::Stub {
            lines: vec![line("OM-REF-1", -5_000)],
        },
        AlertSink::Db { pool: p.clone() },
        vec![ReconcileTask {
            rail: Rail::OrangeMoney,
            currency: Currency::Sle,
        }],
        Duration::hours(1),
        std::time::Duration::from_millis(20),
    );
    let (tx, rx) = tokio::sync::watch::channel(false);
    let handle = scheduler.spawn(rx);

    // Let at least one tick reconcile, then ask the loop to stop.
    tokio::time::sleep(std::time::Duration::from_millis(200)).await;
    tx.send(true).unwrap();
    handle.await.expect("spawned loop exits on shutdown signal");

    let (status,): (String,) =
        sqlx::query_as("SELECT status FROM reconciliation_runs ORDER BY id DESC LIMIT 1")
            .fetch_one(&p)
            .await
            .unwrap();
    assert_eq!(status, "balanced");
}
