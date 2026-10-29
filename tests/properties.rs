//! Property tests for the rate arithmetic.
//!
//! The example tests pin worked cases from IAS 21; these properties pin the
//! *algebra* — the invariants that must hold for every input, which is what
//! a thousand-line revaluation implicitly depends on. Any failure here is a
//! real defect in the money path.

#![allow(clippy::expect_used, clippy::panic)]

use fx_kit::Rate;
use proptest::prelude::*;
use vat_rules::TieMode;

/// Quoted percent strings: `0`–`999.99`, two fraction digits. Enough range
/// to cover every real consumption tax, with ties reachable.
fn quoted_percent() -> impl Strategy<Value = String> {
    (0u32..1000u32, 0u32..100u32).prop_map(|(whole, frac)| {
        if frac == 0 {
            format!("{whole}")
        } else {
            format!("{whole}.{frac:02}")
        }
    })
}

fn rate(quoted: &str) -> Rate {
    Rate::parse(quoted).expect("strategy generates valid rates")
}

proptest! {
    /// The identity rate restates every amount to itself — the baseline the
    /// whole crate leans on.
    #[test]
    fn identity_rate_restates_exactly(amount in -1_000_000_000i64..1_000_000_000i64) {
        let one = Rate::parse("1").expect("parses");
        prop_assert_eq!(one.restate(amount, TieMode::HalfUp), amount);
    }

    /// Restatement is sign-symmetric under every tie mode: restating a
    /// negated amount negates the restatement. This is only guaranteed
    /// because the arithmetic rounds the magnitude and restores the sign —
    /// a signed-toward-zero rounding would break it at ties.
    #[test]
    fn restatement_is_sign_symmetric(
        amount in -1_000_000_000i64..1_000_000_000i64,
        quoted in quoted_percent(),
        mode in prop::sample::select(vec![TieMode::HalfUp, TieMode::HalfEven, TieMode::TowardZero]),
    ) {
        let rate = rate(&quoted);
        let positive = rate.restate(amount.unsigned_abs() as i64, mode);
        let negative = rate.restate(-(amount.unsigned_abs() as i64), mode);
        prop_assert_eq!(negative, -positive);
    }

    /// Restatement is monotone: a larger amount never restates smaller, at
    /// any positive rate, under any tie mode.
    #[test]
    fn restatement_is_monotone(
        a in 0i64..1_000_000_000i64,
        b in 0i64..1_000_000_000i64,
        quoted in quoted_percent(),
        mode in prop::sample::select(vec![TieMode::HalfUp, TieMode::HalfEven, TieMode::TowardZero]),
    ) {
        prop_assume!(a <= b);
        let rate = rate(&quoted);
        prop_assert!(rate.restate(a, mode) <= rate.restate(b, mode));
    }

    /// `parse_percent` is `parse` divided by exactly 100: the multiplier of a
    /// percent rate times 100 is the multiplier parsed directly.
    #[test]
    fn parse_percent_is_parse_over_hundred(quoted in quoted_percent()) {
        let as_percent = Rate::parse_percent(&quoted).expect("parses");
        let as_multiplier = Rate::parse(&quoted).expect("parses");
        // p/100 == p×numerator / (p×denominator×100) cross-multiplied.
        prop_assert_eq!(
            i128::from(as_percent.numerator()) * i128::from(as_multiplier.denominator()) * 100,
            i128::from(as_multiplier.numerator()) * i128::from(as_percent.denominator())
        );
    }

    /// Rates parse without panicking on arbitrary strings — the parser is a
    /// boundary, and boundaries return errors, never panic.
    #[test]
    fn rate_parsing_never_panics(arbitrary in ".*") {
        let _ = Rate::parse(&arbitrary);
        let _ = Rate::parse_percent(&arbitrary);
    }
}
