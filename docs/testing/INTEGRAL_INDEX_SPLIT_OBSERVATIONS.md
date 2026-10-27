# Fixed numeric index-split reference observations

The separate MAX-action phase uses the two unchanged
[fixed numeric source files](INTEGRAL_INDEX_SPLIT_SOURCE_OBSERVATIONS.md), with
Byte, Int16, Int64 and Currency targets on each. It records eight successful
Jackcess updates and eight atomic refusals from the frozen, non-splitting
TableCraft fixed-scalar consumer. It does not qualify the new typed split API,
normal native saves or Microsoft Access behavior.

The [receipt](results/ace16-integral-index-split-actions-2026-10-27.json) pins the
actual first execution, reviewed private freeze, original files, compiled old
consumer and independent analysis. No source was regenerated, no action retried
and no budget, selector or field flag changed. The Java and Python observation
helpers published alongside this document are exact copies of the executed
sources. The saved-proof verifier is a later, read-only derivative.

## Exact operations and outcomes

For each target, the selected row is original ordinal zero, ID 1, with KeyValue
zero and marker 17. Each action uses a fresh copy of its original. Values use
exact Java Byte, Short, Long or BigDecimal setters; Currency uses scaled integer
units without a floating-point conversion.

| Target | Replacement | Rows in target | Rows per new leaf |
| --- | --- | --- | --- |
| Byte | 255 | 900 | 450 |
| Int16 | 32,767 | 900 | 450 |
| Int64 | 9,223,372,036,854,775,807 | 400 | 200 |
| Currency | 922,337,203,685,477.5807 | 400 | 200 |

Both V2016 and V2019 originals contain 2,729 total rows. Their Required properties
are actual Boolean DDL properties. An ascending and a descending physical index
cover each target's KeyValue; Alias shares the ascending physical index.

The actual first execution completes 25 native/JVM fixture commands: one javac,
then eight groups of old-consumer attempt, fresh public UPDATE and fresh read-only
description. All eight old-consumer attempts exit 1 with
`Unsupported(IndexedColumn)`, empty stdout and no output file. All eight public
updates and descriptions exit zero. The source files and frozen tools remain
unchanged. Java and javac each use an explicit 512 MiB heap ceiling; files remain
under 64 MiB and each process has a 120-second limit.

## Measured storage behavior

All sixteen affected physical indexes retain their original root page and grow
from height zero to one. Each root has one complete key/ROW left bound and a
right tail. Each physical index allocates its right leaf before its left leaf;
physical data number 1 precedes number 2. Each edited copy therefore appends four
index pages (16,384 bytes), without retiring a page. The new leaves have prefix
zero. Alias retains the same physical tree as Ascending.

Independent raw decoders and fresh getters compare every table's typed values,
markers and opaque fields, all physical ROW references, complete ordered key/ROW
streams, index/map membership and selected field bytes. Exactly one raw record
body changes, only in the requested field. No row moves, historical/raw or getter
counter changes, or unrelated index-page rewrites occur in these eight actions.

A separate saved-copy framing analysis finds all sixteen old root suffixes
unchanged and no changes outside the engineering root spans. All 32 new public
leaves contain nonzero unused scratch: Byte leaves have 456/457 nonzero bytes,
Int16 233/234, and Int64/Currency 312/858, by direction. These bytes are measured
public-writer residue, not expected TableCraft padding. Kernel candidates must
instead follow their own independently derived complete-image and fresh-padding
contracts.

## Saved evidence check

From the repository root, with the recorded ignored artifacts still available:

```sh
python3 docs/testing/integral_split_originals/verify_actions.py \
  --report .cache/ace16-integral-split-actions-first-20261027/report.json
```

This rehashes saved inputs, tools, logs and outputs, replays the pure row/key/map
analysis against saved getters, and checks the framing observations. It does not
invoke Java, a native consumer, generation or an UPDATE. Its first execution and
subsequent framing extension both pass; the raw first report remains unchanged.

Typed candidate acceptance, recursive typed parent splits, selected indirect maps,
map growth, general allocation, normal saves and native Access save/reopen
qualification remain separate work.
