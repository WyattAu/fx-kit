# Changelog

All notable changes to this project are documented here. Format: [Keep a
Changelog](https://keepachangelog.com/); versions follow [semver](https://semver.org).

## [Unreleased]

## [0.1.5] - 2026-10-27

### Added

- `Rate::numerator` / `Rate::denominator`; the reduced rational parts, exposed for property-test cross-validation.
- Property tests: identity rate restates exactly, restatement is sign-symmetric and monotone, `parse_percent` is `parse` over exactly 100, and the parser boundary never panics on arbitrary strings.


## [0.1.4] - 2026-10-27

### Added

- `Rate::parse_percent` - quoted percentages to exact reduced rationals ("6.25" as 6.25%, i.e. a multiplier of 1/16). US rate data is quoted as percents by convention; parsing it as a multiplier makes every rate a hundred times too large.


## [0.1.3] - 2026-10-27

### Changed

- Tracks `vat-rules 0.1.1`; the graph's convergence pin. No behavioural change.


## [0.1.0] - 2026-10-24

Initial release.

### Added

- `Rate::parse`; decimal rate strings to exact reduced rationals; floats
  never enter the path from quote to posted difference.
- `Rate::restate`; `foreign_minor × num / den` in i128, one rounding at the
  final division, tie mode a parameter.
- `revalue`; closing-rate restatement of monetary items with the exchange
  difference per item, net and per-currency totals. Base-currency items and
  missing rates are errors, not silent no-ops.
- A `double-entry` integration test posting the month-end revaluation and
  proving debits still equal credits afterwards.
