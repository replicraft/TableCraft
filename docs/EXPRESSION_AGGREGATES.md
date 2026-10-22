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

## Standard Error extrema

An Error-only non-NULL group compares codes in 0 through 65535, using the shared
[Error comparison service](VARIANT_ERRORS.md#error-predicates-and-logical-operators).
Code zero is Error data, not NULL. Each Error candidate must have a standard code,
including singletons; opaque implementation-defined codes are refused. Repeated
codes retain an encounter representative. No early exit at code zero or 65535 skips
later expressions or failures.

Ordinary scalar extrema retain strict key ordering, exact carrier comparisons and
explicit field text policies. They do not adopt contextual predicate coercions just
because Error comparisons now share the expression path. Empty/Missing remain
unsupported query cells. Sorting, grouping and DISTINCT/UNION keys are not widened.

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
operation budgets. Extrema retain one selected cell rather than a second value
collection. Inspection and metadata preparation do not evaluate aggregate inputs.
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
checkpoint. The original MIT [comparison runner](testing/compare_error_consumers.py)
passes 410 independent controls. Python derives extrema from seeded code sequences
and read-only ID getters on four unchanged 20,000-row native inputs. Sequential and
both ID-index directions cover raw/converted/nullable extrema, domain functions,
numeric aggregate refusals and concatenation. No Access SQL executes in the oracle.
