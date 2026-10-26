# Declared numeric/String independent replay

The [128-command report](results/declared-numeric-text-replay-2026-10-25.json)
passes 96 successful queries comparing 2,592 Boolean/NULL cells and 32 expected
buffered overflow failures. Four registered original native inputs and four
artifact/source pins remain unchanged. These controls exercise the explicit
[declared predicate rule](../expressions/numeric/DECLARED_NUMERIC_TEXT.md); only TableCraft executes
SQL, and native Microsoft Access qualification remains unrun.

The original [Python runner](compare_declared_numeric_text.py) uses independent
Decimal half-even rounding and explicit destination ranges for integer Text,
Boolean zero/nonzero conversion, IEEE binary32 packing and binary64 conversion.
It does not call TableCraft conversion code to generate expected answers.
Each successful source-free query compares all six relational operators in both
orders with Scalar, Variant and unknown String declarations. Cases include ties,
negative fractions, narrow overflow, exact LongLong values beyond 2^53 and
Single precision loss. Both en-us and de-de numeric input profiles run without
an inferred machine locale. Large integers enter through exact Text constructors,
separately from the declared literal-preparation profile.

Native controls reuse the registered [Scalar inputs](FIXTURES.md#interrupted-seek-scalar-replay)
for Jet 4, ACE 2007 and ACE 2016/2019. The existing read-only Jackcess getter
returns stored ID, Byte and Integer values, including NULL and boundary carriers;
it evaluates no expressions. The same independent rounding/range model generates
all six comparison results for these actual fields. Out-of-range Text rejects
the entire buffered query. All 32 expected failures have empty stdout.
No database is generated, mutated or executed by the reference tool.

The frozen debug CLI has SHA-256
`4c43e4e2a237f22bbc110b707847ed1e5bebe5a320c3a5fd26ca6a18e91845e1`,
built from `79f4c2cbfb7cc836700037a688cd5724fd22570b`. The first complete run
already passed all 128 controls. The final run adds expected-result digests and
readability cleanup; every overlapping output hash, expected-cell count and exit
status remains identical. Repeated runs are counted once. Exact source/tool/input
hashes and commands remain in the report. No product behavior or expected value
changed during replay.

```sh
python3 docs/testing/compare_declared_numeric_text.py \
  --cli .cache/tablecraft-bin/declared-numeric-text-cli-20261025 \
  --jar .cache/tablecraft-fixtures/jackcess-5.0.4.jar \
  --directory .cache/tablecraft-fixtures/scalar-seek-20261023 \
  --report .cache/declared-numeric-text-new-replay.json
```

Reports must be fresh; native inputs and the reviewed JAR must match registered
hashes. The runner adds no product dependency or runtime. This evidence does not
establish native query declaration inference, regional parser parity, complete
Variant/Error semantics, write eligibility or completion of P0.

## Pre-change baseline replay

The frozen CLI built from `3b1e2344`, before the helper introduced in `79f4c2cb`,
also passes all 128 controls with identical successful outputs and failures.
The existing effective-type service already implemented the rule. The redundant
helper was removed; the baseline audit verified exact production-source equality at that
correction, before subsequent StrConv work. Six additional query/CLI regressions and this independent evidence are
retained. The [baseline audit](results/declared-numeric-text-baseline-audit-2026-10-25.json)
records the correction and current checks. Do not count the helper as new P0
functionality or add the repeated replay controls together.
