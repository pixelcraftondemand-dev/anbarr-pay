//! Tests for the government transaction tax (e-levy-style) capability, per
//! master prompt §1/§12.1: a per-country transaction tax must be addable
//! WITHOUT a schema change, must never be confused with platform revenue, and
//! must round exactly (banker's rounding, like fees). Tax legs ride in the
//! payment journal, so reversals return tax automatically.
//!
//! All DB tests are serialized on a process-local mutex and truncate between
//! tests: platform singletons are shared, so exact absolute balances are only
//! meaningful when one test observes them at a time (same pattern as
//! FEES_LOCK in fees.rs / REVERSALS_LOCK in reversals.rs).

mod common;

use anbarr_ledger::engine::{
    EngineError, FeePolicy, LedgerEngine, PaymentRequest, ReversalRequest, TaxPolicy,
};
use anbarr_ledger::money::Currency;
use anbarr_ledger::types::{Direction, JournalType, Origin};
use sqlx::PgPool;
use uuid::Uuid;

use common::{create_wallet, platform_account, pool, truncate_all};

static TAX_LOCK: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());

fn origin() -> Origin {
    Origin {
        user_id: Some(Uuid::new_v4()),
        channel: "tax-test".into(),
        session_id: None,
        payment_code: None,
    }
}

fn payment_req(
    key: &str,
    payer: Uuid,
    payee: Uuid,
    amount: i64,
    fee_bps: u32,
    tax_bps: Option<u32>,
) -> PaymentRequest {
    payment_req_with(key, origin(), payer, payee, amount, fee_bps, tax_bps)
}

/// The request hash covers the origin (actor traceability), so a genuine
/// duplicate must reuse the same origin — a different actor under the same
/// key is a payload change and must be rejected.
fn payment_req_with(
    key: &str,
    org: Origin,
    payer: Uuid,
    payee: Uuid,
    amount: i64,
    fee_bps: u32,
    tax_bps: Option<u32>,
) -> PaymentRequest {
    PaymentRequest {
        idempotency_scope: "tax-test".into(),
        idempotency_key: key.into(),
        journal_type: JournalType::P2p,
        currency: Currency::Sle,
        origin: org,
        payer_account_id: payer,
        payee_account_id: payee,
        amount_minor: amount,
        fee: FeePolicy { bps: fee_bps },
        tax: tax_bps.map(|bps| TaxPolicy { bps }),
        agent_commission: None,
    }
}

async fn balance(p: &PgPool, account: Uuid) -> i64 {
    anbarr_ledger::balances::get_balance(p, account)
        .await
        .expect("balance")
        .available_minor
}

/// 1.5% tax on 10_000 = 150: the payer pays principal + fee + tax, the payee
/// receives the principal, and the tax lands in the `tax_payable` LIABILITY —
/// a separate account from fee revenue, so collected tax is never mistaken
/// for earned fees.
#[tokio::test]
async fn tax_is_collected_into_tax_payable_not_revenue() {
    let _guard = TAX_LOCK.lock().await;
    let p = pool().await;
    truncate_all(&p).await;
    let engine = LedgerEngine::new(p.clone());
    let payer = create_wallet(&p, Currency::Sle, 100_000).await;
    let payee = create_wallet(&p, Currency::Sle, 0).await;
    let fee_rev = platform_account(&p, "fee_revenue", Currency::Sle).await;
    let tax_payable = platform_account(&p, "tax_payable", Currency::Sle).await;

    let result = engine
        .post_payment(payment_req("tax-1", payer, payee, 10_000, 50, Some(150)))
        .await
        .expect("post taxed payment");

    assert_eq!(result.fee_minor, 50);
    assert_eq!(result.tax_minor, 150);
    assert_eq!(balance(&p, payer).await, 100_000 - 10_200);
    assert_eq!(balance(&p, payee).await, 10_000);
    assert_eq!(balance(&p, fee_rev).await, 50);
    assert_eq!(
        balance(&p, tax_payable).await,
        150,
        "tax is owed to the authority, not earned by the platform"
    );

    // The tax leg is a real credit on the journal, distinct from the fee leg.
    let tax_legs: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM entries e JOIN accounts a ON a.id = e.account_id \
         WHERE e.journal_id = $1 AND a.type = 'tax_payable' AND e.direction = 'credit'",
    )
    .bind(result.journal_id)
    .fetch_one(&p)
    .await
    .unwrap();
    assert_eq!(tax_legs, 1);
}

/// No tax configured (None): identical to the pre-tax behavior — payer pays
/// principal + fee only, and tax_payable stays untouched.
#[tokio::test]
async fn no_tax_policy_posts_no_tax_leg() {
    let _guard = TAX_LOCK.lock().await;
    let p = pool().await;
    truncate_all(&p).await;
    let engine = LedgerEngine::new(p.clone());
    let payer = create_wallet(&p, Currency::Sle, 100_000).await;
    let payee = create_wallet(&p, Currency::Sle, 0).await;
    let tax_payable = platform_account(&p, "tax_payable", Currency::Sle).await;

    let result = engine
        .post_payment(payment_req("tax-2", payer, payee, 10_000, 50, None))
        .await
        .expect("post untaxed payment");

    assert_eq!(result.tax_minor, 0);
    assert_eq!(balance(&p, payer).await, 100_000 - 10_050);
    assert_eq!(balance(&p, payee).await, 10_000);
    assert_eq!(balance(&p, tax_payable).await, 0);
}

/// Dust tax: a rate that rounds to zero carries no tax leg and no tax charge
/// (same rule as dust fees).
#[tokio::test]
async fn dust_tax_carries_no_tax_leg() {
    let _guard = TAX_LOCK.lock().await;
    let p = pool().await;
    truncate_all(&p).await;
    let engine = LedgerEngine::new(p.clone());
    let payer = create_wallet(&p, Currency::Sle, 1_000).await;
    let payee = create_wallet(&p, Currency::Sle, 0).await;
    let tax_payable = platform_account(&p, "tax_payable", Currency::Sle).await;

    // 0.5% of 100 = 0.5 -> banker's rounding -> 0 (even). The fee rounds to
    // zero too, so this journal is principal-only.
    let result = engine
        .post_payment(payment_req("tax-3", payer, payee, 100, 50, Some(50)))
        .await
        .expect("post dust-tax payment");

    assert_eq!(result.fee_minor, 0);
    assert_eq!(result.tax_minor, 0);
    assert_eq!(balance(&p, payer).await, 900);
    assert_eq!(balance(&p, payee).await, 100);
    assert_eq!(balance(&p, tax_payable).await, 0);
}

/// A taxed payment reverses fully: principal, fee AND tax return to the payer
/// and the tax liability returns to zero — because the tax legs ride in the
/// payment journal, the reversal negates them like any other leg.
#[tokio::test]
async fn reversal_returns_tax_as_well() {
    let _guard = TAX_LOCK.lock().await;
    let p = pool().await;
    truncate_all(&p).await;
    let engine = LedgerEngine::new(p.clone());
    let payer = create_wallet(&p, Currency::Sle, 100_000).await;
    let payee = create_wallet(&p, Currency::Sle, 0).await;
    let fee_rev = platform_account(&p, "fee_revenue", Currency::Sle).await;
    let tax_payable = platform_account(&p, "tax_payable", Currency::Sle).await;

    let original = engine
        .post_payment(payment_req("tax-4", payer, payee, 10_000, 50, Some(150)))
        .await
        .expect("post taxed payment")
        .journal_id;

    let reversal = engine
        .reverse_journal(ReversalRequest {
            idempotency_scope: "tax-test".into(),
            idempotency_key: "tax-4-rev".into(),
            original_journal_id: original,
            origin: origin(),
        })
        .await
        .expect("reverse taxed payment");

    assert_eq!(balance(&p, payer).await, 100_000);
    assert_eq!(balance(&p, payee).await, 0);
    assert_eq!(balance(&p, fee_rev).await, 0);
    assert_eq!(balance(&p, tax_payable).await, 0);

    // The reversal has one leg per original leg (principal, fee, tax).
    let legs: i64 = sqlx::query_scalar("SELECT count(*) FROM entries WHERE journal_id = $1")
        .bind(reversal.journal_id)
        .fetch_one(&p)
        .await
        .unwrap();
    assert_eq!(legs, 4, "reversal negates every leg including tax");
}

/// A taxed payment submitted twice under one idempotency key moves money and
/// collects tax exactly once (duplicate-request rule, §4.5).
#[tokio::test]
async fn duplicate_taxed_payment_collects_tax_once() {
    let _guard = TAX_LOCK.lock().await;
    let p = pool().await;
    truncate_all(&p).await;
    let engine = LedgerEngine::new(p.clone());
    let payer = create_wallet(&p, Currency::Sle, 100_000).await;
    let payee = create_wallet(&p, Currency::Sle, 0).await;
    let tax_payable = platform_account(&p, "tax_payable", Currency::Sle).await;

    let org = origin();
    let first = engine
        .post_payment(payment_req_with(
            "tax-5",
            org.clone(),
            payer,
            payee,
            10_000,
            50,
            Some(150),
        ))
        .await
        .expect("first submission");
    let second = engine
        .post_payment(payment_req_with(
            "tax-5",
            org,
            payer,
            payee,
            10_000,
            50,
            Some(150),
        ))
        .await
        .expect("replayed submission");

    assert_eq!(first.journal_id, second.journal_id);
    assert_eq!(second.tax_minor, 150, "replay returns the original result");
    assert_eq!(balance(&p, payer).await, 100_000 - 10_200);
    assert_eq!(balance(&p, tax_payable).await, 150, "no double collection");
}

/// An out-of-range tax rate is rejected before any money moves.
#[tokio::test]
async fn tax_bps_above_100_percent_rejected() {
    let _guard = TAX_LOCK.lock().await;
    let p = pool().await;
    truncate_all(&p).await;
    let engine = LedgerEngine::new(p.clone());
    let payer = create_wallet(&p, Currency::Sle, 100_000).await;
    let payee = create_wallet(&p, Currency::Sle, 0).await;

    let err = engine
        .post_payment(payment_req("tax-6", payer, payee, 10_000, 50, Some(10_001)))
        .await
        .unwrap_err();
    assert!(matches!(err, EngineError::InvalidJournal(_)), "got {err:?}");
    assert_eq!(balance(&p, payer).await, 100_000, "no money moved");
}

/// Pure leg structure with a tax: payer pays amount + fee + tax in one debit,
/// the payee is credited the principal only.
#[test]
fn payment_legs_with_tax_charge_payer_once() {
    use anbarr_ledger::engine::payment_legs;
    let (payer, payee, fee_rev, tax) = (Uuid::nil(), Uuid::nil(), Uuid::nil(), Uuid::nil());

    let legs = payment_legs(10_000, 50, payer, payee, fee_rev, 150, tax);
    assert_eq!(legs.len(), 4);
    assert_eq!(legs[0].direction, Direction::Debit);
    assert_eq!(
        legs[0].amount_minor, 10_200,
        "one debit covers principal + fee + tax"
    );
    assert_eq!(
        legs[1].amount_minor, 10_000,
        "payee receives principal only"
    );
    assert_eq!(legs[2].amount_minor, 50, "fee leg");
    assert_eq!(legs[3].amount_minor, 150, "tax leg");

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
}
