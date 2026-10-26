# Mixed fixed/binary key replay

The [exact numeric key implementation](../query/keys/NUMERIC_KEYS.md#exact-fixedbinary-numeric-cohorts)
passes 3,956 current CLI queries against independent Fraction/IEEE models and
unchanged native scalar getters. A further 333 earlier-executable controls retain
275 former mixed-key refusals and 58 already-correct results. The
[report](results/fixed-binary-key-replay-2026-10-25.json) contains all 4,289 CLI
observations, 8,648 returned query rows and 25,112 cells compared by the candidate
model. Four separate Java getter invocations supply native values.

Only TableCraft executes SQL. This is independent engineering evidence for the
strict key profile, not Microsoft Access Variant coercion, grouping or optimizer
qualification. The default binder depth remains 16, and row-group crosstabs do
not qualify dynamic column heading names.

## Data, model and observations

The [original runner](compare_fixed_binary_keys.py) reuses the historical
[numeric-key Fraction/IEEE model](compare_numeric_keys.py), preserving that
runner and its recorded results. Nine literal model goldens pin IEEE values,
subnormals, signed zero and existing decimal rounding helpers. Expected key
partitions and ordering use exact fractions, independently of production's
bounded u128 bit-length/shift algorithm.

Fifteen host value cohorts cover:

- Positive/negative decimal tenths and distinct Currency/Single/Double tenths.
- Exact dyadic halves and mixed signed-zero representations.
- LongLong values beyond 2^53 and both signed Int64 limits.
- The 96-bit Decimal maximum, scale-28 values and the smallest positive Double.
- Decimal values immediately around stored Double values, including differences
  below the precision of ordinary binary64 conversion.
- Boolean TRUE with Currency/Single -1 and small Currency rounding boundaries.

These cohorts use the existing typed `VALUE` parameter contract, with exact
bindings recorded in the report. Their equivalent literal-cast descriptions are
not constructor-execution evidence. Carrier codes and raw numeric output observe
retained values directly. The standard binary32 decoder reconstructs Single's
shortest TSV spelling before the historical model widens it; SQL does not cast
or change the actual key. Fixed coefficients never pass through binary64.

Each cohort runs all six encounter permutations and both ordinary and declared
arithmetic execution profiles. Seven consumers exercise GROUP BY, DISTINCT,
dynamic UNION duplicate removal, ORDER BY, TOP ties, Min/Max and crosstab row
membership. Equal classes retain eligible original representatives and carriers;
ordering and TOP also check identities to preserve row/value association.

The native phase reuses the four registered `Scalar-V2000.mdb`,
`Scalar-V2007.accdb`, `Scalar-V2016.accdb` and `Scalar-V2019.accdb` originals.
The reviewed Jackcess 5.0.4 [getter](TableDataOracle.java) opens them read-only
with expression evaluation disabled. It supplies signed/scaled Money and Numeric
coefficients, exact Single/Double IEEE bits and stored Boolean/integer values.
No new database or third-party implementation is copied.

Eight cohorts per file read actual stored fields, including the minimum Currency,
large positive/negative Decimal values, signed zero, Single, Double, Boolean,
Byte and short integers. Three cohorts additionally compare a stored fixed value
with its explicit Double conversion. Domain Min/Max checks use actual Money and
Double fields from the first two records. Every original has three getter rows;
returned query rows are repeated consumer observations, not distinct authored data.

## Coverage and audit

| Observation | Count |
| --- | ---: |
| Current parameter-cohort queries | 1,260 |
| Current native field-cohort queries | 2,688 |
| Current native domain-extrema queries | 8 |
| Earlier-executable controls | 333 |
| Earlier mixed-key refusals | 275 |
| Earlier model-agreeing successes | 58 |
| Literal model goldens | 9 |
| Immutable input/tool/executable pins | 12 |
| Selected committed source snapshots | 17 |

All current queries succeed and agree with the model. The baseline refusals have
empty stdout and match the exact earlier numeric-combination/precision diagnostic
hashes. A separate audit verifies the unique command coverage, recomputes every
returned row/cell count from the fraction cohorts, and rehashes all twelve
immutable artifacts and seventeen committed/live source snapshots. Selected
snapshots are not an exhaustive compilation manifest.

Four diagnostic attempts remain preserved by private report hashes. The initial
runner duplicated an entire UNION in an empty arm; later cast-heavy compound
observers also crossed the existing binding-depth limit. A typed empty arm,
direct observations and the existing VALUE bindings retain every value cohort,
permutation and consumer within that profile. Generated native field names were
then bracketed where required. Runtime code, limits, fixtures and expected
numeric relations did not change during the replay. Larger compound-expression
support remains separate work.

## Reproduction

The candidate executable was built at
`50d3e4d9681042773f63b8c1142140de78500d65` and frozen with SHA-256
`29f187a3904efa04bdb474f9c0e6192a33c7387900423ef9e6f7c2efe5e20598`.
The baseline was built at `bf1df067a4d4cabe09f65544d2c1c6fc8281d1ea` with SHA-256
`284414fef496a705f0382d11851df43a58233f6975ccd95b08933d4a6fa91f74`.
Ignored fixture/tool/executable artifacts must already match their reviewed pins;
the runner downloads or builds nothing and requires a fresh report path.

```sh
python3 -B docs/testing/compare_fixed_binary_keys.py \
  --cli .cache/tablecraft-bin/fixed-binary-keys-cli-20261025 \
  --cli-sha256 29f187a3904efa04bdb474f9c0e6192a33c7387900423ef9e6f7c2efe5e20598 \
  --baseline .cache/tablecraft-bin/encryption-pages-cli-20261025 \
  --baseline-sha256 284414fef496a705f0382d11851df43a58233f6975ccd95b08933d4a6fa91f74 \
  --jar .cache/tablecraft-fixtures/jackcess-5.0.4.jar \
  --directory .cache/tablecraft-fixtures/scalar-seek-20261023 \
  --revision 50d3e4d9681042773f63b8c1142140de78500d65 \
  --report FRESH_REPORT.json
```

The CLI observation ceiling is 5,000, with a timeout per command and a 256 MiB
heap ceiling for independent getters. No native Access execution, timing or peak
memory benchmark was run. Full P0 value/result promotion, text semantics and
broader query-composition acceptance remain open.
