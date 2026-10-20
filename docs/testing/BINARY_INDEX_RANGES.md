# Typed Short Binary index ranges

`read_table_binary_index_range` and `IndexProjectionSelection::Binary` accept
`BinaryIndexSelection`: All, Null or a borrowed byte Range with inclusive,
exclusive or unbounded endpoints. Bounds use ascending lexicographic bytes in
either native index direction. Ranges exclude NULL; empty Binary is a value.
Reversed/empty intervals return no rows, without SQL coercion or BETWEEN swapping.
Each endpoint is capped at `BinaryIndexSelection::MAX_BOUND_BYTES` (510); readers
validate that before traversal. An endpoint may exceed a particular field's
declared length, since it is a comparison bound rather than a stored value.

Selections borrow caller slices and allocate no endpoint copies. The projection
selector carries that lifetime; returned snapshots still own all schema/values.
Even All/Null require a qualified single native Short Binary index. Numeric,
composite, OLE and other unqualified profiles cannot become complete scans through
this API. No index fallback, automatic optimizer selection or write support is added.

The existing walker prunes disjoint physical spans before reading their pages and
fetches only matching row roots. Bounds are deliberately coarse at duplicate
separator keys, whose row references may outlive deleted values. Native empty keys
precede nonempty values even descending, so a logical interval can select separated
physical regions. Pruning uses their conservative continuous hull: intervening
index keys can be scanned but unselected row payloads are not decoded. This preserves
the walker's contiguous leaf-chain validation rather than skipping across gaps.
It does not promise the minimum number of index-page reads.

Only visited pages/selected roots are checked. All retains full count validation;
Null/Range do not audit skipped subtrees or establish full table integrity. Existing
ownership-map, depth, row, physical-read and caller resource/cancellation limits
remain. Projected reads still decode/check required binary key fields before
removing them from output. Normal query execution retains complete Binary snapshots
and residual filtering; automatic predicate pushdown remains separate work.

## Evidence and reproduction

Codec/range tests cover empty/NULL, included/excluded/unbounded bounds, reversals,
too-long endpoints, width qualification, cancellation/budgets and exact selected
rows across all profiles/directions. A fixed-seed property checks that physical
hulls never reject a selected candidate against independently encoded key order.
Selected padding damage fails even when the binary key is absent from output.

The unchanged [original Binary fixtures](BINARY_INDEX_READS.md) pass twelve
selections in both directions through full/projected APIs: 48 controls per profile.
Expected values come from independent read-only native cursors, filtered using
Python's raw byte ordering. A separate temporary copy damages the ascending root's
rightmost subtree. Null selection still matches the oracle; All fails atomically.
These add two controls per profile, 200 total. Original hashes are checked after
all experiments. No fixture was regenerated or new dependency adopted.

```sh
CARGO_INCREMENTAL=0 cargo build -p tablecraft-ace --example binary_range --release --locked --offline
python3 docs/testing/compare_binary_ranges.py \
  --jar /tmp/reviewed/jackcess-5.0.4.jar --cli target/release/tablecraft \
  --reader target/release/examples/binary_range --directory /tmp/existing-binary-index-inputs
```

The engineering example uses clap::Parser, explicit read-only input and bounded
atomic tagged output. Empty hexadecimal endpoint text denotes empty Binary.
Its controls belong to the comparison tool, not the ordinary CLI. Native Access
SQL, primary/foreign Binary fixtures, writes, Windows and live-file qualification
remain unrun; these results qualify stored Binary range retrieval only.
