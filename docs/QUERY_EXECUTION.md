# Bounded retrieval execution

`crates/query/tablecraft-query` implements read-only preparation and execution over
explicit host sources. `crates/foundation/tablecraft-schema` supplies validated
[scalar schema contracts](SCHEMA.md) with an actual query consumer. Neither package
opens databases, implements storage transactions or imports application hosts.
This is a usable bounded retrieval profile, not complete Access SQL compatibility.

## Host API and ownership

Use `prepare(&Query, &dyn Source)` to resolve source/field slots and declared
parameters without scanning rows, then `PreparedQuery::execute` with a source,
exact named parameter values and `ExecutionOptions`. The crate's public doctest
shows a validated `MemoryTable`/`MemorySource` and a filtered SELECT. Sources
provide borrowed schemas and visit rows through a fallible callback. The host
must maintain a stable schema/data snapshot for each execution and stop on visitor
errors. The executor checks schemas at each table scan and validates every row's
width, exact carriers and nullability. Visitor errors are retained even if a
source incorrectly ignores them. Source I/O and time spent inside host callbacks
remain the host's responsibility.

Prepared plans own TableCraft slots/operators, not SQLParser ASTs or source rows.
They can be reused with new values against matching schemas. No cursor, bookmark,
updateability, schema mutation or full result-type inference is implied. Results
carry ordered names and buffered scalar rows; duplicate output names survive and
become ambiguous when referenced. Plans, sources and results omit Debug/Serde
payload representations. Typed errors contain no names, SQL or row payloads.

The query-owned schema profile accepts 1–256 input columns and limits host schema/
parameter names to 255 UTF-8 bytes. Shared SchemaName invariants require nonempty,
control-free names; the generic schema crate has no fixed byte/width ceiling.
Initial identifier matching folds ASCII case and otherwise
requires exact spelling; this is explicitly not Access's locale identifier policy.
Unknown unqualified names are errors unless explicitly declared PARAMETERS;
there is no implicit parameter prompt. Parameters require their declared carriers,
with NULL allowed, no silent coercion and optional Text limits counted in Unicode
scalar values (not a claim of native length-unit fidelity), using the same schema
TypeDescriptor as constrained fields. SQL
INTEGER/INT/LONG map to Int32; SHORT/SMALLINT map to Int16. Parameterized precision/
scale and other unsupported type arguments fail rather than being ignored.

## Implemented retrieval

- Source-free SELECT, named host tables, comma cross-products, explicit INNER/LEFT/
  RIGHT joins, aliases, relation groups and wildcards. Derived queries are supported
  but not lateral. Outer joins produce actual NULL-extended unmatched rows.
- Projection, WHERE and ORDER BY; only TRUE passes WHERE/HAVING/ON. Sorting accepts
  bound input expressions, bare output aliases and one-based output ordinals.
  NULL sorts low ascending/high descending. Equal keys retain input order.
- GROUP BY bound input fields, HAVING and Count/Sum/Avg/Min/Max (case-insensitive,
  one argument; Count also accepts `*`). NULL keys group together; aggregates
  ignore NULL arguments; Count over no values is zero and other aggregates yield
  NULL. A global empty group still returns its aggregate row. Ungrouped input
  fields and nested aggregate calls are rejected.
- DISTINCT output rows; TOP count/PERCENT with upward-rounded percentages and
  boundary ties across the complete ORDER BY key. Without ordering TOP returns a
  source-order subset, which is not a native guarantee of a particular subset.
- UNION and UNION ALL in written order, final ordering using first-arm names,
  and equal-width enforcement. No implicit union-wide carrier promotion occurs;
  ALL retains carriers, while duplicate removal uses the scalar comparison policy
  and rejects unsupported mixed comparisons.
- Scalar/EXISTS/IN/ALL/ANY/SOME retrieval subqueries, including correlated field
  scopes. Empty scalar results become NULL; multiple scalar rows are errors.
  Empty IN/ANY is FALSE and empty ALL is TRUE, including a NULL left operand.
- The existing scalar arithmetic/logic/text operators plus IS NULL, reversed-bound
  BETWEEN and three-valued IN/NOT IN. Both binary operands/list elements evaluate
  eagerly. Subqueries, including EXISTS, materialize their supported result; there
  is no optimizer or short-circuit guarantee suppressing errors/effects.

Text comparison/grouping/duplicate removal/ordering uses an explicit host
`Collation`; Rust string order is never selected implicitly. Grouping and DISTINCT
use SQL-style equality (including grouping NULLs), not representational `Value::Eq`.
Numeric operations retain the [initial scalar profile](EXPRESSIONS.md): checked
Int64/integer sums, supported exact Currency sums and Double averages where
supported. Decimal arithmetic, Currency averages and native Variant promotion are
not supplied by aggregation. Min/Max use the existing comparison rules.

## Explicit unsupported cases

DISTINCTROW, crosstabs, action/DDL/make-table statements, saved-query references,
external IN and OWNERACCESS are rejected before scans. So are LIKE, identity,
member/complex-field access, date/GUID literal materialization and scalar function
calls (including VBA/UDFs). Full Access built-ins, locale conversions and
calendar/GUID/binary ordering remain absent. Such carriers may still pass through
plain field projection. GROUP BY expressions beyond fields, grouped subqueries,
ORDER BY expressions involving output aliases and DISTINCT sorting beyond bare
output references/ordinals are outside this profile. Unsupported forms are never
silently discarded because an input table is empty.

## Limits and cancellation

Preparation bounds combined recursive planning depth to 16, plan visits to 2,048,
retained-plan estimates to 1 MiB, input/output width to 256, sources per SELECT to
16, grouping/sort keys to 10 and IN/UNION arm counts to 256. These include work on
publicly constructed syntax models, independently of parser limits. Larger names
or literal carriers are rejected. Depth counts planning helper calls as well as
syntax nesting, so the accepted syntax subset can be shallower than the parser's.

Execution defaults/clamps to 16 MiB cumulative logical payload/container allocation,
100,000 cumulative scanned/intermediate/output rows and 1,000,000 work steps.
These are work allowances, not returned-row counts or exact allocator/OS memory
measurements. Text values remain at most 65,536 UTF-8 bytes. Failed queries return
no partial result; budget is shared by joins, aggregates, comparisons and nested
queries. Cancellation is checked before row/node/comparison/sort work. Host source
and collation callbacks must enforce their own limits and responsiveness.

Sorting uses fallible stable merge ordering. Joins, grouping and duplicate removal
use bounded nested scans; no query-selected index probes, cost optimizer, spill engine, parallelism or
network runtime is introduced. Work limits can reject large valid queries. Future
optimizations must preserve errors, collation and resource/cancellation contracts.

## References and validation

Primary references reviewed 18 October 2026:
[SELECT](https://learn.microsoft.com/en-us/office/client-developer/access/desktop-database-reference/select-statement-microsoft-access-sql),
[TOP ties and DISTINCT](https://learn.microsoft.com/en-us/office/vba/access/concepts/structured-query-language/all-distinct-distinctrow-top-predicates-microsoft-access-sql),
[GROUP BY and NULL groups](https://learn.microsoft.com/en-us/office/vba/access/concepts/structured-query-language/group-by-clause-microsoft-access-sql),
[UNION](https://learn.microsoft.com/en-us/office/client-developer/access/desktop-database-reference/union-operation-microsoft-access-sql),
[Avg](https://learn.microsoft.com/en-us/office/vba/access/Concepts/Criteria-Expressions/avg-function-microsoft-access-sql),
[SQL data types](https://learn.microsoft.com/en-us/office/client-developer/access/desktop-database-reference/sql-data-types)
and [SQL INTEGER versus VBA Integer](https://learn.microsoft.com/en-us/office/vba/access/concepts/error-codes/comparison-of-data-types).
The parser's [reference mapping](SQL_PARSER.md) covers predicate/subquery syntax.
These guide original tests; they are not native comparison results.

Acceptance tests cover binding before scan, joins, aggregates, ordering, TOP ties,
NULL/Decimal equality, parameters, derived/correlated queries, errors and limits.
Three fixed-seed properties run 256 cases each against independent integer models
for filter/order, DISTINCT/NULL and Count/Sum. Native Access, file-backed sources,
other platforms and application corpus execution remain unqualified.

## CLI integration

`query run` without --database executes against an empty catalog, supporting inline row sources:

```sh
cargo run --locked -p tablecraft-cli -- query run "SELECT q.ID FROM (SELECT 1 AS ID UNION ALL SELECT 3 UNION ALL SELECT 2) AS q WHERE q.ID > 1 ORDER BY q.ID DESC"
cargo run --locked -p tablecraft-cli -- query run "PARAMETERS Floor INTEGER; SELECT q.ID FROM (SELECT 1 AS ID UNION ALL SELECT 3) AS q WHERE q.ID > Floor" --param Floor=long:2
```

The first prints ID rows 3 and 2; the second prints ID row 3. Typed parameters,
explicit ordinal collation and buffered output follow the
[CLI contract](DEVELOPMENT.md#inline-retrieval-cli). Named tables are exposed to
library hosts through Source; optional `--database PATH` supplies supported native
scalar snapshots through the separate adapter described below.

The CLI now optionally supplies native scalar tables through `--database PATH`.
[NATIVE_QUERIES.md](NATIVE_QUERIES.md) records the separate `tablecraft-access`
Source adapter, snapshots, budgets and format/type limitations. The query library
still has no concrete native I/O, engine or adapter dependency.

## Completion and container accounting

Execution checks cancellation and remaining work before publishing a completed
result, including an empty result after the final host comparison callback.
Parameter reference buffers, output column containers, implicit source rows and
ungrouped aggregate containers consume the shared byte budget through fallible
reservation. Outer-join NULL padding is created inside the charged output row.
A result with zero rows still consumes work and allocation allowance; exhausted
limits and callback failures never become successful partial results. Individual
host callbacks remain responsible for their own work/time limits.

Native hosts may explicitly supply an indexed snapshot through the Access adapter;
[Native index reads](NATIVE_INDEXES.md#bounded-indexed-reads) describe that source
behavior. It does not change the query planner or introduce predicate pushdown.
