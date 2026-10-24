# Native index column collections

The recommended native index metadata candidate keeps its at-most-ten key
columns in private `ArrayVec<IndexColumn, INDEX_KEY_COLUMNS>` storage. `NativeIndex::columns()` still
returns `&[IndexColumn]`; physical flags, identities, ordering and qualification
rules remain separate. The parser uses fallible insertion, retains malformed
definition refusals and charges the complete inline `NativeIndex` vector before
construction. It does not separately charge a heap column buffer that does not
exist. No format ceiling, write qualification or public collection type changes.

This is a bounded storage/allocation improvement. It is **not a general query
speedup claim**. Inspection of one-index tables remains a timing tie in this
experiment. All measured differences below are microseconds per inspection or
nanoseconds per isolated collection construction, not seconds of user waiting.

## Contenders and retained memory

The original parser reserved ten heap slots for every logical index. Isolated
copies of the same engine compare that implementation against fixed ArrayVec
storage and SmallVec with two inline slots. SmallVec reserves the observed prefix
capacity before checked parsing, charging a spill before allocation; the normal
ten-slot parser still validates all remaining slots, holes, duplicates and flags.

Observed sizes on Darwin arm64, Rust 1.99.0:

| Storage | Collection object | Whole NativeIndex | Column heap, 1/2/10 columns | Whole index plus column capacity |
| --- | ---: | ---: | ---: | ---: |
| Original Vec, capacity ten | 24 B | 88 B | 160/160/160 B | 248 B |
| ArrayVec, capacity ten | 168 B | 232 B | 0/0/0 B | 232 B |
| SmallVec, two inline slots | 48 B | 112 B | 0/0/160 B | 112/112/272 B |

These totals exclude unchanged names and other shared inspection structures.
ArrayVec removes one column-buffer heap allocation and retains 16 fewer bytes
per index than the former ten-slot Vec reservation on this target. Its enclosing
object is larger, so moving it can cost more; inline storage is not free stack
space. SmallVec has attractive short-key memory use, but adds a spill path and
costs more than the original representation at ten columns. It remains a
benchmark-only dependency here.

Constructor allocation counts are derived from Vec capacity and SmallVec spill
state, not observed global malloc totals. No unsafe counting allocator or extra
instrumentation dependency was added. Constructor timings include this small
capacity/spill bookkeeping; full native inspection timings do not.

## Independent correctness and refusal checks

The original [fixture recipe](IndexColumnsFixture.java) creates fresh inert ACE
16 and Jet 4 databases. Each contains one-, two- and ten-column index tables with
one or 24 logical indexes, rotated component order and alternating directions.
The 24-index case deliberately stresses metadata allocation; it is not a survey
of typical user databases. Existing [Jackcess getters](IndexOracle.java) supply
independent logical/physical IDs, names, root pages, kinds, raw flags and key
columns. No reference SQL or active database content executes.

[Differential verification](verify_index_column_variants.py) passes 2,094
component/inspection/constructor controls over all three implementations and both
formats. Complete returned descriptors, including usage-map references, agree
between variants. Every input and executable hash is unchanged across replay and
timing. The existing `indexes` and `composite_indexes` integration suites pass all
32 tests for each isolated variant, covering malformed definitions, ten-column
keys, corruption, resource limits, cancellation, I/O faults and mutation controls.
Those 96 passes are separate from production-tree checks after adoption.

## Measurements and limits

The [initial raw run](results/index-columns-2026-10-24.json) retains 405 samples
(27 commands, 15 runs, three warmups). A parent `cargo fmt --all -- --check`
finished around the start of that batch and may have affected initial samples;
the [sidecar](results/index-columns-2026-10-24.metadata.json) records that event.
The [reverse-order confirmation](results/index-columns-reverse-2026-10-24.json)
retains another 270 samples, with its own
[sidecar](results/index-columns-reverse-2026-10-24.metadata.json). No samples were
discarded. All executables use ordinary release defaults and owned in-memory
input bytes; each timed inspection repeats the public `inspect_table_indexes`
operation 1,000 times, including normal budgets and result destruction.

Initial whole-process means ± standard deviation, divided by 1,000 inspections,
in **microseconds per inspection**. Process setup and initial file/metadata reads
are included; these are amortized batch costs:

| Key columns / logical indexes | Original Vec | ArrayVec | SmallVec |
| --- | ---: | ---: | ---: |
| 1 / 1 | 19.14 ± 0.32 | 19.27 ± 0.38 | 19.41 ± 0.31 |
| 2 / 1 | 19.26 ± 0.31 | 19.49 ± 0.44 | 19.19 ± 0.29 |
| 10 / 1 | 19.38 ± 0.40 | 19.34 ± 0.37 | 22.87 ± 6.58 |
| 1 / 24 | 23.96 ± 0.59 | 22.34 ± 0.39 | 24.35 ± 4.15 |
| 2 / 24 | 24.08 ± 0.67 | 23.51 ± 0.76 | 23.23 ± 0.53 |
| 10 / 24 | 24.84 ± 0.40 | 23.76 ± 0.45 | 25.71 ± 0.64 |

The reversed ten-column/24-index case again measured about 24.8 versus 23.6
microseconds for Vec/ArrayVec. Other confirmation cases have large unexplained
outliers, including a 205 ms Vec batch and 46–52 ms ArrayVec batches. They do not
support a broad latency improvement claim. Retained samples and original means
remain available rather than selecting the favorable subset.

Amortizing each whole 250,000-construction process measured approximately 35 ns
for Vec, 9 ns for ArrayVec and 15 ns for SmallVec with one-column inputs. These
figures include process setup, input/metadata reads and accounting; they are not
pure kernel latencies. They do not imply the same ratio for native inspection or
SQL execution. The adoption recommendation rests on the actual fixed format bound, one fewer heap allocation, smaller combined
retained storage and passing correctness checks. No query speed, RSS reduction,
other-target layout or native Microsoft Access qualification is inferred.

Only binary/input hashes were captured before and after timing. Exact isolated
source, recipe, toolchain and reviewed Jackcess JAR hashes were added to the
sidecars **after the runs** and are explicitly labelled as post-run observations.
No before-run source fingerprint is claimed.

## Reproduction

Keep original fixtures; generation refuses to overwrite an existing database.
Compile the original Java recipe and existing IndexOracle against the reviewed
Jackcess 5.0.4 JAR. Generate V2016 and V2000 sources and save read-only IndexOracle
observations as `V2016.full-oracle.tsv` and `V2000.full-oracle.tsv`. Hash and record
new originals rather than assigning them the hashes from another generation.

Run [the isolated-workspace recipe](prepare_index_column_variants.py) with a fresh
ignored directory. It recognizes the qualified Vec or current ArrayVec source,
normalizes a Vec counterfactual only inside each copy, then derives the two inline
contenders. It preserves current source files and references existing local
dependencies/docs for builds and fixture reads. It does not compile or run code.

Build each copy's `index_columns` release bench, retain its executable separately
as `index-columns-{vec,arrayvec,smallvec}`, then run the verifier. The benchmark
directory contains those executables and a `fixtures/` subdirectory with the two
databases and getter observations. Description output deliberately requires
simple ASCII fixture index names; it is not a generic metadata export protocol.

The [timing runner](benchmark_index_columns.py) requires a fresh output path and
never builds. Coordinate a quiet window first. `--inspection-only --reverse-order`
repeats public inspections in the opposite contender order without constructor
timings. Retain noisy, tied and contrary results alongside favorable ones.
