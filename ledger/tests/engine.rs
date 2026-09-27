//! Integration tests for the ledger engine against a real Postgres.
//! Shared harness lives in `common` (pool, migrations, reset, wallet helpers).

mod common;

use anbarr_ledger::engine::{CaptureRequest, HoldRequest, LedgerEngine, ReleaseRequest};
use anbarr_ledger::money::Currency;
use anbarr_ledger::types::{Direction, JournalSpec, JournalType, Leg, Origin};
use chrono::{Duration, Utc};
use sqlx::{Acquire, PgPool};
use uuid::Uuid;

use common::{create_wallet, platform_account, pool};

fn origin() -> Origin {
    Origin {
        user_id: Some(Uuid::new_v4()),
        channel: "test".into(),
        session_id: Some("sess-1".into()),
        payment_code: None,
    }
}

/// Hold-lifecycle tests assert exact escrow balances, but the escrow account is
/// a shared singleton (0002_platform_accounts.sql) — the engine resolves one per
/// currency. Serialize those tests so each observes the escrow alone.
static HOLD_LOCK: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());

async fn setup(p: &PgPool) -> (LedgerEngine, Uuid, Uuid, Uuid, Uuid) {
    let engine = LedgerEngine::new(p.clone());
    let a = create_wallet(p, Currency::Sle, 100_000).await;
    let b = create_wallet(p, Currency::Sle, 0).await;
    let escrow = platform_account(p, "hold_escrow", Currency::Sle).await;
    let fee = platform_account(p, "fee_revenue", Currency::Sle).await;
    (engine, a, b, escrow, fee)
}

// ----------------------------------------------------------------------
// Posting & balances
// ----------------------------------------------------------------------

#[tokio::test]
async fn transfer_moves_funds_and_fee() {
    let p = pool().await;
    let (engine, a, b, _escrow, fee) = setup(&p).await;

    let spec = JournalSpec {
        journal_type: JournalType::P2p,
        currency: Currency::Sle,
        origin: origin(),
        legs: vec![
            Leg {
                account_id: a,
                direction: Direction::Debit,
                amount_minor: 25_000,
            },
            Leg {
                account_id: a,
                direction: Direction::Debit,
                amount_minor: 125,
            }, // 0.5% fee
            Leg {
                account_id: b,
                direction: Direction::Credit,
                amount_minor: 25_000,
            },
            Leg {
                account_id: fee,
                direction: Direction::Credit,
                amount_minor: 125,
            },
        ],
    };
    // fee_revenue is a shared singleton, so assert the delta this journal added.
    let fee_before = anbarr_ledger::balances::get_balance(&p, fee)
        .await
        .unwrap()
        .available_minor;
    let result = engine
        .post_journal("user-1", "tx-1", spec)
        .await
        .expect("post");

    let bal_a = anbarr_ledger::balances::get_balance(&p, a).await.unwrap();
    let bal_b = anbarr_ledger::balances::get_balance(&p, b).await.unwrap();
    let bal_fee = anbarr_ledger::balances::get_balance(&p, fee).await.unwrap();

    assert_eq!(bal_a.available_minor, 100_000 - 25_125);
    assert_eq!(bal_b.available_minor, 25_000);
    assert_eq!(bal_fee.available_minor, fee_before + 125);
    assert_eq!(bal_a.total_minor, bal_a.available_minor);
    assert_eq!(result.status, anbarr_ledger::types::JournalStatus::Posted);

    // Journals must balance: debits == credits. (SUM over bigint is numeric,
    // so cast back to bigint for i64 decoding.)
    let (debits, credits): (i64, i64) = sqlx::query_as(
        "SELECT
           COALESCE(SUM(amount_minor) FILTER (WHERE direction='debit'), 0)::bigint,
           COALESCE(SUM(amount_minor) FILTER (WHERE direction='credit'), 0)::bigint
         FROM entries WHERE journal_id = $1",
    )
    .bind(result.journal_id)
    .fetch_one(&p)
    .await
    .unwrap();
    assert_eq!(debits, credits);
    assert_eq!(debits, 25_125);
}

#[tokio::test]
async fn insufficient_funds_rejected_atomically() {
    let p = pool().await;
    let (engine, a, b, _escrow, _fee) = setup(&p).await;

    let spec = JournalSpec {
        journal_type: JournalType::P2p,
        currency: Currency::Sle,
        origin: origin(),
        legs: vec![
            Leg {
                account_id: a,
                direction: Direction::Debit,
                amount_minor: 200_000,
            },
            Leg {
                account_id: b,
                direction: Direction::Credit,
                amount_minor: 200_000,
            },
        ],
    };
    let err = engine
        .post_journal("user-1", "tx-over", spec)
        .await
        .unwrap_err();
    assert!(
        matches!(
            err,
            anbarr_ledger::engine::EngineError::InsufficientFunds { .. }
        ),
        "expected InsufficientFunds, got {err:?}"
    );

    // Nothing was written.
    let bal_a = anbarr_ledger::balances::get_balance(&p, a).await.unwrap();
    assert_eq!(bal_a.available_minor, 100_000);
    let count: i64 =
        sqlx::query_scalar("SELECT count(*) FROM journals WHERE idempotency_key = 'tx-over'")
            .fetch_one(&p)
            .await
            .unwrap();
    assert_eq!(count, 0);
}

#[tokio::test]
async fn unbalanced_journal_rejected() {
    let p = pool().await;
    let (engine, a, b, _escrow, _fee) = setup(&p).await;

    let spec = JournalSpec {
        journal_type: JournalType::P2p,
        currency: Currency::Sle,
        origin: origin(),
        legs: vec![
            Leg {
                account_id: a,
                direction: Direction::Debit,
                amount_minor: 1_000,
            },
            Leg {
                account_id: b,
                direction: Direction::Credit,
                amount_minor: 999,
            },
        ],
    };
    let err = engine
        .post_journal("user-1", "tx-unbal", spec)
        .await
        .unwrap_err();
    assert!(matches!(
        err,
        anbarr_ledger::engine::EngineError::Unbalanced { .. }
    ));
}

// ----------------------------------------------------------------------
// Idempotency
// ----------------------------------------------------------------------

#[tokio::test]
async fn duplicate_key_replays_without_double_posting() {
    let p = pool().await;
    let (engine, a, b, _escrow, _fee) = setup(&p).await;

    // The spec is shared (including origin) so the retry carries an identical
    // request hash — the request-hash check must see a genuine duplicate.
    let o = origin();
    let spec = || JournalSpec {
        journal_type: JournalType::P2p,
        currency: Currency::Sle,
        origin: o.clone(),
        legs: vec![
            Leg {
                account_id: a,
                direction: Direction::Debit,
                amount_minor: 10_000,
            },
            Leg {
                account_id: b,
                direction: Direction::Credit,
                amount_minor: 10_000,
            },
        ],
    };

    let first = engine
        .post_journal("user-1", "dup-key", spec())
        .await
        .unwrap();
    let second = engine
        .post_journal("user-1", "dup-key", spec())
        .await
        .unwrap();

    assert_eq!(
        first.journal_id, second.journal_id,
        "replay must return the original journal"
    );

    let count: i64 = sqlx::query_scalar("SELECT count(*) FROM entries WHERE journal_id = $1")
        .bind(first.journal_id)
        .fetch_one(&p)
        .await
        .unwrap();
    assert_eq!(count, 2, "no duplicate entries on replay");

    let bal_a = anbarr_ledger::balances::get_balance(&p, a).await.unwrap();
    assert_eq!(bal_a.available_minor, 90_000, "single debit only");
}

// ----------------------------------------------------------------------
// Holds
// ----------------------------------------------------------------------

#[tokio::test]
async fn hold_capture_cycle() {
    let _guard = HOLD_LOCK.lock().await;
    let p = pool().await;
    let (engine, a, b, escrow, _fee) = setup(&p).await;

    // Hold 10_000 on A for 15 minutes.
    let hold = engine
        .hold_funds(HoldRequest {
            idempotency_scope: "merchant-1".into(),
            idempotency_key: "hold-1".into(),
            account_id: a,
            amount_minor: 10_000,
            currency: Currency::Sle,
            expires_at: Utc::now() + Duration::minutes(15),
            origin: origin(),
        })
        .await
        .expect("hold");

    let bal_a = anbarr_ledger::balances::get_balance(&p, a).await.unwrap();
    assert_eq!(bal_a.available_minor, 90_000, "available drops by the hold");
    assert_eq!(bal_a.held_minor, 10_000);
    assert_eq!(bal_a.total_minor, 100_000, "total unchanged while held");

    let bal_escrow = anbarr_ledger::balances::get_balance(&p, escrow)
        .await
        .unwrap();
    assert_eq!(bal_escrow.available_minor, 10_000);

    // Capture to B (a merchant wallet in this test).
    engine
        .capture_hold(CaptureRequest {
            idempotency_scope: "merchant-1".into(),
            idempotency_key: "capture-1".into(),
            hold_id: hold.hold_id,
            target_account_id: b,
            origin: origin(),
        })
        .await
        .expect("capture");

    let bal_a = anbarr_ledger::balances::get_balance(&p, a).await.unwrap();
    let bal_b = anbarr_ledger::balances::get_balance(&p, b).await.unwrap();
    let bal_escrow = anbarr_ledger::balances::get_balance(&p, escrow)
        .await
        .unwrap();
    assert_eq!(bal_a.available_minor, 90_000);
    assert_eq!(bal_a.held_minor, 0);
    assert_eq!(bal_a.total_minor, 90_000, "buyer paid 10_000");
    assert_eq!(bal_b.available_minor, 10_000, "merchant received 10_000");
    assert_eq!(bal_escrow.available_minor, 0, "escrow empty after capture");

    // Regression: capture changes the payer wallet's held_minor without any
    // entry on that wallet, so its snapshot must be rebuilt in the same
    // transaction or the audit job flags drift.
    let (snap_held, snap_avail): (i64, i64) = sqlx::query_as(
        "SELECT held_minor, available_minor FROM wallet_snapshots WHERE account_id = $1",
    )
    .bind(a)
    .fetch_one(&p)
    .await
    .unwrap();
    assert_eq!(
        snap_held, 0,
        "payer wallet snapshot held_minor stale after capture"
    );
    assert_eq!(snap_avail, 90_000);
}

#[tokio::test]
async fn hold_release_cycle() {
    let _guard = HOLD_LOCK.lock().await;
    let p = pool().await;
    let (engine, a, b, escrow, _fee) = setup(&p).await;

    let hold = engine
        .hold_funds(HoldRequest {
            idempotency_scope: "merchant-1".into(),
            idempotency_key: "hold-2".into(),
            account_id: a,
            amount_minor: 7_500,
            currency: Currency::Sle,
            expires_at: Utc::now() + Duration::minutes(15),
            origin: origin(),
        })
        .await
        .expect("hold");

    engine
        .release_hold(ReleaseRequest {
            idempotency_scope: "merchant-1".into(),
            idempotency_key: "release-2".into(),
            hold_id: hold.hold_id,
            origin: origin(),
        })
        .await
        .expect("release");

    let bal_a = anbarr_ledger::balances::get_balance(&p, a).await.unwrap();
    let bal_escrow = anbarr_ledger::balances::get_balance(&p, escrow)
        .await
        .unwrap();
    let bal_b = anbarr_ledger::balances::get_balance(&p, b).await.unwrap();
    assert_eq!(bal_a.available_minor, 100_000, "released funds return");
    assert_eq!(bal_a.held_minor, 0);
    assert_eq!(bal_escrow.available_minor, 0);
    assert_eq!(bal_b.available_minor, 0);

    // Releasing again must fail (state guard).
    let err = engine
        .release_hold(ReleaseRequest {
            idempotency_scope: "merchant-1".into(),
            idempotency_key: "release-2b".into(),
            hold_id: hold.hold_id,
            origin: origin(),
        })
        .await
        .unwrap_err();
    assert!(matches!(
        err,
        anbarr_ledger::engine::EngineError::HoldNotHeld { .. }
    ));
}

#[tokio::test]
async fn hold_fails_without_funds() {
    let p = pool().await;
    let (engine, a, _b, _escrow, _fee) = setup(&p).await;

    let err = engine
        .hold_funds(HoldRequest {
            idempotency_scope: "merchant-1".into(),
            idempotency_key: "hold-3".into(),
            account_id: a,
            amount_minor: 500_000, // > 100_000 available
            currency: Currency::Sle,
            expires_at: Utc::now() + Duration::minutes(15),
            origin: origin(),
        })
        .await
        .unwrap_err();
    assert!(matches!(
        err,
        anbarr_ledger::engine::EngineError::InsufficientFunds { .. }
    ));
}

// The write-path balance helpers (balances.rs) are exercised indirectly by the
// engine above; these two pin the branches the engine never hits: the empty
// account list early return in `available_for` and the batch `held_for` query.

#[tokio::test]
async fn available_for_empty_account_list_returns_empty_map() {
    let p = pool().await;
    let mut tx = p.begin().await.unwrap();
    let available = anbarr_ledger::balances::available_for(&mut tx, &[])
        .await
        .unwrap();
    assert!(available.is_empty(), "no accounts => no balances");
    tx.rollback().await.unwrap();
}

#[tokio::test]
async fn held_for_reports_open_holds_and_matches_read_path() {
    // Takes the hold lock: placing a hold moves funds into the shared escrow
    // singleton, which the other hold tests assert on.
    let _guard = HOLD_LOCK.lock().await;
    let p = pool().await;
    let (engine, a, b, escrow, _fee) = setup(&p).await;

    let hold = engine
        .hold_funds(HoldRequest {
            idempotency_scope: "balances-hold".into(),
            idempotency_key: "held-for-1".into(),
            account_id: a,
            amount_minor: 4_000,
            currency: Currency::Sle,
            expires_at: Utc::now() + Duration::minutes(15),
            origin: origin(),
        })
        .await
        .expect("hold");

    let mut tx = p.begin().await.unwrap();
    let held = anbarr_ledger::balances::held_for(&mut tx, &[a, b])
        .await
        .unwrap();
    assert_eq!(held.get(&a), Some(&4_000), "open hold is reported");
    assert!(
        !held.contains_key(&b),
        "wallet with no open holds is absent from the map"
    );
    let none = anbarr_ledger::balances::held_for(&mut tx, &[])
        .await
        .unwrap();
    assert!(none.is_empty(), "no accounts => no held amounts");
    tx.rollback().await.unwrap();

    // Cross-check the batch query against the read path.
    let bal = anbarr_ledger::balances::get_balance(&p, a).await.unwrap();
    assert_eq!(bal.available_minor, 100_000 - 4_000);
    assert_eq!(bal.held_minor, 4_000);
    assert_eq!(bal.total_minor, 100_000);

    // Clean up: the escrow is a shared singleton and the other hold tests
    // assert its absolute balance, so leave it at zero.
    engine
        .release_hold(ReleaseRequest {
            idempotency_scope: "balances-hold".into(),
            idempotency_key: "held-for-release".into(),
            hold_id: hold.hold_id,
            origin: origin(),
        })
        .await
        .expect("release");
    let bal_escrow = anbarr_ledger::balances::get_balance(&p, escrow)
        .await
        .unwrap();
    assert_eq!(bal_escrow.available_minor, 0, "escrow left clean");
}

#[tokio::test]
async fn expired_holds_are_swept() {
    let _guard = HOLD_LOCK.lock().await;
    let p = pool().await;
    let (engine, a, _b, escrow, _fee) = setup(&p).await;

    let hold = engine
        .hold_funds(HoldRequest {
            idempotency_scope: "merchant-1".into(),
            idempotency_key: "hold-4".into(),
            account_id: a,
            amount_minor: 5_000,
            currency: Currency::Sle,
            expires_at: Utc::now() - Duration::seconds(1), // already expired
            origin: origin(),
        })
        .await
        .expect("hold");

    // Capturing an expired hold must fail.
    let err = engine
        .capture_hold(CaptureRequest {
            idempotency_scope: "merchant-1".into(),
            idempotency_key: "cap-4".into(),
            hold_id: hold.hold_id,
            target_account_id: a,
            origin: origin(),
        })
        .await
        .unwrap_err();
    assert!(matches!(
        err,
        anbarr_ledger::engine::EngineError::HoldExpired(_)
    ));

    let swept = engine.expire_holds().await.expect("sweep");
    assert_eq!(swept, 1);

    let bal_a = anbarr_ledger::balances::get_balance(&p, a).await.unwrap();
    let bal_escrow = anbarr_ledger::balances::get_balance(&p, escrow)
        .await
        .unwrap();
    assert_eq!(
        bal_a.available_minor, 100_000,
        "expired hold released automatically"
    );
    assert_eq!(bal_escrow.available_minor, 0);
}

// ----------------------------------------------------------------------
// Concurrency
// ----------------------------------------------------------------------

#[tokio::test(flavor = "multi_thread", worker_threads = 8)]
async fn concurrent_transfers_never_overdraw() {
    let p = pool().await;
    let (engine, a, b, _escrow, _fee) = setup(&p).await;
    let engine = std::sync::Arc::new(engine);

    // A has 100_000. 8 concurrent transfers of 10_000 each to B.
    let mut handles = Vec::new();
    for i in 0..8 {
        let engine = engine.clone();
        handles.push(tokio::spawn(async move {
            let spec = JournalSpec {
                journal_type: JournalType::P2p,
                currency: Currency::Sle,
                origin: origin(),
                legs: vec![
                    Leg {
                        account_id: a,
                        direction: Direction::Debit,
                        amount_minor: 10_000,
                    },
                    Leg {
                        account_id: b,
                        direction: Direction::Credit,
                        amount_minor: 10_000,
                    },
                ],
            };
            engine
                .post_journal("user-1", &format!("conc-{i}"), spec)
                .await
                .expect("concurrent post")
        }));
    }
    for h in handles {
        h.await.expect("join");
    }

    let bal_a = anbarr_ledger::balances::get_balance(&p, a).await.unwrap();
    let bal_b = anbarr_ledger::balances::get_balance(&p, b).await.unwrap();
    assert_eq!(bal_a.available_minor, 100_000 - 8 * 10_000);
    assert_eq!(bal_b.available_minor, 8 * 10_000);
    assert!(bal_a.available_minor >= 0, "never overdraw");

    // And an overdraw attempt in the mix must fail while valid ones succeed.
    let over = JournalSpec {
        journal_type: JournalType::P2p,
        currency: Currency::Sle,
        origin: origin(),
        legs: vec![
            Leg {
                account_id: a,
                direction: Direction::Debit,
                amount_minor: 999_999,
            },
            Leg {
                account_id: b,
                direction: Direction::Credit,
                amount_minor: 999_999,
            },
        ],
    };
    let err = engine
        .post_journal("user-1", "conc-over", over)
        .await
        .unwrap_err();
    assert!(matches!(
        err,
        anbarr_ledger::engine::EngineError::InsufficientFunds { .. }
    ));
}

// ----------------------------------------------------------------------
// Audit / invariants
// ----------------------------------------------------------------------

#[tokio::test]
async fn snapshots_match_recomputation_and_audit_is_clean() {
    let p = pool().await;
    let (engine, a, b, _escrow, fee) = setup(&p).await;

    let spec = JournalSpec {
        journal_type: JournalType::P2p,
        currency: Currency::Sle,
        origin: origin(),
        legs: vec![
            Leg {
                account_id: a,
                direction: Direction::Debit,
                amount_minor: 3_333,
            },
            Leg {
                account_id: a,
                direction: Direction::Debit,
                amount_minor: 17,
            }, // 0.5% of 3333 ≈ 16.7 -> 17
            Leg {
                account_id: b,
                direction: Direction::Credit,
                amount_minor: 3_333,
            },
            Leg {
                account_id: fee,
                direction: Direction::Credit,
                amount_minor: 17,
            },
        ],
    };
    engine.post_journal("user-1", "aud-1", spec).await.unwrap();

    let drifted = anbarr_ledger::balances::audit_snapshots(&p).await.unwrap();
    assert!(drifted.is_empty(), "snapshots drifted: {drifted:?}");

    let (snap_avail, snap_held, snap_total): (i64, i64, i64) = sqlx::query_as(
        "SELECT available_minor, held_minor, total_minor FROM wallet_snapshots WHERE account_id = $1",
    )
    .bind(a)
    .fetch_one(&p)
    .await
    .unwrap();
    let bal = anbarr_ledger::balances::get_balance(&p, a).await.unwrap();
    assert_eq!(snap_avail, bal.available_minor);
    assert_eq!(snap_held, bal.held_minor);
    assert_eq!(snap_total, bal.total_minor);
}

// ----------------------------------------------------------------------
// Failure paths & defensive branches
// ----------------------------------------------------------------------

#[tokio::test]
async fn hold_funds_rejects_non_positive_amount() {
    let p = pool().await;
    let (engine, a, _b, _escrow, _fee) = setup(&p).await;

    let err = engine
        .hold_funds(HoldRequest {
            idempotency_scope: "merchant-1".into(),
            idempotency_key: "hold-zero".into(),
            account_id: a,
            amount_minor: 0,
            currency: Currency::Sle,
            expires_at: Utc::now() + Duration::minutes(15),
            origin: origin(),
        })
        .await
        .unwrap_err();
    assert!(matches!(
        err,
        anbarr_ledger::engine::EngineError::InvalidJournal(_)
    ));
}

#[tokio::test]
async fn capture_already_captured_hold_fails() {
    let _guard = HOLD_LOCK.lock().await;
    let p = pool().await;
    let (engine, a, b, _escrow, _fee) = setup(&p).await;

    let hold = engine
        .hold_funds(HoldRequest {
            idempotency_scope: "merchant-1".into(),
            idempotency_key: "hold-cap-dup".into(),
            account_id: a,
            amount_minor: 5_000,
            currency: Currency::Sle,
            expires_at: Utc::now() + Duration::minutes(15),
            origin: origin(),
        })
        .await
        .expect("hold");

    engine
        .capture_hold(CaptureRequest {
            idempotency_scope: "merchant-1".into(),
            idempotency_key: "cap-dup-1".into(),
            hold_id: hold.hold_id,
            target_account_id: b,
            origin: origin(),
        })
        .await
        .expect("first capture");

    // A second capture (fresh key) must hit the status guard.
    let err = engine
        .capture_hold(CaptureRequest {
            idempotency_scope: "merchant-1".into(),
            idempotency_key: "cap-dup-2".into(),
            hold_id: hold.hold_id,
            target_account_id: b,
            origin: origin(),
        })
        .await
        .unwrap_err();
    assert!(matches!(
        err,
        anbarr_ledger::engine::EngineError::HoldNotHeld { .. }
    ));
}

#[tokio::test]
async fn release_fails_when_idempotency_key_in_progress() {
    let _guard = HOLD_LOCK.lock().await;
    let p = pool().await;
    let (engine, a, _b, escrow, _fee) = setup(&p).await;

    let hold = engine
        .hold_funds(HoldRequest {
            idempotency_scope: "rel-ip".into(),
            idempotency_key: "hold-rel-ip".into(),
            account_id: a,
            amount_minor: 3_000,
            currency: Currency::Sle,
            expires_at: Utc::now() + Duration::minutes(15),
            origin: origin(),
        })
        .await
        .expect("hold");

    // A stale in-progress key makes the guarded reversal post fail.
    sqlx::query(
        "INSERT INTO idempotency_keys (scope, key, request_hash, status)
         VALUES ('rel-ip', 'release-ip', 'x', 'in_progress')",
    )
    .execute(&p)
    .await
    .unwrap();

    let err = engine
        .release_hold(ReleaseRequest {
            idempotency_scope: "rel-ip".into(),
            idempotency_key: "release-ip".into(),
            hold_id: hold.hold_id,
            origin: origin(),
        })
        .await
        .unwrap_err();
    assert!(matches!(
        err,
        anbarr_ledger::engine::EngineError::IdempotencyInProgress { .. }
    ));

    // Clean up so the shared escrow singleton is left at zero.
    sqlx::query("DELETE FROM idempotency_keys WHERE scope = 'rel-ip' AND key = 'release-ip'")
        .execute(&p)
        .await
        .unwrap();
    engine
        .release_hold(ReleaseRequest {
            idempotency_scope: "rel-ip".into(),
            idempotency_key: "release-ip-fresh".into(),
            hold_id: hold.hold_id,
            origin: origin(),
        })
        .await
        .expect("release after clearing stale key");
    let bal_escrow = anbarr_ledger::balances::get_balance(&p, escrow)
        .await
        .unwrap();
    assert_eq!(bal_escrow.available_minor, 0);
}

#[tokio::test]
async fn capture_fails_when_idempotency_key_in_progress() {
    let _guard = HOLD_LOCK.lock().await;
    let p = pool().await;
    let (engine, a, b, escrow, _fee) = setup(&p).await;

    let hold = engine
        .hold_funds(HoldRequest {
            idempotency_scope: "cap-ip".into(),
            idempotency_key: "hold-cap-ip".into(),
            account_id: a,
            amount_minor: 3_000,
            currency: Currency::Sle,
            expires_at: Utc::now() + Duration::minutes(15),
            origin: origin(),
        })
        .await
        .expect("hold");

    sqlx::query(
        "INSERT INTO idempotency_keys (scope, key, request_hash, status)
         VALUES ('cap-ip', 'capture-ip', 'x', 'in_progress')",
    )
    .execute(&p)
    .await
    .unwrap();

    let err = engine
        .capture_hold(CaptureRequest {
            idempotency_scope: "cap-ip".into(),
            idempotency_key: "capture-ip".into(),
            hold_id: hold.hold_id,
            target_account_id: b,
            origin: origin(),
        })
        .await
        .unwrap_err();
    assert!(matches!(
        err,
        anbarr_ledger::engine::EngineError::IdempotencyInProgress { .. }
    ));

    // Clean up so the shared escrow singleton is left at zero.
    sqlx::query("DELETE FROM idempotency_keys WHERE scope = 'cap-ip' AND key = 'capture-ip'")
        .execute(&p)
        .await
        .unwrap();
    engine
        .release_hold(ReleaseRequest {
            idempotency_scope: "cap-ip".into(),
            idempotency_key: "capture-ip-cleanup".into(),
            hold_id: hold.hold_id,
            origin: origin(),
        })
        .await
        .expect("release for cleanup");
    let bal_escrow = anbarr_ledger::balances::get_balance(&p, escrow)
        .await
        .unwrap();
    assert_eq!(bal_escrow.available_minor, 0);
}

#[tokio::test]
async fn expire_fails_when_wallet_frozen() {
    let _guard = HOLD_LOCK.lock().await;
    let p = pool().await;
    let (engine, a, _b, escrow, _fee) = setup(&p).await;

    engine
        .hold_funds(HoldRequest {
            idempotency_scope: "merchant-1".into(),
            idempotency_key: "hold-frozen".into(),
            account_id: a,
            amount_minor: 5_000,
            currency: Currency::Sle,
            expires_at: Utc::now() - Duration::seconds(1), // already expired
            origin: origin(),
        })
        .await
        .expect("hold");

    // Freezing the wallet makes the expiry journal's account lock fail.
    sqlx::query("UPDATE accounts SET status = 'frozen' WHERE id = $1")
        .bind(a)
        .execute(&p)
        .await
        .unwrap();
    let err = engine.expire_holds().await.unwrap_err();
    assert!(matches!(
        err,
        anbarr_ledger::engine::EngineError::AccountNotActive(_)
    ));

    // Unfreeze and sweep for real so the shared escrow is left at zero.
    sqlx::query("UPDATE accounts SET status = 'active' WHERE id = $1")
        .bind(a)
        .execute(&p)
        .await
        .unwrap();
    let swept = engine.expire_holds().await.expect("sweep after unfreeze");
    assert_eq!(swept, 1);
    let bal_escrow = anbarr_ledger::balances::get_balance(&p, escrow)
        .await
        .unwrap();
    assert_eq!(bal_escrow.available_minor, 0);
}

#[tokio::test]
async fn posting_with_unknown_account_fails() {
    let p = pool().await;
    let (engine, a, _b, _escrow, _fee) = setup(&p).await;

    let spec = JournalSpec {
        journal_type: JournalType::P2p,
        currency: Currency::Sle,
        origin: origin(),
        legs: vec![
            Leg {
                account_id: a,
                direction: Direction::Debit,
                amount_minor: 1_000,
            },
            Leg {
                account_id: Uuid::new_v4(), // never provisioned
                direction: Direction::Credit,
                amount_minor: 1_000,
            },
        ],
    };
    let err = engine
        .post_journal("user-1", "bogus-acct", spec)
        .await
        .unwrap_err();
    assert!(matches!(
        err,
        anbarr_ledger::engine::EngineError::AccountNotFound(_)
    ));
}

#[tokio::test]
async fn posting_to_frozen_account_fails() {
    let p = pool().await;
    let (engine, a, b, _escrow, _fee) = setup(&p).await;
    sqlx::query("UPDATE accounts SET status = 'frozen' WHERE id = $1")
        .bind(a)
        .execute(&p)
        .await
        .unwrap();

    let spec = JournalSpec {
        journal_type: JournalType::P2p,
        currency: Currency::Sle,
        origin: origin(),
        legs: vec![
            Leg {
                account_id: a,
                direction: Direction::Debit,
                amount_minor: 1_000,
            },
            Leg {
                account_id: b,
                direction: Direction::Credit,
                amount_minor: 1_000,
            },
        ],
    };
    let err = engine
        .post_journal("user-1", "frozen-acct", spec)
        .await
        .unwrap_err();
    assert!(matches!(
        err,
        anbarr_ledger::engine::EngineError::AccountNotActive(_)
    ));
}

#[tokio::test]
async fn idempotency_claim_race_fails() {
    let p = pool().await;
    let (engine, a, b, _escrow, _fee) = setup(&p).await;

    // A concurrent request owns the key in an uncommitted transaction: the
    // engine's existence check misses it, then its claim INSERT loses the race.
    let mut conn = p.acquire().await.unwrap();
    let mut other = conn.begin().await.unwrap();
    sqlx::query(
        "INSERT INTO idempotency_keys (scope, key, request_hash, status)
         VALUES ('race', 'race-1', 'x', 'in_progress')",
    )
    .execute(&mut *other)
    .await
    .unwrap();

    let spec = JournalSpec {
        journal_type: JournalType::P2p,
        currency: Currency::Sle,
        origin: origin(),
        legs: vec![
            Leg {
                account_id: a,
                direction: Direction::Debit,
                amount_minor: 1_000,
            },
            Leg {
                account_id: b,
                direction: Direction::Credit,
                amount_minor: 1_000,
            },
        ],
    };
    let engine2 = engine.clone();
    let task = tokio::spawn(async move { engine2.post_journal("race", "race-1", spec).await });

    // Wait until the post is blocked on the uncommitted key row, then release it.
    for _ in 0..100 {
        let waiting: i64 = sqlx::query_scalar(
            "SELECT count(*) FROM pg_stat_activity
             WHERE state = 'active' AND wait_event_type IS NOT NULL",
        )
        .fetch_one(&p)
        .await
        .unwrap();
        if waiting >= 1 {
            break;
        }
        tokio::time::sleep(std::time::Duration::from_millis(10)).await;
    }
    other.commit().await.unwrap();

    let err = task.await.expect("join").unwrap_err();
    assert!(matches!(
        err,
        anbarr_ledger::engine::EngineError::IdempotencyInProgress { .. }
    ));
}

#[tokio::test]
async fn replay_with_missing_response_fails() {
    let p = pool().await;
    let (engine, a, b, _escrow, _fee) = setup(&p).await;

    // Post once (journal + cache row with a matching hash), then corrupt the
    // cache: a done key with no cached response is a corrupted cache entry.
    let o = origin();
    let spec = || JournalSpec {
        journal_type: JournalType::P2p,
        currency: Currency::Sle,
        origin: o.clone(),
        legs: vec![
            Leg {
                account_id: a,
                direction: Direction::Debit,
                amount_minor: 1_000,
            },
            Leg {
                account_id: b,
                direction: Direction::Credit,
                amount_minor: 1_000,
            },
        ],
    };
    engine
        .post_journal("replay", "missing-resp", spec())
        .await
        .unwrap();
    sqlx::query(
        "UPDATE idempotency_keys SET response = NULL
         WHERE scope = 'replay' AND key = 'missing-resp'",
    )
    .execute(&p)
    .await
    .unwrap();

    let err = engine
        .post_journal("replay", "missing-resp", spec())
        .await
        .unwrap_err();
    assert!(matches!(
        err,
        anbarr_ledger::engine::EngineError::InvalidJournal(_)
    ));
}

#[tokio::test]
async fn replay_with_malformed_response_fails() {
    let p = pool().await;
    let (engine, a, b, _escrow, _fee) = setup(&p).await;

    let o = origin();
    let spec = || JournalSpec {
        journal_type: JournalType::P2p,
        currency: Currency::Sle,
        origin: o.clone(),
        legs: vec![
            Leg {
                account_id: a,
                direction: Direction::Debit,
                amount_minor: 1_000,
            },
            Leg {
                account_id: b,
                direction: Direction::Credit,
                amount_minor: 1_000,
            },
        ],
    };
    engine
        .post_journal("replay", "malformed-resp", spec())
        .await
        .unwrap();
    sqlx::query(
        "UPDATE idempotency_keys SET response = '{\"foo\": 1}'
         WHERE scope = 'replay' AND key = 'malformed-resp'",
    )
    .execute(&p)
    .await
    .unwrap();

    let err = engine
        .post_journal("replay", "malformed-resp", spec())
        .await
        .unwrap_err();
    assert!(matches!(
        err,
        anbarr_ledger::engine::EngineError::InvalidJournal(_)
    ));
}

#[tokio::test]
async fn same_key_with_different_payload_is_rejected() {
    let p = pool().await;
    let (engine, a, b, _escrow, _fee) = setup(&p).await;

    // Identical origin, different amount: a genuine payload mismatch.
    let o = origin();
    let first = JournalSpec {
        journal_type: JournalType::P2p,
        currency: Currency::Sle,
        origin: o.clone(),
        legs: vec![
            Leg {
                account_id: a,
                direction: Direction::Debit,
                amount_minor: 10_000,
            },
            Leg {
                account_id: b,
                direction: Direction::Credit,
                amount_minor: 10_000,
            },
        ],
    };
    let different = JournalSpec {
        journal_type: JournalType::P2p,
        currency: Currency::Sle,
        origin: o,
        legs: vec![
            Leg {
                account_id: a,
                direction: Direction::Debit,
                amount_minor: 20_000,
            },
            Leg {
                account_id: b,
                direction: Direction::Credit,
                amount_minor: 20_000,
            },
        ],
    };

    engine
        .post_journal("user-1", "mismatch-1", first)
        .await
        .expect("first post");
    let err = engine
        .post_journal("user-1", "mismatch-1", different)
        .await
        .unwrap_err();
    assert!(
        matches!(
            err,
            anbarr_ledger::engine::EngineError::IdempotencyMismatch { .. }
        ),
        "expected IdempotencyMismatch, got {err:?}"
    );

    // Only the first debit happened.
    let bal_a = anbarr_ledger::balances::get_balance(&p, a).await.unwrap();
    assert_eq!(bal_a.available_minor, 100_000 - 10_000);
}

#[tokio::test]
async fn expired_done_key_replays_from_journal() {
    let p = pool().await;
    let (engine, a, b, _escrow, _fee) = setup(&p).await;

    let o = origin();
    let spec = || JournalSpec {
        journal_type: JournalType::P2p,
        currency: Currency::Sle,
        origin: o.clone(),
        legs: vec![
            Leg {
                account_id: a,
                direction: Direction::Debit,
                amount_minor: 10_000,
            },
            Leg {
                account_id: b,
                direction: Direction::Credit,
                amount_minor: 10_000,
            },
        ],
    };
    let first = engine
        .post_journal("user-1", "exp-done", spec())
        .await
        .unwrap();

    // The cache row expires; the durable journals row must still replay.
    sqlx::query(
        "UPDATE idempotency_keys SET expires_at = now() - interval '1 hour'
         WHERE scope = 'user-1' AND key = 'exp-done'",
    )
    .execute(&p)
    .await
    .unwrap();

    let replay = engine
        .post_journal("user-1", "exp-done", spec())
        .await
        .unwrap();
    assert_eq!(
        first.journal_id, replay.journal_id,
        "expired cache still replays the original journal"
    );

    let bal_a = anbarr_ledger::balances::get_balance(&p, a).await.unwrap();
    assert_eq!(bal_a.available_minor, 90_000, "single debit only");
}

#[tokio::test]
async fn pruned_cache_still_replays_from_journal() {
    let p = pool().await;
    let (engine, a, b, _escrow, _fee) = setup(&p).await;

    let o = origin();
    let spec = || JournalSpec {
        journal_type: JournalType::P2p,
        currency: Currency::Sle,
        origin: o.clone(),
        legs: vec![
            Leg {
                account_id: a,
                direction: Direction::Debit,
                amount_minor: 10_000,
            },
            Leg {
                account_id: b,
                direction: Direction::Credit,
                amount_minor: 10_000,
            },
        ],
    };
    let first = engine
        .post_journal("user-1", "pruned-1", spec())
        .await
        .unwrap();

    // The whole cache row is gone (pruned); the journals row is the backstop.
    sqlx::query("DELETE FROM idempotency_keys WHERE scope = 'user-1' AND key = 'pruned-1'")
        .execute(&p)
        .await
        .unwrap();

    let replay = engine
        .post_journal("user-1", "pruned-1", spec())
        .await
        .unwrap();
    assert_eq!(
        first.journal_id, replay.journal_id,
        "pruned cache replays the original journal"
    );

    let bal_a = anbarr_ledger::balances::get_balance(&p, a).await.unwrap();
    assert_eq!(bal_a.available_minor, 90_000, "single debit only");
}

#[tokio::test]
async fn expired_in_progress_key_is_reclaimed() {
    let p = pool().await;
    let (engine, a, b, _escrow, _fee) = setup(&p).await;

    // A stale claim: the request died before posting anything. After expiry
    // the key must be reclaimable, not blocked forever.
    sqlx::query(
        "INSERT INTO idempotency_keys (scope, key, request_hash, status, expires_at)
         VALUES ('stale', 'stale-1', 'x', 'in_progress', now() - interval '1 hour')",
    )
    .execute(&p)
    .await
    .unwrap();

    let o = origin();
    let spec = JournalSpec {
        journal_type: JournalType::P2p,
        currency: Currency::Sle,
        origin: o,
        legs: vec![
            Leg {
                account_id: a,
                direction: Direction::Debit,
                amount_minor: 10_000,
            },
            Leg {
                account_id: b,
                direction: Direction::Credit,
                amount_minor: 10_000,
            },
        ],
    };
    let result = engine
        .post_journal("stale", "stale-1", spec)
        .await
        .expect("expired in-progress key is reclaimed and posts");
    assert_eq!(result.status, anbarr_ledger::types::JournalStatus::Posted);
    let bal_a = anbarr_ledger::balances::get_balance(&p, a).await.unwrap();
    assert_eq!(bal_a.available_minor, 90_000, "funds actually moved");
}

#[tokio::test]
async fn expired_in_progress_key_with_journal_replays() {
    let p = pool().await;
    let (engine, a, b, _escrow, _fee) = setup(&p).await;

    let o = origin();
    let spec = || JournalSpec {
        journal_type: JournalType::P2p,
        currency: Currency::Sle,
        origin: o.clone(),
        legs: vec![
            Leg {
                account_id: a,
                direction: Direction::Debit,
                amount_minor: 10_000,
            },
            Leg {
                account_id: b,
                direction: Direction::Credit,
                amount_minor: 10_000,
            },
        ],
    };
    let first = engine
        .post_journal("crash", "crash-1", spec())
        .await
        .unwrap();

    // Simulate a crash after commit but before the cache response was written:
    // the cache is stuck in-progress and expired, while the journal exists.
    sqlx::query(
        "UPDATE idempotency_keys SET status = 'in_progress', response = NULL,
                expires_at = now() - interval '1 hour'
         WHERE scope = 'crash' AND key = 'crash-1'",
    )
    .execute(&p)
    .await
    .unwrap();

    let replay = engine
        .post_journal("crash", "crash-1", spec())
        .await
        .unwrap();
    assert_eq!(
        first.journal_id, replay.journal_id,
        "journal exists => replay, never a second post"
    );
    let bal_a = anbarr_ledger::balances::get_balance(&p, a).await.unwrap();
    assert_eq!(bal_a.available_minor, 90_000, "single debit only");
}

#[tokio::test]
async fn prune_removes_only_expired_keys() {
    let p = pool().await;
    let engine = LedgerEngine::new(p.clone());

    // Two expired rows (one done, one in-progress) and two rows that must
    // survive: a future-expiry in-progress claim and a legacy NULL expiry.
    sqlx::query(
        "INSERT INTO idempotency_keys (scope, key, request_hash, status, expires_at)
         VALUES ('prune', 'expired-done', 'x', 'done', now() - interval '1 hour')",
    )
    .execute(&p)
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO idempotency_keys (scope, key, request_hash, status, expires_at)
         VALUES ('prune', 'expired-in-progress', 'x', 'in_progress', now() - interval '1 hour')",
    )
    .execute(&p)
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO idempotency_keys (scope, key, request_hash, status, expires_at)
         VALUES ('prune', 'valid', 'x', 'in_progress', now() + interval '1 hour')",
    )
    .execute(&p)
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO idempotency_keys (scope, key, request_hash, status)
         VALUES ('prune', 'never', 'x', 'done')",
    )
    .execute(&p)
    .await
    .unwrap();

    let removed = engine
        .prune_expired_idempotency_keys()
        .await
        .expect("prune");
    assert!(
        removed >= 2,
        "the two expired prune rows are always removed (got {removed})"
    );

    let remaining: i64 =
        sqlx::query_scalar("SELECT count(*) FROM idempotency_keys WHERE scope = 'prune'")
            .fetch_one(&p)
            .await
            .unwrap();
    assert_eq!(remaining, 2, "valid and NULL-expiry rows survive");
}
