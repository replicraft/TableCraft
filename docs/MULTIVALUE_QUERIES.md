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
undeclared members and object/VBA access remain unsupported.

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

Direct single-table SELECTs with scalar parent projection now support hidden
`.Value` WHERE criteria through [parent membership](#parent-membership-criteria).
Member access in JOIN/HAVING/ORDER BY without a projected/grouped sibling collection,
mixed flattened/parent modes,
parent-criteria crosstabs and nested subqueries inside those criteria remain
unsupported. Plain collection projection/parent display values, history
member SQL, native lookup conversion and collection mutation remain unimplemented.
Active DISTINCTROW cannot compare unavailable complete parent records.

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

## Parent membership criteria

```sh
tablecraft query run 'SELECT ID FROM Items WHERE Tags.Value=7 AND Tags.Value=8' --database copy.accdb --multivalues
tablecraft query run 'SELECT Count(*) FROM Items WHERE NOT Tags.Value=7' --database copy.accdb --multivalues
```

When no member is projected/grouped, a direct single-table SELECT can match whole
parents without emitting child combinations. Each Boolean atom referencing one
collection evaluates over its stored values; any TRUE makes that atom TRUE,
otherwise NULL comparisons retain UNKNOWN. An empty list folds to FALSE.
AND/OR/XOR/EQV/IMP combine independently matched atoms, and NOT negates the
parent result. Root `<>`, NOT LIKE/IN/BETWEEN and IS NOT NULL similarly negate
the positive membership test. Thus `contains 7 AND contains 8` can match different
stored children. Parent rows, including duplicate scalar parent records, remain
separate; no output-value DISTINCT or grouping substitutes for identity.

The current explicit NULL policy distinguishes empty membership from stored NULL:
`Tags.Value IS NULL` matches a stored NULL element; it does not match an empty list.
A negative equality membership can select an empty list, while a NULL-only list
remains UNKNOWN and fails WHERE. Native Access NULL/negative-operator fidelity
still needs Windows comparisons; this profile is not qualified native behavior.

Binding lowers the complete WHERE into a private membership predicate tree.
Scalar dependencies, parameters and correlated *scalar* outer fields retain
pruning/slot rewriting. Each member atom uses one private item operand, not a fake
scalar parent key/NULL. Multiple collections can participate in separate atoms;
an atom comparing different collections is unsupported. Pure scalar functions
inside an atom use the same element. Complex subqueries inside criteria, joins as
the direct parent source, mixed parent/flattened modes and ambiguous member sorting
are rejected before scans. Derived queries can compose supported parent retrieval.
Crosstabs keep their existing flattened/domain rules and reject parent criteria.

Every atom/member and both logical operands evaluate eagerly, preserving later
errors after a match or decisive Boolean. Empty atoms visit no element expression.
All field validation, copies, allocations and cancellation/work/row limits remain
shared. Matching rows are returned at parent cardinality before normal GROUP BY,
DISTINCT, ORDER BY and TOP. Native collection snapshots stay complete and cached
separately; matching subsets are never stored as complete native tables or pushed
through scalar seek hints.

Six additional authored/generated tests cover parent cardinality, independent
atoms, NULL/empty folds, logical negation, parameters, correlated scalar slots,
derived composition, eager failures and unsupported shapes. A CLI test and native
cache assertions cover the adapter path. The
[45-control recipe](testing/compare_multivalue_parent_criteria.py) compares original
parent/member rows with an independent nullable membership model across the three
qualified ACE families. These models support the documented execution policy;
Microsoft Access SQL, NULL/error ordering and return-type qualification remain open.

## Aligned named collection members

`Source::collection_members` declares validated `CollectionMember` pairs.
Collection names also occur in unavailable fields and cannot overlap legacy
`.Value` declarations. Member pairs are unique under the source name policy;
the combined scalar/unavailable/value/member width retains the existing limit.
Preparation reads metadata only. `scan_collection_members` accepts separate sorted
unique legacy-value and named-member indices. Callbacks supply legacy lists first,
then named lists, each in requested order. The default bridges existing `.Value`
providers when no named member is requested.

Members sharing a collection name must have equal lengths and identical child
order. Execution checks lengths, exact scalar carriers and captured metadata,
aligns member positions within a child and takes products only across independent
collections. Interleaved member declarations retain their original slot order.
Empty collections NULL-pad all requested members together under the existing
explicit expansion policy. Direct parent predicates retain the existing independent
membership-atom policy; this does not qualify native cross-member criteria.

### Unprojected members of displayed collections

```sql
SELECT Files.FileName FROM Items WHERE Files.FileType = 'txt';
```

Once a member is projected or grouped, its collection is expanded into aligned
child rows. Unprojected siblings can participate in WHERE, ORDER BY, JOIN
conditions and otherwise valid HAVING expressions without adding those fields to
output. They filter/sort the **same child**, retaining normal eager scalar
evaluation and the existing NULL extension for an empty collection. For example,
displaying FileType while testing `FileName IS NULL` can include both a stored
NULL child name and an empty parent's NULL-extended row. Scalar-only parent
membership retains its separate empty/NULL/negative-atom rules above.

Binding and repeated projection pruning share a bounded sorted set of borrowed
collection identities. Identity includes both the relation qualifier and
collection name under the existing ASCII-insensitive identifier policy; it does
not conflate aliases in self-joins. Hidden predicate/sort/HAVING/correlated inputs
remain in scan projections, preserving alignment, carrier validation and later
errors. No metadata scan, new public source contract or runtime coercion is added.

A wholly hidden independent collection mixed with displayed collections still
requires parent-versus-child semantics that are not implemented. Joined
parent-only membership, parent-criteria crosstabs and native Access qualification
also remain open. This feature does not turn every hidden member into a new
Cartesian expansion or silently choose parent membership for it.

Native `--collections` supplies attachment metadata through this contract; see
[the supported fields and limits](NATIVE_QUERIES.md#attachment-metadata-member-queries).
Generic hosts may declare other named scalar members. Arbitrary object/property
access and automatic payload decoding are not added to this generic contract.
The native adapter separately offers an explicit
[decoded FileData view](NATIVE_QUERIES.md#explicit-decoded-attachment-values);
it does not prescribe native SQL's binary representation.
