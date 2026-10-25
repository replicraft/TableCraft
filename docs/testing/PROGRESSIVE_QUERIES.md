# Original large native query streaming replay

The [accepted replay](results/progressive-queries-2026-10-24.json) passes 88
commands and compares 1,341,336 modeled result cells over four original native
files containing 20,000 ordinary rows each. All 1,898 pinned artifact, input,
getter, tool and source hashes remain unchanged before and after execution,
including every entry in the 1,879-file frozen Rust inventory.

There are 52 successful commands and 36 classified failures: 16 division errors,
16 resource refusals and four output-limit refusals. This verifies the explicit
[streaming query host](../STREAMING_QUERIES.md) against original getter records.
It is engineering correctness evidence, not Microsoft Access qualification,
a benchmark, arbitrary-scale support or shared-file consistency qualification.

## Original producer and read-only oracle

[ProgressiveQueryFixture.java](ProgressiveQueryFixture.java) creates fresh Jet 4
V2000, ACE2007 V2007 and ACE16-family V2016/V2019 databases through public
Jackcess database/table/column builders. The single ordinary `StreamingRows`
table has these declarations:

| Field | Reference type | Original values |
| --- | --- | --- |
| ID | LONG | Integers 1 through 20,000 |
| Score | LONG | ID minus 10,000; NULL every eleventh row |
| Label | TEXT, 64 UTF16 units | NULL every seventh row; otherwise ID-bearing Unicode or escaped text |

Each file has 1,818 NULL scores, 2,857 NULL labels and 206 rows divisible by 97.
Non-NULL labels include accented text, a supplementary crab character, quotation
marks, tabs, newlines and embedded NUL. These are stored values, not code. The
producer disables expression evaluation and authors no saved queries, VBA,
linked targets or external providers.

Getter mode reopens each original read-only, disables expression evaluation,
installs a throwing link resolver and checks that no linked handle was opened.
It captures public column type declarations and all 60,000 cells per file,
using base64 UTF8 for Text and an explicit NULL marker. It executes no reference
SQL. The original hashes are checked immediately around getter capture and again
around the accepted TableCraft replay.

The getter reports ID order 1 through 20,000 for these originals. The runner
compares that observed order; it does not infer sorted SQL results or promise
that natural cursor order sorts arbitrary databases.

## Independent model and parity

[compare_progressive_queries.py](compare_progressive_queries.py) decodes the
getter records and builds expected values independently in Python. The three
successful query shapes are:

- All ID/Score/Label cells, preserving NULL and exact escaped Unicode Text.
- Label then ID projection, filtered by `ID Mod 97 = 0`.
- The filtered IDs with explicit declaration-sensitive `'02' = CVar(CLng(2))`
  results and invariant `CStr(ID) & 'x'` captions under ordinal comparison.

Each shape runs with stream batch sizes 1, 128 and 1,000, then as an ordinary
buffered query with explicitly raised snapshot/read allowances. All four modes
match the complete getter-derived rows. The declared comparison has independent
ASCII string goldens (`"02"` differs from rendered `"2"`), rather than using
TableCraft's comparison output as the expected result. The runner independently
models the existing display escapes and then decodes the csv crate's TSV
transport before comparing cells. A separate filtered-empty command per profile
checks the successful header-only stream.

Successful commands explicitly supply native bytes/rows/steps, query allocation/
rows/steps and output ceilings. Defaults remain unchanged. Native and query pools
are separate; streaming does not refill either pool or require retaining a
complete 20,000-row snapshot. The buffered parity controls deliberately permit
that complete snapshot.

## Failure and prefix publication

The late-error query computes `ID Mod (ID - 15000)`. An independent exact integer
model uses truncation toward zero (`r = a - trunc(a / b) * b`) for preceding
rows. ID 15,000 causes division by zero. On all four profiles, stdout contains
exactly these fully verified completed prefixes:

| Execution mode | Result rows published before failure |
| --- | ---: |
| Stream batch 1 | 14,999 |
| Stream batch 128 | 14,976 |
| Stream batch 1,000 | 14,000 |
| Ordinary buffered query | 0 |

No partial pending output batch escapes. Every command remains a failed
operation; a valid earlier prefix is not treated as complete input.

Positive native-row and query-row allowances of 1,000 each independently produce
a nonempty, fully verified prefix consisting only of complete 128-row output
batches, followed by a classified runtime resource refusal. The observed prefixes
are 768 and 256 rows respectively; those counts record this implementation's
accounting, not a universal per-row charging promise. Positive native-step and
query-allocation allowances of one fail without output. An output allowance of
one fails without publishing a header or data. These are actual runtime resource/
transport errors, not rejected CLI argument values.

The report retains the command, exit status, exact stderr classification,
verified row/cell count, stdout byte length and SHA-256 for every observation.
Large stdout is fully compared before hashing; megabytes of repeated rows are
not committed. All original databases and getter captures remain unchanged.

## Provenance and reproduction

Fresh originals, getter stdout/stderr and generation.json live under ignored
`.cache/tablecraft-fixtures/progressive-query-20261024/`. generation.json records
actual producer/getter/compilation commands, compiled-class hash and unchanged
original hashes. The original Java/Python tools follow the repository MIT
license. No third-party fixture bytes or implementation source were copied.
The reviewed Apache-2.0 Jackcess 5.0.4 JAR has SHA-256
`19a7494a56f439a57fda85c059b019b665802085cec5ae8186d1e028cc1bdfbc`.

```sh
javac -cp .cache/tablecraft-fixtures/jackcess-5.0.4.jar \
  -d .cache/progressive-query-classes-20261024 docs/testing/ProgressiveQueryFixture.java
java -cp .cache/tablecraft-fixtures/jackcess-5.0.4.jar:.cache/progressive-query-classes-20261024 \
  ProgressiveQueryFixture write \
  .cache/tablecraft-fixtures/progressive-query-20261024/V2016.accdb V2016
java -cp .cache/tablecraft-fixtures/jackcess-5.0.4.jar:.cache/progressive-query-classes-20261024 \
  ProgressiveQueryFixture read .cache/tablecraft-fixtures/progressive-query-20261024/V2016.accdb
```

Repeat for V2000/V2007/V2019, using `.mdb` for V2000 and `.accdb` otherwise.
Capture getter stdout/stderr beside each original and retain generation hashes.
Refuse existing output files; newly authored timestamps can change binary hashes.

The immutable accepted executable is
`.cache/tablecraft-bin/progressive-query-cli-20261024`, SHA-256
`771116caa0b27854de4d2737ef9962e7c3d696020a98b309cd2277cab58fe3b0`.
The frozen Rust manifest is
`.cache/progressive-query-rust-source-20261024.manifest`, SHA-256
`2bbac71ab950bc897326a120dcf6cf460a7a51ac6ec2487b5bfc7df2f4751e5c`.
The accepted report has SHA-256
`60c04f1939fb0c084707b92fd11229fb237ffcfb9d5ae7477b19b7c7d7053171`.

```sh
python3 docs/testing/compare_progressive_queries.py \
  --cli .cache/tablecraft-bin/progressive-query-cli-20261024 \
  --cli-sha256 771116caa0b27854de4d2737ef9962e7c3d696020a98b309cd2277cab58fe3b0 \
  --jar .cache/tablecraft-fixtures/jackcess-5.0.4.jar \
  --directory .cache/tablecraft-fixtures/progressive-query-20261024 \
  --source-manifest .cache/progressive-query-rust-source-20261024.manifest \
  --manifest-sha256 2bbac71ab950bc897326a120dcf6cf460a7a51ac6ec2487b5bfc7df2f4751e5c \
  --report .cache/progressive-query-replay.json
```

The runner refuses to overwrite an existing report and verifies the frozen
source inventory against live files before and after replay. A later checkout
must restore the matching production revision before reproducing these source
checks; a newer binary or moving source tree is not silently substituted. The
accepted raw completion log is `.cache/progressive-query-final-20261024.log`.
Java compilation, Python syntax and manual line-width/spacing checks passed.
