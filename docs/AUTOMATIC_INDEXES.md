# Automatic native index selection

`query run --database PATH --auto-index` opts into native index selection for
existing safe scalar query constraints. Library hosts use
`NativeSource::with_auto_indexes()`. The Access adapter chooses access paths;
the reusable query engine retains binding, predicate evaluation, ordering and
limits, and ACE retains physical interpretation and traversal.

```sh
tablecraft query run --database docs/examples/Access_Example_VBA.accdb --auto-index \
  'SELECT TOP 3 ORDR_NBR FROM ROULETTE_TB WHERE ORDR_NBR>4 ORDER BY ORDR_NBR DESC'
```

The CLI flag requires `--database` and conflicts with `--index` and `--sqlite`.
ACCDT template sources do not supply native indexes and reject the option.
Library callers can combine automatic mode with an explicit table index; that
explicit choice takes priority, including its normal eligibility/error rules.

## Selection and execution

Selection starts only when execution supplies an existing wholly safe constraint.
Preparation discovers schema without reading index pages or user records. The
first matching single-key definition in native definition order is used. Eligible
keys are Byte, Int16, Int32, Int64, Currency and GUID under their existing engine
profiles: exact typed intervals/equality and qualified NULL tests retain their
existing semantics. NULL-only constraints also select qualified single General
Text and Short Binary indexes; empty strings/bytes are separate non-NULL values.
These NULL constraints require no SQL string comparator or locale provider.
Single direct Short Text equality can additionally use
[comparer-qualified hints](TEXT_EQUALITY_SEEKS.md) under `--collation database`;
its actual resolver and source must agree before index selection. Native field identities map to exposed scalar columns by
validated names; mixed native schemas retain their scalar-view boundary.

Declarations must have known local ordinary/primary constraints and include NULL
keys. Unknown flags, foreign references, ignore-NULL declarations, compound keys,
unrelated fields and unsupported constraint/carrier pairs are skipped before
traversal. This does not implement costing or selectivity estimates. No compatible
candidate means an ordinary sequential read. Malformed/ineligible metadata that
prevents complete inspection remains an error.

Once a candidate is selected, read, corruption, unsupported-profile, cancellation
and quota failures propagate. No other index or sequential scan is tried after
that failure. Both dense and pruned projections use the same selection service.
The original WHERE remains mandatory. Partial AND extraction, coercive predicates,
SQL Text inequalities and non-NULL Binary bounds, Boolean/floating/date seeks and composite seeks
remain unsupported. Physical Text/Binary ranges are not SQL predicate semantics.
Unconstrained reads remain sequential unless a library caller explicitly requires
an index. Automatic selections retain sequential table root order, preserving
TOP and encounter-aggregate behavior; explicit required indexes retain their
chosen key order. SQL output ordering still requires ORDER BY for a public guarantee.

Selected snapshots are ephemeral and never published as complete table/projection
caches. Later unconstrained consumers must load a complete snapshot. Existing
complete caches can be reused without consulting an index. Metadata inspection,
candidate mapping and native reads share cumulative work/allocation/I/O budgets;
snapshot and index structural ceilings remain distinct. Tracing retains operation
names/counts without table/index names, keys or SQL. Skipped subtrees are not
validated as part of a bounded selection. Ordinary write gates are unchanged.

## Evidence

Six focused adapter tests cover ascending/descending Byte and Int16 indexes in
four priority header profiles, metadata-only preparation, dense/pruned reads,
tiny snapshot limits, NULL keys, full-count-after-selection refusal, skipped
ignore-NULL/unknown declarations, corrupt chosen indexes, explicit precedence,
whole-predicate eager errors, table order through descending keys, TOP/First/Last
and every observed selection cancellation checkpoint. Engine controls compare
relocated-root reads with sequential order and reject invalid row ceilings before I/O.
Existing typed/projection/GUID adapter controls and CLI argument/order checks
retain their separate coverage.

The original [independent runner](testing/compare_automatic_indexes.py) passes 86
controls against inert Jackcess getters over four unchanged registered
[20,000-row inputs](FIXTURES.md#original-large-index-inputs). It checks all six key
carriers, NULL/equality/ranges, exact LongLong boundaries, prepared parameters,
nested full counts, sequential encounter order, TOP/First/Last and buffered refusals. Successful default-limit selections
also demonstrate that the oversized complete table was not materialized. Only
TableCraft executes SQL; this is independent engineering evidence, not Windows
Access execution or native optimizer equivalence.

```sh
cargo build -p tablecraft-cli --locked --offline
python3 docs/testing/compare_automatic_indexes.py \
  --jar .cache/tablecraft-fixtures/jackcess-5.0.4.jar \
  --cli target/debug/tablecraft --directory .cache/tablecraft-fixtures/large-indexes
```

## Debug CLI point-read measurement

Hyperfine 2.0.0 on Darwin arm64, rustc 1.99.0, debug CLI, three warmups and 15
runs per command. The query selects ID 19999 with ORDER BY ID from each registered
20,000-row input. Both paths receive identical raised native/execution allowances;
all return the independently checked single row. Input files are stable and OS
cache is warm. No Cargo build, independent runner or competing benchmark ran
during these measurements. Times include process startup and output formatting.

| Profile | Sequential mean ± SD, ms | Explicit index, ms | Automatic, ms | Sequential / automatic |
| --- | --- | --- | --- | --- |
| Jet 4 | 97.045 ± 16.279 | 5.254 ± 2.617 | 5.047 ± 0.482 | 19.23× |
| ACE 2007 | 90.014 ± 2.244 | 4.883 ± 0.423 | 4.730 ± 0.455 | 19.03× |
| ACE 2016 | 99.157 ± 9.231 | 4.967 ± 0.365 | 4.918 ± 0.165 | 20.16× |
| ACE 2019 | 90.227 ± 1.090 | 4.812 ± 0.678 | 5.375 ± 0.451 | 16.79× |

Raw [three-path measurements](testing/results/automatic-index-2026-10-24.json)
retain every sample and exact command. Earlier
[two-path](testing/results/automatic-index-initial-2026-10-24.json) and
[three-path](testing/results/automatic-index-pre-order-2026-10-24.json) runs are
retained as preliminary measurements before the source-order correction; they
are excluded from the table above. No outliers were removed, including the noisy
Jet 4 and ACE 2016 samples. Automatic and explicit index times overlap here and
do not establish a systematic winner between those paths; this feature
chooses an already implemented seek rather than making that seek kernel faster.
These debug point reads do not establish release throughput, cold-cache latency,
broad range performance, a general optimizer speedup or native Access performance.
Measured binary SHA-256:
`5ab69087462235a3a018b02ce930aa8158d131ed273d7c15543ce038c4e414fa`.

## NULL-only Text and Binary selection

An existing `ScanConstraint::Null` now maps to the native typed Text/Binary NULL
request for both explicit and automatic matching indexes. Column matching,
ignore-NULL/foreign/compound exclusions, traversal qualification, key/row agreement,
shared quotas and cancellation retain the same rules as other selected reads.
Text profiles still need the qualified native General descriptor even though the
SQL predicate itself does not compare strings. An unsupported chosen profile is
an error; it is never reinterpreted with another policy or retried sequentially.

Preparation still does not traverse indexes. The residual IS NULL predicate stays
in the executor. Automatic results retain sequential root order, including TOP
and First/Last. A later unfiltered consumer loads a complete table independently
of the ephemeral selected snapshot. IS NOT NULL and non-NULL string/binary
predicates do not acquire this NULL selection. Short Text equality has its
separate comparer-qualified contract above. An unsafe AND does not contribute a
partial NULL hint, preserving eager errors.

The original [NULL selection runner](testing/compare_null_indexes.py) passes
216 controls over four-family Binary indexes and both General versions of Text indexes against
inert sequential Jackcess getters. It compares automatic and explicit ascending/
descending selections, complete reads versus a smaller row ceiling, TOP/First/
Last, empty versus NULL, empty tables, complete-cache boundaries, ignored/unrelated
indexes and corrupt chosen roots. Inputs remain immutable; corrupt experiments
use disposable copies. This evidence qualifies the explicit engineering read
profile; Windows Access SQL/optimizer behavior remains unrun.
