# Statistical query aggregates

Read-only retrieval now executes `Var`, `VarP`, `StDev` and `StDevP` over bounded
materialized numeric values. They use the existing aggregate binder and work in
projections, grouped HAVING/ORDER BY, derived/correlated reads, unions, parameters,
crosstab measures/totals and qualified native saved/named queries. The separate
[domain profile](DOMAIN_FUNCTIONS.md) reuses these kernels. No VBA provider or
application code is invoked.

## Counts, carriers and errors

NULL operands are omitted. With `n` remaining values, results follow this profile:

| Function | Minimum n | Divisor | Result |
| --- | ---: | --- | --- |
| Var | 2 | n - 1 | Sample variance |
| VarP | 2 | n | Population variance |
| StDev | 2 | n - 1 | Sample standard deviation |
| StDevP | 1 | n | Population standard deviation |

Insufficient values return NULL. StDevP of one finite value returns Double zero.
An empty/all-NULL group returns NULL for all four functions. The distinct singleton
rules follow Microsoft's [Var/VarP reference](https://learn.microsoft.com/en-us/office/vba/access/concepts/criteria-expressions/var-varp-functions-microsoft-access-sql)
and [StDev/StDevP reference](https://learn.microsoft.com/en-us/office/vba/access/concepts/criteria-expressions/stdev-stdevp-functions-microsoft-access-sql):
the former documents a two-record minimum for both functions; the latter permits
one value for StDevP. This reference-derived boundary still needs native Windows
qualification, including how native NULL elimination interacts with singleton cases.

Numeric/Boolean admission is unchanged. For groups containing only integral,
Boolean, Currency and Decimal values, exact fixed-point differences are formed
before binary64 rounding, as described below. Groups containing Single/Double
retain the existing CDbl conversion of every input, including rounding of large
Int64/Currency/Decimal values. Successful results remain Double; no implicit
Text/date/GUID/Binary conversion or inferred locale is introduced. Non-finite inputs and overflowing results fail. Gradual binary64
underflow can round very small variance to zero. These are explicit current
conversion/result rules, not full native Variant promotion or bitwise ACE precision
qualification. The builtin tracker therefore records these functions as Partial.

Numeric admission is checked before calling the shared converter: explicit CDbl
accepts a legacy DATE serial, but that does not make an unconverted Date field a
statistical numeric operand. This closes an accidental DATE admission introduced
when the general converter gained legacy Date support. Microsoft's
[query aggregate table](https://support.microsoft.com/en-us/access/sum-data-by-using-a-query)
lists Number/Currency for variance and deviation. The implementation retains its
existing numeric/Boolean scalar profile; exact native SQL admission and failure
ordering remain unqualified.

To compute statistics over stored serials explicitly, use e.g.
`SELECT VarP(CDbl(Stamp)) FROM T WHERE Stamp IS NOT NULL`. A domain equivalent is
`DVarP('CDbl(Stamp)','T','Stamp IS NOT NULL')`. NULL elimination by the
aggregate does not suppress an earlier CDbl(NULL) conversion failure. Metadata
preparation still does not inspect rows; empty/all-NULL unconverted Date groups
retain the existing NULL result policy.

Each function requires one expression. Wildcards, missing/additional arguments,
qualified calls and nested aggregate arguments fail while binding, even for empty
sources. Unsupported runtime carriers and later argument errors remain visible;
TOP or an unselected IIf branch cannot hide them. Source-free `query eval` retains
its scalar-only profile; source-free aggregate SELECTs use `query run`.

## Numerical work and resource ownership

Each argument evaluates once per encountered row, in encounter order. A fallibly
reserved f64 buffer is charged to the cumulative execution allocation budget
before growth. All-fixed groups retain an additional, equally bounded f64 offset
buffer. If a floating value is encountered, that offset buffer is dropped and
the original CDbl buffer remains authoritative; consumed allocation allowance is
not refunded. A group starting with a floating value never allocates offsets.
Repeated aggregate calls each consume their own allowance; no volatile expression
or selected snapshot is silently cached/replayed.

For all-fixed groups, Boolean/integral/Currency/Decimal inputs first become exact
coefficient/scale pairs. Each sample is translated by the first non-NULL sample
using checked i128 alignment/subtraction. Unequal-scale alignment beyond i128
uses the existing BigInt dependency under a bounded scratch charge issued before
allocation. A stack scientific spelling converts the exact difference to Double;
no stored Decimal is widened, normalized or modified. The largest aligned
coefficient is bounded by the 96-bit carrier and scale-28 envelope. Narrow offsets
require no heap scratch allowance.

This corrects a concrete loss of information: LongLong values 9007199254740992
and 9007199254740993 now produce population variance 0.25, rather than zero.
Adjacent Currency units near the signed 64-bit coefficient boundary and Decimal
units at scale 28 likewise retain nonzero spread. Explicit `CDbl(X)` still rounds
before aggregation and can intentionally collapse those neighbors. Mixed
floating groups keep that original conversion policy; they do not reconstruct
fixed samples from rounded offsets.

The existing numeric kernel then shifts by an input anchor, scales by the maximum
absolute retained value and uses compensated accumulation for the centered mean
and squared residuals. Finite original subtraction preserves small spreads around large
offsets; normalized subtraction handles opposite-sign extremes without overflowing
an intermediate difference. Two centered passes visit retained numbers, never
re-evaluate SQL or reread source records. Every pass checks work/cancellation.
Variance multiplies in a scaled order rather than forming scale squared first;
standard deviation never forms a potentially overflowing variance before sqrt.
Constant groups return nonnegative zero. Results never publish NaN/Infinity.

This avoids the unstable `sum(x*x) - sum(x)*sum(x)/n` shortcut. It does not promise
native Access's exact floating accumulation order. Existing group/row/snapshot,
expression and output ceilings remain, with buffered publication on failure.
The implementation stays in the query crate and reuses the expression conversion
kernel; no engine/adapter dependency or new library is introduced.

## Evidence

The sixteen-test statistical integration suite covers exact rational integer-group properties, documented
NULL/singleton rules, all finite numeric carrier families, large offsets, tiny
spreads, representable deviation with overflowing variance, and cumulative
buffer/work limits. A changing host clock proves retained numeric passes do not
re-evaluate the argument. Every observed execution checkpoint can cancel without a
result. Two CLI controls cover actual numeric output and buffered late failures.
A native collection test covers stored `.Value` groups and empty parents over the
three priority ACE header families.

The original [SavedStatisticsFixture.java](testing/SavedStatisticsFixture.java)
adds nine inert statistical definitions to the fourteen-query forest recipe.
Four fresh Jet 4 / ACE 2007/2016/2019 files pass 180 controls through
[compare_saved_statistics.py](testing/compare_saved_statistics.py). Read-only
Jackcess getters/rendering supply storage/SQL evidence; explicit rational models
supply expected results. They cover raw inspection, direct/saved/named execution,
grouping, parameters, derived reads, UNION, crosstabs, singleton/empty groups and
wrong-type refusal. All four hashes remain unchanged.
[Fixture records](FIXTURES.md#statistical-saved-query-inputs) retain provenance.
These are independent stored-data/model checks, not Access SQL execution evidence.

```sh
tablecraft query run 'SELECT Var([ORDR_NBR]) AS SampleVariance, StDevP([ORDR_NBR]) AS PopulationDeviation FROM ROULETTE_TB' --database docs/examples/Access_Example_VBA.accdb
```


### Fixed-point offset verification

Five focused integration tests cover signed LongLong endpoints, 2^53 neighbors,
Currency coefficient endpoints, signed 96-bit Decimal endpoints at scale 28,
unequal scales including alignment beyond i128, both mixed-floating encounter
orders, grouped/derived/domain/crosstab consumers and the extra retained-buffer
budget. The latter independently observes 160 additional bytes for twenty
binary64 offsets. Two library controls cover allocation-free narrow offsets,
wide scratch refusal and every observed wide-helper cancellation checkpoint.
Existing NULL/insufficient populations, nonnumeric/raw/Error admission, eager
failures, volatile once-only evaluation and statistical cancellation controls
remain active.

The original [Fraction runner](testing/compare_fixed_statistics.py) constructs
source-free typed SQL values and checks all permutations against exact Python
centered moments, with binary64 conversion/sqrt only for expected outputs. It
also retains explicit CDbl and mixed-floating reference controls. These are
independent numerical checks of the implementation model, not native Access
precision, wrapper or floating accumulation qualification. No database fixture
or new dependency is introduced.

The 2026-10-24 replay passed 404 Fraction-model result controls across 101 SQL
queries against immutable CLI SHA-256
`f2ed2b85aa1b665043df3eaaedcac7f1753be01b6cb3d2c2f05c424726c49e99`.
The [tracked report](testing/results/fixed-statistics-2026-10-24.json) records
actual terminal output, matching binary hashes before/after, runner hash and
generated SQL digest. The focused 16-test statistics, 9-test domain and 6-test
Variant aggregate targets pass, together with 9 library tests and scoped Clippy.
These statistics are the default engineering numerical profile; enabling or
disabling `--declared-arithmetic` does not change their fixed-offset selection.
