# Bounded retrieval execution

`crates/query/tablecraft-query` implements read-only preparation and execution over
explicit host sources. `crates/foundation/tablecraft-schema` supplies validated
[scalar schema contracts](SCHEMA.md) with an actual query consumer. Neither package
opens databases, implements storage transactions or imports application hosts.
This is a usable bounded retrieval profile, not complete Access SQL compatibility.
The separate [query editing API](QUERY_EDITING.md) plans bounded single-table
UPDATE actions over shared snapshot contracts. Retrieval APIs remain read-only.

## Host API and ownership

Use `prepare(&Query, &dyn Source)` to resolve source/field slots and declared
parameters without scanning rows, then `PreparedQuery::execute` with a source,
exact named parameter values and `ExecutionOptions`. The crate's public doctest
shows a validated `MemoryTable`/`MemorySource` and a filtered SELECT. Sources
provide borrowed schemas and visit rows through a fallible callback. The host
must maintain a stable schema/data snapshot for each execution and stop on visitor
errors. The executor checks schemas at each table scan and validates every requested row's
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

## Requested source fields

After complete binding, retrieval plans request only scalar fields used by outputs,
filters, grouping, ordering, joins, correlated subqueries or active DISTINCTROW
whole-record keys. Unsupported references
and matching wildcards still fail before scanning. All derived output expressions
remain evaluated, preserving their errors; pruning does not short-circuit expression
branches, change cardinality or introduce NULL placeholders. UPDATE plans retain
their existing complete-row contract.

`Source::scan_projected` receives sorted unique positions in the complete
`columns()` schema, with constraint positions in that same original schema.
`ProjectedRow` borrows selected values in requested order. The default implementation
validates complete supplied rows and propagates the first validation/visitor failure,
including from a source that ignores callback rejection. Providers may override it
to avoid decoding unrequested payloads. The executor validates selected row width,
carriers and nullability; full source schema/unavailable declarations remain part
of execution checks. An empty field request still visits one zero-width row per
record. Sources own their underlying I/O/validation scope and resource controls.

The bounded planner accounts for projection maps and traversal work. Execution
still materializes bounded rows and subqueries; this is neither streaming nor TOP
pushdown. Native behavior is described in [native reads](NATIVE_QUERIES.md#query-projection-pruning).

## Implemented retrieval

- Source-free SELECT, named host tables, comma cross-products, explicit INNER/LEFT/
  RIGHT joins, aliases, relation groups and wildcards. Derived queries are supported
  but not lateral. Outer joins produce actual NULL-extended unmatched rows.
- Projection, WHERE and ORDER BY; only TRUE passes WHERE/HAVING/ON. Sorting accepts
  bound input expressions, bare output aliases and one-based output ordinals.
  NULL sorts low ascending/high descending. Equal keys retain input order.
- GROUP BY bound input fields or supported pure scalar expressions, HAVING and
  Count/Sum/Avg/Min/Max/First/Last (case-insensitive,
  one argument; Count also accepts `*`). NULL keys group together; aggregates
  other than First/Last ignore NULL arguments; Count over no values is zero and other aggregates yield
  NULL. A global empty group still returns its aggregate row. Ungrouped input
  fields and nested aggregate calls are rejected.
- DISTINCT output rows and bounded [DISTINCTROW source records](#distinctrow-source-records); TOP count/PERCENT with upward-rounded percentages and
  boundary ties across the complete ORDER BY key. Without ordering TOP returns a
  source-order subset, which is not a native guarantee of a particular subset.
- UNION and UNION ALL in written order, final ordering using first-arm names,
  and equal-width enforcement. No implicit union-wide carrier promotion occurs;
  ALL retains carriers, while duplicate removal uses the scalar comparison policy
  and rejects unsupported mixed comparisons.
- Scalar/EXISTS/IN/ALL/ANY/SOME retrieval subqueries, including correlated field
  scopes. Empty scalar results become NULL; multiple scalar rows are errors.
  Empty IN/ANY is FALSE and empty ALL is TRUE, including a NULL left operand.
- Explicit-mode LIKE/NOT LIKE over Text/NULL, with wildcard lists/ranges and
  bounded iterative matching. [Pattern contracts](PATTERN_MATCHING.md) describe
  the required mode/collation, Unicode units and native locale limits.
- [Switch/Choose selection](CONDITIONAL_FUNCTIONS.md), with eager operands and
  unchanged selected carriers across pure/grouped/correlated expressions.
- The existing scalar arithmetic/logic/text operators plus
  [Abs/Sgn/Int/Fix/Round](MATH_FUNCTIONS.md), IS NULL, reversed-bound
  BETWEEN and three-valued IN/NOT IN. Both binary operands/list elements evaluate
  eagerly. Subqueries, including EXISTS, materialize their supported result; there
  is no short-circuit guarantee suppressing errors/effects. Safe source constraints
  below preserve the original residual filter.

Text comparison/grouping/duplicate removal/ordering uses an explicit host
`Collation`; Rust string order is never selected implicitly. Grouping and DISTINCT
use SQL-style equality (including grouping NULLs), not representational `Value::Eq`.
Numeric operations retain the [initial scalar profile](EXPRESSIONS.md): checked
Int64/integer sums, supported exact Currency sums and Double averages where
supported. Decimal inputs now retain Decimal sums/averages through the checked
kernel below; Currency-only averages and full native Variant promotion remain
unsupported. Min/Max use the existing comparison rules.

## Explicit unsupported cases

Action/DDL/make-table statements and
external IN and OWNERACCESS are rejected before scans. So are identity,
member/complex-field access and scalar function
calls outside the explicit NULL/conditional, numeric, text and date built-ins (including VBA/UDFs). Other Access built-ins, locale conversions and
GUID/binary ordering remain absent. Qualified date/GUID literal materialization
and date comparison follow the [scalar contracts](EXPRESSIONS.md). Grouped subqueries,
ORDER BY expressions involving output aliases and DISTINCT sorting beyond bare
output references/ordinals are outside this profile. Unsupported forms are never
silently discarded because an input table is empty.

## Limits and cancellation

Preparation bounds recursive syntax planning depth within each definition to 16,
combined plan visits to 2,048,
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
use bounded nested scans; no index join probes, cost optimizer, spill engine, parallelism or
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

## Safe source constraints

`Source::scan_constrained` is an optional backend-neutral interface for a NULL
selection or an exact Int32/Int64 interval on one source column ordinal. The default calls
ordinary scan. A source may supply a superset of matching rows; the original WHERE
always evaluates in the executor. Matching rows must retain source order and exact
carriers, and callback/source failures must propagate. Physical index types stay
out of the query crate.

The analysis accepts a single table with a wholly safe predicate: Int32/Int64
field comparisons `=`, `<`, `<=`, `>`, `>=` against in-range integer literals or
plain non-NULL Int32/Int64 parameters; IS NULL/IS NOT NULL; and AND combinations on the
same column. Reversed comparison operands and unary signs on integer literals are
supported. Same-column bounds intersect without converting NULL equality into IS
NULL. Int32 bounds must fit Int32; Int64 bounds preserve full signed precision.
Integer comparisons use the evaluator's exact fixed-number semantics, without
floating conversion or saturating endpoints. Parameters are considered anew on each execution. Analysis itself charges
work/checks cancellation; the one-table plan omits an unnecessary unit cross-product.

Arithmetic, unary parameter expressions, OR, mixed-column conjunctions, BETWEEN,
coercive/non-integer comparisons and predicates containing subqueries retain full
scans. Joins/derived relations receive no pushed outer constraint. In particular,
extracting only part of `X > 0 AND 1 / 0 = 0` would hide eager errors and is forbidden.
TOP, grouping and ORDER BY remain executor operations.

The Access adapter uses a constraint only with the explicitly selected matching
index. Other predicates retain its complete access path; an unsupported/corrupt
required index never causes a sequential fallback. Selected snapshots are not
cached as whole tables. Seeks validate visited pages/rows, not skipped integrity;
see [native index limits](NATIVE_INDEXES.md#native-int32-range-selections) and
[Int64 selections](NATIVE_INDEXES.md#native-int64-range-selections). Constraint
width must match the selected native key; a wrong-width host hint is declined
through the same required full index path.

Tests compare sources that honor versus ignore constraints, independent numeric
filtering over 256 fixed-seed ranges, parameter reuse/NULLs, eager overflow/division
errors, rejected callbacks and late source failures. Adapter tests count exact
selected root reads and verify subsequent full scans/caches. The
[query benchmark](testing/NATIVE_BENCHMARKS.md#query-filter-pushdown) measures
source construction, preparation, execution and sorting with constraints accepted
versus declined in the same build.

Int64 coverage adds a 256-case independent signed-range model, full-width literals
and rebound parameters, NULL/contradictory bounds, and eager mixed-float/unary
overflow failures. Adapter counters prove an exact >2^53 equality fetches one
matching root and that later complete scans/cache reuse do not retain the subset.
CLI tests preserve exact output and failure buffering. Original V2016/V2019
fixtures independently match 20 additional query intervals/profile against the
same cursor rows used for direct seeks. Native Access conformance remains unrun.

Bound BETWEEN and IN-list execution now delegates to SQL-independent scalar
predicate kernels shared with source-free evaluation and native pure-rule audits.
Eager candidate/comparison order, exact carriers, NULL/negation, explicit collation
and per-comparison work/cancellation behavior remain intact. Subquery evaluation
and its cardinality/security boundaries remain owned by the query executor.


## Explicit unavailable source fields

A Source may return stable unavailable field names separately from its scalar
columns through `unavailable_columns`; existing scalar-only hosts default to an
empty list. Combined names are bounded, unique and disjoint under the source
schema policy. Available columns define dense row positions and exact carriers;
unavailable names define no row values or NULL placeholders. A source with only
unavailable declarations may provide zero-width rows for COUNT(*) and constant
projections. Ordinary empty source schemas remain invalid.

Binding retains these names for ambiguity and unsupported-reference errors,
including correlated lookup and wildcard expansion. A matching unavailable field
makes `*`/`qualifier.*` fail rather than dropping columns. Explicit derived scalar
outputs have their normal independent schema. Unavailable metadata is copied into
the prepared table contract and compared again during execution; changes fail as
SchemaChanged before scanning. Join width and outer padding count only available
cells. Hosts must still propagate visitor failure and resource/cancellation controls.
This adds no collection values or special SQL coercions.


## Named retrieval sources

`Source::query(name)` optionally exposes borrowed, stable TableCraft-owned Query
metadata in the same source namespace as ordinary tables. Existing sources default
to None. Providers must reject ambiguous table/query names and must not scan user
records or activate providers/functions while resolving metadata. The binder
prepares named SELECTs as independent derived relations, preserving the original
name or explicit alias for field/wildcard qualification. Named sources are not
lateral. Joins, correlations outside the named definition, nested derived reads
and UNION operands use existing execution semantics. Bare saved-query UNION
references require an actual named definition; TABLE forms retain table support.
Non-read definitions and unsupported expressions/owner flags remain rejected.

Each named definition resolves only its own declared parameters, using the existing
field-before-parameter rules. Declarations merge into the execution binding list
by ASCII-insensitive name and equal validated TypeDescriptor, including text length.
Type spelling synonyms with equal descriptors can share a value; conflicting
carriers/constraints fail with a parameter error. One supplied value satisfies
compatible repeated declarations. Child-only parameters do not become implicit
parameters in a parent/sibling definition, and parent declarations do not make
undeclared child references valid. Missing/wrong/extra supplied values retain
ordinary execution errors; no parameter prompting or coercion is introduced.
This explicit bounded policy still needs native Access comparison evidence.

Active-path cycles return DependencyCycle before scans; repeated acyclic sources
are allowed. A separate 16-definition dependency-path ceiling bounds named nesting.
Each definition retains the 16-helper syntax depth ceiling; all expanded work and
allocations share the 2,048-node and 1 MiB planning limits. Expanded plans therefore
remain bounded even across names. Parameter/name/map reservations are fallible and
charged before allocation. UPDATE binding never resolves named query targets.

Preparation captures definitions in the owned executable plan; execution does not
reload or reinterpret their metadata. Reprepare when definitions change. Base table
schema/unavailable-field checks, row validation, query budgets and cancellation
remain in force. Hosts must maintain the stable source snapshot contract; this API
does not provide cross-session schema invalidation or native Access transactions.
The native adapter's lazy metadata cache and evidence are in
[NATIVE_SAVED_QUERIES.md](NATIVE_SAVED_QUERIES.md#nested-saved-select-sources).


`IsNull` and eager `IIf` now bind through shared expression kernels in outputs,
filters, sorting and supported aggregate arguments. Built-in arguments participate
in aggregate/group validation, correlation mapping and source projection pruning;
IIf retains both branches and their errors. Arity/qualification checks run during
binding, including on empty sources. UPDATE retains its no-call policy. See
[built-in contracts and evidence](EXPRESSIONS.md#explicit-isnull-and-iif-built-ins).

Query-context `Nz(value[, fallback])` uses the same bound traversal and shared
expression kernel. Both supplied arguments evaluate in order, even when the first
is non-NULL; absent fallback and explicit NULL remain distinct in the plan.
An omitted fallback replaces NULL with empty text, without introducing Empty or
numeric coercion. See [Nz scope](EXPRESSIONS.md#query-context-nz).

`Len`, `LTrim`, `RTrim`, `Trim`, `Left`, `Right` and `Mid` share bounded expression
kernels in retrieval. All argument fields, aggregate inputs and correlations stay
in the plan; text outputs consume the existing allocation budget and scans consume
steps/check cancellation. See [text semantics and gaps](EXPRESSIONS.md#bounded-text-built-ins),
including exact integer positions, UTF-16 boundaries and absent Variant coercion.

Year/Month/Day/Hour/Minute/Second also share scalar kernels, with Int16 results,
NULL propagation and full argument traversal. They accept the two explicit date
carriers; no text/numeric coercion is inferred. See [calendar and rounding scope](EXPRESSIONS.md#gregorian-date-component-built-ins).

DateAdd now binds all three arguments through shared eager calendar arithmetic,
including dynamic interval/count fields, aggregate arguments and correlations.
The original filter remains authoritative and unsafe partial predicate pushdown
cannot suppress date errors. See [DateAdd rules and current limitations](EXPRESSIONS.md#gregorian-dateadd).

DateDiff supports three required expressions and two optional week settings,
retaining omission separately from NULL. Every provided option participates in
binding/evaluation, projection pruning, correlation and grouping, including on
intervals whose boundary count is independent of week labels. Missing required
arguments and unsupported call shapes fail binding. [DateDiff rules and qualification
gaps](EXPRESSIONS.md#gregorian-datediff) record week/system policy and overflow.

DatePart supports two required expressions and two optional week settings with
shared Int16 component/week-number kernels. Every provided option participates
in aggregate detection, grouping and source/correlation mapping. Function and
aggregate binding is feature-sliced into binding/calls.rs, preserving retrieval
versus UPDATE gates and unsupported-call refusal. [DatePart scope](EXPRESSIONS.md#gregorian-datepart)
includes the documented Monday anomaly and unqualified native week boundaries.

CByte/CInt/CLng/CLngLng/CBool now bind through shared checked numeric conversion
kernels, preserving result widths, all argument fields and explicit NULL/overflow
failures. CByte's Boolean true exception is separate from signed numeric -1.
No locale-dependent input parsing or UPDATE call permission is introduced. See
[conversion scope and evidence](EXPRESSIONS.md#checked-numeric-and-boolean-conversion-functions).

CSng/CDbl use the existing shared conversion call node, preserving Single/Double
carriers and all argument/aggregate/correlation behavior. Explicit requested IEEE
rounding is separate from implicit PrecisionLoss checks. [Float conversion scope](EXPRESSIONS.md#explicit-single-and-double-conversions)
records signed zero, range/underflow rules, exact fixed-value input and native gaps.

## Explicit Currency conversion

CCur extends the existing Convert node, including requested source fields,
parameters, aggregate/group validation and correlated references. It returns the
Currency carrier with the [explicit conversion rules](EXPRESSIONS.md#explicit-currency-conversion).
It does not enable implicit Decimal arithmetic, Currency averages or different
comparison/promotion rules. NULL failures remain visible through eager evaluation;
unsupported calls and UPDATE calls remain rejected before execution.

## Explicit Decimal conversion

CDec uses the existing Convert planning/traversal node and full-range scalar
Decimal values, independently of declared native field precision. Grouped,
correlated, parameter and source-field validation retain their existing rules;
wrong NULL/input shapes fail explicitly and UPDATE calls remain blocked.
[Conversion contracts](EXPRESSIONS.md#explicit-decimal-conversion-and-expression-range)
distinguish exact inputs from the unqualified normalized floating profile.
Decimal arithmetic and aggregates now use the checked kernel below.

## Decimal arithmetic and aggregates

Retrieval and qualified saved SELECT expressions now use the shared
[checked Decimal arithmetic](EXPRESSIONS.md#checked-decimal-arithmetic) for
projection, predicates, sort keys, aggregate arguments and correlated references.
The existing Sum/Avg fold automatically retains Decimal results once a Decimal
input participates; Count/Min/Max retain their existing rules. NULL inputs are
ignored, empty/all-NULL sums/averages return NULL and Count returns zero. Group
validation, source projection/caching, eager errors and resource limits remain.
SQL duplicate removal compares values even when division retains another scale.

Aggregates still fold in input order with bounded intermediates; there is no
whole-group carrier inference or arbitrary-precision accumulator. A preceding
Int64 subtotal can overflow before a later Decimal input promotes the result.
Bounded Decimal intermediate rounding can also make reassociation observable.
Native Access aggregate result types, promotion and edge rounding remain
unqualified. Decimal/float comparison policy is separate from arithmetic mixing.


## Computed group keys and encounter aggregates

GROUP BY accepts supported pure bound expressions such as `Len(Label)`,
`Year(Stamp)` or `Amount Mod 2`. Projection, HAVING and sorting can use the same
bound expression or compose it with other valid grouped expressions. Field and
parameter slots and resolved function kinds establish structural membership;
qualified/unqualified references to the same field agree. No algebraic rewriting
or locale-based expression equivalence is inferred: grouping on `A+B` does not
make `A` or `B` separately available. Aggregates and subqueries remain invalid
grouping keys; grouped subquery projections retain their existing limitation.
Validation work uses the binder's shared node/depth/byte ceilings. Existing source
projection pruning, evaluator errors, runtime grouping comparisons, cancellation
and resource limits remain in force.

`First(expr)` and `Last(expr)` return the first/last encountered operand value,
including NULL. A global empty group yields NULL. Operand values keep their exact
carriers, including full Text/Binary values; no collation is required merely to
return them. Every group operand still evaluates in encounter order, retaining
later expression errors and cancellation rather than silently stopping at First.
Nested aggregates, wildcard arguments, extra/missing arguments and qualifiers are
rejected while binding, including for empty sources. These are retrieval functions;
source-free scalar `query eval` and restricted UPDATE keep their existing profiles.

Encounter order follows the chosen host source/derived relation. Final ORDER BY
sorts aggregate result rows; it does not choose an endpoint inside each group.
An explicitly ordered derived input supplies its resulting encounter order.
This is separate from native Access optimizer/index-order qualification. Microsoft's
[First/Last reference](https://learn.microsoft.com/en-us/office/vba/access/concepts/miscellaneous/first-last-functions-microsoft-access-sql)
distinguishes these functions from minimum/maximum values; the
[GROUP BY reference](https://learn.microsoft.com/en-us/office/vba/access/concepts/structured-query-language/group-by-clause-microsoft-access-sql)
records native grouping and field restrictions. Native field-type restrictions,
coercion, collation and optimizer/evaluation-order fidelity remain unqualified.

Five authored/generated tests cover computed grouping/HAVING/sorting, bound field
aliases, invalid ungrouped dependencies, NULL/empty endpoints, ordered derived
inputs, eager failures and independent integer partitions. The registered ACE 2007
sample passes eight CLI controls derived from independent read-only Jackcess
sequential and explicit-index rows. This is a row-based reference model, not an
Access SQL execution oracle; source hashes remain unchanged.

```sh
tablecraft query run 'SELECT Len([NBR_VALUE]) AS Width, Count(*) AS Total FROM ROULETTE_TB GROUP BY Len([NBR_VALUE]) ORDER BY Width' --database docs/examples/Access_Example_VBA.accdb
```


## Top-level crosstabs

Top-level TRANSFORM/PIVOT retrieval now supports bounded fixed/discovered Text or
integral headings, grouped row totals, WHERE/HAVING, parameters and row ordering.
Preparation retains metadata-only binding; execution buffers dynamic names and
all cells before publication. The source pruner retains pivot dependencies, and
heading discovery prevents unsafe WHERE pushdown. [Crosstab contracts](CROSSTABS.md)
record NULL/missing cells, name collisions, 256-column/resource ceilings and
explicit native naming/format/collation/saved-query gaps. Named/derived crosstabs
remain unsupported rather than acquiring a guessed schema.

## DISTINCTROW source records

The bounded DISTINCTROW implementation follows Microsoft's
[UniqueRecords contract](https://learn.microsoft.com/en-us/office/vba/access/concepts/miscellaneous/uniquerecords-property)
and [SELECT predicate reference](https://learn.microsoft.com/en-us/office/vba/access/concepts/structured-query-language/all-distinct-distinctrow-top-predicates-microsoft-access-sql).
With output fields from some but not all joined sources, duplicate comparison uses
**all fields of each selected source record**, including unprojected fields. It
uses full scalar values and the explicit host collation, not physical row ordinals,
index key prefixes or visible output alone. Identical full records collapse;
records differing in a hidden field remain separate even when output values match.
NULL record fields compare together through the existing duplicate policy.

A single source, no source, or projection using fields from every source ignores
DISTINCTROW. Each table alias counts as its own source; grouped joins retain leaf
source ranges independently of their exposed aliases. Active keys support ordinary
scalar tables and pure scalar projection expressions. Complete selected records
survive projection pruning, while unselected tables retain ordinary field pruning.
Outer-join padding remains NULL. Binding reads only metadata; unavailable fields in
a selected record reject active DISTINCTROW rather than becoming placeholder NULLs.

Projection/sort expressions still evaluate for every qualifying joined row before
duplicate removal, preserving late errors. Duplicate removal precedes ORDER BY and
TOP count/PERCENT, including complete output-order ties. UNION arms retain their
own keys; UNION then applies its usual output-value duplicate policy. The same
shared allocation/work/cancellation limits apply to hidden fields and comparisons.

Active DISTINCTROW over aggregates/grouping, subquery projection expressions,
derived/named selected records, or projections referencing no local source is
unsupported. Its ORDER BY accepts output aliases/ordinals only, avoiding arbitrary
sort keys from discarded joined records. Full native Memo/OLE comparison rules,
record/NULL-extension edge cases, locale equality, native optimizer ordering and
Windows Access comparisons remain unqualified. This is a documented scalar
execution policy, not native Access compatibility evidence.

Eight authored/generated tests cover hidden fields, duplicate entire records,
NULL/text comparison, outer/nested joins, ignored cases, errors and TOP/UNION. A
projection-provider test ensures hidden selected fields cannot be silently pruned.
The [reference-row recipe](testing/compare_distinctrow.py) independently computes
semijoin record sets and visible multiplicities from registered native rows.
