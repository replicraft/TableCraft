# Named collection presence criteria

Explicit retrieval now supports bare named collection fields in WHERE `IS NULL`
and `IS NOT NULL` criteria. With native Access inputs, `--collections` exposes
qualified attachment collections:

```sh
tablecraft query run 'SELECT ID FROM Items WHERE Files IS NOT NULL ORDER BY ID' --database copy.accdb --collections
tablecraft query run 'SELECT Count(*) FROM Items WHERE Files IS NULL' --database copy.accdb --collections
```

`IS NULL` means that the parent has no child members. `IS NOT NULL` means that it
has at least one. A child with NULL metadata still counts as an attachment, as
does an attachment containing a zero-byte file. This differs from testing
`Files.FileName IS NULL`, which uses the existing member criteria. No bare field
is converted to scalar NULL, Text or a physical child key.

Microsoft's [attachment query criteria](https://support.microsoft.com/en-us/access/examples-of-query-criteria)
explicitly distinguish parents with and without attachments. This implementation
uses that emptiness contract; native Access SQL comparisons remain pending.

## Binding and execution

The source's named `CollectionMember` declarations identify eligible containers.
Multivalue `.Value` slots have a distinct internal role and do not acquire this
attachment-style bare-field predicate. Scalar fields retain ordinary SQL NULL
tests. Names, aliases, case policy and ambiguous references follow existing
binding rules. Binding reads no parent/child records or payloads.

WHERE admits direct tests, NOT, logical operators and composition inside supported
scalar expressions. The binder retains typed presence operands and resolves them
to the current parent-membership frame before pruning or execution. A representative named member supplies the complete membership view;
its values do not determine emptiness. The requested view is validated through
EOF, including stored NULL children. Read/schema/provider errors, cumulative
work/row/allocation ceilings and cancellation remain failures. An earlier match,
TOP or a dominating Boolean operand does not suppress later errors.

Scalar-only output keeps one row per qualifying parent. Displayed child members
retain the existing aligned expansion, and independent collections retain their
bounded product. Qualified joins, derived queries, grouped queries and crosstabs
use the existing parent-membership machinery. An absent outer-join side follows
the current empty/NULL container policy. This is an explicit engineering contract,
not qualification of all native join/collection semantics.

Native attachment declarations select metadata before the optional decoded
FileData member. Presence queries therefore use the metadata-only cache and do
not load encoded or decoded attachment payload values. Explicit FileData selection
retains its full-snapshot/payload limits, even when a filter would discard a
large file. Original bytes and source budgets remain unchanged.

## Current limits and evidence

Bare collection projection, `IsNull(Files)` calls on the collection itself,
non-WHERE uses and correlated outer-container tests remain unsupported. These
forms fail during binding. Compound scalar calls mixing presence with hidden
member criteria whose empty-list fold is unqualified also fail before scanning.
Displayed member criteria remain supported within composed calls. This adds no
collection display values, history SQL, lookup execution or writes.

Focused source tests distinguish empty membership from nullable children and
cover aliases, logical combinations, displayed members, counts, joins/outer joins,
derived queries, crosstabs, malformed views, eager failures, limits and every
observed cancellation checkpoint. Adapter tests prohibit external FileData page
reads and verify cache reuse. CLI tests reuse original synthetic attachment
fixtures across ACE 2007/2016/2019-family headers and verify unchanged bytes.

The original [independent runner](../../testing/compare_attachment_presence.py) checks
criteria against Jackcess-decoded parent membership counts on the registered
[3 MiB attachment corpus](../../access/storage/NATIVE_TABLES.md#attachment-metadata-projection-snapshots).
It verifies original hashes before and after each run. Only TableCraft executes
SQL; these controls do not establish native Access execution or coherent reads
while another application modifies the file.


## Domain criteria

Runtime domain criteria now admit the same direct named-container NULL tests and
Boolean combinations. Domain value expressions remain scalar; child-member and
bare collection values are still refused. For example:

```sh
tablecraft query run "SELECT DCount('*','Items','Files IS NOT NULL'),DSum('ID','Items','Files IS NULL')" --database copy.accdb --collections
```

The predicate filters parent records before the existing domain aggregate. A
parent with two attachments counts once, and a NULL child filename still counts
as presence. The twelve domain functions retain their existing numeric,
encounter-order, NULL/Error and native precision contracts. Dynamic criteria are
parsed/bound on each explicit execution with the same cumulative budget and
isolated string scope. Missing criteria fields still return NULL; unsupported
member syntax remains an error. A failed execution does not poison reused plans.

[Microsoft's DCount contract](https://support.microsoft.com/en-us/access/functions/dcount-function)
defines criteria as a WHERE-style restriction on the selected domain. Combining
that contract with the documented attachment emptiness criteria guides this
engineering profile; it does not establish native Access domain/attachment
execution equivalence. Native metadata-only caches retain their existing payload
boundary and no query string opens an external source or activates VBA.


## Scalar predicate composition

Presence Boolean operands now compose in comparisons, conversions, selectors,
list/range predicates and other supported scalar calls in WHERE and domain
criteria. For example:

```sql
SELECT ID FROM Items WHERE (Files IS NULL)=True;
SELECT ID FROM Items WHERE IIf(Files IS NULL,True,False);
SELECT DCount('*','Items','CBool(Files IS NOT NULL)');
```

The runtime retains a borrowed parent frame only while evaluating that predicate.
A resolved presence operand validates its member view at its written scalar
position, then returns Boolean. Other arguments retain their existing eager
rules: a leading division error fails before a later presence operand is inspected;
an invalid view in an earlier operand fails before a later division. Rnd controls
still evaluate once per evaluated parent predicate, without one draw per child.
Completion rechecks cancellation after successful view validation.

Scalar-child traversal shares binding limits and excludes nested read plans.
Each read binds its own clause context; enabling presence in an outer WHERE does
not enable bare collection projection or capture outer containers in a subquery.
Nested reads with their own qualified WHERE criteria retain their own parent
frame. Stored records, public expression values and output schemas acquire no
collection reference or extra Boolean column.

A displayed child remains the current row: `IIf(Files IS NULL,False,
Files.FileName='a')` filters that child. The corresponding compound call over
wholly hidden members remains refused because empty-list membership/selector
semantics need a separate contract. Simple independently foldable Boolean criteria
retain their existing parent-membership behavior. Native Access composition and
error-order qualification remain pending.
