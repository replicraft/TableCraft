# Statistical query aggregates

Read-only retrieval now executes `Var`, `VarP`, `StDev` and `StDevP` over bounded
materialized numeric values. They use the existing aggregate binder and work in
projections, grouped HAVING/ORDER BY, derived/correlated reads, unions, parameters,
crosstab measures/totals and qualified native saved/named queries. No domain
function, VBA provider or application code is invoked.

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

Finite numeric/Boolean operands use the existing CDbl value conversion, including
Currency/Decimal and explicit binary64 rounding of large Int64 values. Successful
results are Double; no implicit Text/date/GUID/Binary conversion or inferred locale
is introduced. Non-finite inputs and overflowing results fail. Gradual binary64
underflow can round very small variance to zero. These are explicit current
conversion/result rules, not full native Variant promotion or bitwise ACE precision
qualification. The builtin tracker therefore records these functions as Partial.

Each function requires one expression. Wildcards, missing/additional arguments,
qualified calls and nested aggregate arguments fail while binding, even for empty
sources. Unsupported runtime carriers and later argument errors remain visible;
TOP or an unselected IIf branch cannot hide them. Source-free `query eval` retains
its scalar-only profile; source-free aggregate SELECTs use `query run`.

## Numerical work and resource ownership

Each argument evaluates once per encountered row, in encounter order. A fallibly
reserved f64 buffer is charged to the cumulative execution allocation budget
before growth. Repeated aggregate calls each consume their own allowance; no
volatile expression or selected snapshot is silently cached/replayed.

The numeric kernel shifts by an input anchor, scales by the maximum absolute
input and uses compensated accumulation for the centered mean and squared
residuals. Finite original subtraction preserves small spreads around large
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

Nine query tests cover exact rational integer-group properties, documented
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
