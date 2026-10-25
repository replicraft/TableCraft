# Native progressive Count(*) evidence

The 2026-10-25 [accepted replay](results/progressive-counts-2026-10-25.json)
passes 132 CLI controls across original Jet 4, ACE 2007 and ACE 16-family files.
Its 108 successful controls compare 132 output rows against independent getter
models. Twenty-four runtime failures preserve empty stdout: eight native/query
resource refusals, four output refusals and twelve division errors. These are
engineering comparisons, without native Microsoft Access execution qualification.
No benchmark or full-backend compatibility claim follows from these results.

## Original inputs and model

This replay reuses the four unchanged originals registered in
[Decimal index evidence](DECIMAL_INDEX_SELECTIONS.md#original-inputs-and-independent-model)
and [fixture provenance](../FIXTURES.md#original-exact-decimal-equality-selection-replay-inputs).
No new database files or third-party fixtures were introduced. The original MIT
[DecimalIndexFixture.java](DecimalIndexFixture.java) uses public Jackcess builders;
[IndexRowsOracle.java](IndexRowsOracle.java) reopens files read-only and returns
stored cells through public index cursors with expression evaluation disabled.
Neither tool executes reference SQL or active content.

The [runner](compare_progressive_counts.py) reuses the reviewed getter invocation
from [compare_decimal_seeks.py](compare_decimal_seeks.py). It counts actual getter
records and NULLs, and uses exact Python `Fraction` values for sign and typed-point
predicates. Native declared row counts and TableCraft query results are not used
as its expected-value oracle. All four Decimal4 getter outputs have SHA-256
`ede9a228e39462288251fa9205415eae6b3a301221fcbc69712e5a5bc903f1f9`.
Each independently yields:

| Predicate | Count |
| --- | ---: |
| All records | 1,000 |
| Value IS NULL | 91 |
| Value IS NOT NULL | 909 |
| Value > 0 | 457 |
| Value < 0 | 395 |
| Value = typed Decimal 0.0001 | 57 |
| False | 0 |

Each predicate runs buffered and streamed with batch sizes 1 and 128. Additional
controls compare prior-written Count aliases and mixed Count/row/empty-Count
UNION ALL arms in written order. Failure controls use positive native-row,
query-step and output limits of one, eager false-conjunction division, a filter
that fails at the maximum getter identity, and division by an empty Count result.
These are runtime checks, rather than argument-parser refusals. Every failed
operation is a single Count result whose incomplete aggregate must stay unpublished;
prior completed stream prefixes are covered by the focused Rust tests.

The existing 1,000-row originals fit the normal snapshot retention ceiling, so
this CLI replay establishes parity and failures, not a larger-than-default scale
claim. Native integration tests separately use retention ceiling one with four
records to prove progressive Count bypasses retention while retaining cumulative
native limits. No new large corpus or timings were needed.

## Reproduction and immutable records

```sh
python3 docs/testing/compare_progressive_counts.py \
  --cli .cache/tablecraft-bin/progressive-count-cli-20261025 \
  --jar .cache/tablecraft-fixtures/jackcess-5.0.4.jar \
  --inputs .cache/tablecraft-fixtures/decimal-seeks-V2000-20261025.mdb \
    .cache/tablecraft-fixtures/decimal-seeks-V2007-20261025.accdb \
    .cache/tablecraft-fixtures/decimal-seeks-V2016-20261025.accdb \
    .cache/tablecraft-fixtures/decimal-seeks-V2019-20261025.accdb \
  --report docs/testing/results/progressive-counts-2026-10-25.json
```

The runner refuses an existing report, validates the reviewed Jackcess artifact
and frozen executable hashes before use, and gives each subprocess a 60-second
ceiling. The executable SHA-256 is
`b51b570ab768359f734a6a3f37e88deb8d1b8f693d99e5bd83fa209a3402d0d4`.
The runner SHA-256 is
`5682f2a46d98eefc2cc56173601d77884993c0ad68289eb61cc9983ab41e69c8`.
The accepted report SHA-256 is
`cee09e0773973b3c066723f110a93aba70ee0d20052503aaea18429d261d1faf`.
Its terminal log is `.cache/progressive-count-replay-final-20261025.log`.

Ten immutable executable/reader/tool/original pins agree before and after the
replay. Eleven additional production-path hashes are read from committed revision
`905b885a` using `git show revision:path`, separately from immutable live pins.
They describe a scoped compiled-source inventory, not the mutable current tree or
a complete Rust manifest. Concurrent later work cannot invalidate this replay by
changing unrelated live source. Successful stdout hashes follow complete output
comparisons; the report retains error classifications and exact stderr.

The initial excluded probe mistakenly used ID 1000 as a division boundary, while
the original producer stores IDs 0 through 999. It successfully returned a count,
exposing the oracle setup error. The final runner derives the maximum identity
from getter records. The failed log remains
`.cache/progressive-count-replay-probe-20261025.log`; the corrected exploratory
132-control pass remains `.cache/progressive-count-replay-probe2-20261025.json`.
Neither contributes to accepted counts.

## Focused native integration checks

The new `progressive_counts` target passes seven tests alongside all 17 existing
`progressive_source` tests. Coverage includes four-profile snapshot-cap bypass,
checked EOF, zero-width pruning of invalid unselected Decimal values, selected
invalid-carrier refusal, late page/count/I/O/filter failures, no activation of a
later UNION arm after failure, cumulative byte/row/step budgets, cancellation,
sink failures, retries and complete-cache isolation. The visitor can borrow the
native budget after aggregate publication, proving the cursor releases that borrow.
No adapter production change was needed.

```sh
cargo test --locked -p tablecraft-access \
  --test progressive_counts --test progressive_source
cargo clippy --locked -p tablecraft-access --lib \
  --test progressive_counts --test progressive_source -- -D warnings
```

Both commands completed successfully. Logs are
`.cache/progressive-count-native-tests-final-20261025.log` and
`.cache/progressive-count-native-clippy-final-20261025.log`.
Formatting, Python compilation and whitespace checks pass. The initial new test
expected Source instead of the existing Unsupported classification for an invalid
Decimal sign; correcting that golden left runtime behavior unchanged. Initial
Clippy requested a type alias for a test helper return tuple; final Clippy passes.
The full workspace suite was not run for this slice.
