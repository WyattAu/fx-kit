# Pattern adoption

FX and tax revaluation arithmetic over integer minor units.

| Pattern | Status | Evidence |
|---|---|---|
| T1 | adopted | src/lib.rs:61 (Rate::parse, and parse_percent at src/lib.rs:137, parse quoted rate strings into validated components once at the boundary) |
| P11 | partial | src/lib.rs:176 (remainder inspected and rounding applied once at the final division, src/lib.rs:11; no general partition or allocate API) |
| P64 | adopted | tests/properties.rs (proptest pins the rate algebra the IAS 21 revaluation depends on; worked cases pinned in tests/ledger.rs) |
