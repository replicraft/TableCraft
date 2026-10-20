# Bounded allocation-map paging

The sequential row-page path validates complete usage-map metadata, then yields
page addresses incrementally rather than building a u32 vector. It retains the
bounded usage row and at most one 4 KiB bitmap page. Inline and indirect maps
preserve sorted unique addresses, pointer/signature checks, out-of-file rejection,
resource charging and cancellation. The existing word bitmap kernel is reused.

All indirect bitmap metadata is read and checked before any selected data page
is exposed. The first nonempty bitmap is cached. Subsequent bitmap pages are
reread only if address iteration reaches them and are revalidated before use.
This is incremental address enumeration, not permission to skip later map
corruption or a guarantee of lazy metadata I/O. Stable/offline source ownership
remains mandatory; it is not concurrent-file consistency.

Catalogs, the original complete table read APIs, index ownership and write consumers
retain their existing materialized lists and 4,096-address ceiling. Sequential
NativeSource query loads now use [batched snapshot materialization](BATCHED_QUERY_SNAPSHOTS.md)
under cumulative caller controls. Row paging
retains its 4,096 physical-read ceiling, page-size and caller byte/row/work limits.
Each visited root still consumes traversal work; repeated offset requests rescan
prior roots. A 5,000-row offset can still fail under these bounds. Resumable
cursors and explicit batched snapshots now have their own implementation/evidence;
streaming query execution remains separate work.

## Independent original large-table inputs

[LargeTableFixture.java](LargeTableFixture.java) creates six uncompressed Short
Text fields of 255 UTF-16 units plus a Long ID, and inserts 6,000 original rows.
Each fixture has 6,000 owned pages, confirmed through Jackcess's usage-map getter.
The original recipe uses the existing reviewed Jackcess 5.0.4 artifact; no upstream
implementation or fixture was copied. Binary files remain local/untracked.

[LargeTableOracle.java](LargeTableOracle.java) opens read-only, disables expression
evaluation and returns bounded stored getters. [The comparator](compare_large_tables.py)
checks windows at offsets 0, 1, 500 and 1,000; declared row counts; and explicit
large-query/deep-offset limit refusals. Seven controls per Jet 4/ACE 2007/2016/2019
input passed, with unchanged hashes. These are independent storage comparisons,
not observed Microsoft Access SQL, live-file or complete large-table qualification.

```sh
python3 docs/testing/compare_large_tables.py --jar path/to/jackcess-5.0.4.jar --cli target/release/tablecraft --directory /tmp/new-large-table-inputs --generate
```

[Fixture records](../FIXTURES.md#large-owned-map-paging-inputs) retain recipe/license
provenance, sizes and hashes. Synthetic tests additionally exercise dense maps,
all four profiles, corruption in later bitmap metadata before first data reads,
revalidated rereads, empty/extreme bases, malformed map roots, bit-model properties,
cancellation and resource limits. Full reads and queries still refuse their
unexpanded profiles; a failure never returns a successful partial page.

## Page-read measurements

Darwin arm64, Rust 1.99.0, release defaults; Hyperfine two warmups and seven measured
runs, 5,000 public read_table_page calls per process, two returned rows each:

| Workload | Batch mean ± sample deviation | Amortized time per page |
| --- | --- | --- |
| Previous implementation, example small table | 287.454 ± 7.203 ms | 57.49 µs |
| Address cursor, same example table | 287.554 ± 5.061 ms | 57.51 µs |
| Address cursor, 6,000-page ACE 2019 table | 164.402 ± 7.335 ms | 32.88 µs |

The small-table difference is much smaller than observed run-to-run variation;
these measurements establish no speedup. The large and small tables are different
workloads and cannot be compared as interchangeable performance cases. The prior
implementation refuses the 6,000-page fixture with StructureLimit before a first
page; it has no successful comparable large-table timing.

The native benchmark uses Cursor over a whole input loaded once per process,
including startup/input loading/setup and final checksum in elapsed time. The
large process therefore retains its roughly 25 MB input by harness design; its
RSS does not measure the engine's streaming file-reader memory. No own Cargo build
or comparison runner overlapped measured runs. No new dependency was introduced.

The harness now has a page workload, --offset and --limit. Build it through
cargo bench --no-run and use the executable Cargo reports:

```sh
CARGO_INCREMENTAL=0 cargo bench -p tablecraft-ace --bench native --no-run --locked --offline
# Substitute the reported executable for path/to/native-bench.
hyperfine --warmup 2 --runs 7 \
  'path/to/native-bench page --iterations 5000 --database docs/examples/Access_Example_VBA.accdb --table ROULETTE_TB --limit 2' \
  'path/to/native-bench page --iterations 5000 --database /tmp/new-large-table-inputs/LargeRows-V2019.accdb --table LargeRows --limit 2'
```

The baseline was a temporary export of commit f548d0d, compiled with the same
current benchmark harness copied into it to expose the already existing page
API. Production source was unchanged in that export; only the harness differed
from the recorded commit. Baseline and current used the same compiler/release
settings, source fixture and --iterations/--limit. The main checkout and branch
were preserved. Benchmarks do not qualify native Access behavior.
