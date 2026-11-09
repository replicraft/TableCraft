# Original native progressive Sum/Avg evidence

The 2026-10-25 [accepted replay](results/progressive-numeric-2026-10-25.json)
passes 244 controls across original Jet 4, ACE 2007 and ACE 16-family files.
Its 204 successful controls compare 228 output rows and 996 cells with independent
getter/Fraction expectations. Forty failures retain empty stdout: 24 Decimal
result overflows, eight resource refusals, four output refusals and four division
errors. Buffered queries and explicit streams with batch sizes 1 and 128 agree
with the model. This is engineering evidence, without native Microsoft Access
execution or subtype qualification, and makes no performance claim.

## Independent data and numeric model

The replay reuses the four unchanged originals registered in
[Decimal selection evidence](DECIMAL_INDEX_SELECTIONS.md#original-inputs-and-independent-model).
The original MIT [DecimalIndexFixture.java](DecimalIndexFixture.java) public-builder
recipe and reviewed Jackcess 5.0.4 artifact are unchanged. The read-only
[IndexRowsOracle.java](IndexRowsOracle.java) returns actual stored cells with
expression evaluation disabled; no reference SQL or active content executes.
All three 1,000-row tables have 909 non-NULL values and 91 NULLs. The runner also
reopens each actual `DecimalEmpty4` table through the getter and confirms zero
records, rather than inferring emptiness from native declared counts. Its raw
empty stdout has SHA-256
`e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855`.

The [new runner](compare_progressive_numeric.py) uses exact Python `Fraction`
values parsed from getter Decimal strings. It computes population sizes, totals
and means independently of TableCraft results. Every candidate Decimal scale from
28 through zero represents a grid of rational points with a 96-bit coefficient
magnitude and a separate sign. Exact integer quotient/remainder chooses the nearest point, resolving
halfway values to even. The highest-resolution fitting candidate supplies the
expected numeric result; if no candidate fits, the expected result is Overflow.
Mean candidates use the exact total divided by non-NULL count, never the already
rounded Sum result. Empty and all-NULL populations instead produce NULL totals
and means. This models the documented current
[Decimal sum](../query/aggregates/DECIMAL_SUMS.md) and [mean](../query/aggregates/AVERAGES.md) result envelope,
without using its Rust implementation or CLI output as the oracle.

Numeric cells are compared as exact Fractions after TSV decoding. `VarType`
observations separately require Decimal results for non-NULL Decimal aggregates
and Null for empty/all-NULL results. Integral controls require the current Int64
Sum and Double mean of the stored IDs. The replay does not independently qualify
retained trailing-zero scale or native Access result typing. Native adapter tests
separately check source carriers, exact scale-28 residuals and cursor/cache behavior.

The three source scales are 0, 4 and 28. Their shared coefficient total is
`623834205931377921501697583`. Positive-only coefficients total
`588091193000388920235092509551`; negative-only coefficients total
`-587467358794457542313590811968`. The sign totals exceed the 96-bit envelope at
scale zero, making their Sum overflow independently predictable. Separate Avg
queries succeed because their final means fit. At scales 4 and 28, sign totals
exercise bounded final scale reduction. Input precision does not constrain the
aggregate output to the original table's precision/scale.

Each scale has whole-population and sign-filtered controls. Further controls cover
populated all-NULL selections, filtered-empty and actual empty tables, typed
Decimal-point filters, explicit declared arithmetic, prior projection aliases,
mixed Sum/Avg/Count UNION ALL arms and ordinary integral aggregates. Positive
native-row, query-step and output limits of one exercise runtime refusals.
A final-ID operand division control derives its boundary from getter identities.
Each failed operation has a single aggregate result, so no incomplete aggregate
row may escape; completed streaming-prefix behavior has separate Rust coverage.

## Reproduction and immutable evidence

```sh
python3 docs/testing/compare_progressive_numeric.py \
  --cli .cache/tablecraft-bin/progressive-numeric-cli-20261025 \
  --jar .cache/tablecraft-fixtures/jackcess-5.0.4.jar \
  --inputs .cache/tablecraft-fixtures/decimal-seeks-V2000-20261025.mdb \
    .cache/tablecraft-fixtures/decimal-seeks-V2007-20261025.accdb \
    .cache/tablecraft-fixtures/decimal-seeks-V2016-20261025.accdb \
    .cache/tablecraft-fixtures/decimal-seeks-V2019-20261025.accdb \
  --report docs/testing/results/progressive-numeric-2026-10-25.json
```

The runner refuses an existing report, requires the reviewed reader and frozen
CLI hashes, uses repository-relative labels and bounds each subprocess to 60
seconds. It reuses the reviewed getter invocation from
[compare_decimal_seeks.py](compare_decimal_seeks.py), which is pinned separately.
Successful stdout hashes follow full modeled-cell comparisons.

| Artifact | SHA-256 |
| --- | --- |
| Immutable CLI | `c1e7c12722c31c31fbcdf47283b2ef0c92089d698b51f09cc200efda1cbc90c5` |
| New runner | `fd96b5c2eef593337ce08e49e99217138aeb0ce96429e6063c6c78958e255ac8` |
| Accepted report | `4e639ae4d404e30b56b37687f72eeddb8c2d25b946dfe41c7d7eeae0a71b9543` |

Ten immutable executable/reader/tool/original hashes agree before and after replay.
Twenty-six scoped compiled-source hashes are separately read using `git show`
from revision `e837d675137c8df82f296cfe9aab9f7b43ea6bb6`. They cover the
progressive implementation, shared aggregate kernels and relevant host/source
boundaries; they are neither mutable live-source pins nor a full Rust inventory.
The four original byte hashes are unchanged. The terminal replay log is
`.cache/progressive-numeric-replay-final-20261025.log`.

The first exploratory 244-control pass remains
`.cache/progressive-numeric-probe-20261025.json`, outside accepted counts. Before
the final run, the runner gained explicit sign-subset model traces, derived
mixed-arm expectations, a complete scoped entry-point pin and readable wrapping.
Expected outcomes were unchanged. No failed product or oracle probe preceded the
accepted Sum/Avg replay, and no old accepted report was overwritten.

## Focused native checks

Eight new native Sum/Avg tests pass alongside thirteen Count tests and seventeen
progressive-source tests. Four priority profiles exceed snapshot-retention ceiling
one while retaining checked EOF and cumulative native limits. Tests independently
check Int32 and Decimal totals/means, NULL exclusion, actual empty inputs, a tiny
Decimal residual after wider accumulation, selected-corrupt/pruned fields,
no-cache retries, late read/count/eager failures, cancellation, cumulative
byte/row/step budgets and refusal to reuse a previously complete snapshot.

```sh
cargo test --locked -p tablecraft-access \
  --test progressive_numeric_aggregates --test progressive_counts \
  --test progressive_source
cargo clippy --locked -p tablecraft-access --lib \
  --test progressive_numeric_aggregates --test progressive_counts \
  --test progressive_source -- -D warnings
```

Both commands pass, with logs
`.cache/progressive-numeric-native-tests-20261025.log` and
`.cache/progressive-numeric-native-clippy-20261025.log`. Formatting, whitespace
and Python compilation checks also pass. No adapter/engine production change was
needed for the numeric aggregate tests. The full workspace suite was not run for
this slice. The reused 1,000-row CLI corpus fits normal snapshot limits; the
separate native tests establish retention-cap bypass, without a large-file or
benchmark claim.
