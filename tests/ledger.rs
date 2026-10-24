//! The revaluation posting, executed against a real `double-entry` ledger.
//!
//! A computed difference proves arithmetic, not accounting. These tests take
//! the month-end revaluation and post it: each revalued item's difference
//! lands in the exchange gain/loss account, and the ledger's own balance check
//! is what proves the posting is complete. The closing invariant is the point:
//! after revaluation, every foreign-currency balance equals its restated base
//! amount to the penny.

#![allow(clippy::expect_used, clippy::panic)]

use double_entry::money::currencies;
use double_entry::{Account, AccountId, AccountType, JournalEntry, Ledger, Line};
use fx_kit::{MonetaryItem, Rate, revalue};
use std::collections::HashMap;
use vat_rules::TieMode;

fn chart() -> Ledger {
    let mut ledger = Ledger::new();
    for (id, name, kind) in [
        ("1100", "Trade debtors", AccountType::Asset),
        ("1200", "Foreign cash (USD)", AccountType::Asset),
        ("2100", "Trade payables", AccountType::Liability),
        ("3100", "Owner's equity", AccountType::Equity),
        ("4900", "Exchange gain/loss", AccountType::Revenue),
    ] {
        ledger.add_account(Account::new(id, name, kind));
    }
    ledger
}

#[test]
fn the_month_end_revaluation_posts_and_the_sheet_still_balances() {
    let mut ledger = chart();
    let mut version = 0u64;

    // The books at 2026-09-30, all booked at the old rates:
    // - €10,000.00 debtor booked at £8,500.00
    // - $5,000.00 cash booked at £3,950.00
    // - €4,000.00 payable booked at £3,400.00 (a credit)
    // Opening equity against the three foreign balances, all booked at the
    // old rates.
    version = ledger
        .post(
            &JournalEntry::draft(
                "JE-OPENDR",
                "2026-09-30",
                "2026-09",
                vec![
                    Line::debit(
                        "1100",
                        double_entry::Amount::minor(850_000, currencies::GBP),
                    ),
                    Line::credit(
                        "3100",
                        double_entry::Amount::minor(850_000, currencies::GBP),
                    ),
                ],
            ),
            version,
        )
        .expect("the debtor opening posts");

    // Cash in USD: debit 1200 / credit equity £3,950.00.
    version = ledger
        .post(
            &JournalEntry::draft(
                "JE-CASH",
                "2026-09-30",
                "2026-09",
                vec![
                    Line::debit(
                        "1200",
                        double_entry::Amount::minor(395_000, currencies::GBP),
                    ),
                    Line::credit(
                        "3100",
                        double_entry::Amount::minor(395_000, currencies::GBP),
                    ),
                ],
            ),
            version,
        )
        .expect("cash posts");

    // The payable: credit 2100 €4,000.00 booked £3,400.00 / debit equity.
    version = ledger
        .post(
            &JournalEntry::draft(
                "JE-PAY",
                "2026-09-30",
                "2026-09",
                vec![
                    Line::debit(
                        "3100",
                        double_entry::Amount::minor(340_000, currencies::GBP),
                    ),
                    Line::credit(
                        "2100",
                        double_entry::Amount::minor(340_000, currencies::GBP),
                    ),
                ],
            ),
            version,
        )
        .expect("payable posts");

    // The revaluation population: monetary items only.
    let items = [
        MonetaryItem {
            foreign_minor: 1_000_000,
            currency: "EUR".into(),
            booked_base_minor: 850_000,
        },
        MonetaryItem {
            foreign_minor: 500_000,
            currency: "USD".into(),
            booked_base_minor: 395_000,
        },
        MonetaryItem {
            foreign_minor: -400_000,
            currency: "EUR".into(),
            booked_base_minor: -340_000,
        },
    ];
    let mut rates = HashMap::new();
    rates.insert("EUR".to_string(), Rate::parse("0.8871").expect("rate"));
    rates.insert("USD".to_string(), Rate::parse("0.7842").expect("rate"));

    let report = revalue(&items, &rates, "GBP", TieMode::HalfUp).expect("revalues");

    // Post each difference: the item's account takes the restatement, the
    // exchange account takes the difference.
    for entry in &report.entries {
        let account = match entry.currency.as_str() {
            "EUR" => {
                if entry.booked_base_minor < 0 {
                    "2100"
                } else {
                    "1100"
                }
            }
            "USD" => "1200",
            _ => unreachable!("fixture currencies"),
        };
        // The adjusting entry: restate the item to its new base amount.
        // The difference is a single line pair against exchange.
        let adjustment = entry.difference_minor;
        let (gain_line, loss_line) = if adjustment >= 0 {
            (
                Line::debit(
                    account,
                    double_entry::Amount::minor(adjustment, currencies::GBP),
                ),
                Line::credit(
                    "4900",
                    double_entry::Amount::minor(adjustment, currencies::GBP),
                ),
            )
        } else {
            (
                Line::debit(
                    "4900",
                    double_entry::Amount::minor(-adjustment, currencies::GBP),
                ),
                Line::credit(
                    account,
                    double_entry::Amount::minor(-adjustment, currencies::GBP),
                ),
            )
        };
        version = ledger
            .post(
                &JournalEntry::draft(
                    format!("JE-FX-{}", entry.item_index),
                    "2026-09-30",
                    "2026-09",
                    vec![gain_line, loss_line],
                ),
                version,
            )
            .expect("the revaluation posting balances");
    }

    // The close invariant: each foreign balance now equals its restated base.
    // EUR debtor: £8,871.00. USD cash: £3,921.00 (500,000 × 0.7842).
    // EUR payable: −£3,548.40 (−400,000 × 0.8871).
    assert_eq!(ledger.balance(&AccountId::new("1100")), 887_100);
    assert_eq!(ledger.balance(&AccountId::new("1200")), 392_100);
    assert_eq!(ledger.balance(&AccountId::new("2100")), -354_840);
    // And the exchange account carries the net of the three differences:
    // gain 37,100 (EUR debtor), loss 2,900 (USD cash), loss 14,840 (the EUR
    // payable restated further negative) — a net gain of 19,360. 4900 is a
    // credit account, so a net credit balance is negative in the raw number.
    let exchange = ledger.balance(&AccountId::new("4900"));
    assert_eq!(exchange, -19_360);
    assert_eq!(report.net_difference(), 19_360);
    // The ledger never broke balance: every revaluation posted as a pair.
    let tb_total: i64 = ["1100", "1200", "2100", "3100", "4900"]
        .iter()
        .map(|id| ledger.balance(&AccountId::new(*id)))
        .sum();
    assert_eq!(tb_total, 0, "debits still equal credits after revaluation");
}
