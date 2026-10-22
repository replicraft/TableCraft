# Bounded retrieval execution

LCase/UCase now use an explicit execution-local language and streamed ICU output;
see [text casing](TEXT_CASING.md) for policies, limits and pending native qualification.

`crates/query/tablecraft-query` implements read-only preparation and execution over
explicit host sources. `crates/foundation/tablecraft-schema` supplies validated
[scalar schema contracts](SCHEMA.md) with an actual query consumer. Neither package
opens databases, implements storage transactions or imports application hosts.
This is a usable bounded retrieval profile, not complete Access SQL compatibility.
The separate [query editing API](QUERY_EDITING.md) plans bounded single-table
UPDATE actions over shared snapshot contracts. Retrieval APIs remain read-only.

[CVErr Error data](VARIANT_ERRORS.md) now supports explicit inspection, eager
selection and numeric consumers. `PreparedQuery::execute_expression` returns
`ExpressionQueryResult` with explicit Error cells, including through derived rows
and subqueries; the CLI uses this path. `execute` retains checked scalar Value
publication. Broader operator/aggregate propagation remains implementation work.

[Integer logical operators and numeric predicates](INTEGER_LOGIC.md) now preserve
bit patterns until final WHERE/HAVING/ON selection, including collection criteria.
Rounded float/fixed/date operands now use checked Long/LongLong targets;
Text operators and final predicates share the explicit NumberFormat and budgets.
Full Variant/subtype inference and native Access qualification remain open.

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

## Source text-policy identity

`Source::text_collations(table)` optionally exposes opaque `TextCollationId`s,
aligned with available scalar columns. `multivalue_text_collations()` and
`collection_member_text_collations()` supply the same optional metadata aligned
with collection elements and named members. Only Text elements/members may
carry identities; binding retains their provenance through expanded fields and
hidden membership items, including correlated scopes. The default empty slice supplies no
metadata. Otherwise the length must match `columns()` and only Text columns may
have an identity; None means no explicit field policy. Malformed declarations
fail preparation as source errors, including for unused fields. Policy discovery
must read metadata only and never activate provider/query content.

Identities belong to the host's stable Source policy namespace. Query code never
decodes native LCIDs/versions, guesses locale from them or imports a concrete
comparer. A host combining providers must remap identities consistently. Every
opaque key is representable, including unknown policies; keys do not qualify
comparison semantics or form portable database IDs.

Binding owns a budgeted copy beside the original table schema. Projection pruning
retains complete policy metadata, and ordinary, expanded, parent and joined-parent
execution checks it before invoking row visitors. Changed identities, presence or
layout fail as `SchemaChanged`; provider failures propagate without partial output.
Checks also apply through named/derived queries, unions, grouping and subqueries.
The Access adapter now publishes exact text descriptor identities and checks them
against newly read full/projected snapshot schemas. Other sources retain the
empty metadata default.

This is the working metadata/stability boundary needed by per-expression
resolution. The lineage model below carries policies through selected expression
results; the explicit resolver now routes comparisons, while native integration
and native mixed-policy precedence remain unfinished. Existing `ExecutionOptions::collation` stays
explicit; unknown identity metadata permits projection but does not enable text
comparison. Native `--collation database` now composes the explicit resolver.

## Bound expression text-policy lineage

`PreparedQuery::output_text_policies()` exposes static output provenance without
reading rows or activating comparison. Ordinary and fixed-width crosstab schemas
return one `TextPolicy` per output; dynamic crosstabs return None because their
width depends on values. This is provenance, not full result-type inference or an
effective execution setting. Global explicit comparison overrides remain unchanged.

| Provenance | Meaning |
| --- | --- |
| `Default` | No explicit contributing field policy; host default remains applicable |
| `Column(id)` | Exactly one contributing field identity, possibly through aliases |
| `Conflict` | Distinct contributing identities; no precedence has been guessed |

The current provisional model carries field policies through ordinary/qualified
references, wildcards, aliases, derived/named sources and correlated scalar
subqueries. Concatenation and text-capable `+` merge operand provenance; `IIf`
merges result branches, excluding its condition; `Nz` merges value/replacement.
First/Last/Min/Max retain their argument's provenance. UNION merges corresponding
outputs across all branches. Default is neutral, equal identities remain one,
and distinct identities yield Conflict. Fixed crosstab cells repeat the measure's
provenance; row headings retain their projection policy. Column and ORDER BY
output slots retain identity when pruning rewrites dense positions.

Literals/parameters, Boolean/numeric results and built-in/conversion results
use Default. In particular, functions
such as Left/LCase/CStr do not yet inherit a field policy in this model. This is
an explicit provisional choice pending function-level/native evidence; it does
not establish native Access expression precedence. Static conflicts do not block
projection or override an explicitly supplied global comparer.

Policy vectors reserve fallibly under the bind allocation ceiling. A separate
visit ceiling derived from existing bind depth/node limits bounds lineage walks;
they do not evaluate expressions, inspect rows or invent dynamic schemas. The
original owned syntax/branches remain available for later precedence refinement.
The explicit resolver below routes comparison operations and now has a native
provider/CLI consumer. Native mixed-policy precedence remains unfinished.

## Explicit per-operation collation resolver

Library hosts may supply `ExecutionOptions::collation_resolver` implementing
`CollationResolver`. It resolves exactly one opaque field identity to a borrowed
comparer and must fail unknown keys, preserve its stable policy namespace and
own cumulative callback budgets/cancellation. Query code imports no concrete
engine, adapter or locale implementation. Omitting the resolver preserves the
existing global `collation` override; with a resolver, that same field supplies
the host-default comparer. String built-ins keep that explicit default/database
policy rather than choosing a field policy from their arguments.

Actual Text/Text comparisons merge bound provenance and select the resulting
Column policy. Default uses the host default; distinct contributing policies fail
as payload-free `CollationConflict`. Unknown-key failures propagate without a
default fallback. NULL/nontext operations retain their existing guards, so inert
unknown-policy projection, NULL comparison and numeric operations do not activate
resolution. LIKE retains its explicit mode requirement before policy callbacks.
Type errors and eager argument/list/subquery evaluation remain explicit.

Routing covers scalar comparisons and LIKE, BETWEEN, list/subquery membership,
ANY/ALL, join/HAVING predicates, grouping, DISTINCT/DISTINCTROW, UNION de-duplication,
sorting/TOP ties, min/max and crosstab pivot matching/heading validation/ordering.
Tuple operations select each key's policy. UNION de-duplication uses contributing
branches up to that operation, so a later UNION ALL arm cannot retroactively
change an earlier comparison; final output/sort provenance still includes all arms.
Crosstab pivot comparisons use the pivot's bound policy while row grouping retains
its own keys. Original filters remain; no text-index pushdown is enabled.

Lineage traversal and callback boundaries check query work/cancellation. Temporary
policy vectors reserve fallibly and charge the query allocation allowance; hosts
also retain their own provider quotas across calls/executions. Callback resolution
uses a payload-free tracing span with no keys, SQL, names or values. Results remain
buffered; failures/cancellation return no partial QueryResult.

This is an explicit provisional host-policy model, verified with independent
folded/reverse/ordinal test providers. It does not qualify native Access expression
precedence. The Access adapter now composes a closed General resolver with one
shared quota; native CLI `--collation database` routes supported per-field policies
and defaults through it. Unknown required field policies/conflicts fail, while
projection remains inert. [Native policy scope](NATIVE_TEXT.md#stored-database-general-policy)
records that consumer. Query collection policies now route through the same
resolver; the native adapter now supplies stored child identities and checks
them against complete snapshots, enabling CLI database-mode collection queries.

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

Explicit host sources can additionally expose [scalar multivalue `.Value`
expansion](MULTIVALUE_QUERIES.md). Native hosts opt in through
`NativeSource::with_multivalues()` or CLI `query run --database PATH --multivalues`.
Direct single-table scalar-parent WHERE membership additionally retains parent
cardinality, with explicit nullable set semantics and scope limits.
Scalar-only parent WHERE membership also composes across qualified joins under
[explicit joined-parent rules](MULTIVALUE_QUERIES.md#parent-membership-over-joins),
including separate absent-parent, empty-list and stored-NULL behavior.
[Mixed displayed/hidden collection criteria](MULTIVALUE_QUERIES.md#displayed-children-with-independent-parent-membership)
combine aligned child rows with independent parent folds, including joined aliases
and displayed scalar inputs to hidden-member atoms. This remains an explicit
TableCraft profile pending native Access qualification.
[Crosstab WHERE membership](CROSSTABS.md#collection-membership-criteria) also
retains these rules; pivot-only dependencies expand children, and dynamic headings
are discovered before membership without reevaluating pivots.
[Membership subqueries](MULTIVALUE_QUERIES.md#subqueries-inside-membership-criteria)
now preserve scalar/set/quantified and nested correlated item scopes under the
same budgets, eager errors and explicit empty/NULL/absence policy.
[ON member fields](MULTIVALUE_QUERIES.md#collection-members-in-join-conditions)
now drive child-row expansion even without output projection, preserving matching
child duplicates and existing outer-join behavior. WHERE sibling fields remain
aligned, while independent hidden collections keep their membership folds.
[Several hidden members in one atom](MULTIVALUE_QUERIES.md#several-hidden-members-in-one-atom)
now align by collection/alias or use bounded actual-item products, preserving
parent cardinality and scoped tuple captures.
Named collection members additionally support unprojected siblings of an already
displayed/grouped collection in child filters/sorts and valid HAVING/joins;
[the identity and pruning rules](MULTIVALUE_QUERIES.md#unprojected-members-of-displayed-collections)
preserve qualifier scopes and keep wholly hidden independent collections explicit.

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
  [numeric built-ins](MATH_FUNCTIONS.md), [DATE serial operators](DATE_ARITHMETIC.md), IS NULL, reversed-bound
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
kernel below. [Wider means](AVERAGES.md) now support Currency averaging without
a source-width intermediate total; full native Variant promotion remains
unsupported. Min/Max use the existing comparison rules.

## Explicit unsupported cases

Action/DDL/make-table statements and
external IN and OWNERACCESS are rejected before scans. So are identity,
object/unsupported complex-field access and scalar function
calls outside the explicit NULL/conditional, numeric, text and date built-ins (including VBA/UDFs). Other Access built-ins, locale conversions and
GUID/binary ordering remain absent. Qualified date/GUID literal materialization
and date comparison follow the [scalar contracts](EXPRESSIONS.md). [Grouped subqueries](GROUPED_SUBQUERIES.md) now allow key-safe correlations;
[DISTINCT projected ordering](QUERY_ORDERING.md) now accepts structurally matching
stable scalar/aggregate expressions as well as output references/ordinals.
ORDER BY expressions involving output aliases inside larger expressions, unselected
DISTINCT keys and repeated volatile/opaque/read-plan identities remain outside this
profile. Unsupported forms are never
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
explicit ordinal/provisional General collation and buffered output follow the
[CLI contract](DEVELOPMENT.md#inline-retrieval-cli). Named tables are exposed to
library hosts through Source; optional `--database PATH` supplies supported native
scalar snapshots through the separate adapter described below. Native-only
`--collation database` composes stored General default/per-field policies through
that adapter's shared-budget resolver. Required unknown identities and conflicts
fail without fallback. This adds no native types/I/O or inferred native expression
precedence to the executor.

The CLI now optionally supplies native scalar tables through `--database PATH`.
[NATIVE_QUERIES.md](NATIVE_QUERIES.md) records the separate `tablecraft-access`
Source adapter, snapshots, budgets and format/type limitations. The query library
still has no concrete native I/O, engine or adapter dependency.

The explicit General comparison adapter and CLI choices are documented in
[NATIVE_TEXT.md](NATIVE_TEXT.md#general-query-policy). The supplied callback owns
a cumulative allocation/work budget; selecting it does not infer field locales
or enable text index pushdown, native LIKE units or native Access qualification.

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
prepares named SELECT/UNION queries as independent derived relations, preserving the original
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
NULL propagation and full argument traversal. Explicit date carriers retain their
precision; supported numeric/Text sources now share the evaluator's
[bounded CDate conversion](EXPRESSIONS.md#date-component-source-coercion). See [calendar and rounding scope](EXPRESSIONS.md#gregorian-date-component-built-ins).

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
including NULL and explicit [Error data](VARIANT_ERRORS.md#encounter-order-aggregates-and-domains).
A global empty group yields NULL. Operand values keep their exact
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
explicit native naming/format/collation gaps. [Native saved crosstabs](NATIVE_SAVED_CROSSTABS.md)
now reconstruct into this executor. [Named crosstabs](NAMED_CROSSTABS.md) with
literal or qualified policy-independent constant IN headings also bind as sources
without record scans. The [explicit execution-stage service](NAMED_SCHEMA_DISCOVERY.md)
now resolves qualified parameter/policy/data-dependent relational schemas. Standalone
prepare still refuses them. Parameterless runtime domain targets now use the same
staging; inline TRANSFORM and wider native semantics remain unfinished.

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
scalar tables, complete logical derived/named records and pure scalar projection
expressions. A derived record includes every column exposed by that query, even
when hidden by the enclosing SELECT; base-table columns omitted inside the derived
query are outside that logical record. UNION, grouped and fixed/discovered crosstab
outputs use the same complete logical-row boundary. Complete selected records
survive projection pruning, while unselected tables retain ordinary field pruning.
Outer-join padding remains NULL. Binding reads only metadata; unavailable fields in
a selected record reject active DISTINCTROW rather than becoming placeholder NULLs.

Projection/sort expressions still evaluate for every qualifying joined row before
duplicate removal, preserving late errors. Duplicate removal precedes ORDER BY and
TOP count/PERCENT, including complete output-order ties. UNION arms retain their
own keys; UNION then applies its usual output-value duplicate policy. The same
shared allocation/work/cancellation limits apply to hidden fields and comparisons.

Active DISTINCTROW over aggregates/grouping in the enclosing query, subquery
projection expressions or projections referencing no local source remains
unsupported. Its ORDER BY accepts output aliases/ordinals and
[matching projected expressions](QUERY_ORDERING.md), avoiding arbitrary sort keys
from discarded joined records. Expression-only Error/Empty/byte-string payloads
cannot become scalar record keys; encountered unsupported states fail rather than
becoming NULL. Dynamic scalar carriers such as arithmetic results remain valid
when their actual values support the existing duplicate comparison policy. Full native Memo/OLE comparison rules,
record/NULL-extension edge cases, locale equality, native optimizer ordering and
Windows Access comparisons remain unqualified. This is a documented scalar
execution policy, not native Access compatibility evidence.

Fourteen authored/generated tests cover hidden fields, duplicate entire records,
NULL/text comparison, outer/nested joins, ignored cases, errors and TOP/UNION. A
projection-provider test ensures hidden selected fields cannot be silently pruned.
The [reference-row recipe](testing/compare_distinctrow.py) independently computes
semijoin record sets and visible multiplicities from registered native rows.
It now passes 26 sequential/indexed controls including complete/partial/computed,
UNION and grouped derived records. Focused library controls additionally cover
named definitions, fixed/dynamic crosstabs, metadata-only binding, Error/late
failure and inherited per-field text policy. Original synthetic Jet 4/ACE 2007/
ACE 16-family reads check derived keys through the native adapter. This establishes
our logical-row profile, not native Access optimizer flattening, physical row
identity or updateability of derived recordsets.

`DateSerial` and `TimeSerial` now bind closed three-argument calls through shared
[Gregorian constructors](DATE_CONSTRUCTORS.md). Operand dependencies, grouping
identity, pruning, correlations and parameter reuse retain the normal bounded
query paths. Omitted execution options use the documented fixed year window; hosts can supply
a validated `ExecutionOptions::year_window`, including on reused plans and nested
queries. CLI eval/run exposes `--two-digit-year-max YEAR`. No host clock or machine
preference is read implicitly. UPDATE call support is unchanged.

## Explicit StrComp comparison modes

`StrComp` shares checked mode selection and Int16/NULL results between scalar
evaluation and retrieval. Binary UTF-16 comparison needs no host; text/database
and omitted default modes require explicit comparison policies. The CLI ordinal
policy supplies database/default comparison, not locale text comparison.
[Comparison contracts and remaining native limits](STRING_COMPARISON.md) describe
argument conversion, eager errors, metered scans and independent evidence.

## Bounded substring search

`InStr` and `InStrRev` share UTF-16 position scans and explicit search providers.
Forward comparison defaults require a host policy; reverse comparison defaults
to binary. Optional slots retain their function-specific meaning, and provider
matching supports unequal-length locale spans without equating search with
ordering. [Search contracts and evidence](STRING_SEARCH.md) record current
arguments, budgets, SQL integration and native qualification gaps.

## Bounded text replacement

`Replace` now uses a shared borrowed recipe and bounded streaming output across
scalar SELECTs and retrieval. Optional start/count/comparison slots retain their
own defaults; results return only the requested suffix, and inserted text is not
searched again. [Replacement contracts](TEXT_REPLACEMENT.md) describe output-byte
charging before fallible growth, provider-span validation and native limits.

## Bounded repeat and reverse functions

`Space`, `String` and `StrReverse` share metered construction recipes with the
existing bounded text-output sink. Current counts, BMP/ASCII character selection,
NULL rules and surrogate/code-page limitations are explicit in
[construction contracts](TEXT_CONSTRUCTION.md). Eager arguments, grouped/correlated
execution, parameter reuse and buffered errors retain their ordinary query rules.

## Execution-local character codes

[Explicit character-code pages](CHARACTER_CODE_PAGES.md) now extend numeric
`String` byte codes beyond ASCII under a validated per-execution setting.
Unselected high bytes and unmapped selected-page bytes fail explicitly. Prepared
plans, nested calls and worker requests retain isolated settings; existing Text
and native storage decoding remain separate.

## Character-code functions

[Chr/Asc and ChrW/AscW](CHARACTER_FUNCTIONS.md) now share checked single-byte
code-page and signed UTF-16 profiles across scalar evaluation and retrieval.
Character output charges UTF-8 bytes before allocation; numeric codes retain
Int16. Unmappable selected-page input and unrepresentable surrogate output fail
without replacement. Full coercion/DBCS/native qualification remain incomplete.

[Val](VAL.md) now parses bounded Text numeric prefixes with explicit radix and
type-suffix checks. Results use the current finite Double profile; this does not
enable general implicit Text-to-number coercion or establish native compatibility.

[Hex, Oct and Str](NUMERIC_TEXT.md) now provide numeric-to-Text recipes with
checked conversion, NULL propagation and charged output. Exact fixed values are
preserved; native negative widths, float spelling and broader coercions remain
unqualified. This does not replace CLI rendering; [CStr](CSTR.md) has its own
explicit output policies and general Format remains unimplemented.

[NumberFormat](NUMBER_FORMAT.md) now controls complete numeric Text input to the
nine existing conversion calls. ExecutionOptions carries it through nested calls;
prepared plans retain no locale setting. Source carriers, collation and native
selection/index semantics are unchanged. No automatic host locale is selected.

[IsNumeric](ISNUMERIC.md) now recognizes materialized numeric/Boolean types and
whole numeric Text under the selected NumberFormat. NULL/date/binary/GUID values
return FALSE. Malformed/overflowing numeric Text is separate from propagated host,
allocation, cancellation and limit failures. It does not change eager evaluation.

[CStr](CSTR.md) reuses the existing TextCall node, including field/aggregate/group
and correlation traversal. ExecutionOptions.date_format is separate from numeric
formatting and year-window controls; preparation captures neither policy. No date
or numeric conversion is pushed into native storage/SQLite or enabled during inspection.

[Date conversions](DATE_CONVERSIONS.md) use typed DateConvert nodes with complete
argument dependency traversal. ExecutionOptions.reference_year is shared across
nested queries while prepared plans capture no clock/locale context. Date parsing
validates both components before projection; no new calls are enabled in UPDATE.

[IsDate](ISDATE.md) now recognizes supported Gregorian dates/times with explicit
input/year policies and checked date/numeric carriers. NULL returns FALSE; missing
policies and operational failures remain errors. Full regional/Variant behavior
and native qualification remain open.

[FormatDateTime](DATE_FORMATTING.md) now supplies all five named modes under
explicit Gregorian profiles. It shares CStr rendering and CDate input policy,
with private charged normalization and request-local year settings. Full regional/
Variant behavior and native formatting precision/error qualification remain open.

[MonthName and WeekdayName](CALENDAR_NAMES.md) now provide explicit English/German
full/short names through typed Text nodes. WeekdayName system/default numbering
requires caller-issued FirstDayOfWeek; SQL first-day arguments override it. Settings
remain execution-local. Full regional/Variant and native NLS qualification remain open.

[Host week settings](WEEK_SETTINGS.md) now resolve explicit system-zero options in
DatePart/DateDiff. Omitted options retain Sunday/Jan-1 defaults; concrete SQL options
override host settings. First-day and first-week values remain execution-local,
including nested reads and optional worker requests. Native NLS qualification and
DateDiff w non-Sunday anchors remain open.

[Standalone Weekday](WEEKDAY.md) now supplies checked Gregorian relative numbering
with numeric/Text/date input, optional first-day expressions and caller-issued system
context. Its owned plan node retains date/option dependencies, grouping identity,
aggregates, correlations and qualified multivalue predicates. Native NLS/Variant/
calendar, precision and error qualification remain open.

[FormatNumber/FormatPercent/FormatCurrency](STYLED_NUMBER_FORMATTING.md) now provide
fixed-place output under explicit NumberFormat profiles, checked optional controls
and private normalization. Exact fixed values and bounded normalized IEEE magnitudes
use the shared formatter; output settings remain request-local. Full NLS/Variant
behavior and native precision/rounding/error qualification remain open.

[Date()/Time()/Now()](CLOCK_FUNCTIONS.md) now use caller-issued clock providers.
Each executed call reads once; binding/inspection never read clocks. Callback failures
remain eager, plans capture no readings, and volatile group keys do not imply
structural equivalence. CLI hosts own Jiff system time/zone discovery and --clock
fixed overrides; libraries/worker requests require explicit providers. Native
precision, platform timezone and invocation/grouping qualification remain open.

The Access adapter now supplies [native saved UNION definitions](NATIVE_SAVED_UNIONS.md)
to this existing retrieval path; the generic query layer has no native metadata dependency.

Crosstab TOP now shares ordinary SELECT's positive count/percentage and complete
ORDER BY tie selection after full row/cell evaluation. Heading columns remain
complete and omitted groups cannot hide errors or bypass budgets. TOP 0, owner/
DISTINCT options and dynamic or context-dependent named schemas retain explicit refusals. See the
[TOP group contract](CROSSTABS.md#top-row-groups) for checks and native scope.

## Literal prompt references

[Bracketed parameter prompts](PARAMETER_PROMPTS.md) may contain punctuation such
as periods and bang characters without becoming qualified fields or control paths.
Existing input-field precedence, exact declared carriers, prepared-plan reuse and
isolated named scopes remain. Values are supplied explicitly; unknown references
do not prompt or acquire application authority.


## Zero TOP counts

The binder retains its positive-count TOP profile and rejects zero before row
scanning. Syntax-only parsing and native metadata reconstruction may preserve
`TOP 0` for inspection; representability is not execution support or validation
by Microsoft Access. This is not a planned SQL Server-style empty-schema query.
Microsoft's [TOP reference](https://learn.microsoft.com/en-us/office/vba/access/concepts/structured-query-language/all-distinct-distinctrow-top-predicates-microsoft-access-sql)
documents integer counts and positive examples, while a
[Microsoft-hosted Access report](https://learn.microsoft.com/en-us/answers/questions/5352798/error-with-select-statement-that-has-never-happene)
identifies TOP 0 as rejected by Access. The latter is observed community evidence,
not a normative grammar or a TableCraft Windows qualification run. The earlier
roadmap designation of zero-count execution as a missing feature was incorrect.
Existing zero/PERCENT refusals remain; native edge-case qualification is deferred.


## Statistical aggregates

Var/VarP/StDev/StDevP now use the ordinary aggregate binder/executor over finite
numeric/Boolean values. NULLs are omitted; each operand evaluates once, with
fallible budgeted numeric retention and compensated shifted/scaled centered passes.
Results are Double or NULL under the [function-specific count rules](STATISTICAL_AGGREGATES.md).
Grouping, HAVING/sorting, subqueries, crosstabs, collections and qualified native
saved/named retrieval use the same implementation. Runtime carrier errors,
overflow, late failures and cancellation remain buffered. Domain statistical
functions reuse these kernels through the bounded [domain profile](DOMAIN_FUNCTIONS.md).
Native arithmetic/Variant precision qualification remains separate.

## Read-only domain retrieval

The twelve [domain functions](DOMAIN_FUNCTIONS.md) now resolve runtime expression,
domain and scalar criteria strings through the selected source under the same
query budgets. Parameterless named reads work; dynamic source resolution occurs
only during execution. Missing criteria fields return NULL, while unsupported
syntax and source failures remain errors. Broader native criteria, collection,
coercion and host behavior remain incomplete.

## Financial annuities

Pmt/PV/FV now bind as closed scalar calls through the shared
[financial kernel](FINANCIAL_FUNCTIONS.md), including optional zero defaults,
aggregate arguments, grouping, correlated reads, domains and native saved/named
queries. Operand dependencies participate in existing pruning and scope checks.
They return Double and retain eager evaluation and buffered failure semantics.
Full native period/coercion/rounding behavior remains unqualified.

[NPer/IPmt/PPmt](PERIOD_PAYMENTS.md) now extend financial dispatch to period counts
and payment components, including function-specific required slots and six-argument
calls. Native saved/named queries use the same bounded kernels and eager failures.

[SLN/SYD/DDB](DEPRECIATION.md) extend the same closed financial calls to depreciation.
DDB's omitted factor defaults to two; annuity optional balance/timing defaults
remain zero. Queries retain eager errors, argument dependencies and buffered
publication. Native edge/rounding qualification remains separate.

[Rate](RATE.md) adds guess-directed annuity solving under the same argument scopes
and cumulative execution budget. Twenty-iteration exhaustion is an explicit
convergence error; each iteration and successful publication check cancellation.
Optional guess defaults to 0.1 and never substitutes for explicit NULL.

## Explicit periodic cashflows

[NPV/IRR/MIRR](CASHFLOW_FUNCTIONS.md) now consume bounded explicit numeric
`Array(...)`/`VBA.Array(...)` inputs under shared allocation/work/cancellation
limits. Every zero and period is retained; supplied expressions evaluate in
written order before conversion. Arrays remain financial inputs rather than stored
scalars or general query results. General Variant arrays, native SQL admissibility,
coercion and numerical equivalence remain unqualified.

## Type/state inspection and array bounds

[IsArray/IsEmpty/IsError/TypeName/VarType/LBound/UBound](VALUE_INSPECTION.md)
now consume the explicit materialized profile, including direct Array/VBA.Array
intermediates. NULL, Empty and Error remain distinct; operational failures are
not caught. Type names use the normal bounded Text output path; binding remains
metadata-only. General arrays, native-only subtype mappings, native SQL admission
and broader Variant/COM behavior remain explicit gaps.

## Materialized CVar identity

[CVar](CVAR.md) retains an explicit unary plan node so ordinary evaluation,
volatility, dependency walks and budgets still apply. It returns the operand's
materialized carrier unchanged and preserves static Text policy provenance.
It does not admit expression-only arrays or create a VBA Variant variable.

`StringFromGUID` now traverses the same bound field/group/aggregate/correlated
dependency paths as GUIDFromString. Its [provisional inverse profile](GUID_CONVERSIONS.md#stringfromguid-inverse-conversion)
converts stored Guid fields or sixteen GUID-structure Binary bytes to fresh Text.
Source schemas and scalar parameters retain their existing carriers; there is no
implicit stored Guid/Binary comparison or native-query qualification claim.

[Closed dynamic Eval](DYNAMIC_EVALUATION.md) binds an explicit argument child,
retaining parameter/field/group/aggregate/correlated and collection dependency
walks. The runtime passes materialized input into the shared scalar evaluator,
retaining its execution context and operation budget. Parsed names do not capture
query rows/parameters or open domains. Runtime group/pivot keys require repeatable
dynamic bodies; hidden clock/Rnd calls fail before provider use. Results have
fresh host-default text provenance, and source constraints retain the original
filter rather than extracting a partial predicate around dynamic work.

[CurrentUser](SESSION_FUNCTIONS.md) now uses the caller's optional
ExecutionOptions::access_session. The prepared leaf node stores no identity and
can execute against different supplied contexts. Grouped/derived/named/correlated,
domain and closed Eval execution retain that immutable data. It is repeatable
within an execution, with fresh host-default Text provenance and normal collation/
allocation requirements. This expression metadata grants no authentication or
permissions; ordinary library defaults leave the session absent.

## Numbered diagnostic messages

[Error(number) and Error$(number)](ERROR_MESSAGES.md) now use the explicit
execution-local ErrorMessages provider and shared Text limits. Zero returns empty
Text without a catalog; valid undefined codes use the provider's selected
fallback. The CLI selects optional Fluent-backed candidate English data.
Missing providers, failed providers and operational errors remain distinct.
Omitted latest-runtime-error state remains deferred; native Access wording,
code coverage, locale and sandbox qualification remain open.

## Legacy DATE numeric conversions

The nine explicit numeric/Boolean conversion calls now consume
[stored legacy DATE serials](DATE_NUMERIC.md) through the same checked function
kernel in all query contexts. Source schemas/parameters retain Date identity;
results use the requested destination carrier. NULL conversion remains an error,
including inside eager AND. Extended numerical conversion remains unimplemented.

## Conditional conversion context

[IIf/Switch/Choose selector inputs](CONDITIONAL_FUNCTIONS.md#date-text-and-explicit-expression-states)
now use legacy DATE serials and the execution's explicit numeric Text policy.
Grouping, domains, derived/correlated queries and closed Eval share that policy;
plans capture neither policy nor values. Selected Text/Error payloads remain
unchanged. Semantic conversion errors are eager; operational failures stop before
later callbacks. Ordinary predicate/logical coercion retains its existing profile.

[Left/Right/Mid count coercion](SLICE_COUNTS.md) now reaches ordinary,
derived/correlated, parameterized and closed Eval retrieval through the shared
text evaluator. Numeric/legacy DATE counts round to Long; SQL numeric Text counts
require NumberFormat. Source Text/NULL, Unicode and native qualification limits
remain unchanged.

[Repetition counts](TEXT_CONSTRUCTION.md#contextual-repetition-counts) and
[search starts](STRING_SEARCH.md#contextual-search-starts) now use shared checked
DATE/numeric conversion and explicit-policy SQL Text normalization. Prepared
parameters, correlated and closed Eval calls retain execution-local policies and
original argument identity; no concrete backend or runtime dependency is added.

[Contextual Replace start/counts](TEXT_REPLACEMENT.md#contextual-start-and-count-conversion)
now share numeric/DATE Long conversion and SQL NumberFormat normalization across
ordinary, prepared-parameter and closed Eval paths. Original Text operands remain
borrowed; comparison/source coercion and native compatibility remain separate gaps.

[Comparison-mode and character-code conversion](STRING_COMPARISON.md#contextual-comparison-mode-conversion)
now reaches ordinary, correlated, prepared and closed Eval queries, including raw
byte-code/slicing/search numeric slots. SQL numeric Text uses execution-local
NumberFormat. Required providers, source carriers and String's literal character
argument remain distinct; no backend import, default locale or runtime is added.

[StrConv Integer/Long options](STRCONV.md#contextual-flag-and-lcid-conversion)
now accept numeric/DATE and explicit-policy SQL Text through the shared evaluator.
Prepared/correlated/closed Eval calls retain locale isolation and raw encoding
boundaries; no backend import, supported locale/flag expansion or runtime is added.

[Numeric formatting options](STYLED_NUMBER_FORMATTING.md#contextual-numeric-options)
and [FormatDateTime selectors](DATE_FORMATTING.md#contextual-named-selectors) now
accept explicit-policy SQL numeric Text in prepared, nested and closed Eval paths.
Their owning parsers preserve validation order and source borrows; number and
date syntax/output policies remain distinct. No backend, runtime or dependency
is added, and native Access qualification remains pending.

DatePart/DateDiff now use the evaluator's [metered week-option service](WEEK_SETTINGS.md#contextual-week-option-conversion),
alongside Weekday and general Format. Numeric Text uses execution-local number
syntax and legacy DATE options use their raw serial; prepared plans capture
neither normalization output nor week policies. Original operands remain borrowed.

[DateAdd/DatePart/DateDiff](DATE_INTERVALS.md) now route supported numeric/Text date
sources through the evaluator's shared bounded CDate service after their owning
control validation. Both DateDiff sources validate before NULL propagation. Count,
week and source normalization share execution budgets; plans retain no converted
source/policy. Existing date carriers, UPDATE gates and native pushdown are unchanged.

DateSerial/TimeSerial now use the evaluator's [bounded constructor component conversion](DATE_CONSTRUCTORS.md#argument-coercion).
Numeric Text syntax is execution-local and independent of date syntax; raw DATE
components use their serials. Arity/progressive component validation, eager operands,
relative calendar kernels, year-window overrides and shared limits remain intact.

[Named fixed crosstabs](NAMED_CROSSTABS.md#policy-independent-computed-fixed-headings)
now admit policy-independent computed Text/integral IN headings through shared
scalar kernels without row scans or provider context. The original expressions
remain executable, full inner evaluation remains authoritative, and names are
checked against the bound schema. Execution-phase discovery now covers qualified parameter/policy/data-dependent
relational schemas and parameterless runtime domain targets; inline TRANSFORM
remains open.

The CLI query-run host now calls [execute_with_schema_discovery](NAMED_SCHEMA_DISCOVERY.md).
One query budget covers binding, dependency materialization, retries and execution;
completed discovered crosstabs preserve expression cells and text-policy provenance.
PreparedQuery APIs retain their original metadata-only preparation and consumable
execution behavior. ExecutionOptions is now Copy/Clone over immutable provider
references and value policies; copying it does not clone or reset provider state.


## Contextual numeric predicate comparisons

[Numeric/Text predicates](NUMERIC_PREDICATES.md) now share scalar evaluation's
explicit number policy across binary comparisons, BETWEEN, IN/quantified sources,
joins, filters, HAVING and supported domain criteria. Checked fixed/floating
conversion uses the same service. Text/Text still resolves captured collation
provenance; set/group/sort keys retain their strict comparison contract. Mixed
numeric predicates do not introduce source hints, stored coercion or schema
inference. Cancellation and resource-limit errors retain operation-level query
identity; results remain buffered on late conversion or source failure.

Legacy DATE/Text predicates also share [explicit CDate conversion](NUMERIC_PREDICATES.md#legacy-date-and-text)
with execution-local date syntax, year controls and optional numeric fallback.
This applies to filters, joins, BETWEEN, membership/quantifiers and supported
nested/domain expressions. Extended DateTime/Text remains refused. Binding does
not parse rows, and no coercive source/index hint or stored conversion is added.


## Result carrier metadata

[Conservative output types](QUERY_RESULT_TYPES.md) now preserve source descriptors,
constraints and nullability through qualified projections/compositions. Prepared
plans expose metadata without scanning; buffered scalar/expression results retain
it even with no rows. Unknown or varying expression carriers remain explicitly
Dynamic. This does not supply complete native declared-expression inference or
change evaluation/promotion rules.
