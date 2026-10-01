//! Property-based tests (proptest) for money rounding, journal validation, and
//! the engine's funds rule. Pure functions only — no database involved.
//!
//! These pin the invariants the whole ledger relies on:
//! - `round_fee` is a correct banker's-rounding percentage fee (bounded error,
//!   monotone, exact on multiples, exact halves to even, never exceeds principal).
//! - `validate_spec` accepts exactly the balanced, positive, multi-leg journals.
//! - The fee model (payer pays principal+fee, merchant gets principal, platform
//!   keeps fee) can never produce an unbalanced journal, whatever the rounding.
//! - `check_funds` (the engine's no-negative-balance rule) accepts exactly the
//!   journals that leave every non-bridge account at a non-negative balance —
//!   the pure form of "balances never go negative without an explicit overdraft
//!   rule". The rail_bridge clearing account is that one explicit overdraft
//!   (it is monitored by reconciliation instead of the funds check).

use anbarr_ledger::engine::{check_funds, payment_legs, validate_spec, EngineError};
use anbarr_ledger::money::round_fee;
use anbarr_ledger::types::{Direction, JournalSpec, JournalType, Leg, Origin};
use proptest::prelude::*;
use std::collections::{HashMap, HashSet};
use uuid::Uuid;

/// Every leg in a generated spec uses the same account; validation is about
/// structure (balance, positivity, count), not account identity.
const ACCOUNT: Uuid = Uuid::nil();

fn leg_strategy() -> impl Strategy<Value = Leg> {
    (any::<bool>(), -1_000i64..1_000).prop_map(|(is_debit, amount)| Leg {
        account_id: ACCOUNT,
        direction: if is_debit {
            Direction::Debit
        } else {
            Direction::Credit
        },
        amount_minor: amount,
    })
}

fn spec_strategy() -> impl Strategy<Value = JournalSpec> {
    prop::collection::vec(leg_strategy(), 0..10).prop_map(|legs| JournalSpec {
        journal_type: JournalType::P2p,
        currency: anbarr_ledger::money::Currency::Sle,
        origin: Origin::default(),
        legs,
    })
}

/// Fixed pool of account ids used by the funds-rule property test, so an
/// account can appear in several legs (netting matters) and bridges can be
/// picked from the same pool.
fn account_pool() -> Vec<Uuid> {
    (1..=8).map(Uuid::from_u128).collect()
}

/// Legs with strictly positive amounts spread across `accounts`.
fn positive_legs_strategy(accounts: &[Uuid]) -> impl Strategy<Value = Vec<Leg>> + '_ {
    let account = prop::sample::select(accounts.to_vec());
    prop::collection::vec(
        (account, any::<bool>(), 1i64..10_000).prop_map(|(account_id, is_debit, amount)| Leg {
            account_id,
            direction: if is_debit {
                Direction::Debit
            } else {
                Direction::Credit
            },
            amount_minor: amount,
        }),
        1..10,
    )
}

proptest! {
    /// Characterization of `validate_spec`: it accepts exactly the journals
    /// that have >= 2 legs, all amounts > 0, and equal debit/credit totals.
    /// Any deviation from this is a bug in the validator, and any journal that
    /// passes this shape is safe to attempt posting.
    #[test]
    fn validate_spec_accepts_exactly_valid_journals(spec in spec_strategy()) {
        let debits: i64 = spec
            .legs
            .iter()
            .filter(|l| l.direction == Direction::Debit)
            .map(|l| l.amount_minor)
            .sum();
        let credits: i64 = spec
            .legs
            .iter()
            .filter(|l| l.direction == Direction::Credit)
            .map(|l| l.amount_minor)
            .sum();
        let expected_ok = spec.legs.len() >= 2
            && spec.legs.iter().all(|l| l.amount_minor > 0)
            && debits == credits;
        prop_assert_eq!(validate_spec(&spec).is_ok(), expected_ok, "spec: {:?}", spec);
    }

    /// The engine's funds rule, in its pure form: `check_funds` accepts a
    /// journal iff posting it would leave every non-bridge account at a
    /// non-negative balance. This is the "balance never goes negative without
    /// an explicit overdraft rule" invariant — the rail_bridge clearing
    /// account is that one explicit overdraft.
    #[test]
    fn check_funds_accepts_exactly_non_negative_postings(
        balances in prop::collection::vec(0i64..100_000, account_pool().len()),
        mut legs in positive_legs_strategy(&account_pool()),
        bridge_count in 0usize..=2usize,
    ) {
        let available: HashMap<Uuid, i64> = account_pool().into_iter().zip(balances).collect();
        let bridge_ids: HashSet<Uuid> = account_pool()
            .into_iter()
            .take(bridge_count)
            .collect();

        // Make the journal balance: append one balancing leg on an account
        // outside the pool (balance 0, never a bridge), so debits == credits.
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
        let outside = Uuid::from_u128(999_999);
        if debits > credits {
            legs.push(Leg {
                account_id: outside,
                direction: Direction::Credit,
                amount_minor: debits - credits,
            });
        } else if credits > debits {
            legs.push(Leg {
                account_id: outside,
                direction: Direction::Debit,
                amount_minor: credits - debits,
            });
        }

        // Post-state: available + net per account, accounts missing from
        // `available` treated as balance 0 (the same convention as
        // `check_funds`).
        let mut post: HashMap<Uuid, i64> = available.clone();
        for leg in &legs {
            let balance = post.entry(leg.account_id).or_insert(0);
            *balance += match leg.direction {
                Direction::Debit => -leg.amount_minor,
                Direction::Credit => leg.amount_minor,
            };
        }
        let every_account_non_negative = post
            .iter()
            .all(|(account_id, balance)| *balance >= 0 || bridge_ids.contains(account_id));

        match check_funds(&legs, &available, &bridge_ids) {
            Ok(()) => prop_assert!(
                every_account_non_negative,
                "accepted a posting that drives an account negative: \
                 legs {legs:?} available {available:?} post {post:?}"
            ),
            Err(EngineError::InsufficientFunds { .. }) => prop_assert!(
                !every_account_non_negative,
                "rejected a journal that leaves every non-bridge account \
                 non-negative: legs {legs:?} available {available:?} post {post:?}"
            ),
            Err(other) => prop_assert!(
                false,
                "check_funds returned an unexpected error: {other:?}"
            ),
        }
    }

    /// Rounding is monotone in the amount: charging more never yields a
    /// smaller fee.
    #[test]
    fn fee_is_monotone_in_amount(a in 0i64..1_000_000, b in 0i64..1_000_000, bps in 1u32..10_000) {
        if a <= b {
            prop_assert!(
                round_fee(a, bps) <= round_fee(b, bps),
                "fee({}, {}) = {} > fee({}, {}) = {}",
                a,
                bps,
                round_fee(a, bps),
                b,
                bps,
                round_fee(b, bps)
            );
        }
    }

    /// Banker's rounding: an exact half rounds to the even neighbour.
    #[test]
    fn fee_rounds_exact_halves_to_even(q in 0i64..1_000_000) {
        let fee = round_fee(q * 10_000 + 5_000, 1);
        prop_assert_eq!(fee % 2, 0, "half at {}.5 rounded to odd {}", q, fee);
    }

    /// Exact multiples round exactly.
    #[test]
    fn fee_is_exact_on_multiples(q in 0i64..1_000_000) {
        prop_assert_eq!(round_fee(q * 10_000, 1), q);
    }

    /// A zero amount always yields a zero fee.
    #[test]
    fn fee_zero_for_zero_amount(bps in 0u32..10_000) {
        prop_assert_eq!(round_fee(0, bps), 0);
    }

    /// The fee never exceeds the principal while bps <= 100% (10000 bps).
    #[test]
    fn fee_never_exceeds_principal(amount in 0i64..10_000_000, bps in 1u32..=10_000) {
        let fee = round_fee(amount, bps);
        prop_assert!(fee <= amount, "fee {} > amount {} at {} bps", fee, amount, bps);
    }

    /// Fee-model conservation: the 3-leg fee journal (payer pays
    /// principal + fee, merchant gets principal, platform keeps fee) always
    /// validates. Rounding the fee can never unbalance the journal — this is
    /// the invariant that makes the 0.5% model safe at any amount.
    #[test]
    fn fee_journal_always_balances(principal in 1i64..10_000_000, bps in 1u32..10_000) {
        let fee = round_fee(principal, bps);
        let spec = JournalSpec {
            journal_type: JournalType::P2p,
            currency: anbarr_ledger::money::Currency::Sle,
            origin: Origin::default(),
            legs: vec![
                Leg {
                    account_id: ACCOUNT,
                    direction: Direction::Debit,
                    amount_minor: principal + fee,
                },
                Leg {
                    account_id: ACCOUNT,
                    direction: Direction::Credit,
                    amount_minor: principal,
                },
                Leg {
                    account_id: ACCOUNT,
                    direction: Direction::Credit,
                    amount_minor: fee,
                },
            ],
        };
        prop_assert!(
            validate_spec(&spec).is_ok(),
            "unbalanced fee journal: principal {} bps {} (fee {})",
            principal,
            bps,
            fee
        );
    }

    /// `payment_legs` always builds a valid, balanced journal: the payer is
    /// debited `amount + fee + tax`, the payee credited `amount`, fee revenue
    /// credited the rounded fee, and the tax liability credited the rounded
    /// tax. Whatever the rounding, debits == credits — a payment can never
    /// unbalance the ledger.
    #[test]
    fn payment_legs_always_balance(
        amount in 1i64..10_000_000,
        bps in 0u32..=10_000,
        tax_bps in 0u32..=10_000,
    ) {
        let payer = Uuid::from_u128(101);
        let payee = Uuid::from_u128(102);
        let fee_revenue = Uuid::from_u128(103);
        let tax_payable = Uuid::from_u128(104);
        let legs = payment_legs(amount, bps, payer, payee, fee_revenue, tax_bps, tax_payable);
        let spec = JournalSpec {
            journal_type: JournalType::P2p,
            currency: anbarr_ledger::money::Currency::Sle,
            origin: Origin::default(),
            legs: legs.clone(),
        };
        prop_assert!(
            validate_spec(&spec).is_ok(),
            "unbalanced payment journal for amount {amount} bps {bps}: {legs:?}"
        );

        let fee = round_fee(amount, bps);
        let tax = round_fee(amount, tax_bps);
        let payer_net: i64 = legs
            .iter()
            .filter(|l| l.account_id == payer && l.direction == Direction::Debit)
            .map(|l| l.amount_minor)
            .sum();
        let payee_net: i64 = legs
            .iter()
            .filter(|l| l.account_id == payee && l.direction == Direction::Credit)
            .map(|l| l.amount_minor)
            .sum();
        let fee_net: i64 = legs
            .iter()
            .filter(|l| l.account_id == fee_revenue && l.direction == Direction::Credit)
            .map(|l| l.amount_minor)
            .sum();
        prop_assert_eq!(
            payer_net,
            amount + fee + tax,
            "payer must pay principal + fee + tax"
        );
        prop_assert_eq!(payee_net, amount, "payee must receive exactly the principal");
        prop_assert_eq!(fee_net, fee, "fee revenue must receive exactly the rounded fee");
        prop_assert!(fee_net <= amount, "fee must never exceed the principal");

        let tax_net: i64 = legs
            .iter()
            .filter(|l| l.account_id == tax_payable && l.direction == Direction::Credit)
            .map(|l| l.amount_minor)
            .sum();
        prop_assert_eq!(tax_net, tax, "tax liability must receive exactly the rounded tax");
        prop_assert!(tax_net <= amount, "tax must never exceed the principal");
    }

    /// Payments and the funds rule together: a payer with exactly
    /// `amount + fee + tax` can pay, and a payer with a single minor unit
    /// less cannot — the engine can never let a wallet go negative, even by a
    /// rounding artefact.
    #[test]
    fn payment_funds_rule_is_exact(
        amount in 1i64..10_000_000,
        bps in 0u32..=10_000,
        tax_bps in 0u32..=10_000,
    ) {
        let payer = Uuid::from_u128(201);
        let payee = Uuid::from_u128(202);
        let fee_revenue = Uuid::from_u128(203);
        let tax_payable = Uuid::from_u128(204);
        let legs = payment_legs(amount, bps, payer, payee, fee_revenue, tax_bps, tax_payable);
        let cost = amount + round_fee(amount, bps) + round_fee(amount, tax_bps);

        let no_bridges: HashSet<Uuid> = HashSet::new();
        let enough: HashMap<Uuid, i64> =
            [(payer, cost), (payee, 0), (fee_revenue, 0), (tax_payable, 0)].into();
        prop_assert!(
            check_funds(&legs, &enough, &no_bridges).is_ok(),
            "exact funds rejected: amount {amount} bps {bps}"
        );

        let short: HashMap<Uuid, i64> =
            [(payer, cost - 1), (payee, 0), (fee_revenue, 0), (tax_payable, 0)].into();
        prop_assert!(
            matches!(
                check_funds(&legs, &short, &no_bridges),
                Err(EngineError::InsufficientFunds { .. })
            ),
            "short payer not rejected: amount {amount} bps {bps}"
        );
    }
}
