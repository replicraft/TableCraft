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

EOF means all live roots were enumerated and their count matched the declaration.
A full final batch may require another empty fetch to establish EOF. Deleted roots
do not consume row positions; selected relocations and long values use the checked
native readers. Unsupported columns fail during open, including on empty tables.
Unvisited payloads remain unchecked until fetched; this is not a whole-file audit.

Invalid batch sizes leave a ready cursor unchanged. Any later fetch error makes it
terminal: no partial batch is returned, `position()` counts only successful batches,
no budget is refunded, and subsequent fetches return `CursorFailed` without I/O or
calling cancellation. This prevents retrying a partially advanced traversal and
silently losing records. Previously returned batches remain valid.

The ordinary CLI `rows` command still uses independent offset requests and its
16 MiB cumulative/1 MiB buffered-output limits. Query execution still materializes
bounded snapshots. Neither path silently switches to streaming or cursor controls.

## Evidence and reproduction

Synthetic tests cover cached-page continuation, exact-once physical data-page reads,
empty/exact/full/partial batches, invalid limits, late count mismatches, deleted
roots/relocations, cancellation, terminal failures, cumulative row budgets and
independent generated row sequences. All four read profiles traverse 6,000-page
inline and indirect maps with explicitly fresh per-fetch budgets; corrupt later
map metadata prevents opening before user data reads.

The unchanged original [large-map fixtures](ALLOCATION_MAP_PAGING.md) independently
match all 6,000 getter rows for V2000, V2007, V2016 and V2019 at batch sizes 1, 127
and 250. Each input's SHA-256 is checked before and after. The reviewed Jackcess
5.0.4 oracle opens read-only with expression evaluation disabled. No new fixture
bytes, library dependency or native Access qualification is implied.

```sh
CARGO_INCREMENTAL=0 cargo build -p tablecraft-ace --example table_data --release --locked --offline
python3 docs/testing/compare_table_cursors.py \
  --jar /tmp/reviewed/jackcess-5.0.4.jar \
  --rows target/release/examples/table_data \
  --directory /tmp/existing-large-table-inputs
```

The engineering example's `--batch-size` explicitly supplies fresh controls per
fetch and writes tagged rows progressively. Its output is a comparison tool, not
the CLI's atomic display protocol; late failures may follow earlier emitted rows.
It conflicts with index/calculation/collection selections.

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
