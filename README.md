# fx-kit

Currency revaluation per IAS 21: restating monetary items at the closing rate
and computing the unrealized exchange gain or loss. Exact rational rates —
no floats anywhere on the path from quoted rate to posted difference.

## The rule the rate type exists to enforce

An FX rate is a **rational number**. `"1.0834"` is exactly 10834/10000
(reduced to 5417/5000); as an f64 it is not, and a thousand-row revaluation
turns "not" into visible pennies of phantom gain. Rates parse from their
decimal strings into exact numerator/denominator pairs, restatement is
`foreign_minor × num / den` in i128 with **one** explicit rounding at the
final division (the tie mode is a parameter, because jurisdictions disagree
about ties), and the difference is `restated − booked` — computed once, never
inferred from two float products.

## What IAS 21 actually asks for

**Monetary items** (cash, receivables, payables — amounts of currency itself)
are restated at the **closing rate** each reporting date, and the exchange
difference goes to profit or loss (IAS 21.28). **Non-monetary items**
(inventory, fixed assets) stay at their historical rate and are *not*
restated — the input type is named `MonetaryItem` so passing the wrong class
is a type error, not a silent misstatement.

## The output

`revalue` returns one `Revaluation` per item — restated, booked, difference —
plus per-currency totals, because a revaluation whose gains and losses are
shown only net hides which currency moved, and the note to the accounts wants
the split.

Base-currency items in the population are **refused**, not treated as 1:1 —
a GBP row in a GBP revaluation is a data error upstream, and "silently no-op"
is how it survives into the accounts. A missing closing rate is likewise an
error, not a zero.

## Licence

MIT OR Apache-2.0.
