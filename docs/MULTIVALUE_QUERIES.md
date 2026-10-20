# Scalar multivalue query expansion

Status: working backend-neutral query-source and execution support, now connected
to qualified native stored collections through `query run --database PATH
--multivalues`. Existing native stored collection browsing remains separate. This does not add collection variants to
core `Value`, run lookup queries or implement collection writes.

## Source contract

`Source::multivalue_columns` provides stable scalar element columns whose names
also occur in `unavailable_columns`. The declared type/nullability describes
stored elements, not a scalar parent field. Ordinary names and wildcards still
reject those parent fields; they never become delimited Text or Int32 keys.

For requested `.Value` fields, `scan_multivalues` visits each parent exactly once,
including empty memberships. Its sorted unique scalar/collection indices refer
to the original schemas. `MultivalueVisitor` receives a borrowed scalar projection
and requested `MultivalueList` views. List lengths retain duplicate/NULL children;
`get` must succeed below `len`. Views own no global child identity or writable
handle. Providers own stable data and callback I/O limits, and must propagate
visitor errors. Default methods expose no collections and reject expanded scans.

The query layer validates metadata, element carriers/nullability, list count and
requested scalar width. It checks complete captured metadata at execution, latches
visitor failures even if a provider ignores rejection, and returns no partial
result. No native storage types enter this boundary.

## Binding and execution

`Tags.Value`, `T.Tags.Value` and bracketed spellings bind explicit member slots.
The parser allows reserved `Value` after a dot without relaxing standalone names
or bang access. Ordinary field/member ambiguity fails; three-part qualification
can disambiguate a table alias from a collection name. Bare collection fields,
other members and object/VBA access remain unsupported.

Only used collections expand. Scalar-only/count queries retain parent cardinality
and ordinary scans, and repeated references to one collection expand it once.
Referenced elements are checked as they enter expanded rows; duplicates and
source order survive. Each empty list supplies one NULL-extended row in the current
explicit policy. Actual stored NULL elements remain distinct in source data,
although both appear as NULL in flattened scalar output. Multiple referenced
collections use a bounded Cartesian product. Native empty/multiple-collection
semantics still need Access qualification.

Projection/grouped `.Value` access supports residual WHERE, sorting, joins,
DISTINCT, aggregates, derived/named queries, correlated slots and crosstabs under
the normal scalar contracts. Expansion precedes WHERE/grouping. Collection-only
sources require no fake scalar placeholders. Source pruning retains member inputs
and remaps scalar/member positions; unused collections are neither scanned nor
expanded. Expanded scans receive no scalar seek constraint. Scalar-only scans
retain the existing safe constraint path.

Member references used only in criteria, joins, HAVING or ordering are currently
rejected unless that member is also used by projection or grouping. Microsoft
distinguishes parent-level criteria from explicitly flattened output; blindly
flattening criteria-only queries would give incorrect parent semantics. Plain
collection projection/parent matching, attachment/history member SQL, native
lookup display conversion and collection mutation remain unimplemented. Active
DISTINCTROW cannot compare unavailable complete parent records.

Every visited element and copied output retains shared work, row, allocation and
cancellation limits. Products materialize under those limits, not a raised or
separate unlimited budget. Input views remain borrowed; result rows are owned and
publish only after completion. Providers must meter expensive callbacks themselves.

## Evidence

Nine authored/generated integration tests use an explicit source implementation
to cover metadata-only preparation, scalar/collection pruning, repeated members,
duplicates, NULL/empty lists, products, joins, grouping, aggregates, DISTINCT,
correlations, crosstabs, zero-width parents, schema changes, bad list contracts,
ignored visitor errors and limits/cancellation. The generated case compares an
independent parent/value expansion model. A parser test checks qualified `.Value`
round trips and reserved-name/bang restrictions.

Microsoft's [multivalue query guide](https://support.microsoft.com/en-us/access/using-lookup-and-multivalued-fields-in-queries)
documents `.Value` flattened output, repeated parent fields and the distinction
from parent-level criteria and display values. It guides this implementation;
synthetic host tests do not establish actual Access SQL compatibility. Native
adapter integration now has separate [native query evidence](NATIVE_QUERIES.md#explicit-multivalue-value-queries).
Windows qualification remains pending.

## Native CLI use

```sh
tablecraft query run 'SELECT ID, Tags.Value AS Tag FROM Items ORDER BY ID' --database copy.accdb --multivalues
tablecraft query run 'SELECT Tags.Value AS Tag, Count(*) AS N FROM Items GROUP BY Tags.Value' --database copy.accdb --multivalues --collation ordinal
```

`--multivalues` requires `--database` and opts into extra complex-schema discovery.
Without it, scalar-only metadata behavior remains unchanged. Expanded reads use
sequential parent order and reject an explicit index selected on that same table;
there is no silent fallback. Independent stored-row models qualify that explicit
execution policy, not native Access SQL or locale semantics.
