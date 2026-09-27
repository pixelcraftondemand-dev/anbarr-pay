//! Tests for the fee + agent-commission journals.
//!
//! The DB tests are serialized on a process-local mutex and truncate between
//! tests: `fee_revenue` is a shared singleton, so exact absolute balances are
//! only meaningful when one test observes it at a time (same pattern as
//! HOLD_LOCK in engine.rs / RECONCILE_LOCK in reconcile.rs).

mod common;

use anbarr_ledger::engine::{AgentCommission, FeePolicy, LedgerEngine, PaymentRequest};
use anbarr_ledger::money::Currency;
use anbarr_ledger::types::{Direction, JournalType, Origin};
use sqlx::PgPool;
use uuid::Uuid;

use common::{create_wallet, platform_account, pool, truncate_all};

static FEES_LOCK: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());

fn origin() -> Origin {
    Origin {
        user_id: Some(Uuid::new_v4()),
        channel: "fees-test".into(),
        session_id: Some("sess-fee".into()),
        payment_code: None,
    }
}

/// A per-agent float account (owner_type agent, type float).
async fn create_float(p: &PgPool, initial: i64) -> Uuid {
    let id = Uuid::new_v4();
    sqlx::query(
        "INSERT INTO accounts (id, owner_type, owner_id, type, currency, name, status)
         VALUES ($1, 'agent', $2, 'float', 'SLE', $3, 'active')",
    )
    .bind(id)
    .bind(id)
    .bind("agent float")
    .execute(p)
    .await
    .expect("insert agent float");
    if initial > 0 {
        common::seed_funds(p, id, Currency::Sle, initial).await;
    }
    id
}

fn payment_req(
    key: &str,
    payer: Uuid,
    payee: Uuid,
    amount: i64,
    fee_bps: u32,
    commission: Option<(u32, Uuid)>,
) -> PaymentRequest {
    PaymentRequest {
        idempotency_scope: "merchant-1".into(),
        idempotency_key: key.into(),
        journal_type: JournalType::Checkout,
        currency: Currency::Sle,
        origin: origin(),
        payer_account_id: payer,
        payee_account_id: payee,
        amount_minor: amount,
        fee: FeePolicy { bps: fee_bps },
        tax: None,
        agent_commission: commission.map(|(bps, float)| AgentCommission {
            bps,
            agent_float_account_id: float,
        }),
    }
}

// ----------------------------------------------------------------------
// Pure leg structure
// ----------------------------------------------------------------------

#[test]
fn payment_legs_charge_fee_to_payer_and_skip_zero_fee() {
    use anbarr_ledger::engine::payment_legs;
    let (payer, payee, fee_rev, tax) = (Uuid::nil(), Uuid::nil(), Uuid::nil(), Uuid::nil());

    // 0.5% of 25_000 = 125: payer pays 25_125, payee gets 25_000, fee leg present.
    let legs = payment_legs(25_000, 50, payer, payee, fee_rev, 0, tax);
    assert_eq!(legs.len(), 3);
    assert_eq!(legs[0].direction, Direction::Debit);
    assert_eq!(legs[0].amount_minor, 25_125);
    assert_eq!(legs[1].direction, Direction::Credit);
    assert_eq!(legs[1].amount_minor, 25_000);
    assert_eq!(legs[2].direction, Direction::Credit);
    assert_eq!(legs[2].amount_minor, 125);
    let debits: i64 = legs
        .iter()
        .filter(|l| l.direction == Direction::Debit)
        .map(|l| l.amount_minor)
        .sum();
    let credits: i64 = legs
        .iter()
        .filter(|l| l.direction == Direction::Credit)
        .map(|l| l.amount_minor)
        .sum();
    assert_eq!(debits, credits, "journal must balance");

    // Dust: 0.5% of 1 rounds to 0 -> no fee leg, exactly 2 legs.
    let legs = payment_legs(1, 50, payer, payee, fee_rev, 0, tax);
    assert_eq!(legs.len(), 2, "dust transaction carries no fee leg");
    assert_eq!(legs[0].amount_minor, 1);
    assert_eq!(legs[1].amount_minor, 1);
}

// ----------------------------------------------------------------------
// End-to-end against Postgres
// ----------------------------------------------------------------------

#[tokio::test]
async fn fee_charged_to_paying_party() {
    let _guard = FEES_LOCK.lock().await;
    let p = pool().await;
    truncate_all(&p).await;
    let engine = LedgerEngine::new(p.clone());
    let payer = create_wallet(&p, Currency::Sle, 100_000).await;
    let payee = create_wallet(&p, Currency::Sle, 0).await;
    let fee_rev = platform_account(&p, "fee_revenue", Currency::Sle).await;
    let fee_before = anbarr_ledger::balances::get_balance(&p, fee_rev)
        .await
        .unwrap()
        .available_minor;

    let res = engine
        .post_payment(payment_req("fee-1", payer, payee, 25_000, 50, None))
        .await
        .expect("post payment");
    assert_eq!(res.fee_minor, 125);
    assert_eq!(res.commission_minor, 0);
    assert!(res.commission_journal_id.is_none());

    let bal_payer = anbarr_ledger::balances::get_balance(&p, payer)
        .await
        .unwrap();
    let bal_payee = anbarr_ledger::balances::get_balance(&p, payee)
        .await
        .unwrap();
    let bal_fee = anbarr_ledger::balances::get_balance(&p, fee_rev)
        .await
        .unwrap();
    assert_eq!(bal_payer.available_minor, 100_000 - 25_125);
    assert_eq!(bal_payee.available_minor, 25_000);
    assert_eq!(bal_fee.available_minor, fee_before + 125);

    let legs: i64 = sqlx::query_scalar("SELECT count(*) FROM entries WHERE journal_id = $1")
        .bind(res.journal_id)
        .fetch_one(&p)
        .await
        .unwrap();
    assert_eq!(legs, 3, "one journal: payer, payee, fee");
}

#[tokio::test]
async fn dust_transaction_posts_without_fee() {
    let _guard = FEES_LOCK.lock().await;
    let p = pool().await;
    truncate_all(&p).await;
    let engine = LedgerEngine::new(p.clone());
    let payer = create_wallet(&p, Currency::Sle, 100).await;
    let payee = create_wallet(&p, Currency::Sle, 0).await;
    let fee_rev = platform_account(&p, "fee_revenue", Currency::Sle).await;
    let fee_before = anbarr_ledger::balances::get_balance(&p, fee_rev)
        .await
        .unwrap()
        .available_minor;

    let res = engine
        .post_payment(payment_req("fee-dust", payer, payee, 1, 50, None))
        .await
        .expect("post dust payment");
    assert_eq!(res.fee_minor, 0);

    let bal_payer = anbarr_ledger::balances::get_balance(&p, payer)
        .await
        .unwrap();
    let bal_payee = anbarr_ledger::balances::get_balance(&p, payee)
        .await
        .unwrap();
    let bal_fee = anbarr_ledger::balances::get_balance(&p, fee_rev)
        .await
        .unwrap();
    assert_eq!(bal_payer.available_minor, 99);
    assert_eq!(bal_payee.available_minor, 1);
    assert_eq!(bal_fee.available_minor, fee_before, "no fee for dust");

    let legs: i64 = sqlx::query_scalar("SELECT count(*) FROM entries WHERE journal_id = $1")
        .bind(res.journal_id)
        .fetch_one(&p)
        .await
        .unwrap();
    assert_eq!(legs, 2, "no fee leg on dust");
}

#[tokio::test]
async fn agent_commission_paid_out_of_fee_revenue_into_float() {
    let _guard = FEES_LOCK.lock().await;
    let p = pool().await;
    truncate_all(&p).await;
    let engine = LedgerEngine::new(p.clone());
    let payer = create_wallet(&p, Currency::Sle, 100_000).await;
    let payee = create_wallet(&p, Currency::Sle, 0).await;
    let float = create_float(&p, 0).await;
    let fee_rev = platform_account(&p, "fee_revenue", Currency::Sle).await;
    let fee_before = anbarr_ledger::balances::get_balance(&p, fee_rev)
        .await
        .unwrap()
        .available_minor;

    // 0.5% fee = 50, 0.2% commission = 20.
    let res = engine
        .post_payment(payment_req(
            "fee-comm",
            payer,
            payee,
            10_000,
            50,
            Some((20, float)),
        ))
        .await
        .expect("post payment with commission");
    assert_eq!(res.fee_minor, 50);
    assert_eq!(res.commission_minor, 20);
    let comm_journal = res
        .commission_journal_id
        .expect("commission journal posted");

    let bal_payer = anbarr_ledger::balances::get_balance(&p, payer)
        .await
        .unwrap();
    let bal_payee = anbarr_ledger::balances::get_balance(&p, payee)
        .await
        .unwrap();
    let bal_fee = anbarr_ledger::balances::get_balance(&p, fee_rev)
        .await
        .unwrap();
    let bal_float = anbarr_ledger::balances::get_balance(&p, float)
        .await
        .unwrap();
    assert_eq!(
        bal_payer.available_minor,
        100_000 - 10_050,
        "payer pays amount + fee, never commission"
    );
    assert_eq!(
        bal_payee.available_minor, 10_000,
        "merchant gets full amount"
    );
    assert_eq!(
        bal_fee.available_minor,
        fee_before + 50 - 20,
        "platform absorbs commission from fee revenue"
    );
    assert_eq!(
        bal_float.available_minor, 20,
        "commission lands in agent float"
    );

    // Commission journal structure: debit fee_revenue, credit agent float.
    let debit_acct: Uuid = sqlx::query_scalar(
        "SELECT account_id FROM entries WHERE journal_id = $1 AND direction = 'debit' LIMIT 1",
    )
    .bind(comm_journal)
    .fetch_one(&p)
    .await
    .unwrap();
    let credit_acct: Uuid = sqlx::query_scalar(
        "SELECT account_id FROM entries WHERE journal_id = $1 AND direction = 'credit' LIMIT 1",
    )
    .bind(comm_journal)
    .fetch_one(&p)
    .await
    .unwrap();
    let amount: i64 =
        sqlx::query_scalar("SELECT MAX(amount_minor)::bigint FROM entries WHERE journal_id = $1")
            .bind(comm_journal)
            .fetch_one(&p)
            .await
            .unwrap();
    assert_eq!(debit_acct, fee_rev);
    assert_eq!(credit_acct, float);
    assert_eq!(amount, 20);
}

#[tokio::test]
async fn zero_commission_is_skipped() {
    let _guard = FEES_LOCK.lock().await;
    let p = pool().await;
    truncate_all(&p).await;
    let engine = LedgerEngine::new(p.clone());
    let payer = create_wallet(&p, Currency::Sle, 100_000).await;
    let payee = create_wallet(&p, Currency::Sle, 0).await;
    let float = create_float(&p, 0).await;

    // amount 1: fee 0 and commission 0 -> payment only, no commission journal.
    let res = engine
        .post_payment(payment_req(
            "fee-comm0",
            payer,
            payee,
            1,
            50,
            Some((50, float)),
        ))
        .await
        .expect("post payment");
    assert_eq!(res.fee_minor, 0);
    assert_eq!(res.commission_minor, 0);
    assert!(res.commission_journal_id.is_none());
    let bal_float = anbarr_ledger::balances::get_balance(&p, float)
        .await
        .unwrap();
    assert_eq!(bal_float.available_minor, 0);
}

#[tokio::test]
async fn replay_posts_fee_and_commission_once() {
    let _guard = FEES_LOCK.lock().await;
    let p = pool().await;
    truncate_all(&p).await;
    let engine = LedgerEngine::new(p.clone());
    let payer = create_wallet(&p, Currency::Sle, 100_000).await;
    let payee = create_wallet(&p, Currency::Sle, 0).await;
    let float = create_float(&p, 0).await;

    let req = payment_req("fee-replay", payer, payee, 10_000, 50, Some((20, float)));
    let first = engine.post_payment(req.clone()).await.expect("first post");
    let second = engine.post_payment(req).await.expect("replay");

    assert_eq!(
        first.journal_id, second.journal_id,
        "replay returns the original journal"
    );
    assert_eq!(first.commission_journal_id, second.commission_journal_id);

    let bal_payer = anbarr_ledger::balances::get_balance(&p, payer)
        .await
        .unwrap();
    let bal_float = anbarr_ledger::balances::get_balance(&p, float)
        .await
        .unwrap();
    assert_eq!(
        bal_payer.available_minor,
        100_000 - 10_050,
        "single debit on replay"
    );
    assert_eq!(
        bal_float.available_minor, 20,
        "commission paid exactly once"
    );

    let comm_journals: i64 =
        sqlx::query_scalar("SELECT count(*) FROM journals WHERE type = 'commission'")
            .fetch_one(&p)
            .await
            .unwrap();
    assert_eq!(comm_journals, 1);
}

#[tokio::test]
async fn commission_over_fee_revenue_rolls_back_the_whole_payment() {
    let _guard = FEES_LOCK.lock().await;
    let p = pool().await;
    truncate_all(&p).await;
    let engine = LedgerEngine::new(p.clone());
    let payer = create_wallet(&p, Currency::Sle, 100_000).await;
    let payee = create_wallet(&p, Currency::Sle, 0).await;
    let float = create_float(&p, 0).await;
    let fee_rev = platform_account(&p, "fee_revenue", Currency::Sle).await;
    let fee_before = anbarr_ledger::balances::get_balance(&p, fee_rev)
        .await
        .unwrap()
        .available_minor;

    // 100% commission on 10_000 = 10_000, but only 50 of fee revenue exists
    // from this very transaction -> commission cannot be funded -> the whole
    // payment (fee included) must roll back atomically.
    let err = engine
        .post_payment(payment_req(
            "fee-over",
            payer,
            payee,
            10_000,
            50,
            Some((10_000, float)),
        ))
        .await
        .unwrap_err();
    assert!(
        matches!(
            err,
            anbarr_ledger::engine::EngineError::InsufficientFunds { .. }
        ),
        "expected InsufficientFunds, got {err:?}"
    );

    let bal_payer = anbarr_ledger::balances::get_balance(&p, payer)
        .await
        .unwrap();
    let bal_payee = anbarr_ledger::balances::get_balance(&p, payee)
        .await
        .unwrap();
    let bal_fee = anbarr_ledger::balances::get_balance(&p, fee_rev)
        .await
        .unwrap();
    assert_eq!(
        bal_payer.available_minor, 100_000,
        "payer untouched on failure"
    );
    assert_eq!(bal_payee.available_minor, 0);
    assert_eq!(
        bal_fee.available_minor, fee_before,
        "no fee kept on failure"
    );

    let journals: i64 =
        sqlx::query_scalar("SELECT count(*) FROM journals WHERE idempotency_key = 'fee-over'")
            .fetch_one(&p)
            .await
            .unwrap();
    assert_eq!(journals, 0, "no payment journal survived the rollback");
}

// ----------------------------------------------------------------------
// Validation & funds failure paths
// ----------------------------------------------------------------------

// These validation tests seed wallets via `create_wallet(.., initial)` (which
// posts a funding journal) and must not race with the FEES_LOCK tests that
// truncate the shared database mid-flight — take the same lock.
#[tokio::test]
async fn post_payment_rejects_zero_amount() {
    let _guard = FEES_LOCK.lock().await;
    let p = pool().await;
    truncate_all(&p).await;
    let engine = LedgerEngine::new(p.clone());
    let payer = create_wallet(&p, Currency::Sle, 100_000).await;
    let payee = create_wallet(&p, Currency::Sle, 0).await;

    let err = engine
        .post_payment(payment_req("fee-zero", payer, payee, 0, 50, None))
        .await
        .unwrap_err();
    assert!(matches!(
        err,
        anbarr_ledger::engine::EngineError::InvalidJournal(_)
    ));
}

#[tokio::test]
async fn post_payment_rejects_fee_over_100_percent() {
    let _guard = FEES_LOCK.lock().await;
    let p = pool().await;
    truncate_all(&p).await;
    let engine = LedgerEngine::new(p.clone());
    let payer = create_wallet(&p, Currency::Sle, 100_000).await;
    let payee = create_wallet(&p, Currency::Sle, 0).await;

    let err = engine
        .post_payment(payment_req(
            "fee-over-100",
            payer,
            payee,
            10_000,
            10_001,
            None,
        ))
        .await
        .unwrap_err();
    assert!(matches!(
        err,
        anbarr_ledger::engine::EngineError::InvalidJournal(_)
    ));
}

#[tokio::test]
async fn post_payment_rejects_commission_over_100_percent() {
    let _guard = FEES_LOCK.lock().await;
    let p = pool().await;
    truncate_all(&p).await;
    let engine = LedgerEngine::new(p.clone());
    let payer = create_wallet(&p, Currency::Sle, 100_000).await;
    let payee = create_wallet(&p, Currency::Sle, 0).await;
    let float = create_float(&p, 0).await;

    let err = engine
        .post_payment(payment_req(
            "comm-over-100",
            payer,
            payee,
            10_000,
            50,
            Some((10_001, float)),
        ))
        .await
        .unwrap_err();
    assert!(matches!(
        err,
        anbarr_ledger::engine::EngineError::InvalidJournal(_)
    ));
}

#[tokio::test]
async fn post_payment_rejects_insufficient_payer_funds() {
    let _guard = FEES_LOCK.lock().await;
    let p = pool().await;
    truncate_all(&p).await;
    let engine = LedgerEngine::new(p.clone());
    let payer = create_wallet(&p, Currency::Sle, 100).await;
    let payee = create_wallet(&p, Currency::Sle, 0).await;

    let err = engine
        .post_payment(payment_req("fee-nofunds", payer, payee, 10_000, 50, None))
        .await
        .unwrap_err();
    assert!(matches!(
        err,
        anbarr_ledger::engine::EngineError::InsufficientFunds { .. }
    ));
}
