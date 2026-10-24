//! Currency revaluation per IAS 21: restating monetary items at the closing
//! rate and accounting for the difference.
//!
//! # The rule the rate type exists to enforce
//!
//! An FX rate is a **rational number**, and every float on the path from
//! quoted rate to posted difference corrupts it. `"1.0834"` is exactly
//! 10834/10000; as an f64 it is not, and a thousand-row revaluation turns
//! "not" into visible pennies of phantom gain. Rates here parse from their
//! decimal strings into exact `(numerator, denominator)` pairs, restatement
//! is `foreign_minor × num / den` in i128 with one explicit rounding at the
//! final division, and the difference is `restated − booked` — computed once,
//! never inferred from two float products.
//!
//! # What IAS 21 actually asks for
//!
//! **Monetary items** (cash, receivables, payables — amounts of currency
//! itself) are restated at the **closing rate** each reporting date. The
//! resulting exchange difference goes to profit or loss (IAS 21.28), with the
//! narrow exception of qualifying net investment hedges (21.32) — outside
//! this crate's scope, and the caller allocates the posting if they have one.
//! **Non-monetary items** (inventory, fixed assets) stay at their historical
//! rate and are *not* restated; passing them to [`revalue`] would be a
//! category error, which is why the input type is named `MonetaryItem`.
//!
//! # What the output is
//!
//! [`revalue`] returns one [`Revaluation`] per item — restated, booked base
//! amount, and difference — plus the caller-summed totals per currency. The
//! posting is deliberately the caller's: the difference is a signed amount in
//! base minor units, gain positive, loss negative, and which account absorbs
//! it is a chart decision, not a currency decision.

use vat_rules::TieMode;

/// Why revaluation cannot proceed from the inputs given.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FxError {
    /// A rate string was not a decimal number.
    MalformedRate(String),
    /// A rate of zero (or negative — rates are positive by definition).
    NonPositiveRate(String),
    /// The item's currency equals the base currency: no restatement is
    /// meaningful, and silently treating it as 1:1 would hide a data error.
    BaseCurrencyItem,
}

/// An FX rate, exact: `"1.0834"` is 10834/10000, never a float.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Rate {
    numerator: i64,
    denominator: i64,
}

impl Rate {
    /// Parse a quoted rate from its decimal string.
    ///
    /// `"0.8871"`, `"1.0834"`, `"157.30"` — any positive decimal. The
    /// denominator is a power of ten bounded by the string's fraction digits,
    /// so the parse is exact and the arithmetic downstream stays exact.
    pub fn parse(raw: &str) -> Result<Self, FxError> {
        let trimmed = raw.trim();
        if trimmed.starts_with('-') {
            return Err(FxError::NonPositiveRate(raw.to_string()));
        }
        let (integer_part, fraction_part) = match trimmed.split_once('.') {
            Some((i, f)) => (i, f),
            None => (trimmed, ""),
        };
        if integer_part.is_empty() && fraction_part.is_empty() {
            return Err(FxError::MalformedRate(raw.to_string()));
        }
        if !integer_part.chars().all(|c| c.is_ascii_digit())
            || !fraction_part.chars().all(|c| c.is_ascii_digit())
        {
            return Err(FxError::MalformedRate(raw.to_string()));
        }
        let integer: i64 = if integer_part.is_empty() {
            0
        } else {
            integer_part
                .parse()
                .map_err(|_| FxError::MalformedRate(raw.to_string()))?
        };
        if integer == 0 && fraction_part.chars().all(|c| c == '0') {
            return Err(FxError::NonPositiveRate(raw.to_string()));
        }
        let mut numerator = integer;
        let mut denominator: i64 = 1;
        for digit in fraction_part.chars() {
            numerator = numerator
                .checked_mul(10)
                .and_then(|n| n.checked_add(digit.to_digit(10).map(|d| d as i64).unwrap_or(0)))
                .ok_or_else(|| FxError::MalformedRate(raw.to_string()))?;
            denominator *= 10;
        }
        // Reduce by gcd: `157.30` is 1573/10, not 15730/100 — smaller
        // numbers stay i64-safe longer, and equality is by value.
        let mut a = numerator;
        let mut b = denominator;
        while b != 0 {
            let r = a % b;
            a = b;
            b = r;
        }
        Ok(Self {
            numerator: numerator / a,
            denominator: denominator / a,
        })
    }

    /// One unit of the foreign currency buys `rate` of the base.
    #[must_use]
    pub fn as_rational(&self) -> (i64, i64) {
        (self.numerator, self.denominator)
    }

    /// Restate `foreign_minor` at this rate, rounded once by `mode`.
    ///
    /// The multiplication happens in i128 — minor units times a rate with four
    /// fraction digits is comfortably inside i64 but there is no honest reason
    /// to sit near the edge — and the single division is where the policy's
    /// tie mode applies.
    #[must_use]
    pub fn restate(&self, foreign_minor: i64, mode: TieMode) -> i64 {
        let sign = if foreign_minor < 0 { -1i128 } else { 1i128 };
        let magnitude = (foreign_minor.unsigned_abs() as i128) * (self.numerator as i128);
        let denominator = self.denominator as i128;
        // round_div on the magnitude, sign restored: the tie parity question
        // ("is the lower candidate even?") is only well-defined on
        // non-negative quotients, for the same reason as invoice-kit's
        // Decimal::round_div.
        let quotient = magnitude / denominator;
        let remainder = magnitude % denominator;
        let rounded = if remainder == 0 {
            quotient
        } else {
            let doubled = remainder * 2;
            let takes_upper = doubled > denominator
                || (doubled == denominator && mode.takes_upper_on_tie(quotient % 2 == 0));
            if takes_upper { quotient + 1 } else { quotient }
        };
        (rounded * sign) as i64
    }
}

/// A monetary item held in a foreign currency: the foreign amount and the
/// base-currency amount it was booked at.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MonetaryItem {
    /// The foreign amount, in the foreign currency's minor units. Signed:
    /// a payable is negative in the same sense its ledger line is.
    pub foreign_minor: i64,
    /// The ISO 4217 code of the foreign currency.
    pub currency: String,
    /// The base-currency amount the item currently sits at, signed the same
    /// way.
    pub booked_base_minor: i64,
}

/// The restatement of one monetary item at the closing rate.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Revaluation {
    /// The item's own identifier, as the caller will need for the posting.
    pub item_index: usize,
    /// The currency.
    pub currency: String,
    /// The restated base amount, signed.
    pub restated_base_minor: i64,
    /// What the item sat at before revaluation.
    pub booked_base_minor: i64,
    /// `restated − booked`: positive is an exchange gain, negative a loss.
    pub difference_minor: i64,
}

/// The period's revaluation, per currency.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct RevaluationReport {
    /// One entry per item, input order.
    pub entries: Vec<Revaluation>,
}

impl RevaluationReport {
    /// The net exchange gain or loss across all items: positive is a net
    /// gain.
    #[must_use]
    pub fn net_difference(&self) -> i64 {
        self.entries.iter().map(|e| e.difference_minor).sum()
    }

    /// The difference per currency, sorted: a revaluation whose gains and
    /// losses are shown only net hides which currency moved, and the note to
    /// the accounts wants the split.
    #[must_use]
    pub fn per_currency(&self) -> Vec<(String, i64)> {
        let mut order: Vec<String> = Vec::new();
        let mut totals: std::collections::HashMap<String, i64> = std::collections::HashMap::new();
        for entry in &self.entries {
            if !order.contains(&entry.currency) {
                order.push(entry.currency.clone());
            }
            *totals.entry(entry.currency.clone()).or_insert(0) += entry.difference_minor;
        }
        order.sort();
        order
            .into_iter()
            .map(|currency| {
                let difference = totals.get(&currency).copied().unwrap_or(0);
                (currency, difference)
            })
            .collect()
    }
}

/// Restate every monetary item at the closing rate and compute the exchange
/// difference.
///
/// Items already in the base currency are refused rather than treated as 1:1 —
/// a base-currency row in the revaluation population is a data error upstream,
/// and "silently no-op" is how it survives into the accounts.
pub fn revalue(
    items: &[MonetaryItem],
    closing_rates: &std::collections::HashMap<String, Rate>,
    base_currency: &str,
    mode: TieMode,
) -> Result<RevaluationReport, FxError> {
    let mut entries = Vec::new();
    for (index, item) in items.iter().enumerate() {
        if item.currency == base_currency {
            return Err(FxError::BaseCurrencyItem);
        }
        let rate = closing_rates.get(&item.currency).ok_or_else(|| {
            FxError::MalformedRate(format!("no closing rate for {}", item.currency))
        })?;
        let restated = rate.restate(item.foreign_minor, mode);
        entries.push(Revaluation {
            item_index: index,
            currency: item.currency.clone(),
            restated_base_minor: restated,
            booked_base_minor: item.booked_base_minor,
            difference_minor: restated - item.booked_base_minor,
        });
    }
    Ok(RevaluationReport { entries })
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::panic)]
mod tests {
    use super::*;

    fn rate(raw: &str) -> Rate {
        Rate::parse(raw).expect("a valid rate")
    }

    #[test]
    fn rates_are_exact_rationals_not_floats() {
        assert_eq!(
            rate("1.0834").as_rational(),
            (5_417, 5_000),
            "reduced by gcd"
        );
        assert_eq!(rate("0.8871").as_rational(), (8_871, 10_000));
        assert_eq!(rate("157.30").as_rational(), (1573, 10));
        assert_eq!(rate("1").as_rational(), (1, 1));
        assert_eq!(rate("157").as_rational(), (157, 1));

        for garbage in ["", ".", "-", "-1.5", "abc", "1.2.3", "1,5", "0", "0.00"] {
            assert!(Rate::parse(garbage).is_err(), "{garbage:?} must not parse");
        }
    }

    #[test]
    fn restatement_is_exact_where_the_math_is_exact() {
        // €10,000.00 at 1.0834 is exactly $10,834.00 — 1,083,400 cents, no
        // rounding involved, and the result must not be a cent off.
        let usd = rate("1.0834");
        assert_eq!(usd.restate(1_000_000, TieMode::HalfUp), 1_083_400);
        // Negative amounts (payables) restate symmetrically.
        assert_eq!(usd.restate(-1_000_000, TieMode::HalfUp), -1_083_400);
    }

    #[test]
    fn the_rounding_happens_once_at_the_final_division() {
        // ¥100 at 0.5105 pence is 51.05p — rounds to 51.
        let gbp = rate("0.5105");
        assert_eq!(gbp.restate(100, TieMode::HalfUp), 51);
        // And the tie case: ¥1,000 at 0.5125 is exactly 512.5p.
        let tie = rate("0.5125");
        assert_eq!(tie.restate(1000, TieMode::HalfUp), 513);
        assert_eq!(tie.restate(1000, TieMode::HalfEven), 512);
        assert_eq!(tie.restate(1000, TieMode::TowardZero), 512);
    }

    #[test]
    fn an_unrealized_gain_is_the_restated_amount_minus_the_booked_one() {
        // €10,000.00 booked when the rate was 0.85 (£8,500.00); closing rate
        // 0.8871 (£8,871.00): an unrealized gain of £37.10.
        let items = [MonetaryItem {
            foreign_minor: 1_000_000,
            currency: "EUR".into(),
            booked_base_minor: 850_000,
        }];
        let mut rates = std::collections::HashMap::new();
        rates.insert("EUR".to_string(), rate("0.8871"));
        let report = revalue(&items, &rates, "GBP", TieMode::HalfUp).expect("revalues");
        assert_eq!(report.entries.len(), 1);
        let entry = report.entries.first().expect("one entry");
        assert_eq!(entry.restated_base_minor, 887_100);
        assert_eq!(entry.booked_base_minor, 850_000);
        assert_eq!(entry.difference_minor, 37_100);
        assert_eq!(report.net_difference(), 37_100);
    }

    #[test]
    fn a_loss_is_negative_and_the_report_names_which_currency_moved() {
        let items = [
            MonetaryItem {
                foreign_minor: 100_000,
                currency: "USD".into(),
                booked_base_minor: 79_000,
            },
            MonetaryItem {
                foreign_minor: 50_000,
                currency: "JPY".into(),
                booked_base_minor: 27_000,
            },
        ];
        let mut rates = std::collections::HashMap::new();
        rates.insert("USD".to_string(), rate("0.7842"));
        rates.insert("JPY".to_string(), rate("0.5105"));
        let report = revalue(&items, &rates, "GBP", TieMode::HalfUp).expect("revalues");

        // USD: £784.20 restated vs £790.00 booked — a loss of £5.80.
        assert_eq!(
            report
                .entries
                .first()
                .expect("two entries")
                .difference_minor,
            -580
        );
        // JPY: 50,000 × 0.5105 = 25,525p restated vs £270.00 booked — a loss
        // of £14.75.
        assert_eq!(
            report.entries.get(1).expect("two entries").difference_minor,
            -1_475
        );
        assert_eq!(report.net_difference(), -2_055);
        assert_eq!(
            report.per_currency(),
            vec![("JPY".to_string(), -1_475), ("USD".to_string(), -580)]
        );
    }

    #[test]
    fn a_base_currency_item_is_refused_not_treated_as_one_to_one() {
        let items = [MonetaryItem {
            foreign_minor: 1_000,
            currency: "GBP".into(),
            booked_base_minor: 1_000,
        }];
        let rates = std::collections::HashMap::new();
        assert_eq!(
            revalue(&items, &rates, "GBP", TieMode::HalfUp),
            Err(FxError::BaseCurrencyItem)
        );
    }

    #[test]
    fn a_missing_closing_rate_is_a_data_error_not_a_zero() {
        let items = [MonetaryItem {
            foreign_minor: 1_000,
            currency: "CHF".into(),
            booked_base_minor: 900,
        }];
        let rates = std::collections::HashMap::new();
        assert!(revalue(&items, &rates, "GBP", TieMode::HalfUp).is_err());
    }
}
