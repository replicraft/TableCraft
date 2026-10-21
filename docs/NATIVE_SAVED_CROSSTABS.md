# Native saved crosstab reads

`reconstruct_saved_read` and `NativeSource::read_saved_query` now reconstruct
qualified native crosstab definitions beside SELECT/UNION. The CLI `query saved`
prints their owned syntax inertly; `query run --saved NAME --database PATH`
explicitly executes the intersection with [bounded crosstab retrieval](CROSSTABS.md).
No inventory/open operation activates a query, provider, macro or VBA.

## Native metadata and owned syntax

The qualified catalog family is 0x10 under the existing 0xf0 mask, with a required
Type row of 6. Column rows distinguish three roles by their Flag: 2 supplies an
ordinary SELECT output, 0/NULL supplies the single TRANSFORM expression and 1
supplies the single PIVOT expression. Missing/duplicate transform/pivot roles,
combined/unknown bits, missing expressions, ignored second names and aliases on
the pivot fail. An aggregate alias remains a separate owned `transform_alias`;
it does not replace generated result headings.

GroupBy Flag 2 supplies an ordinary row-group expression. An optional Flag 1 row
describes pivot grouping. Its owned expression must agree exactly with the pivot
body after extracting fixed IN headings; it is retained in original metadata and
consumed as pivot membership, rather than becoming a row-group key. Conflicting
or duplicate pivot grouping, zero/NULL/unknown GroupBy role flags and unhandled
payload fields fail. Wider grouping layouts remain unqualified.

SELECT and crosstab now share validated SELECT-body composition. Existing source/
join orientation, column/group/sort order, WHERE/HAVING, flags and parameter rules
retain their limits, including the [bounded join-forest profile](NATIVE_SAVED_JOINS.md)
and qualified parameter types/names. Stored derived SELECT/UNION sources use
[the shared guarded source adapter](NATIVE_SAVED_DERIVED.md). Column and grouping rows require increasing four-byte native Order
keys in stored order. Original native fragments, scope, identity and flags are
unchanged; no temporary mutated definition or native AST clone is manufactured.
Crosstab role interpretation stays in the feature slice, outside the engine.

Each expression passes existing parser guards under shared cumulative syntax,
work/row/allocation budgets. A non-negated outer IN on the pivot supplies the
ordered fixed heading list; other expression forms retain their ordinary meaning.
A bounded parser-owned boxed crosstab carrier receives the compiled expressions,
SELECT clauses and declarations. Cancellation is checked before publication.
Statement/clause injection and unsupported metadata produce buffered errors.

The SQL model/parser/formatter now round-trip `TRANSFORM expr AS alias` and a
trailing `WITH OWNERACCESS OPTION`. Owner syntax never grants authority. Stored
SELECT options remain visible in the crosstab's owned SELECT. Notably, reviewed
Jackcess 5.0.4 intentionally omits crosstab TOP from rendered SQL even though it
reports the stored option. TableCraft preserves it as TOP syntax and now executes
positive counts/percentages through the [shared TOP contract](CROSSTABS.md#top-row-groups);
silently dropping it would change the query's result. TOP 0 remains unsupported.
The raw metadata getter is the independent control here.

## Execution and limits

```sh
tablecraft query saved --database copy.accdb CrossFixed
tablecraft query run --database copy.accdb --saved CrossParameter --param Floor=long:2
```

Execution uses the existing grouped/pivot engine, including supported aggregate
expressions, row totals, WHERE/HAVING, parameters, qualified joins and named SELECT/
UNION input dependencies. Fixed/discovered Text or integral heading policies,
explicit text comparison, NULL/missing intersections and cumulative limits remain
unchanged. Aliases on TRANSFORM are preserved metadata, not output-name overrides.
TOP 0/DISTINCT/owner options, UDFs and other unsupported execution forms still fail.

Native named/derived crosstab sources remain unsupported, including fixed-heading
forms. Binding does not scan records or invent a static schema for dynamic result
names. Missing TRANSFORM expressions, wider native role layouts, joins beyond the bounded forest profile and
wider parameter/naming/Variant behavior remain acceptance work. The parser still
requires TRANSFORM even though Microsoft's syntax reference describes it as
optional. This feature does not claim complete native crosstab compatibility.

Dynamic heading discovery currently precedes WHERE and retains its complete source
domain; fixed headings follow their explicit list. Native Access heading discovery,
WHERE/HAVING interactions, naming/formatting, alias binding, type promotion, options
and error ordering need Windows comparison. Independent models describe the current
explicit profile, rather than qualifying those choices as native behavior.

## Evidence

One parser test covers alias/owner round trips and invalid aliases; a query test
checks unchanged generated headings and owner refusal. Four authored adapter tests
cover roles, aliases, parameters, fixed/dynamic execution on four header layouts,
option retention, metadata conflicts/injection, budgets and early/late cancellation.
Two CLI tests cover inert output, explicit execution, named-source refusal and
payload-free buffered diagnostics. SELECT/UNION regression tests remain in the batch.

[SavedCrosstabFixture.java](testing/SavedCrosstabFixture.java) creates fourteen
original inert definitions on fresh Jet 4 / ACE 2007/2016/2019 files with expression
evaluation disabled. The read-only getter/renderer and
[comparison recipe](testing/compare_saved_crosstabs.py) pass 320 controls for SQL
assembly, raw metadata, getter-derived grids, parameters, HAVING, joins, dependencies,
counts, text/fixed headings, stored options and copy-only malformed metadata.
Original bytes remain unchanged; [FIXTURES.md](FIXTURES.md#saved-crosstab-inputs)
records hashes and provenance. The reference never executes SQL. Windows Access
qualification remains unrun; no third-party fixture or upstream code/data was adopted.

References: [Microsoft TRANSFORM](https://learn.microsoft.com/en-us/office/client-developer/access/desktop-database-reference/transform-statement-microsoft-access-sql),
[Microsoft HAVING](https://support.microsoft.com/en-us/access/having-clause),
[Jackcess 5.0.4 source artifact](https://repo.maven.apache.org/maven2/com/healthmarketscience/jackcess/jackcess/5.0.4/jackcess-5.0.4-sources.jar).

## Stored TOP execution evidence

Positive stored counts and PERCENT now use the same final row limiter as ordinary
SELECT. An adapter test exercises encoded counts, percentage flags and sort ties
on all four current layouts, retaining complete columns and source bytes. A CLI
regression checks saved TOP and omitted-cell errors. The extended original
SavedCrosstabFixture --top profile has twenty definitions, including percentage,
full-key tie, HAVING, TOP 0 and excluded-cell-error cases. Its independent comparison
passes 440 controls; the unchanged fourteen-definition inputs still pass 320.
No native Access execution is implied. See the [fixture record](FIXTURES.md#crosstab-top-inputs).
