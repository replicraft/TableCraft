# Exact Byte and Int16 index ranges

`ByteIndexSelection` and `Int16IndexSelection` carry unsigned `u8` and signed
`i16` bounds through `IndexProjectionSelection` and `IndexReadRequest`.
All, NULL and range requests require the matching single-column native key kind;
equal widths or a request without numeric bounds do not relax that check.
Private widening reuses the checked integer walker without changing stored bytes.
Numeric ranges exclude NULL, preserve duplicate/source order and support inclusive,
exclusive, empty and reversed bounds in either physical index direction.

## Query integration

Backend-neutral `ScanConstraint::Byte` and `ScanConstraint::Int16` carry exact
bounds. Wholly safe comparisons and same-column conjunctions can use an explicitly
selected matching index. Integer literals, plain non-NULL Byte/Int16/Int32/Int64
parameters and checked closed integral `CByte`/`CInt` literal conversions are
eligible. Unary literal signs are supported; unary parameter expressions are not.
Out-of-width endpoints and invalid casts decline the hint rather than clamp it
or introduce a new semantic error on an empty source. Text, fractional constants,
wider conversions and arbitrary calls are not evaluated by this analysis.

The original WHERE always runs. No partial AND extraction, automatic index choice
or sequential fallback for a failed required index is introduced. Selected
snapshots remain ephemeral; a later full-table query must load the complete source.
Physical interpretation stays in ACE, adaptation in Access and planning in query.

## Verification

Focused authored tests cover Jet 4, ACE 2007 and both supported ACE 16-family
headers, both directions, exact endpoints, duplicates, NULL, zero-width projections,
wrong-kind requests, parameter reuse and cancellation. The existing complete
narrow-index tests remain unchanged. The focused batch passes 12 engine, 13 Access
and 40 retrieval tests. All-target/all-feature Clippy for those three packages,
standalone minimal engine/query checks and a debug CLI build pass.

The original [comparison runner](testing/compare_narrow_ranges.py) reuses four
registered 20,000-row inputs and reviewed read-only Jackcess getters. Its 128
controls compare exact counts/ID sums against independent integer models, including
typed parameters, out-of-width endpoints, eager failures and a 6,000-row retention
cap that permits a point selection but refuses a subsequent complete read.
JAR/input hashes and unchanged originals are checked.

```sh
python3 docs/testing/compare_narrow_ranges.py --cli target/debug/tablecraft --jar path/to/jackcess-5.0.4.jar --directory path/to/large-index-inputs
```

This extends selection coverage, not the index encoding algorithm. No speedup is
claimed. Native Access SQL comparison remains unrun; no dependency, write gate,
stored value or shared-file guarantee changes.
