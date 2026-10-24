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

WHERE's Boolean structure admits direct tests, NOT and the five logical operators.
The binder lifts the test into a typed parent predicate before pruning or
execution. A representative named member supplies the complete membership view;
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

Bare collection projection, `IsNull(Files)` calls, comparisons of a presence test,
function-wrapped tests, direct domain criteria and correlated outer-container tests remain unsupported.
These forms fail during binding. Presence mixed with a non-Boolean member operand
that cannot be independently folded also fails before scanning. This adds no
collection display values, history SQL, lookup execution or writes.

Focused source tests distinguish empty membership from nullable children and
cover aliases, logical combinations, displayed members, counts, joins/outer joins,
derived queries, crosstabs, malformed views, eager failures, limits and every
observed cancellation checkpoint. Adapter tests prohibit external FileData page
reads and verify cache reuse. CLI tests reuse original synthetic attachment
fixtures across ACE 2007/2016/2019-family headers and verify unchanged bytes.

The original [independent runner](testing/compare_attachment_presence.py) checks
criteria against Jackcess-decoded parent membership counts on the registered
[3 MiB attachment corpus](NATIVE_TABLES.md#attachment-metadata-projection-snapshots).
It verifies original hashes before and after each run. Only TableCraft executes
SQL; these controls do not establish native Access execution or coherent reads
while another application modifies the file.
