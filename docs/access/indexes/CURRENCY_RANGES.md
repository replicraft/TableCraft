# Exact Currency index selections

Qualified single-Currency indexes now accept exact bounded reads through
`IndexReadRequest` / `IndexProjectionSelection::Currency`. The standalone engine
owns `CurrencyIndexSelection::{All, Null, Range}`. Bounds contain core Currency
values and use ascending monetary order in either physical index direction.
Only the engine maps them to signed ten-thousandths for the existing integer-key
walker. No bounds pass through floating point.

Currency and Int64 remain distinct semantic carriers despite their equal encoded
width. A Currency selection rejects Int64/other/composite indexes, including
All/Null; the existing integer APIs still reject Currency. Complete traversal
checks complete counts. Range/NULL selections audit visited pages and returned
records, excluding skipped subtrees/payloads. Requested projections still decode
and validate hidden key fields. Row, byte, work and cancellation limits remain.

## Query and CLI integration

Backend-neutral `ScanConstraint::Currency` supplies exact intervals to an explicit
matching native index. The original WHERE always evaluates. Supported hints are
single-Currency-field `=`, `<`, `<=`, `>`, `>=`, reversed operands, IS NULL/IS NOT
NULL and wholly safe same-column AND intersections. Constants are non-NULL typed Currency
parameters or closed numeric `CCur` literals, optionally signed. Numeric literals
retain the evaluator's current materialization/rounding profile; this is not new
native SQL literal inference.

Text conversions, arbitrary arithmetic/calls, mixed fields and unsafe conjunctions
receive no Currency hint. Invalid/overflowing constants decline hints so they do
not create semantic errors on empty inputs. Analysis still consumes shared work
and observes cancellation. A partial safe AND cannot hide an eager error.

```sh
tablecraft query run 'PARAMETERS P CURRENCY; SELECT ID FROM Payments WHERE Amount=P' \
  --database example.accdb --index Payments AmountIndex \
  --param P=currency:900719925474.0993
```

`currency:` CLI parameters use the existing invariant numeric Text conversion:
period decimal, no grouping/currency symbol, checked Currency range and four-place
half-even rounding. Signed endpoints and adjacent scaled values beyond exact Double
integer precision are preserved. Input copies/normalization share one bounded
conversion budget across parameter bindings. No machine locale is inferred.

Required index errors never cause sequential fallback. Selected snapshots remain
ephemeral and cannot become complete-table caches. Other predicates still use the
required complete index path. Automatic index choice, composite Currency seeks,
shared-file coordination, write expansion and native Access qualification remain
separate work.

## Correctness evidence

Authored layouts cover Jet 4, ACE 2007 and ACE 16-family headers, both directions,
NULL/zero/duplicates, signed endpoints, adjacent scaled units above 2^53, open/closed/
empty/reversed bounds, hidden key projections and wrong-carrier rejection. Adapter
controls reuse one-row retention limits across successive exact seeks, then require
a complete read to fail rather than reuse a subset. Corrupt required roots fail.
Planner controls compare honoring/ignoring sources, parameter reuse, binary-literal
rounding, empty inputs, unsafe predicates and every observed cancellation checkpoint.

The original MIT [comparison runner](../../testing/compare_currency_ranges.py) passes
136 independent controls on four unchanged 20,000-row inputs plus the two registered
700-row ACE 16 Currency fixtures. Read-only Jackcess getters supply exact scaled
keys; Python models counts/sums and bounds. Both physical directions, NULL tests,
full endpoints, a 6,000-row allowance and cache-isolation failures are checked.
No Access SQL or application content executes in the oracle.

## Release measurements

[The runner](../../testing/benchmark_currency_ranges.py) times identical release SQL with
sequential or explicitly indexed access. Hyperfine 2.0.0 ran 15 process samples per
command with three warmups on macOS 14.7.2 arm64. Input hashes and counts agree.
Process wall time includes startup, metadata, reads, materialization and evaluation;
OS file caches are warm. These are milliseconds, not scalar-kernel nanoseconds.

| Workload | Matches | Sequential mean ± SD, ms | Currency index mean ± SD, ms |
| --- | ---: | ---: | ---: |
| 20,000-row equality | 5,334 | 11.888 ± 0.357 | 7.848 ± 1.859 |
| 20,000-row empty match | 0 | 11.725 ± 2.216 | 3.242 ± 2.608 |
| 700-row equality | 54 | 3.662 ± 1.442 | 2.466 ± 0.155 |
| 700-row IS NOT NULL | 636 | 2.608 ± 0.159 | 4.032 ± 1.837 |

The broad small-table indexed read is slower. Some runs have substantial variance;
no statistical significance, universal speedup or cold-storage result is claimed.
An initial large-point run overlapped compilation/validation and is excluded from
conclusions. Hyperfine warned about the first 6.2 ms sequential small-point sample;
all its samples remain included. [Raw samples](../../testing/results/currency-ranges-2026-10-22.json)
retain both observed runs and the excluded run, with local paths/host metadata
removed. Bounded index reads and sequential reads have different integrity-audit
scope even when their outputs agree on these valid fixtures. Native Access and
mixed-client behavior remain unqualified.
