# Expression data in query aggregates

This is the implemented read-only profile. Native Access Error-cell aggregate
behavior and error ordering remain unqualified; the Error-only extrema rules below
are provisional. Stored sources and parameters still accept ordinary scalar values.
Expression Error cells originate in explicit query expressions or derived results.

| Consumer | Error data | NULL and empty groups |
| --- | --- | --- |
| Count / DCount | Counts each non-NULL Error cell without converting its code | NULL is excluded; empty count is zero |
| First / Last / DFirst / DLast / DLookup | Retains the selected expression cell, without code interpretation | Encounter selection includes NULL; empty selection returns NULL |
| Min / Max / DMin / DMax | Selects among standard Error codes and retains Error data; mixing Error with a non-NULL scalar fails TypeMismatch | NULL is excluded; empty/all-NULL groups return NULL |
| Sum / Avg and numeric/statistical domain counterparts | Rejects Error with TypeMismatch; numeric code conversion must be explicit | Existing NULL elimination and per-function empty/population rules remain |

The [statistical contract](STATISTICAL_AGGREGATES.md) retains each SQL/domain
population boundary. This change does not unify those boundaries or change ordinary
numeric accumulation, precision or rounding policies.

## Counting lossless String expressions

Count and DCount now consume raw String expressions through the shared lossless
operand service. Complete unpaired UTF-16 units and odd byte ranges count as
non-NULL Strings without Unicode decoding, numeric conversion or an extra decoded
payload allocation. For example, `Count(ChrW(Code))` can count surrogate units,
and `Count(LeftB(ChrW(Code),1))` can count one-byte String ranges. Empty Strings
count as values; a NULL result such as `ChrW(Code)+NULL` does not count.

Microsoft's [Access SQL Count contract](https://learn.microsoft.com/en-us/office/vba/access/concepts/criteria-expressions/count-function-microsoft-access-sql)
allows expression inputs, counts text and excludes NULL. Applying that nullness
contract to TableCraft's lossless intermediates is the implemented profile;
native Access raw-string execution remains unqualified. Invalid constructors and
conversion errors still fail: `ChrW(NULL)` and `CStr(NULL)` do not become omitted
NULL rows. Inputs evaluate normally for every selected row. Filtered/empty groups,
HAVING, domain criteria, shared quotas and cancellation retain their existing
contracts. Empty/Missing query cells remain unsupported.

Raw Strings remain expression data with [retained cells](../../expressions/text/RETAINED_STRINGS.md)
and [explicit comparison keys](../keys/RAW_STRING_KEYS.md). Stored values retain their
own admission rules. The original [cardinality runner](../../testing/compare_raw_string_count.py)
passes 774 independent source-free controls, including seeded unit sequences,
odd bytes, explicit CStr/CVar identity, filtering, NULL results and failures.

## Retained String extrema

Eligible direct-table global Min/Max can also use the explicit
[progressive aggregate service](../PROGRESSIVE_QUERIES.md#global-min-and-max).
It retains the same result cells and comparisons, with a documented checked-EOF
policy cohort and stream error phases; ordinary buffered extrema remain unchanged.

Min/Max and DMin/DMax now retain the selected String cell instead of forcing each
candidate to Unicode. NULL is omitted; empty/all-NULL populations return NULL.
A singleton, including an odd byte range or isolated surrogate, performs no
unused comparison or locale lookup. Raw-aware observers can consume the selected
data through derived rows, grouped/HAVING queries, correlations, fixed named
crosstabs and domain calls:

```sh
tablecraft query run "SELECT LenB(Min(S)) AS N FROM (SELECT ChrW(55296) AS S) AS Q"
```

This returns 2. Actual String comparisons use the [strict key boundary](../keys/RAW_STRING_KEYS.md):
explicit UTF-16 providers compare complete units, Unicode-only providers publish
strictly, and odd byte counts remain refused when comparison needs units.
Numeric/String mixtures retain TypeMismatch without adopting predicate coercion
or the Variant category exception. Field/static/runtime policies select the
comparer; unknown/conflicting required policies and callback failures propagate.
Equal extrema retain their first encountered representative and its exact bytes.

Every selected row's argument still evaluates normally. An unrepresentable String
is data; invalid constructors, division by zero, cancellation and resource errors
remain failures. A later operational failure is not hidden by an earlier String
candidate or a possible minimum/maximum bound. Shared allocations/work remain
cumulative. Global policies and non-String populations retain one selected cell.
With a resolver, direct retained fields (including CVar wrappers) keep that path
only when a metered pass over the group's existing cells proves identical policy
identity and completion metadata. The pass evaluates no expressions or providers
and allocates no candidate storage. Other String expressions retain charged
candidates until the final runtime policy is known; no argument evaluates twice.
Conservative nondefault static lineage alone cannot establish stability: completed
construction can discard a numeric/Boolean/date peer's latent Text policy.
A late field order can make an already discarded candidate the true extremum.
The final cohort policy and completion apply to every candidate comparison,
including pairs preceding the contributor that completed the cohort. NULL contributors
still supply policy, singleton raw Strings require no unused comparison, and
numeric/Error/String admission remains explicit. This can require more storage
for dynamic String extrema; it does not define native mixed-policy precedence.

Scalar consumers explicitly publish the selected String before Value conversion.
Public query/CLI String publication remains strict, so selecting an unpaired
result directly still fails before buffered output. Error-only extrema retain
their separate standard-code contract below; mixed Error/String values fail
TypeMismatch. No stored/source/parameter type or native write gate changed.

Microsoft's [SQL Min/Max reference](https://learn.microsoft.com/en-us/office/vba/access/concepts/criteria-expressions/min-max-functions-microsoft-access-sql)
defines group extrema. Its [DMin/DMax reference](https://support.microsoft.com/en-gb/access/functions/dmin-dmax-functions)
explicitly permits query expressions, describes numeric/String results and
excludes NULL values. Applying those selection rules to lossless String cells
is TableCraft's implemented profile; raw native Access execution remains unqualified.

The original [extrema runner](../../testing/compare_raw_string_extrema.py) passes 791
controls: 215 source-free controls over reordered independent General key vectors
and 576 native-source aggregate/domain controls. Jackcess getters supply the six
stored IDs in each of four unchanged inputs; Python supplies selection/length/
representative expectations. Database/default overrides, sequential and selected
primary-index reads, criteria and empty populations are covered. Only TableCraft
evaluates SQL. Native Access comparison and benchmarks were not run.

## Standard Error extrema

An Error-only non-NULL group compares codes in 0 through 65535, using the shared
[Error comparison service](../../expressions/VARIANT_ERRORS.md#error-predicates-and-logical-operators).
Code zero is Error data, not NULL. Each Error candidate must have a standard code,
including singletons; opaque implementation-defined codes are refused. Repeated
codes retain an encounter representative. No early exit at code zero or 65535 skips
later expressions or failures.

Ordinary scalar extrema retain strict key ordering, exact carrier comparisons and
explicit field text policies. They do not adopt contextual predicate coercions just
because Error comparisons now share the expression path. Empty/Missing remain
unsupported query cells. Error sort/group/set consumers retain their separate
limits; [retained String keys](../keys/RAW_STRING_KEYS.md) use their own explicit profile.

Both direct and domain extrema return expression data through
`PreparedQuery::execute_expression`. Derived/named, grouped/HAVING, correlated and
crosstab consumers retain it. Explicit CStr or numeric conversion can consume the
selected Error. Scalar-only `execute` still refuses a retained Error result.
Missing crosstab intersections remain NULL independently of aggregate behavior.

```sh
tablecraft query run 'SELECT Min(CVErr(ID)),Max(CVErr(ID)) FROM Codes' --database example.accdb
tablecraft query run 'SELECT CLng(Min(CVErr(ID))) FROM Codes' --database example.accdb
```

## Concatenation and numeric admission

Ampersand evaluates both operands before rejecting Error with TypeMismatch. It does
not implicitly call CStr on Error, although explicit `CStr(CVErr(...))` remains
available. Numeric aggregates likewise refuse implicit conversion of Error codes.
An argument's operational failure is not an Error data value: division by zero,
invalid constructors, cancellation and resource failures still abort the query.
An operation that already failed need not evaluate subsequent rows.

All groups, domain reads, comparisons, copies and output publication use the shared
operation budgets. Numeric/Error extrema and stable-policy String paths retain
one selected cell; unresolved String policies use the bounded candidate path
described above. Inspection and metadata preparation do not evaluate aggregate inputs.
No native write, stored type or dependency is added.

## References and evidence

Microsoft's [Min/Max reference](https://learn.microsoft.com/en-us/office/vba/access/concepts/criteria-expressions/min-max-functions-microsoft-access-sql)
defines selecting extrema over a query group. The
[MS-VBAL relational rules](https://learn.microsoft.com/en-us/openspecs/microsoft_general_purpose_programming_languages/ms-vbal/f8acd631-55c1-4199-bc1e-022aaab6d9c8)
define standard Error-code comparisons; combining these motivates this provisional
Error-only aggregate profile, not a claim of native SQL qualification. The
[ampersand specification](https://learn.microsoft.com/en-us/openspecs/microsoft_general_purpose_programming_languages/ms-vbal/9f072ffc-e943-4fcc-a4d0-f3c7db96abd9)
requires Error rejection. Actual Access Error aggregates, mixed-result admissibility,
subtypes and failure precedence still need Windows comparisons.

Focused Rust tests cover explicit/scalar publication, code zero and endpoints,
NULL/all-empty inputs, domain/group/pivot consumers, mixed-type and late failures,
256 generated NULL/Error sequences, shared quotas and every observed cancellation
checkpoint. The original MIT [comparison runner](../../testing/compare_error_consumers.py)
passes 410 independent controls. Python derives extrema from seeded code sequences
and read-only ID getters on four unchanged 20,000-row native inputs. Sequential and
both ID-index directions cover raw/converted/nullable extrema, domain functions,
numeric aggregate refusals and concatenation. No Access SQL executes in the oracle.

Six focused late-policy extrema regressions cover reverse orders in multiple
encounter positions, NULL policy contributors, an absent unused default comparer,
explicit global overrides, one evaluation per operand/aggregate, retained raw
singletons, every observed cancellation checkpoint and byte limits. A numeric
control succeeds under the same minimal byte allowance with or without a field
resolver, ensuring non-String extrema do not acquire a candidate collection.
The collation/domain/Error regression batch passes 98 tests. These are provider
contract checks, not native Access precedence evidence.
