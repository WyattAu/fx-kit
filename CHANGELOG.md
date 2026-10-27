# Changelog

All notable changes to this project are documented here. Format: [Keep a
Changelog](https://keepachangelog.com/) — versions follow [semver](https://semver.org).

## [Unreleased]

## [0.1.3] - 2026-10-27

### Changed

- Tracks `vat-rules 0.1.1` — the graph's convergence pin. No behavioural change.


## [0.1.0] - 2026-10-24

Initial release.

### Added

- `Rate::parse` — decimal rate strings to exact reduced rationals; floats
  never enter the path from quote to posted difference.
- `Rate::restate` — `foreign_minor × num / den` in i128, one rounding at the
  final division, tie mode a parameter.
- `revalue` — closing-rate restatement of monetary items with the exchange
  difference per item, net and per-currency totals. Base-currency items and
  missing rates are errors, not silent no-ops.
- A `double-entry` integration test posting the month-end revaluation and
  proving debits still equal credits afterwards.
