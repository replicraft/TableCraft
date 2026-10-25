# Prior projection aliases

Ordinary buffered SELECT projections can refer to an earlier explicit alias in
that same SELECT. The reference reads the already computed retained cell; it does
not copy the defining expression into the dependent expression.

```sql
SELECT N + 1 AS Seed, Seed * 2 AS Twice FROM Items;

SELECT N,
       (SELECT Max(Old.N) FROM Items AS Old WHERE Old.N < Current.N) AS Prior,
       N - Prior AS Distance
FROM Items AS Current
ORDER BY N;
```

Microsoft's Access
[subquery tutorial](https://support.microsoft.com/en-us/access/nest-a-query-inside-another-query-or-in-an-expression-by-using-a-subquery)
defines a correlated subquery column for the previous order date, then uses that
alias in another calculated column to compute an interval. This establishes a
useful same-SELECT dependency shape. The implementation stages described here do
not establish complete native alias resolution or optimizer evaluation behavior.

## Binding and execution

Only unqualified references to prior explicitly named projections are admitted.
Existing source and correlated outer fields take precedence, followed by explicitly
declared parameters; alias lookup is a fallback. This precedence is an explicit
implementation choice pending native qualification, not a rule inferred from the
tutorial. Automatically generated output names do not become dependency declarations.
Duplicate prior aliases are ambiguous when referenced. Forward and self dependencies
cannot bind, so admitted dependencies form a bounded graph in written projection order.

Every SELECT and read boundary starts its own alias scope and restores the prior
scope on success or failure. A nested scalar query, UNION-level sort or named
source cannot capture an enclosing SELECT's projection aliases. Named and derived
queries can publish their completed alias results through their ordinary schemas;
external ownership and authorization stay with the existing namespaced read plan.

A bound reference carries the defining expression's two arithmetic-profile types,
declaration and provisional text policy. Runtime copies the retained cell's actual
carrier, declaration, policy and completion marker. Raw String bytes are preserved:
a derived outer projection may observe or complete them, while a selected final
raw column still requires strict Unicode publication. A later alias reference does
not hide an earlier projection's eager failure or replace it with a cached value.
Binding remains metadata-only; alias registration, lookup and stability traversal
consume the existing binding allocation/work/depth allowances. Runtime references
and retained payload copies consume the same cumulative query budget as other
expression cells and honor cancellation.

## Current boundaries

Alias dependencies are limited to stable definitions under this engineering
profile. Clocks, Rnd, Eval and opaque domain calls are refused when their alias is
referenced, including inside a dependent scalar subquery or derived source.
Ordinary comparison providers remain explicitly supplied and deterministic; a
reused comparison alias reads the computed Boolean cell rather than invoking its
provider again. An unused alias does not make an otherwise supported volatile or
opaque definition unavailable.

References are not introduced into WHERE, GROUP BY, HAVING, aggregate operands or
nested subqueries. Grouped/aggregate SELECT alias dependencies, crosstab projection
dependencies and progressive execution remain unsupported and fail before row
scans/output. Existing bare ORDER BY aliases and ordinals keep their separate
output-reference behavior; compound ordering aliases are not added by this slice.
Forward dependencies, broader native name precedence, volatile alias evaluation
counts and native Access qualification remain open work.
