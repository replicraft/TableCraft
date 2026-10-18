# Native library benchmarks

Measured 18 October 2026 on the local macOS ARM host, Rust/Cargo 1.99.0 and
Hyperfine 2.0.0, default release profile without target-cpu=native/LTO overrides.
Twenty measured runs follow three warmups per command. Commands run serially;
compilation/tests are completed before timing. This is one host with warm inputs,
not cross-platform qualification, cold-disk performance or a native Access oracle.
Background/thermal state was not controlled. Do not turn these numbers into CI
thresholds or general speedup promises.

## Workloads and evidence

[Native API harness](../../crates/engines/tablecraft-ace/benches/native.rs) repeats
header parsing, catalog inspection, complete sequential/indexed snapshots and
Automation calendar conversion through the public API. Each native operation gets
a fresh bounded budget/cursor; the explicitly selected stable input is loaded once
per process, outside the repeated operations. Every result is consumed with
black_box. Workload checksums compare row/object counts or calendar bits, not every
cell; correctness suites and independent tagged-row comparisons remain separate.

[Bitmap harness](../../crates/engines/tablecraft-ace/benches/bitmaps.rs) compares the
original bit-loop reduction, the actual production word iterator and the retained
SIMD candidate on 4,096-byte zero/sparse/mixed/dense inputs. Checksums include every
selected bit position, and all densities must agree before any measurements run.
The compiler may auto-vectorize the bit-loop reduction; "scalar" names source
structure, not a guaranteed instruction set. These kernels omit parser callbacks,
budget/cancellation and page I/O, so their ratios cannot be applied to queries.

[Recorded samples/statistics](results/native-libraries-2026-10-18.json) retain wall
samples, mean/SD/range, resident-memory summaries, binary/input SHA-256 and tool
versions without local paths. Raw Hyperfine JSON remains under ignored
target/benchmarks. The registered fixture is unchanged. Generated Jet 4/ACE inputs
come from the original [index recipe](IndexFixture.java), not vendored databases.

## Results

| Workload | Baseline mean ± SD | Current mean ± SD | Change in mean |
| --- | --- | --- | --- |
| header (5,000,000 iterations) | 27.48 ± 1.06 ms | 28.74 ± 0.51 ms | +4.6% |
| calendar (1,000,000 iterations) | 23.85 ± 0.96 ms | 39.30 ± 1.24 ms | +64.8% |
| catalog (500 iterations) | 51.50 ± 1.94 ms | 44.66 ± 1.53 ms | -13.3% |
| sequential (100 iterations) | 107.17 ± 2.41 ms | 83.85 ± 2.03 ms | -21.8% |
| indexed (500 iterations) | 66.79 ± 3.57 ms | 58.02 ± 1.72 ms | -13.1% |
| indexed-ace (100 iterations) | 61.87 ± 3.29 ms | 52.13 ± 1.67 ms | -15.7% |
| indexed-jet (100 iterations) | 60.77 ± 1.85 ms | 51.40 ± 1.74 ms | -15.4% |

Native catalog/table workloads improved about 13–22% in this run. These are combined
changes: typed headers, bitmap traversal and ASCII decoding. No isolated speedup
is attributed to zerocopy or bytemuck. Native ASCII text avoids an intermediate
UTF-16 allocation; non-ASCII/compressed mode-switch paths retain strict decoding.

The Jiff calendar workload is about 65% slower (roughly 24 to 39 ns per repeated
conversion round trip). This regression is retained and reported: civil calendar
APIs and future calendar arithmetic are the selection rationale, not a claimed
date-conversion speedup. Exact stored bits, negative Automation DATE rules and the
full Access year envelope remain checked. The unchanged header workload varies by
about 5%, illustrating process/scheduling noise and non-improvement cases.

Bitmap results vary by density: SIMD wins this synthetic zero/sparse reduction;
the compiler-optimized bit-loop reduction wins mixed/dense reductions. Earlier
direct word loops also differed materially from iterator-shaped code. A separate
whole-catalog word/SIMD comparison measured 43.3 ± 0.3 ms versus 43.9 ± 2.0 ms:
no reliable whole-reader SIMD advantage was established. Keep the simpler word
iterator with eight-byte work/cancellation blocks; use wide/bytemuck for the
qualified text classifier. No density-based dispatch or numeric batch executor is
introduced merely to win a synthetic benchmark.

## Reproduction

Build current workloads before timing:

```sh
cargo build --release -p tablecraft-ace --benches --locked
```

Cargo prints benchmark executable paths when using cargo bench --no-run; use the
native and bitmaps executables under target/release/deps. These are separate from
the CLI binary. The harness uses clap Parser derives; --help documents its bounded
iteration counts and explicit input/table/index arguments.

For the baseline, use a separate checkout/archive of 59ae245. Copy only
benches/native.rs from the library milestone bd098cd into its ACE package
(later versions add seek APIs absent from the baseline), add clap.workspace = true to that package's
development dependencies and add the native harness=false bench declaration. Build
that benchmark in the baseline checkout; its public workload source works with
both time and Jiff calendar APIs. This adds only harness/dev wiring to the baseline,
not the new native/date implementations. Keep its executable separate from the
current build. Resolve the local development edge once, then retain its lockfile.

Run the original Java index recipe for optional fresh V2000/V2007 inputs as
documented in [NATIVE_INDEXES.md](../NATIVE_INDEXES.md). Never overwrite a registered
fixture or run application content. Then invoke the comparison runner:

```sh
python3 docs/testing/benchmark_native.py \
  --baseline path/to/baseline-native-executable \
  --current path/to/current-native-executable \
  --bitmaps path/to/current-bitmaps-executable \
  --database docs/examples/Access_Example_VBA.accdb \
  --generated-ace path/to/fresh-indexes.accdb \
  --generated-jet path/to/fresh-indexes.mdb \
  --output target/benchmarks/native-libraries.json
```

The runner requires Hyperfine 2's JSON schema and uses shell-free invocations with
quoted arguments. It validates checksums before timing, refuses failed commands
and saves each completed case. Kernel disagreements invalidate the run; do not
keep timings from an incorrect contender. Review results rather than automatically
accepting a dependency because its benchmark completed.

## Native index seeks

The native harness now compares complete indexed snapshots plus numeric filtering
with the range API. Both paths repeat 500 fresh budgeted reads and consume matching
selected-row counts before timing. Inputs are the original generated 1,000-row
Jet 4/ACE index fixtures. PrimaryKey selects ID 0 through 9 (ten rows); CategoryDesc
selects Category = 0 (101 rows). This measures the native reader, not query planning
or CLI startup. It omits validation of skipped pages on the seek path, as described
in [native index limits](../NATIVE_INDEXES.md#native-int32-range-selections).

| Input / index | Full snapshot + filter | Seek | Ratio of means |
| --- | --- | --- | --- |
| ACE / PrimaryKey | 258.87 ± 13.48 ms | 23.53 ± 0.30 ms | 11.0× |
| ACE / CategoryDesc | 272.63 ± 24.94 ms | 46.22 ± 1.40 ms | 5.9× |
| Jet 4 / PrimaryKey | 253.05 ± 7.55 ms | 19.66 ± 0.30 ms | 12.9× |
| Jet 4 / CategoryDesc | 275.38 ± 34.88 ms | 47.53 ± 16.96 ms | 5.8× |

These are 20-run means ± SD after three warmups on the same host/toolchain above.
The Jet descending case includes a large scheduling outlier, retained in the
[complete samples and memory summaries](results/index-seeks-2026-10-18.json).
These numbers do not predict other selectivities or cold-disk performance.

```sh
cargo build --release --locked -p tablecraft-ace --benches
python3 docs/testing/benchmark_index_seeks.py \
  --binary path/to/native-benchmark-executable \
  --ace path/to/fresh-indexes.accdb --jet path/to/fresh-indexes.mdb
```

The runner preflights count checksums and saves all four cases. Exact row/value/order
correctness is covered separately by the independent range comparison and tests.

## Query filter pushdown

The [Access adapter harness](../../crates/storage/tablecraft-access/benches/query.rs)
compares the same current source, preparation, execution and sorting path, with
optional source constraints accepted versus declined by a thin Source wrapper.
SQL parses once per process; each of 500 iterations constructs a fresh source,
inventories its catalog, binds schema, executes and sorts ID/Category output.
Stable fixture bytes are loaded once per process. Preflight checks compare row
counts, ID sums and NULL counts before timing; exact rows have separate tests and
[independent CLI comparisons](compare_query_filters.py).

| Input / predicate | Constraint declined | Constraint accepted | Ratio of means |
| --- | --- | --- | --- |
| ACE / ID 0 through 9 | 399.32 ± 69.07 ms | 61.97 ± 2.36 ms | 6.4× |
| ACE / Category = 0 | 376.50 ± 3.81 ms | 107.98 ± 4.76 ms | 3.5× |
| ACE / Category IS NULL | 367.84 ± 3.90 ms | 98.84 ± 2.44 ms | 3.7× |
| Jet 4 / ID 0 through 9 | 383.63 ± 65.48 ms | 45.51 ± 0.88 ms | 8.4× |
| Jet 4 / Category = 0 | 367.00 ± 9.42 ms | 91.81 ± 3.04 ms | 4.0× |
| Jet 4 / Category IS NULL | 354.36 ± 4.57 ms | 82.71 ± 2.71 ms | 4.3× |

Twenty-run means ± SD follow three warmups, using the same host/toolchain/profile
above. Range/equality/NULL select 10/101/91 rows respectively. Both formats' range
full reads include substantial scheduling outliers; all samples and
memory summaries remain in the [report](results/query-pushdown-2026-10-18.json).
This includes metadata discovery and query work, so native API-only seek ratios
do not apply. It measures fresh sources, not repeated cached complete tables or
CLI process startup; no improvement is promised for broad filters or other hosts.

```sh
cargo bench --no-run --locked -p tablecraft-access --bench query
python3 docs/testing/benchmark_query.py \
  --binary path/to/query-benchmark-executable \
  --ace path/to/fresh-indexes.accdb --jet path/to/fresh-indexes.mdb
```
