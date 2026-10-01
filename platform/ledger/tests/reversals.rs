//! Tests for engine-level reversal (`reverse_journal`) and refund
//! (`refund_payment`) operations, per docs/transaction-state-machine.md §3.
//!
//! All DB tests are serialized on a process-local mutex and truncate between
//! tests: `fee_revenue` is a shared singleton, so exact absolute balances are
//! only meaningful when one test observes it at a time (same pattern as the
//! HOLD_LOCK in engine.rs / FEES_LOCK in fees.rs).

mod common;

use anbarr_ledger::engine::{
    EngineError, FeePolicy, LedgerEngine, PaymentRequest, RefundRequest, ReversalRequest,
};
use anbarr_ledger::money::Currency;
use anbarr_ledger::types::{Direction, JournalSpec, JournalType, Leg, Origin};
use sqlx::PgPool;
use uuid::Uuid;

use common::{create_wallet, platform_account, pool, truncate_all};

static REVERSALS_LOCK: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());

fn origin() -> Origin {
    Origin {
        user_id: Some(Uuid::new_v4()),
        channel: "reversals-test".into(),
        session_id: Some("sess-rev".into()),
        payment_code: None,
    }
}

/// Post a simple p2p payment (payer -> payee, 0.5% fee charged to the payer).
async fn post_payment(
    engine: &LedgerEngine,
    payer: Uuid,
    payee: Uuid,
    key: &str,
    amount: i64,
) -> Uuid {
    engine
        .post_payment(PaymentRequest {
            idempotency_scope: "rev-test".into(),
            idempotency_key: key.into(),
            journal_type: JournalType::P2p,
            currency: Currency::Sle,
            origin: origin(),
            payer_account_id: payer,
            payee_account_id: payee,
            amount_minor: amount,
            fee: FeePolicy { bps: 50 },
            tax: None,
            agent_commission: None,
        })
        .await
        .expect("post payment")
        .journal_id
}

async fn balance(p: &PgPool, account: Uuid) -> i64 {
    anbarr_ledger::balances::get_balance(p, account)
        .await
        .unwrap()
        .available_minor
}

// ----------------------------------------------------------------------
// Reversals
// ----------------------------------------------------------------------

#[tokio::test]
async fn reversal_returns_everything_including_fee() {
    let _guard = REVERSALS_LOCK.lock().await;
    let p = pool().await;
    truncate_all(&p).await;
    let engine = LedgerEngine::new(p.clone());
    let payer = create_wallet(&p, Currency::Sle, 100_000).await;
    let payee = create_wallet(&p, Currency::Sle, 0).await;
    let fee_rev = platform_account(&p, "fee_revenue", Currency::Sle).await;

    let original = post_payment(&engine, payer, payee, "rev-pay-1", 25_000).await;
    assert_eq!(balance(&p, payer).await, 100_000 - 25_125);
    assert_eq!(balance(&p, payee).await, 25_000);
    assert_eq!(balance(&p, fee_rev).await, 125);

    let reversal = engine
        .reverse_journal(ReversalRequest {
            idempotency_scope: "rev-test".into(),
            idempotency_key: "rev-1".into(),
            original_journal_id: original,
            origin: origin(),
        })
        .await
        .expect("reverse");

    // Everything — principal and fee — returns to the payer.
    assert_eq!(balance(&p, payer).await, 100_000);
    assert_eq!(balance(&p, payee).await, 0);
    assert_eq!(balance(&p, fee_rev).await, 0);

    // The reversal is a real posted journal, linked to the original, and the
    // original's entries are untouched.
    let (jtype, status, reference): (String, String, Option<String>) =
        sqlx::query_as("SELECT type, status, reference FROM journals WHERE id = $1")
            .bind(reversal.journal_id)
            .fetch_one(&p)
            .await
            .unwrap();
    assert_eq!(jtype, "reversal");
    assert_eq!(status, "posted");
    assert_eq!(reference, Some(original.to_string()));

    let legs: i64 = sqlx::query_scalar("SELECT count(*) FROM entries WHERE journal_id = $1")
        .bind(reversal.journal_id)
        .fetch_one(&p)
        .await
        .unwrap();
    assert_eq!(legs, 3, "reversal negates every leg of the original");
    let original_legs: i64 =
        sqlx::query_scalar("SELECT count(*) FROM entries WHERE journal_id = $1")
            .bind(original)
            .fetch_one(&p)
            .await
            .unwrap();
    assert_eq!(original_legs, 3, "original entries are never touched");
}

#[tokio::test]
async fn double_reversal_rejected() {
    let _guard = REVERSALS_LOCK.lock().await;
    let p = pool().await;
    truncate_all(&p).await;
    let engine = LedgerEngine::new(p.clone());
    let payer = create_wallet(&p, Currency::Sle, 100_000).await;
    let payee = create_wallet(&p, Currency::Sle, 0).await;

    let original = post_payment(&engine, payer, payee, "rev-pay-2", 10_000).await;
    engine
        .reverse_journal(ReversalRequest {
            idempotency_scope: "rev-test".into(),
            idempotency_key: "rev-2a".into(),
            original_journal_id: original,
            origin: origin(),
        })
        .await
        .expect("first reversal");

    // A second reversal of the same original (fresh key) is rejected.
    let err = engine
        .reverse_journal(ReversalRequest {
            idempotency_scope: "rev-test".into(),
            idempotency_key: "rev-2b".into(),
            original_journal_id: original,
            origin: origin(),
        })
        .await
        .unwrap_err();
    assert!(
        matches!(err, EngineError::AlreadyReversed(_)),
        "got {err:?}"
    );

    // The reversal journal itself is not reversible (corrections are new
    // journals, not chains).
    let rev_journal_id = sqlx::query_scalar::<_, Uuid>(
        "SELECT id FROM journals WHERE reference = $1::text AND type = 'reversal'",
    )
    .bind(original)
    .fetch_one(&p)
    .await
    .unwrap();
    let err = engine
        .reverse_journal(ReversalRequest {
            idempotency_scope: "rev-test".into(),
            idempotency_key: "rev-2c".into(),
            original_journal_id: rev_journal_id,
            origin: origin(),
        })
        .await
        .unwrap_err();
    assert!(
        matches!(err, EngineError::NotReversible { .. }),
        "got {err:?}"
    );

    // Money still returned exactly once.
    assert_eq!(balance(&p, payer).await, 100_000);
    assert_eq!(balance(&p, payee).await, 0);
}

#[tokio::test]
async fn hold_journal_is_not_reversible() {
    let _guard = REVERSALS_LOCK.lock().await;
    let p = pool().await;
    truncate_all(&p).await;
    let engine = LedgerEngine::new(p.clone());
    let payer = create_wallet(&p, Currency::Sle, 100_000).await;
    let escrow = platform_account(&p, "hold_escrow", Currency::Sle).await;

    let hold = engine
        .hold_funds(anbarr_ledger::engine::HoldRequest {
            idempotency_scope: "rev-test".into(),
            idempotency_key: "rev-hold".into(),
            account_id: payer,
            amount_minor: 5_000,
            currency: Currency::Sle,
            expires_at: chrono::Utc::now() + chrono::Duration::minutes(15),
            origin: origin(),
        })
        .await
        .expect("hold");

    let err = engine
        .reverse_journal(ReversalRequest {
            idempotency_scope: "rev-test".into(),
            idempotency_key: "rev-hold-rev".into(),
            original_journal_id: hold.journal_id,
            origin: origin(),
        })
        .await
        .unwrap_err();
    assert!(
        matches!(err, EngineError::NotReversible { .. }),
        "holds are owned by the holds lifecycle, got {err:?}"
    );

    // Clean up the shared escrow singleton via the proper path.
    engine
        .release_hold(anbarr_ledger::engine::ReleaseRequest {
            idempotency_scope: "rev-test".into(),
            idempotency_key: "rev-hold-rel".into(),
            hold_id: hold.hold_id,
            origin: origin(),
        })
        .await
        .expect("release");
    assert_eq!(balance(&p, escrow).await, 0);
}

#[tokio::test]
async fn unposted_journal_is_not_reversible() {
    let _guard = REVERSALS_LOCK.lock().await;
    let p = pool().await;
    truncate_all(&p).await;
    let engine = LedgerEngine::new(p.clone());

    // A journal stuck in pending (no entries) has never moved money.
    let pending = Uuid::new_v4();
    sqlx::query(
        "INSERT INTO journals (id, type, status, currency, amount_minor, idempotency_scope, idempotency_key)
         VALUES ($1, 'p2p', 'pending', 'SLE', 1000, 'rev-test', 'rev-pending')",
    )
    .bind(pending)
    .execute(&p)
    .await
    .unwrap();

    let err = engine
        .reverse_journal(ReversalRequest {
            idempotency_scope: "rev-test".into(),
            idempotency_key: "rev-pending-rev".into(),
            original_journal_id: pending,
            origin: origin(),
        })
        .await
        .unwrap_err();
    assert!(
        matches!(err, EngineError::JournalNotPosted { .. }),
        "got {err:?}"
    );
}

// ----------------------------------------------------------------------
// Refunds
// ----------------------------------------------------------------------

#[tokio::test]
async fn refund_returns_principal_to_payer_fee_stays_earned() {
    let _guard = REVERSALS_LOCK.lock().await;
    let p = pool().await;
    truncate_all(&p).await;
    let engine = LedgerEngine::new(p.clone());
    let payer = create_wallet(&p, Currency::Sle, 100_000).await;
    let payee = create_wallet(&p, Currency::Sle, 0).await;
    let fee_rev = platform_account(&p, "fee_revenue", Currency::Sle).await;

    let original = post_payment(&engine, payer, payee, "ref-pay-1", 10_000).await;

    let refund = engine
        .refund_payment(RefundRequest {
            idempotency_scope: "rev-test".into(),
            idempotency_key: "ref-1".into(),
            original_journal_id: original,
            refund_from_account_id: payee,
            refund_to_account_id: payer,
            amount_minor: 4_000,
            origin: origin(),
        })
        .await
        .expect("partial refund");

    assert_eq!(balance(&p, payee).await, 6_000, "payee returns principal");
    assert_eq!(
        balance(&p, payer).await,
        100_000 - 10_050 + 4_000,
        "payer gets the principal back, not the fee"
    );
    assert_eq!(balance(&p, fee_rev).await, 50, "platform fee stays earned");

    let (jtype, reference): (String, Option<String>) =
        sqlx::query_as("SELECT type, reference FROM journals WHERE id = $1")
            .bind(refund.journal_id)
            .fetch_one(&p)
            .await
            .unwrap();
    assert_eq!(jtype, "refund");
    assert_eq!(reference, Some(original.to_string()));
}

#[tokio::test]
async fn partial_refunds_cannot_exceed_principal() {
    let _guard = REVERSALS_LOCK.lock().await;
    let p = pool().await;
    truncate_all(&p).await;
    let engine = LedgerEngine::new(p.clone());
    let payer = create_wallet(&p, Currency::Sle, 100_000).await;
    let payee = create_wallet(&p, Currency::Sle, 0).await;

    let original = post_payment(&engine, payer, payee, "ref-pay-2", 10_000).await;

    engine
        .refund_payment(RefundRequest {
            idempotency_scope: "rev-test".into(),
            idempotency_key: "ref-2a".into(),
            original_journal_id: original,
            refund_from_account_id: payee,
            refund_to_account_id: payer,
            amount_minor: 6_000,
            origin: origin(),
        })
        .await
        .expect("first partial refund");

    let err = engine
        .refund_payment(RefundRequest {
            idempotency_scope: "rev-test".into(),
            idempotency_key: "ref-2b".into(),
            original_journal_id: original,
            refund_from_account_id: payee,
            refund_to_account_id: payer,
            amount_minor: 5_000,
            origin: origin(),
        })
        .await
        .unwrap_err();
    assert!(
        matches!(err, EngineError::RefundExceedsPrincipal { .. }),
        "got {err:?}"
    );

    assert_eq!(balance(&p, payee).await, 4_000, "no over-refund");
    assert_eq!(balance(&p, payer).await, 100_000 - 10_050 + 6_000);
}

#[tokio::test]
async fn refund_source_must_be_the_payee() {
    let _guard = REVERSALS_LOCK.lock().await;
    let p = pool().await;
    truncate_all(&p).await;
    let engine = LedgerEngine::new(p.clone());
    let payer = create_wallet(&p, Currency::Sle, 100_000).await;
    let payee = create_wallet(&p, Currency::Sle, 0).await;
    let stranger = create_wallet(&p, Currency::Sle, 0).await;

    let original = post_payment(&engine, payer, payee, "ref-pay-3", 10_000).await;

    let err = engine
        .refund_payment(RefundRequest {
            idempotency_scope: "rev-test".into(),
            idempotency_key: "ref-3".into(),
            original_journal_id: original,
            refund_from_account_id: stranger,
            refund_to_account_id: payer,
            amount_minor: 1_000,
            origin: origin(),
        })
        .await
        .unwrap_err();
    assert!(
        matches!(err, EngineError::RefundFromMismatch { .. }),
        "got {err:?}"
    );
    assert_eq!(balance(&p, stranger).await, 0, "nothing moved");
}

#[tokio::test]
async fn refund_is_idempotent() {
    let _guard = REVERSALS_LOCK.lock().await;
    let p = pool().await;
    truncate_all(&p).await;
    let engine = LedgerEngine::new(p.clone());
    let payer = create_wallet(&p, Currency::Sle, 100_000).await;
    let payee = create_wallet(&p, Currency::Sle, 0).await;

    let original = post_payment(&engine, payer, payee, "ref-pay-4", 10_000).await;
    let req = RefundRequest {
        idempotency_scope: "rev-test".into(),
        idempotency_key: "ref-4".into(),
        original_journal_id: original,
        refund_from_account_id: payee,
        refund_to_account_id: payer,
        amount_minor: 2_000,
        origin: origin(),
    };

    let first = engine.refund_payment(req.clone()).await.expect("refund");
    let replay = engine.refund_payment(req).await.expect("replay");
    assert_eq!(
        first.journal_id, replay.journal_id,
        "replay returns the original refund journal"
    );

    let count: i64 = sqlx::query_scalar("SELECT count(*) FROM journals WHERE type = 'refund'")
        .fetch_one(&p)
        .await
        .unwrap();
    assert_eq!(count, 1, "exactly one refund journal");
    assert_eq!(balance(&p, payee).await, 8_000, "refunded once");
}

#[tokio::test]
async fn non_refundable_journal_type_rejected() {
    let _guard = REVERSALS_LOCK.lock().await;
    let p = pool().await;
    truncate_all(&p).await;
    let engine = LedgerEngine::new(p.clone());
    let fee_rev = platform_account(&p, "fee_revenue", Currency::Sle).await;
    let platform_rev = platform_account(&p, "platform_revenue", Currency::Sle).await;

    // Fund fee revenue so the standalone fee journal has something to debit.
    let bridge = platform_account(&p, "rail_bridge", Currency::Sle).await;
    engine
        .post_journal(
            "rev-test",
            "ref-fee-fund",
            JournalSpec {
                journal_type: JournalType::Topup,
                currency: Currency::Sle,
                origin: origin(),
                legs: vec![
                    Leg {
                        account_id: bridge,
                        direction: Direction::Debit,
                        amount_minor: 1_000,
                    },
                    Leg {
                        account_id: fee_rev,
                        direction: Direction::Credit,
                        amount_minor: 1_000,
                    },
                ],
            },
        )
        .await
        .expect("fund fee revenue");

    // A standalone fee journal has no payee to refund.
    let fee_journal = engine
        .post_journal(
            "rev-test",
            "ref-fee-1",
            JournalSpec {
                journal_type: JournalType::Fee,
                currency: Currency::Sle,
                origin: origin(),
                legs: vec![
                    Leg {
                        account_id: fee_rev,
                        direction: Direction::Debit,
                        amount_minor: 500,
                    },
                    Leg {
                        account_id: platform_rev,
                        direction: Direction::Credit,
                        amount_minor: 500,
                    },
                ],
            },
        )
        .await
        .expect("post fee journal")
        .journal_id;

    let err = engine
        .refund_payment(RefundRequest {
            idempotency_scope: "rev-test".into(),
            idempotency_key: "ref-fee".into(),
            original_journal_id: fee_journal,
            refund_from_account_id: fee_rev,
            refund_to_account_id: platform_rev,
            amount_minor: 100,
            origin: origin(),
        })
        .await
        .unwrap_err();
    assert!(
        matches!(err, EngineError::NotRefundable { .. }),
        "got {err:?}"
    );
}

#[tokio::test]
async fn reversal_and_refund_are_mutually_exclusive() {
    let _guard = REVERSALS_LOCK.lock().await;
    let p = pool().await;
    truncate_all(&p).await;
    let engine = LedgerEngine::new(p.clone());
    let payer = create_wallet(&p, Currency::Sle, 100_000).await;
    let payee = create_wallet(&p, Currency::Sle, 0).await;

    // Refund first, then try to reverse: the partial refund settles part of
    // the payment, so a full reversal would double-return money.
    let original = post_payment(&engine, payer, payee, "mut-pay-1", 10_000).await;
    engine
        .refund_payment(RefundRequest {
            idempotency_scope: "rev-test".into(),
            idempotency_key: "mut-ref".into(),
            original_journal_id: original,
            refund_from_account_id: payee,
            refund_to_account_id: payer,
            amount_minor: 2_000,
            origin: origin(),
        })
        .await
        .expect("refund");

    let err = engine
        .reverse_journal(ReversalRequest {
            idempotency_scope: "rev-test".into(),
            idempotency_key: "mut-rev".into(),
            original_journal_id: original,
            origin: origin(),
        })
        .await
        .unwrap_err();
    assert!(matches!(err, EngineError::HasRefunds(_)), "got {err:?}");

    // And the mirror image: reversed first, then refund is rejected.
    let other_payer = create_wallet(&p, Currency::Sle, 100_000).await;
    let other_payee = create_wallet(&p, Currency::Sle, 0).await;
    let original2 = post_payment(&engine, other_payer, other_payee, "mut-pay-2", 10_000).await;
    engine
        .reverse_journal(ReversalRequest {
            idempotency_scope: "rev-test".into(),
            idempotency_key: "mut-rev2".into(),
            original_journal_id: original2,
            origin: origin(),
        })
        .await
        .expect("reversal");

    let err = engine
        .refund_payment(RefundRequest {
            idempotency_scope: "rev-test".into(),
            idempotency_key: "mut-ref2".into(),
            original_journal_id: original2,
            refund_from_account_id: other_payee,
            refund_to_account_id: other_payer,
            amount_minor: 1_000,
            origin: origin(),
        })
        .await
        .unwrap_err();
    assert!(
        matches!(err, EngineError::JournalReversed(_)),
        "got {err:?}"
    );
}

#[tokio::test]
async fn refund_fails_without_payee_funds() {
    let _guard = REVERSALS_LOCK.lock().await;
    let p = pool().await;
    truncate_all(&p).await;
    let engine = LedgerEngine::new(p.clone());
    let payer = create_wallet(&p, Currency::Sle, 100_000).await;
    let payee = create_wallet(&p, Currency::Sle, 0).await;
    let third = create_wallet(&p, Currency::Sle, 0).await;

    let original = post_payment(&engine, payer, payee, "ref-pay-5", 10_000).await;

    // The payee spends everything before the refund arrives.
    engine
        .post_journal(
            "rev-test",
            "ref-spend",
            JournalSpec {
                journal_type: JournalType::P2p,
                currency: Currency::Sle,
                origin: origin(),
                legs: vec![
                    Leg {
                        account_id: payee,
                        direction: Direction::Debit,
                        amount_minor: 10_000,
                    },
                    Leg {
                        account_id: third,
                        direction: Direction::Credit,
                        amount_minor: 10_000,
                    },
                ],
            },
        )
        .await
        .expect("payee spends");

    let err = engine
        .refund_payment(RefundRequest {
            idempotency_scope: "rev-test".into(),
            idempotency_key: "ref-5".into(),
            original_journal_id: original,
            refund_from_account_id: payee,
            refund_to_account_id: payer,
            amount_minor: 1_000,
            origin: origin(),
        })
        .await
        .unwrap_err();
    assert!(
        matches!(err, EngineError::InsufficientFunds { .. }),
        "the funds rule applies to refunds too, got {err:?}"
    );
}
