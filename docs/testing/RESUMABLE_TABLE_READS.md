# Resumable scalar table reads

`tablecraft-ace::TableCursor::open` qualifies a local scalar schema and the complete
owned-map metadata, then retains the map position and current data page/slot across
`next_page` calls. It exclusively borrows one stable/offline source. Batches own
their decoded rows and logical offsets; schema/columns remain on the cursor.
Physical order is not SQL ordering. This is not a native snapshot or a persistent,
serializable bookmark, and it acquires no shared-file lock.

Each fetch accepts a nonzero limit up to 16,384, a budget and cancellation callback.
Its physical-read quota starts afresh. Reusing the budget preserves cumulative
accounting; supplying another budget is an explicit host decision. Returned values
and each read/allocation remain charged, including failed fetches. `NativeBrowser`
exposes `cursor(table)` through `BrowserCursor`, borrowing the browser's original
cumulative budget/callback without refilling them. Drop a cursor before another
operation on that source/browser.

`TableCursor::open_projection(source, scope, table, names, budget, cancelled)`
and `NativeBrowser::projected_cursor(table, names)` qualify an explicit ordinary
scalar subset. Output follows native logical column order, irrespective of request
order, using the existing ASCII-insensitive name policy. Duplicate/missing names
fail; unsupported selected fields fail even on empty tables. The captured schema
still contains every declaration, so unavailable fields remain visible metadata.
Unselected payloads are neither decoded nor replaced by NULL. An empty selection
returns zero-width rows, with fetched row framing and live counts still checked.
Full snapshots and projected cursors share selection qualification, including
fallible allocation, comparison work accounting and cancellation.

EOF means all live roots were enumerated and their count matched the declaration.
A full final batch may require another empty fetch to establish EOF. Deleted roots
do not consume row positions; selected relocations and long values use the checked
native readers. Unsupported requested columns fail during open, including on empty tables.
Unvisited payloads remain unchecked until fetched; this is not a whole-file audit.

Invalid batch sizes leave a ready cursor unchanged. Any later fetch error makes it
terminal: no partial batch is returned, `position()` counts only successful batches,
no budget is refunded, and subsequent fetches return `CursorFailed` without I/O or
calling cancellation. This prevents retrying a partially advanced traversal and
silently losing records. Previously returned batches remain valid.

The ordinary buffered CLI `rows` command retains independent offset requests and its
16 MiB cumulative/1 MiB buffered-output limits. Query execution still materializes
bounded snapshots through the batched loading below. Output remains buffered and
the ordinary CLI budget does not refill between internal batches.

An explicit [CLI streaming mode](../cli/STREAMING_ROWS.md) now uses the same browser
cursor with cumulative controls and progressive TSV publication. It can return
more than a complete-snapshot cap without changing ordinary query materialization.

NativeSource now uses `read_table_batched` / `read_table_projection_batched` to
materialize complete cached snapshots through 256-row batches. The same caller
budget spans opening, every fetch and aggregate storage; the physical-read quota
applies per batch. The 16,384-row snapshot cap remains. Complete row/count/schema
checks finish before a snapshot reaches a query visitor or cache. The original
`read_table` and `read_table_projection` functions retain their previous materialized
map behavior for existing consumers. This is [bounded snapshot loading](BATCHED_QUERY_SNAPSHOTS.md),
not streaming query execution or permission to skip unselected row framing.

## Evidence and reproduction

Synthetic tests cover cached-page continuation, exact-once physical data-page reads,
empty/exact/full/partial batches, invalid limits, late count mismatches, deleted
roots/relocations, cancellation, terminal failures, cumulative row budgets and
independent generated row sequences. All four read profiles traverse 6,000-page
inline and indirect maps with explicitly fresh per-fetch budgets; corrupt later
map metadata prevents opening before user data reads.

The unchanged original [large-map fixtures](ALLOCATION_MAP_PAGING.md) independently
match all 6,000 getter rows for V2000, V2007, V2016 and V2019 at batch sizes 1, 127
and 250, for full rows and the explicit ID/P2/P5 subset requested in reversed order.
Existing calculated-scalar V2016/V2019 fixtures add five controls each: ordinary Id
cursor results at three batch sizes and refusals for full/calculated selections.
These compare stored ordinary values without recalculating expressions. Synthetic
mixed-schema tests also traverse all 6,000 selected rows in each read profile and
reject corrupt fetched row framing even with no selected columns.
Each input's SHA-256 is checked before and after. The reviewed Jackcess
5.0.4 oracle opens read-only with expression evaluation disabled. No new fixture
bytes, library dependency or native Access qualification is implied.

```sh
CARGO_INCREMENTAL=0 cargo build -p tablecraft-ace --example table_data --release --locked --offline
python3 docs/testing/compare_table_cursors.py \
  --jar /tmp/reviewed/jackcess-5.0.4.jar \
  --rows target/release/examples/table_data \
  --directory /tmp/existing-large-table-inputs \
  --mixed /tmp/existing-calculated-scalars-2016.accdb \
  --mixed /tmp/existing-calculated-scalars-2019.accdb
```

The engineering example's `--batch-size` explicitly supplies fresh controls per
fetch and writes tagged rows progressively. Its output is a comparison tool, not
the CLI's atomic display protocol; late failures may follow earlier emitted rows.
It conflicts with index/calculation/collection selections.
Repeat `--column NAME` with `--batch-size` to choose ordinary scalar fields;
omitting it preserves the full-column contract. These flags belong to this
engineering example, not the ordinary CLI `rows` command.

## Measurement

Hyperfine 2.0.0 on macOS ARM64, optimized Rust 1.99, two warmups and seven runs,
compared the same first 1,000 rows of the ACE 2019 large fixture in 25-row batches.
Each process performed twenty scans and produced the same row-count checksum.

| Method | Twenty scans, mean ± sample deviation | Amortized per 1,000-row scan |
| --- | --- | --- |
| Independent offset page requests | 212.544 ± 2.362 ms | 10.627 ms |
| Source-bound cursor continuation | 71.264 ± 9.001 ms | 3.563 ms |

Continuation was about 2.98 times faster for this workload. This is a comparison
of two current public APIs; no previous build, changed limit or different prefix
is substituted. Both have explicit fresh controls per page/batch. The harness loads
the whole roughly 25 MB input once per process into memory, so these are traversal
measurements including process/input setup, not disk throughput or reader memory
measurements. No TableCraft build, test or oracle runner overlapped timed runs.

```sh
CARGO_INCREMENTAL=0 cargo bench -p tablecraft-ace --bench native --no-run --locked --offline
# Substitute Cargo's reported executable and an existing immutable fixture path.
hyperfine --warmup 2 --runs 7 \
  'path/to/native-bench page-scan --iterations 20 --database /tmp/existing-large-table-inputs/LargeRows-V2019.accdb --table LargeRows --limit 25 --scan-rows 1000' \
  'path/to/native-bench cursor-scan --iterations 20 --database /tmp/existing-large-table-inputs/LargeRows-V2019.accdb --table LargeRows --limit 25 --scan-rows 1000'
```


A fresh cursor can now be consumed into a complete scalar snapshot through checked
`TableSnapshotLimits`; [larger query materialization](../access/queries/LARGER_NATIVE_QUERIES.md)
retains cumulative controls and refuses advanced/failed cursors. The default
materialization helpers still use 16,384 rows and 256-row batches.
