# Native library benchmarks

The reports below retain unsuccessful performance cases and noisy samples as
well as improvements; algorithmic bounds and correctness are separate evidence.

Measured 18 October 2026 on the local macOS ARM host, Rust/Cargo 1.99.0 and
Hyperfine 2.0.0, default release profile without target-cpu=native/LTO overrides.
Twenty measured runs follow three warmups per command. Commands run serially;
compilation/tests are completed before timing. This is one host with warm inputs,
not cross-platform qualification, cold-disk performance or a native Access oracle.
Background/thermal state was not controlled. Do not turn these numbers into CI
thresholds or general speedup promises.

## GUID-bearing tuple measurements

Measured 19 October 2026 on the same host/release settings above. Baseline
`bc2f844` includes numeric composites and single GUIDs; current adds mixed/all-GUID
tuples and the independently committed explicit-final-child traversal fix.
Both executables were frozen before timing. Each run has 20 samples/three warmups
per command, with compilation/validation completed before timing. All thirteen
workloads passed preflight counts. [The initial report](results/guid-tuples-2026-10-19.json)
retains all 520 samples, source/input/binary hashes, memory and instruction data.

**A complete 1,000-row GUID/numeric index read measured a 0.723 ms median**,
versus 0.223 ms sequentially on that same current binary. Ten-GUID/500-row reads
measured 0.450 ms indexed versus 0.187 ms sequentially. Full indexed reads validate
trees and fetch each row root individually; these add ordered retrieval/checks,
without composite seeks or a query speedup promise.

The first five-numeric-key control's wall median increased about 57 microseconds/
1,000-row operation. Instruction medians increased only about 1.7%, while several
current wall measurements had large scheduling outliers. This prompted a second
experiment adding `#[inline]` to Component max_bytes/decode/value and CompositeKey
empty/set/get. That [contender report](results/guid-tuples-inlined-2026-10-19.json)
retains another 520 samples and both executables' fingerprints. It did not establish
an inlining benefit, so those hints were removed. Final source fingerprints exactly
match the original tested/measured implementation. No samples were removed.

| Existing operation | Rows | Initial baseline/current medians, microseconds | Follow-up baseline/inline contender medians, microseconds |
| --- | --- | --- | --- |
| Original Int32 index | 38 | 87.417 / 87.809 | 84.553 / 84.566 |
| Generated Int32 index | 1,000 | 549.079 / 585.374 | 543.766 / 538.715 |
| Five numeric keys | 1,000 | 566.782 / 623.313 | 552.830 / 561.193 |
| Ten numeric keys | 500 | 393.924 / 378.689 | 358.534 / 372.275 |
| Single GUID | 1,000 | 426.280 / 420.133 | 419.048 / 438.993 |

Follow-up five-/ten-numeric differences are about 8/14 microseconds per operation.
Numeric tuple instruction medians are approximately 1.7–2.1% above baseline across
the reports; the broader component dispatch has a small measured cost despite
retaining compact numeric tuples. The inlining contender changed these instruction
counts by less than 0.2% relative to the initial implementation. Wall medians vary
across sessions, and neither session has thermal/background controls. These data
do not prove performance parity or a general regression percentage. Full means,
SDs, medians, outliers and hardware counters remain available in both reports.

The new GUID workloads have no before implementation. Their initial current
means ± SD are 802.262 ± 198.035 microseconds for 1,000 mixed rows and
484.148 ± 98.147 microseconds for 500 ten-GUID rows. These public APIs run over a
memory-backed Cursor after loading input once/process, including setup and output
destruction. They do not measure cold disk, native Access or shared-file behavior.
Exact content/order comes from independent cursor comparisons, not timing checksums.
Test debug symbols were disabled to fit local disk; release flags were unchanged.

To reproduce, build/freeze `bc2f844` and current with the existing procedure, then
generate original numeric, [single-GUID](GuidIndexFixture.java) and
[GUID-tuple](GuidCompositeIndexFixture.java) inputs using the reviewed artifact:

```sh
python3 docs/testing/benchmark_scans.py \
  --baseline path/to/frozen-before --baseline-metadata path/to/before.json \
  --current path/to/frozen-current --database docs/examples/Access_Example_VBA.accdb \
  --generated-ace path/to/Index-V2016.accdb --composite-ace path/to/compound-2016.accdb \
  --guid-ace path/to/guid-2016.accdb --guid-composite-ace path/to/guid-tuples-2016.accdb \
  --output target/benchmarks/guid-tuples.json
```

## Composite numeric index follow-up

Measured 19 October 2026 with the same release flags/host above, 20 runs and three
warmups. Baseline `4b0fad1` already includes current-page scan reuse and exact
Currency index/FK support. Its executable was copied before changing source;
both builds used the same lockfile/toolchain. No compilation or independent
comparisons ran during timing. [The complete report](results/composite-indexes-2026-10-19.json)
retains source/input/binary hashes, checksums, every sample and memory statistics.
The runner now fingerprints all native index modules and itself as well.

**The existing 38-row indexed operation measured 82.1 → 82.6 microseconds.**
The generated 1,000-row indexed medians measured 531 → 525 microseconds. These
controls give no evidence of a material single-index regression or a general
speedup. Several cases contain scheduling outliers; all remain in the report.

| Same operation, before/current | Iterations/process | Before, microseconds/operation ± SD | Current, microseconds/operation ± SD |
| --- | --- | --- | --- |
| Catalog, 146 objects | 500 | 59.813 ± 4.303 | 58.723 ± 1.031 |
| Sequential, original, 933 rows | 500 | 622.341 ± 107.191 | 610.873 ± 40.715 |
| Indexed, original, 38 rows | 500 | 82.051 ± 0.757 | 82.593 ± 1.177 |
| Sequential, generated, 1,000 rows | 100 | 273.501 ± 2.903 | 274.774 ± 5.851 |
| Indexed, generated, 1,000 rows | 100 | 570.230 ± 120.424 | 539.761 ± 55.167 |

The 1,000-row indexed mean difference includes a 95.6 ms baseline process outlier
and a 77.0 ms current outlier; their medians differ only about 1.1%. Header code
is unchanged. Its control medians are 8.54/8.17 nanoseconds/parse, with a 31.1 ms
baseline process outlier inflating the mean. Do not interpret that mean as a
header optimization. Process/setup costs are included throughout.

The new composite feature has no before implementation. Additional pairs compare
two complete operations on the **same current binary**, not before/after speedups:

| Full operation on current | Rows | Sequential, microseconds/operation ± SD | Compound index, microseconds/operation ± SD |
| --- | --- | --- | --- |
| Mixed widths/directions, FiveMixed | 1,000 | 212.198 ± 14.003 | 583.126 ± 57.591 |
| Ten components, TenMixed | 500 | 157.470 ± 2.416 | 351.904 ± 10.134 |

The complete compound reads are slower: they validate a tree and fetch each live
root individually, whereas sequential scans reuse their current page. They supply
ordered reads and exact consistency coverage, not composite seeks or an automatic
query speedup. Inputs load once/process and repeated public APIs use a memory-backed
Cursor; these are not disk/syscall, native Access or shared-file measurements.
Checksums/counts agree before timing, while separate independent cursors establish
exact row order/carriers. No sample removal, thermal control or CI threshold applies.

To reproduce, build/freeze `4b0fad1` and current using the build/metadata procedure
below. Generate fresh [compound fixtures](CompositeIndexFixture.java) using the
reviewed Jackcess artifact, finish validation/builds, then run:

```sh
python3 docs/testing/benchmark_scans.py \
  --baseline path/to/frozen-before --baseline-metadata path/to/before.json \
  --current path/to/frozen-current --database docs/examples/Access_Example_VBA.accdb \
  --generated-ace path/to/Index-V2016.accdb --composite-ace path/to/compound-2016.accdb \
  --output target/benchmarks/compound-indexes.json
```

## Reusing the current scan page

Measured 19 October 2026 with the host/profile above, 20 runs and three warmups.
Catalog and sequential table scans now borrow their current page for live roots;
relocation targets retain physical reads and all owner/range/cycle checks. There
is no cross-page/call cache or new allocation. Tests assert actual read counts,
exact relocated results and rejection of malformed targets. Existing exhaustive
I/O-failure, cancellation and budget tests remain passing.

**A complete 1,000-row sequential read measured 0.504 → 0.279 milliseconds.**
The original candidate's 933-row read measured 0.831 → 0.596 milliseconds.
These are memory-backed public API measurements: the executable loads input once,
then repeatedly uses a Cursor, including catalog/schema discovery, decoding and
output destruction. They do not measure disk syscalls, cold reads or shared-file
coordination. The fixture/input/binary/source hashes, every raw sample, memory
statistics and checksums are in [the report](results/scans-2026-10-19.json).

| Operation | Iterations/process | Before, microseconds/operation ± SD | Reused page, microseconds/operation ± SD |
| --- | --- | --- | --- |
| Catalog, 146 objects | 500 | 90.951 ± 1.437 | 59.534 ± 1.626 |
| Sequential, original TAG_NME_TB, 933 rows | 500 | 831.391 ± 17.954 | 595.621 ± 8.357 |
| Indexed, original ROULETTE_TB, 38 rows | 500 | 115.401 ± 1.327 | 85.180 ± 1.108 |
| Sequential, generated V2016 Indexed, 1,000 rows | 100 | 503.897 ± 23.225 | 279.194 ± 6.280 |
| Indexed, generated V2016 Indexed, 1,000 rows | 100 | 536.992 ± 12.118 | 534.624 ± 14.041 |

The catalog/first two sequential means improve 34.5%/28.4%/44.6%, respectively.
Indexed root fetching is unchanged: the original candidate benefits from catalog
discovery, while the generated 1,000-row indexed means differ only 0.4%, within
observed noise. A pure header control differs 1.1% around seven nanoseconds/parse;
this is not evidence of a header optimization. All samples, including outliers,
are retained; setup/process costs are included and no background/thermal controls
or CI thresholds were imposed. Before/current preflight counts agree; correctness
also passed 470 workspace tests, 208 ACE all-feature tests and the independent
12-file ACE16 fixture comparisons. Microsoft Access remains unrun.

To reproduce, build `e1a9cf3` in a separate checkout using
`CARGO_INCREMENTAL=0 cargo build -p tablecraft-ace --bench native --release --locked`.
Copy its compiler-reported `native` executable before rebuilding current with
the same flags, lockfile and toolchain. Record the baseline JSON keys
`source_commit`, `binary_sha256`, `cargo_lock_sha256`, `rustc` and `build`.
Generate `Index-V2016.accdb` with [the original recipe](IndexFixture.java) and the
reviewed Jackcess artifact in [FIXTURES.md](../FIXTURES.md). Finish all compilation
and independent checks before invoking [the runner](benchmark_scans.py):

```sh
python3 docs/testing/benchmark_scans.py \
  --baseline path/to/frozen-before --baseline-metadata path/to/before.json \
  --current path/to/frozen-current --database docs/examples/Access_Example_VBA.accdb \
  --generated-ace path/to/Index-V2016.accdb \
  --output target/benchmarks/scans.json
```

The runner checks baseline binary/toolchain/lockfile identity and before/current
checksums, then records sanitized commands, source/input hashes and every sample.
Both databases remain unchanged; the existing provenance applies.

## Jiff conversion follow-up

The first migration's 65% calendar regression below prompted an adapter audit.
Jiff remains responsible for checked civil dates/times. Decoding now folds a
rounded midnight carry into one date addition and validates the time separately,
avoiding a second date arithmetic operation through DateTime. Encoding counts
bounded integral time-of-day nanoseconds directly instead of converting a Jiff
duration through i128. The two conversion entry points permit cross-crate
inlining. No calendar algorithms, timezone conversion, caches or unchecked
constructors were introduced; exact stored/Serde bits and negative DATE rules
remain unchanged.

The [calendar runner](benchmark_calendar.py) compares the historical time adapter,
08b603b's Jiff adapter and the optimized adapter. All builds precede timing. The
original one-million round trips are retained; five million additionally amortize
process overhead. Separate decode/encode workloads identify both conversion
directions. Twenty runs/three warmups and the host/profile above apply.

| Workload | time mean ± SD | Jiff before mean ± SD | Jiff optimized mean ± SD |
| --- | --- | --- | --- |
| round trip, 1,000,000 | 28.48 ± 6.24 ms | 41.46 ± 1.28 ms | 28.92 ± 1.17 ms |
| round trip, 5,000,000 | 124.66 ± 24.53 ms | 200.33 ± 6.77 ms | 144.19 ± 37.42 ms |
| decode, 5,000,000 | — | 119.74 ± 38.62 ms | 80.97 ± 11.13 ms |
| encode, 5,000,000 | — | 71.20 ± 17.26 ms | 42.66 ± 0.80 ms |

Several cases contain large scheduling outliers. All samples, memory summaries,
checksums and binary hashes remain in the [recorded report](results/calendar-2026-10-18.json).
The five-million round-trip **medians** are 115.98 / 197.52 / 135.73 ms for
time / original Jiff / optimized Jiff: approximately 23.20 / 39.50 / 27.15 ns per
round trip. Optimized Jiff improves about 31% by median and 28% by mean over the
previous adapter. Its remaining median gap to time is about 17%; the similar
one-million time/current means do not establish parity given the noisy time
baseline. These are conversion measurements, not whole-query regressions or
cross-platform guarantees. Stored-date comparison uses day/fraction ordering
without constructing calendars.

The historical workload varies days on both sides of the epoch with fixed
quarter-day fractions, not arbitrary times. Decode consumes every calendar field;
encode selects from 1,024 preconstructed calendar inputs behind black_box. Setup
is outside the repeated encode loop but still inside process timing. Checksums
agree across contenders before timing; they do not replace correctness tests.
Golden cases cover century/leap boundaries, field endpoints, nanosecond ties and
midnight carry. Three fixed-seed properties additionally compare both directions
to the previous general Jiff arithmetic over varied civil times and valid raw
DATE inputs. All 273 workspace tests/doc tests and separate core/expressions
feature checks pass locally. Microsoft Access and other targets remain unrun.

To reproduce, build the time baseline as described under Reproduction below. For
the original Jiff adapter, use a separate checkout/archive of 08b603b, copy the
current native.rs harness into its ACE benches directory (direction workloads
only), and build with its unchanged core implementation. Then build current and
invoke the three already-built executables:

```sh
cargo bench --no-run --locked -p tablecraft-ace --bench native
python3 docs/testing/benchmark_calendar.py \
  --baseline path/to/time-native-benchmark \
  --before path/to/original-jiff-native-benchmark \
  --current path/to/optimized-native-benchmark
```

## Table consistency checks

The native harness's check workload repeats 100 complete sequential/qualified-index
checks with fresh bounded budgets. Input is loaded once per process. Twenty runs
follow three warmups on the host/toolchain above; checksums consume completed row
counts. This measures the additional validation operation, without comparing it
to a weaker read as a speedup claim.

| Input | Mean ± SD for 100 checks | Coverage per check |
| --- | --- | --- |
| registered ROULETTE_TB | 49.66 ± 2.70 ms | 38 rows; 2 checked / 1 unverified index |
| generated ACE Indexed | 213.85 ± 87.94 ms | 1,000 rows; 2 checked / 3 unverified indexes |
| generated Jet 4 Indexed | 167.07 ± 19.97 ms | 1,000 rows; 2 checked / 3 unverified indexes |

The generated ACE case has large scheduling outliers; all samples, memory
summaries and input/binary hashes remain in the [report](results/checks-2026-10-18.json).
These are bounded structural/row checks with [explicit gaps](../NATIVE_CHECKS.md),
not whole-database integrity or Microsoft Access qualification.

```sh
cargo bench --no-run --locked -p tablecraft-ace --bench native
python3 docs/testing/benchmark_checks.py \
  --binary path/to/current-native-benchmark \
  --database docs/examples/Access_Example_VBA.accdb \
  --ace path/to/fresh-indexes.accdb --jet path/to/fresh-indexes.mdb
```

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

The initial Jiff calendar workload was about 65% slower (roughly 24 to 39 ns per repeated
conversion round trip); the [follow-up above](#jiff-conversion-follow-up) records
the adapter optimization and remaining gap. This original regression is retained: civil calendar
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

## Usage-map traversal

The [usage-map recipe](UsageMapFixture.java) creates 2,000 owned data pages, each
with one 13-column row (ID and 12 Binary fields). The public native harness repeats
ten full reads with a fresh 256 MiB budget, comparing e083f6e's search through prior
pages with the current monotonicity check. Baseline and current row-count checksums
agree; [full tagged-row comparison](compare_usage_maps.py) is separate.

| Workload | Baseline mean ± SD | Current mean ± SD |
| --- | --- | --- |
| ACE sequential | 48.04 ± 1.83 ms | 60.76 ± 33.68 ms |
| ACE indexed | 45.52 ± 4.09 ms | 43.85 ± 2.21 ms |
| Jet 4 sequential | 47.85 ± 1.87 ms | 48.04 ± 3.19 ms |
| Jet 4 indexed | 47.09 ± 6.52 ms | 42.65 ± 0.78 ms |

Same host/profile, 20 runs and three warmups apply. The ACE sequential current
case has large scheduling outliers and a worse mean; every sample remains in the
[recorded report](results/usage-maps-2026-10-18.json). These measurements establish
no reliable broad whole-reader improvement. The change is retained for linear
bookkeeping and explicit sorted ownership invariants, not a general speed claim.
Normal CLI input/work limits are unchanged.

```sh
java -cp path/to/jackcess-5.0.4.jar docs/testing/UsageMapFixture.java path/to/new-pages.accdb V2007
java -cp path/to/jackcess-5.0.4.jar docs/testing/UsageMapFixture.java path/to/new-pages.mdb V2000
cargo bench --no-run --locked -p tablecraft-ace --bench native
python3 docs/testing/benchmark_usage_maps.py \
  --baseline path/to/e083f6e-native-benchmark --current path/to/current-native-benchmark \
  --ace path/to/new-pages.accdb --jet path/to/new-pages.mdb
cargo build --release --locked -p tablecraft-ace --example table_data
python3 docs/testing/compare_usage_maps.py \
  --example target/release/examples/table_data --jar path/to/jackcess-5.0.4.jar \
  --ace path/to/new-pages.accdb --jet path/to/new-pages.mdb
```

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

## Chained table definitions

The native harness adds a schema workload, consuming returned column counts.
The baseline executable uses f761d69's native implementation/harness; current
includes bounded declared lengths and continuation chains. Paired ordinary-reader
cases detect regressions; the new wide-schema cases have no supported baseline.

| Workload | Baseline mean ± SD | Current mean ± SD |
| --- | --- | --- |
| catalog, 500 operations | 45.75 ± 1.13 ms | 47.25 ± 1.71 ms |
| sequential, 100 operations | 85.08 ± 2.12 ms | 84.98 ± 1.44 ms |
| indexed, 500 operations | 57.76 ± 0.98 ms | 58.96 ± 1.03 ms |
| ACE wide schema, 1,000 operations | unsupported | 123.97 ± 3.22 ms |
| ACE wide indexed, 500 operations | unsupported | 103.39 ± 2.21 ms |
| Jet 4 wide schema, 1,000 operations | unsupported | 115.73 ± 1.89 ms |
| Jet 4 wide indexed, 500 operations | unsupported | 100.28 ± 2.99 ms |

Ordinary-reader means change approximately +3.3% / -0.1% / +2.1%; no speedup is
claimed. Wide inputs have 200 columns and seven-page definitions, with seven rows
per nonempty table. Same host/profile, 20 runs and three warmups apply.
[Recorded samples](results/definitions-2026-10-18.json) include memory summaries,
binary/input hashes and explicit harness options. Exact metadata/rows are checked
separately in [definition comparisons](compare_definitions.py).

```sh
cargo bench --no-run --locked -p tablecraft-ace --bench native
python3 docs/testing/benchmark_definitions.py \
  --baseline path/to/f761d69-native-benchmark \
  --current path/to/current-native-benchmark \
  --database docs/examples/Access_Example_VBA.accdb \
  --ace path/to/new-wide.accdb --jet path/to/new-wide.mdb
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

## Boolean and IEEE index measurements

The [complete report](results/boolean-floating-indexes-2026-10-19.json) retains all
600 samples from 15 cases, 20 runs and three warmups. The frozen baseline is
`9bfe9fb` before Boolean/IEEE qualification; its binary identity and every original
source fingerprint were verified against that commit. Current source/binary,
toolchain, lockfile and input hashes are recorded. Release defaults remain
unchanged. Tests, fixture generation, Java comparisons and builds finished before
timing. The macOS ARM host's background/thermal state is uncontrolled.

The new operations below are **milliseconds per complete 1,000-row read**, derived
from process medians over 100 repeated public API operations. Each process loads
the file once, then reads memory-backed Cursors; setup, catalog/schema discovery,
allocation and destruction are included. These are neither disk nor native
Microsoft Access timings. Exact ordered IEEE/Boolean comparisons provide separate
correctness evidence; the benchmark preflight checks row-count checksums.

| Current workload | Sequential median ms | Full indexed median ms |
| --- | --- | --- |
| Boolean single key | 0.155 | 0.403 |
| Single IEEE key | 0.159 | 0.415 |
| Double IEEE key | 0.158 | 0.420 |
| Mixed IEEE/Boolean/Currency/GUID | 0.218 | 0.641 |

Complete indexed reads are 2.6–2.9 times slower here: they validate the full tree
and fetch referenced rows, while the sequential path reuses its current page.
These new carriers have no qualified selective seek path. No speed improvement
is claimed merely from enabling index coverage.

Existing controls retain mixed results: the 38-row Int32 median is 0.0834/0.0830 ms
before/current; 1,000 Int32 rows are 0.541/0.552 ms; five numeric components over
1,000 rows are 0.564/0.596 ms; ten numeric components over 500 rows are 0.434/0.400 ms;
single GUID over 1,000 rows is 0.471/0.478 ms. Numeric tuple median instructions
increase about 1.6–2.1%; Int32/GUID instruction counts remain close. Scheduling
outliers and inconsistent wall deltas preclude a general speed/parity claim.
All samples, memory and instruction/cycle counters remain in the report.

The existing runner accepts `--boolean-ace` and `--floating-ace` for the original
inert recipes in [FIXTURES.md](../FIXTURES.md). New full-read cases compare the
same current executable's sequential/indexed APIs; existing controls compare
before/current executables. Source test debug-symbol reductions do not change
this release benchmark profile.

## Legacy DateTime index measurements

The [complete report](results/date-indexes-2026-10-19.json) retains all 320 samples
from eight cases, 20 runs and three warmups. Frozen baseline `c87fa7a` predates the
DATE index policy; binary identity and every recorded source fingerprint were
checked against that commit. Current source/binary, toolchain, lockfile and input
hashes are retained. Generation, comparisons, tests and builds finished before
timing on the same macOS ARM host. Release defaults remain unchanged; background/
thermal state is uncontrolled and scheduling outliers are retained.

These are **milliseconds per complete 1,000-row read**, from process medians over
100 operations on memory-backed Cursors. Full metadata discovery, allocation and
destruction are included. No database disk I/O, selective temporal seek or native
Microsoft Access timing is measured.

| Current workload | Sequential median ms | Full indexed median ms |
| --- | --- | --- |
| Legacy DateTime single key | 0.157 | 0.412 |
| Mixed DateTime/GUID/Currency/Boolean | 0.212 | 0.627 |

Full indexed reads cost about 2.6–3.0 times the sequential read in these fixtures.
The reader validates native serial keys, including internal date bounds, and
fetches referenced rows; it performs no calendar rendering. Existing controls
remain close in this run: 38-row Int32 indexed reads are 0.0841/0.0848 ms before/
current, and 1,000-row Int32 indexed reads 0.534/0.537 ms. These medians are
observations, not a speed/parity guarantee. All instruction/cycle and memory
counters remain in the report. The runner's `--date-ace` option uses the original
DATE recipe; exact serial ordering and residual civil semantics have separate
correctness comparisons in [FIXTURES.md](../FIXTURES.md).

## Single-Decimal index measurements

The [complete report](results/decimal-indexes-2026-10-19.json) retains all 360 samples
from nine cases, 20 runs and three warmups. Frozen baseline `46cc9ab` predates the
single-Decimal policy; binary identity and every recorded source fingerprint were
verified against that commit. Current source/binary, toolchain, lockfile and input
hashes are retained. Generation, tests, Java comparisons and builds finished before
timing. Release defaults are unchanged; macOS ARM background/thermal state is
uncontrolled and no outlier was removed.

These are **milliseconds per complete 1,000-row read**, derived from process
medians over 100 operations on memory-backed Cursors. Metadata discovery,
allocation and destruction are included; no disk I/O, selective Decimal seek or
Microsoft Access comparison is measured. All values retain exact coefficients.

| Precision 28 workload | Sequential median ms | Full indexed median ms |
| --- | --- | --- |
| Scale 0 | 0.229 | 0.486 |
| Scale 4 | 0.229 | 0.487 |
| Scale 28 | 0.233 | 0.483 |

Full indexed reads cost about 2.1 times sequential scans in these fixtures;
they validate the entire tree and fetch referenced rows. Existing Int32 controls
are 0.0833/0.0841 ms before/current for 38 rows and 0.542/0.539 ms for 1,000 rows.
These close observations do not establish a speed/parity guarantee. Samples,
memory and instruction/cycle counters stay in the report. The runner accepts
`--decimal-ace` for the original recipe in [FIXTURES.md](../FIXTURES.md); precision,
scale and exact native key/reference correctness have separate comparisons.

## Indexed root-page reuse

The [complete report](results/indexed-page-reuse-2026-10-19.json) retains all 800
samples from 20 cases, 20 runs and three warmups. Frozen baseline `f17d914` already
supports every measured carrier. Its executable identity and recorded source
fingerprints were checked against that commit. Current source/binary, toolchain,
lockfile and input hashes are retained. Both builds use the same default features
and release profile; tests' reduced debug symbols do not affect them. Builds,
tests and independent comparisons finished before timing. Background/thermal
state is uncontrolled; scheduling outliers remain in the report.

The runner's `--compare-indexed` mode compares before/current full indexed reads
for supplied carrier fixtures. The figures below are **milliseconds per complete
read**, from process medians over 100 operations. Inputs load once per process;
public APIs operate on memory-backed Cursors, including metadata discovery,
allocation and destruction. These are not disk, full-application query or native
Microsoft Access timings. Exact ordered rows have separate comparison evidence;
the timing preflight checks count checksums.

| Indexed workload | Rows/read | Before median ms | Current median ms |
| --- | --- | --- | --- |
| Int32 | 1,000 | 0.538 | 0.361 |
| Five numeric components | 1,000 | 0.554 | 0.512 |
| Ten numeric components | 500 | 0.363 | 0.327 |
| GUID single key | 1,000 | 0.413 | 0.222 |
| GUID/numeric mixed | 1,000 | 0.644 | 0.644 |
| Ten GUID components | 500 | 0.433 | 0.429 |
| Boolean single key | 1,000 | 0.409 | 0.208 |
| Single IEEE key | 1,000 | 0.408 | 0.206 |
| Double IEEE key | 1,000 | 0.412 | 0.208 |
| Legacy DateTime single key | 1,000 | 0.406 | 0.203 |
| Mixed DateTime keys | 1,000 | 0.631 | 0.621 |
| Decimal scale four | 1,000 | 0.476 | 0.284 |

Consecutive same-page references gain most: Boolean/IEEE/DateTime single-key
medians roughly halve, GUID falls about 46%, Int32 33% and scale-four Decimal
40%. Corresponding median instruction counts fall about 26–47% on these scalar
workloads. Scattered mixed GUID/DATE tuples gain little because returning to a
different page still fetches it. There is no uniform improvement claim, automatic
index choice or new multi-page/global cache. Header/catalog/sequential controls,
other Decimal scales, mixed IEEE keys, memory and cycle counters remain in the
report. The 38-row registered Int32 table improves 0.0822/0.0742 ms before/current.

Four focused native tests trace root hits/misses, fresh reads, short-read sources,
empty ranges and relocation targets, including same-page and late physical I/O
failures. Existing exhaustive I/O/cancellation and all byte/constraint tests pass.
Adapter tests now measure required-index reads independently from page reads;
range tests retain exact selected-row resource charging. Independent GUID/IEEE/
DATE/Decimal runners still match 25,696 ordered rows per family on immutable inputs.
Native shared-file isolation and actual Access qualification remain separate.

## Decimal tuple measurements

The [initial report](results/decimal-tuples-2026-10-19.json) retains all 520 samples
from 13 cases, 20 runs and three warmups. Frozen baseline `be53c86` includes indexed
root-page reuse and predates Decimal tuples; binary identity and every recorded
source fingerprint were checked against that commit. Source/binary, toolchain,
lockfile and input hashes are retained. Generation, tests, independent comparisons
and builds finished before timing. Release defaults remain unchanged; the macOS
ARM host's background/thermal state is uncontrolled and all outliers remain.

Figures are **milliseconds per complete read**, from process medians over 100
public API operations on memory-backed Cursors, including metadata discovery,
allocation and destruction. No disk, selective tuple seek, full application or
native Microsoft Access timing is measured.

| Initial current workload | Rows/read | Sequential median ms | Full indexed median ms |
| --- | --- | --- | --- |
| Ten mixed Decimal/numeric/GUID components | 1,000 | 0.454 | 0.929 |
| Ten Decimal components | 500 | 0.258 | 0.650 |

Existing controls expose a regression: five numeric components over 1,000 rows
cost 0.502/0.551 ms before/current and ten over 500 rows 0.346/0.372 ms. Median
instructions rise 3.0%/4.9%, with user time/cycles about 10% higher. Compact key
models remain separate, but adding Decimal to shared component metadata affects
their dispatch. This motivates a separate column-metadata specialization rather
than accepting the wall-time increase as mere scheduling noise. Int32 1,000-row
controls are 0.3582/0.3586 ms and single GUID 0.2210/0.2200 ms. All memory/cycle/
instruction counters remain in the report; no general speed/parity claim follows.

The runner accepts `--decimal-composite-ace` for the original recipe. Exact scales,
typed ordering, complete constraints and immutable inputs have separate
[fixture comparisons](../FIXTURES.md#original-ace-16-decimal-tuple-index-inputs).

## Bounded attachment decoding

The `attachments` harness measures complete `decode_attachment` calls for
60,000-byte synthetic files: direct wrappers and zlib over repeated or
pseudorandom bytes. Each iteration creates resource allowances, allocates the
output/codec, validates the wrapper and drops the result. The pre-timing check
requires exact decoded bytes. These are **microseconds per 60,000-byte payload**,
not nanoseconds and not whole-database/query timings. There is no prior decoder
or Microsoft Access performance baseline.

After compilation/tests, the [runner](benchmark_attachments.py) measures four
commands serially with 20 runs, three warmups and 20,000 payloads/process on the
macOS ARM/Rust 1.99 host. Process durations include startup and fixture setup;
inner-loop samples are also retained. Background/thermal state is uncontrolled.
Absolute means and standard deviations derived from process times:

| Wrapper/workload | Mean microseconds/payload | SD microseconds/payload |
| --- | --- | --- |
| stored-repeated | 1.045 | 0.026 |
| zlib-repeated | 22.197 | 0.168 |
| stored-noise | 1.016 | 0.022 |
| zlib-noise | 166.955 | 63.702 |

The [complete report](results/attachments-2026-10-18.json) retains all measurements,
inner samples, dependency versions, source/binary hashes and host/profile metadata.
The noisy zlib case reflects a different encoded workload from a direct memory
copy; no relative-speed or cross-platform compatibility claim follows. Limits,
cancellation, strict stream completion and native-independent byte comparisons
remain separate correctness evidence.

```sh
CARGO_INCREMENTAL=0 cargo bench -p tablecraft-ace --bench attachments --no-run --locked
# Use the executable path printed by Cargo for this target/build.
python3 docs/testing/benchmark_attachments.py --binary target/release/deps/attachments-HASH \
  --iterations 20000 --runs 20 --warmup 3 --output new-attachment-report.json
```

The runner accepts Hyperfine 2's nested wall-clock JSON summaries and legacy
summaries. The recorded report is a successful serial run.
