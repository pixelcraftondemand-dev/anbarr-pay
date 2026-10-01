//! Money primitives. All amounts are integer minor units (2 dp for SLE and USD).
//! Floats are banned in this crate.

use serde::{Deserialize, Serialize};
use std::cmp::Ordering;

/// Minor units (e.g. cents). 1 SLE = 100 minor units.
pub type MinorUnits = i64;

/// Fiat currencies supported by the ledger. Sierra Leone (SLE) primary, USD secondary.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Currency {
    Sle,
    Usd,
}

impl Currency {
    /// Decimal places of the minor unit. Both SLE and USD use 2.
    pub fn minor_units(&self) -> u32 {
        2
    }

    /// ISO 4217 code.
    pub fn as_str(&self) -> &'static str {
        match self {
            Currency::Sle => "SLE",
            Currency::Usd => "USD",
        }
    }

    pub fn parse(s: &str) -> Option<Currency> {
        match s {
            "SLE" => Some(Currency::Sle),
            "USD" => Some(Currency::Usd),
            _ => None,
        }
    }
}

/// Percentage fee in basis points (1 bp = 0.01%). The platform fee is 0.5% = 50 bps.
pub type BasisPoints = u32;

/// Round `amount_minor * bps / 10000` to the nearest minor unit using **banker's
/// rounding** (round half to even), per the architecture decision (docs/architecture.md F3).
///
/// The result is always `>= 0` and never over- or under-rounds by more than 0.5 minor unit.
pub fn round_fee(amount_minor: MinorUnits, bps: BasisPoints) -> MinorUnits {
    debug_assert!(amount_minor >= 0);
    debug_assert!(bps <= 10_000, "bps must be a percentage, <= 10000");
    let scaled = amount_minor.saturating_mul(i64::from(bps));
    let quotient = scaled / 10_000;
    let remainder = scaled % 10_000;
    match remainder.cmp(&5_000) {
        Ordering::Less => quotient,
        Ordering::Greater => quotient + 1,
        Ordering::Equal => {
            if quotient % 2 == 0 {
                quotient
            } else {
                quotient + 1
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;

    #[test]
    fn fee_exact() {
        // 0.5% of 10000 = 50
        assert_eq!(round_fee(10_000, 50), 50);
        // 0.5% of 200 = 1
        assert_eq!(round_fee(200, 50), 1);
        // 0.5% of 100 = 0.5 -> half-even -> 0 (quotient 0 is even)
        assert_eq!(round_fee(100, 50), 0);
        // 0.5% of 300 = 1.5 -> half-even -> 2 (quotient 1 is odd)
        assert_eq!(round_fee(300, 50), 2);
    }

    #[test]
    fn fee_rounds_down_below_half() {
        // 0.5% of 101 = 0.505 -> 1 (0.505 > 0.5, rounds up); check the boundary instead:
        assert_eq!(round_fee(99, 50), 0); // 0.495 -> 0
        assert_eq!(round_fee(201, 50), 1); // 1.005 -> 1
        assert_eq!(round_fee(299, 50), 1); // 1.495 -> 1
    }

    #[test]
    fn fee_never_negative_and_bounded() {
        for amount in [
            0, 1, 2, 5, 49, 50, 51, 99, 100, 101, 999, 1000, 123_456, 10_000_000,
        ] {
            let fee = round_fee(amount, 50);
            assert!(fee >= 0);
            assert!(fee <= amount); // 0.5% of amount is never more than the amount
            let err = (fee * 10_000 - amount * 50).unsigned_abs();
            assert!(
                err <= 5_000,
                "rounding error {err} exceeds half a minor unit"
            );
        }
    }

    #[test]
    fn currency_parse() {
        assert_eq!(Currency::parse("SLE"), Some(Currency::Sle));
        assert_eq!(Currency::parse("USD"), Some(Currency::Usd));
        assert_eq!(Currency::parse("EUR"), None);
        assert_eq!(Currency::parse("sle"), None, "case-sensitive ISO code");
        assert_eq!(Currency::Sle.minor_units(), 2);
        assert_eq!(Currency::Usd.minor_units(), 2);
        assert_eq!(Currency::Sle.as_str(), "SLE");
        assert_eq!(Currency::Usd.as_str(), "USD");
    }

    proptest! {
        #[test]
        fn fee_bounded_error(amount in 0i64..10_000_000, bps in 1u32..10_000) {
            let fee = round_fee(amount, bps);
            prop_assert!(fee >= 0);
            let err = (fee * 10_000 - amount * bps as i64).unsigned_abs();
            prop_assert!(err <= 5_000, "rounding error {err} exceeds half a minor unit");
            prop_assert!(fee <= (amount * bps as i64) / 10_000 + 1);
        }


    }
}
