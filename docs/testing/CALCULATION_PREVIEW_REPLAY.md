# Independent row-local calculation previews

This replay checks the explicit [calculation preview](../CALCULATION_PREVIEWS.md)
against independent models over ordinary stored getters. It does not ask Jackcess
to evaluate formulas, qualify native Access arithmetic or certify cached freshness.

## Original inputs and model

The [read-only oracle](CalculationPreviewOracle.java) opens each original database
read-only, disables expression evaluation and emits the actual stored Expression
property, selected ordinary dependency and cached result separately. No imported
code, macro, UDF or formula executes. The new oracle is original repository code
under its MIT license; the reviewed Jackcess 5.0.4 artifact remains an external
Apache-2.0 reference dependency, identified by SHA-256 in the report.

The [runner](compare_calculation_previews.py) uses existing original Decimal,
Memo and [schema evolution](ADVANCED_FIELD_EVOLUTION.md) files. It also regenerates
the existing inert CalculatedStorageFixture and CalculatedScalarFixture recipes
into fresh ignored files to cover ordinary NULL dependencies and unsupported
stored expressions. No new native declaration variant is admitted.

| Stored expression | Ordinary dependency | Independent model |
| --- | --- | --- |
| `[Score]*2`, `[ID]*2` | Integer getter | Python integer multiplication |
| `[ID]/10000`, `[Id]/2` | Integer getter | Exact Fraction converted to binary64 |
| `"prefix" & [ID]` | Integer getter | Explicit invariant integer-to-Text concatenation |
| `[Label] & "later"` | Text getter | Unicode string concatenation |

The runner checks every returned row in native table iteration order. Division
compares parsed binary64 bits rather than relying on Python and Rust to choose the
same shortest decimal spelling. This output check does not independently prove
runtime carrier metadata; focused library tests own that contract. Text checks
remove TSV/CSV transport quoting while retaining existing escaped Value display.
Successful commands explicitly choose `--number-format invariant`; defaults do
not change.

Stale caches deliberately differ from these independently modeled results. NULL
caches over non-NULL ordinary inputs still recompute, while a NULL Score dependency
propagates to NULL. Evolution inputs retain historical rows with missing later
caches and current rows with relocated or stale results. No result is coerced to
the native cache declaration: a declared Decimal or Currency division remains
the evaluator's natural numerical result.

## Refusals and preservation

The replay also checks empty admitted tables; ordinary or unknown target refusal;
comparison and DateSerial declarations refused even in empty tables; and native,
query-work and output limits. Refused commands must publish no stdout.

A disposable copy corrupts a unique Decimal cache wrapper. Explicit cached
inspection must fail, while the same independent ordinary-ID preview remains
unchanged. This demonstrates dependency pruning rather than converting an invalid
cache into NULL or repairing it. Originals, executable, oracle, runner, helper and
selected implementation hashes must all match after the run. Generated databases,
reference JAR and executable remain ignored artifacts.

## Accepted replay

On 24 October 2026, the [accepted report](results/calculation-preview-2026-10-24.json)
records 44 passing controls and 1,018 independently modeled result cells across
20 selected declarations in ACE 2016/2019 files. Of those result cells, 1,010
numerically or textually differ from the independent cached getter; the other
eight include deliberately matching results and ordinary NULL propagation.
Eight comparison/function declarations are refused, including their empty-table
counterparts. All 26 pinned implementation, producer, oracle, helper, input,
reference JAR and executable hashes remain unchanged after replay.

The immutable executable SHA-256 is
`99ee235b29d5dd4a08f9a699e5f3489d5cc0e6aa88843e9cd53ba91cc8952126`.
No production engine or query admission change was required by this replay.
These are offline engineering results; native Access execution and broader
calculated-field semantics remain unqualified.

```sh
python3 docs/testing/compare_calculation_previews.py \
  --cli .cache/tablecraft-bin/calculation-preview-20261024 \
  --jar .cache/tablecraft-fixtures/jackcess-5.0.4.jar \
  --decimal-directory .cache/tablecraft-fixtures/calculated-decimals-20261024 \
  --memo-directory .cache/tablecraft-fixtures/calculated-memo-modes-20261024 \
  --evolution-directory .cache/tablecraft-fixtures/advanced-evolution-deleted-20261024 \
  --simple-directory .cache/tablecraft-fixtures/preview-simple-20261024 \
  --report /tmp/calculation-preview.json
```

[Fixture provenance](../FIXTURES.md#independent-calculation-preview-inputs) records
the newly regenerated simple input hashes. The runner refuses report overwrites.
