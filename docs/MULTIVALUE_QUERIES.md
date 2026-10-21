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

Scalar-parent SELECTs support hidden `.Value` WHERE criteria through
[parent membership](#parent-membership-criteria), including joined parents.
[Mixed queries](#displayed-children-with-independent-parent-membership) combine
displayed children with wholly hidden independent collection criteria.
Member access in HAVING/ORDER BY requires a projected/grouped or ON-referenced
sibling collection. [ON member fields](#collection-members-in-join-conditions) now
drive child expansion even without output members. [Subqueries inside WHERE membership](#subqueries-inside-membership-criteria)
now retain explicit item scopes and the existing fold rules.
[Crosstabs](CROSSTABS.md#collection-membership-criteria) now retain membership
while preserving dynamic pre-WHERE heading discovery. Plain collection
projection/parent display values, history member SQL, native lookup conversion and collection mutation remain unimplemented.
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
an atom with several hidden members uses [aligned child rows and independent products](#several-hidden-members-in-one-atom). Pure scalar functions
inside an atom use the same element. Subqueries retain scoped item references;
ambiguous member sorting remains rejected before scans. Derived queries can compose supported parent retrieval.
Crosstabs retain these membership rules alongside their existing flattened/domain
rules, classifying pivot-only collections as displayed.

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
[225-control recipe](testing/compare_multivalue_parent_criteria.py) compares original
parent/member rows with an independent nullable membership model across the three
qualified ACE families. These models support the documented execution policy;
Microsoft Access SQL, NULL/error ordering and return-type qualification remain open.

### Parent membership over joins

```sql
SELECT A.Id, B.Id
FROM Items AS A INNER JOIN Items AS B ON A.Id <= B.Id
WHERE A.Tags.Value = B.Id + 6;
```

Scalar-only parent projections now support membership WHERE predicates over
comma products, INNER/LEFT/RIGHT joins, nested joins and independent derived
sources. Atoms may use one or several hidden members, plus scalars from other
joined parents or correlated outer scopes. Members of one collection stay aligned;
independent collections use bounded actual-item products. Separate atoms can refer
to different aliases/collections. Parameters, grouping, distinctness, ordering
and TOP operate on matched **parent rows**, preserving duplicate parents/pairs
without multiplying them by the number of matching child items.

Binding records stable relation-leaf/local-metadata positions before scalar
pruning. Execution captures complete checked owned lists once per parent/alias,
then carries private query-local list handles alongside scalar join rows. Handles
never become core values, source identities or persistent row IDs. Scalar ON
conditions retain ordinary join behavior; the complete membership predicate runs
after the join, with scalar expressions evaluated against that joined row.
Selected native indexes on collection-reading parents remain unsupported, and
no membership or partial scalar filter is pushed down to hide later errors.

The current explicit outer-join policy distinguishes three states: a present
parent's empty collection folds to FALSE; stored NULL items retain the existing
atom rules; an **absent parent** yields UNKNOWN for each collection atom, including
IS NULL. Negation preserves UNKNOWN, while other Boolean operands still evaluate
eagerly. This is a documented TableCraft policy requiring native Access evidence;
it is not inferred compatibility from scalar NULL padding or index order.

Copies, handles, stored items, materialized pairs and predicate evaluation retain
the shared allocation/row/work/cancellation ceilings. Provider/schema/alignment
errors remain latched even if a provider ignores a visitor failure. Missing
outer parents manufacture neither empty collections nor stored NULL elements.
Direct single-table parent reads keep their original bounded scan path.

Parent-membership folding in ON is not part of this profile: ON member fields
produce child rows as described below. [Several hidden members](#several-hidden-members-in-one-atom)
inside one WHERE atom now retain explicit alignment/product rules. Crosstabs now retain the same rules with their own discovery phase.
Six additional authored/generated tests cover joined/correlated/derived scopes, duplicate parents, three-relation
join trees, absent/empty/NULL states, eager late failures, invalid providers,
parameter rebinding, schema changes and all measured cancellation checkpoints.
The independent recipe adds joins/outer joins, grouping, derived sources and
parameter/failure controls across original ACE 2007/2016/2019 stored records.
Microsoft's [multivalued query guide](https://support.microsoft.com/en-us/access/using-lookup-and-multivalued-fields-in-queries)
and [outer-join reference](https://learn.microsoft.com/en-us/office/client-developer/access/desktop-database-reference/left-join-right-join-operations-microsoft-access-sql)
provide the SQL concepts; neither qualifies the complete policy above. Actual
Access SQL, absent-parent behavior and native error order remain unrun.

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

Once a member is projected, grouped or referenced by ON, its collection is
expanded into aligned child rows. Unprojected siblings can participate in WHERE, ORDER BY, JOIN
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

Wholly hidden independent collections can now use the explicit
[mixed membership policy](#displayed-children-with-independent-parent-membership)
below. Parent-criteria crosstabs and native Access qualification remain open.
An unprojected sibling still belongs to the displayed child, rather than becoming
an independent membership test or another Cartesian expansion.

Native `--collections` supplies attachment metadata through this contract; see
[the supported fields and limits](NATIVE_QUERIES.md#attachment-metadata-member-queries).
Generic hosts may declare other named scalar members. Arbitrary object/property
access and automatic payload decoding are not added to this generic contract.
The native adapter separately offers an explicit
[decoded FileData view](NATIVE_QUERIES.md#explicit-decoded-attachment-values);
it does not prescribe native SQL's binary representation.

### Displayed children with independent parent membership

```sql
SELECT Id, Files.FileName
FROM Items
WHERE Files.FileType = 'txt' AND Tags.Value = 7;
```

Projected/grouped/ON-referenced collections expand into aligned scalar children; their
unprojected siblings test that same child. A wholly hidden independent collection
in WHERE instead folds using the parent-membership rules. In the example, a parent
containing tag 7 retains its displayed txt files once each, regardless of how many
matching tags it stores. Several displayed collections keep their existing
bounded Cartesian product; hidden matches do not multiply it. Membership can also
refer to another joined alias or use a displayed child value as a scalar input:
`Other.Value = Tags.Value - 3` folds Other for each displayed Tags child.

Binding chooses this policy from projection/group dependencies and collection
identities, without scanning. Each affected leaf requests the sorted union of
shown/sibling and hidden fields in one collection scan. It validates alignment,
copies complete hidden lists once per parent, and expands only displayed
collections with the ordinary shared child-expansion implementation. Private
handles follow those child rows through joins and repeated projection pruning;
no matching on scalar parent values or reconstruction of parent identity occurs.

Empty displayed collections retain a NULL-extended child; empty hidden collections
have no item. Hidden negative atoms negate positive membership as before, even
when another operand is a NULL-extended displayed value. Absent joined owners
still yield UNKNOWN rather than an invented empty list or stored NULL. Each atom
may reference several wholly hidden members plus ordinary/displayed scalar
fields, using the alignment/product policy below. Hidden
membership folding in HAVING/ORDER BY remains unsupported. ON references use
child-row semantics, including when the member is absent from output. Crosstabs additionally count pivot dependencies as displayed and
retain their [pre-WHERE heading domain](CROSSTABS.md#collection-membership-criteria).
Ordinary displayed-child expressions retain their existing clause support.

Seven new multivalue tests and two named-member tests cover independent generated
nullable models, duplicate parents/children, multiple displayed products,
interleaved members, alias separation, derived/correlated reads, grouping,
empty/NULL/absent states, parameters, metadata changes, provider alignment/carriers,
eager late errors, budgets and every measured execution cancellation checkpoint.
A mixed legacy/named test also checks both metadata index spaces in either mode.
Native/CLI regressions cover opt-in decoded data, cache reuse and buffered late
checksum failures. The independent recipes now pass 225 parent-model and 162
attachment controls, including collection crosstabs, over original ACE
2007/2016/2019 inputs, with unchanged hashes. These explicit row models do not qualify native Access mixed-query, NULL/negation,
binary-representation or error-order behavior; Windows comparisons remain open.

### Subqueries inside membership criteria

```sql
SELECT A.Id
FROM Items AS A
WHERE EXISTS (
    SELECT B.Id FROM Items AS B
    WHERE B.Other.Value = A.Tags.Value - 3
);
```

WHERE membership atoms can now use scalar subqueries, IN/NOT IN, ANY/ALL/SOME and
EXISTS, including nested correlated reads. The example folds the outer Tags list
while the inner query independently folds each B parent's Other list. Private item
references retain query-scope depth, so an inner collection item, an outer item,
ordinary scalar fields and output aliases remain separate. Binding follows the
existing pruning traversal through SELECT/UNION expressions, inner membership and
join conditions, while derived sources and named crosstabs keep independent scopes.
Preparation still performs no record scans and does not execute subqueries.

Each outer membership atom may refer to several wholly hidden members of its own
scope; inner queries can own separate collections and supported scopes. Each item
reference retains both its tuple position and its correlated scope, including
inside one EXISTS subtree.
Ordinary scalar-only subquery atoms alongside membership evaluate once per parent.
Correlated item captures use a private owned row frame only when required; direct
and uncorrelated membership keep the borrowed parent-row path. Frames and nested
reads share the same allocation/row/work/cancellation ceilings. They introduce no
public source column, persisted identity, core carrier or implicit coercion.

Root NOT IN negates positive membership, like root NOT LIKE/inequality under the
existing parent policy. Quantified comparisons evaluate their existing per-item
SQL truth then fold membership. Empty hidden lists visit no item expression, so
contained subqueries do not execute; independent scalar Boolean atoms still run.
A stored NULL is an actual item, while absent joined parents retain UNKNOWN.
Scalar subqueries preserve zero-row NULL and multiple-row cardinality errors.
Subqueries materialize their complete bounded results, and all membership items
and logical operands remain eager: TOP or an early matching item cannot conceal
later query, provider or decoder errors.

Six new multivalue tests and one named-member test cover scalar/set/quantified
forms, several correlated scopes, nested local membership, derived/UNION/grouped/
sorted inner reads, output aliases, mixed/joined/outer/crosstab behavior, NULL/empty
results, cardinality/late errors, generated independent models, parameters,
metadata changes, budgets and every measured cancellation checkpoint. A fixed
named-crosstab test also rebinds a captured item through its inner query. Native
and CLI regressions retain explicit decoding, complete caches and buffered late
failures across the current ACE families. Independent recipes now pass 225 parent
and 162 attachment controls, including ON child joins, with unchanged original ACE 2007/2016/2019 hashes.

Microsoft's [SQL subquery reference](https://learn.microsoft.com/en-us/office/client-developer/access/desktop-database-reference/sql-subqueries-microsoft-access-sql)
describes scalar/predicate forms, aliases and crosstab WHERE use; the multivalued
query reference above guides collection access. Neither qualifies the full
combined profile here. Native Access collection/subquery, NULL/negative operators,
empty-set evaluation, binary representation and error order remain unrun.

### Collection members in join conditions

```sql
SELECT A.Id, B.Id
FROM Items AS A INNER JOIN Items AS B ON A.Tags.Value = B.Id;
```

A collection member referenced by ON now drives child expansion even when no
member is projected or grouped. It uses the same child-value comparison as an
ON member already included in output. Every matching child participates;
duplicates remain duplicates. Two ON collections use the existing bounded
Cartesian child product, while named siblings of one collection stay aligned.
Adding the ON field to output therefore preserves matching-row cardinality.
This differs from WHERE parent membership, which retains a parent at most once
per pair. Bare collections/display values still do not become scalar join keys.

The parent-criteria classifier and projection pruner share the existing scoped
relation-dependency walker. It captures ON dependencies at their local offsets,
including nested joins and supported correlated expressions, without scanning.
Once ON makes a collection row-producing, WHERE/sort/HAVING siblings operate on
that same child. Another wholly hidden collection in WHERE still uses explicit
parent membership and cannot multiply the existing child rows. Derived/named
inputs keep their independent scope boundaries. Runtime joins, private membership
handles, exact carriers and scalar comparison policies remain unchanged.

LEFT/RIGHT matching tracks expanded input rows. Each unmatched preserved child
gets ordinary scalar NULL padding; an empty collection retains the existing
NULL-extended child before ON. NULL comparisons retain SQL UNKNOWN. WHERE follows
matching and cannot turn a matched-but-filtered row into a new unmatched row.
This is the documented TableCraft profile, not native Access qualification of
collection ON admissibility, empty sets, duplicate cardinality or nesting.
Parent-membership folding inside ON is not introduced by this implementation.

All expansion, copies, joined pairs and callback work retain shared budgets and
cancellation. Metadata-only binding, eager Boolean operands, buffered output,
complete native caches and selected-index refusals remain intact. Even False ON
or TOP cannot suppress malformed selected lists or late decoder failures.
Decoded FileData still requires explicit opt-in; NULL tests add no arbitrary
Binary comparison or native SQL representation claim.

Six new multivalue tests and one interleaved-member test cover projection
independence, duplicates, both outer directions, empty/NULL children, two-member
products, grouped counts, nested/derived/subquery/crosstab reads, mixed WHERE
membership, aligned siblings, malformed providers, parameter rebinding, metadata
changes, budgets and every measured cancellation checkpoint. Native/CLI cases
retain complete decoded caches and buffered failures. Independent recipes now
pass 225 parent/child and 162 attachment controls across unchanged original ACE
2007/2016/2019 inputs.

Microsoft's [INNER JOIN reference](https://learn.microsoft.com/en-us/office/client-developer/access/desktop-database-reference/inner-join-operation-microsoft-access-sql)
and [outer-join reference](https://learn.microsoft.com/en-us/office/client-developer/access/desktop-database-reference/left-join-right-join-operations-microsoft-access-sql)
show join keys independently of output, matching and preserved sides; the
multivalued query guide above describes expanded Value fields. These concepts
guide the model; they do not qualify their complete combination. Microsoft
Access comparisons remain required before claiming native collection-join support.

### Several hidden members in one atom

```sql
SELECT Id FROM Items
WHERE EXISTS (
    SELECT 1 WHERE Files.FileType = 'txt' AND Files.FileFlags >= 0
);
```

An atom can now reference several hidden members. Members of one collection,
identified by relation qualifier and collection name under the existing
ASCII-insensitive policy, read the **same child position**. The example checks
type and flags from one stored file. Members of independent collections/aliases
use a bounded Cartesian product of actual items. Each atom folds that product to
one truth value; matching tuples never multiply parent/pair output rows. Several
references to one field use one item position. Scalar/displayed dependencies
remain ordinary row fields.

The existing Boolean tree remains significant. Separate WHERE atoms such as
`Files.FileType='txt' AND Files.FileFlags=0` fold independently and can match
different children. One EXISTS subtree or scalar expression containing both
members is one atom, so same-collection members align. Independent collections
can be compared directly, as in `Tags.Value=Other.Value+3`. Root negative atoms
still negate positive membership; stored NULL comparisons retain UNKNOWN.
An empty participating collection yields FALSE without inventing a NULL item;
negation can select that empty product. An absent joined owner instead yields
UNKNOWN for the atom, including when another participating group is empty. All
present selected lists are validated even when another group is empty or absent.

Binding stores private member keys and group positions independently of scalar
projection pruning. Captured references carry a scope depth, tuple index and frame
width, so nested single/multiple membership, output aliases and source scalars stay
separate. Complete lists are checked, then combinations are visited with bounded
position vectors and one temporary tuple at a time. Tuple expressions and logical
operands remain eager, even after a match or decisive Boolean. Copies, frames,
work and intermediate rows share the existing ceilings; no product-size overflow,
unbounded materialized product table, source API or persistent identity is added.
The single-member path remains in place.

Six new multivalue tests and three aligned-member tests cover independent
nullable models, same-child versus Cartesian behavior, interleaved/case-variant
metadata, duplicate scalar parents, joins/missing owners, mixed displayed inputs,
derived/crosstab/named scopes, nested captures, zero-width parents, late arithmetic
and provider errors, empty-product validation, parameter rebinding, metadata
changes, budgets and every measured cancellation checkpoint. Native/CLI tests
separate aligned decoded-data/flags criteria from independent membership, retain
complete caches, and publish no output on a late decoder error.

The independent recipes pass 225 parent/child and 162 attachment controls on
unchanged original ACE 2007/2016/2019 hashes. These include nullable pair products,
wrapped/deep correlation, mixed/crosstab results, zipped metadata/payload models
and buffered arithmetic/non-finite/checksum failures. A full deeply correlated
100-item product exceeds the unchanged 100,000-row work ceiling; its explicit
refusal is tested, and a smaller scalar ON domain separately checks the same
nested item scopes. This remains a scale limit, not native compatibility evidence
or justification to increase default budgets.

Microsoft's [child-recordset description](https://learn.microsoft.com/en-us/office/vba/access/concepts/data-access-objects/manipulate-multivalued-fields-with-dao)
and [attachment record traversal](https://learn.microsoft.com/en-us/office/client-developer/access/desktop-database-reference/recordset2-object-dao)
guide stored child alignment. Together with the query references above, they do
not qualify all combined SQL, negative/empty/absent rules, cross-collection
admissibility, decoded Binary representation or error order. Those native Access
comparisons remain unrun; independent hidden HAVING/ORDER BY and history-member
SQL remain open.
