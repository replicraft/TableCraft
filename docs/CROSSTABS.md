# Bounded crosstab retrieval

`tablecraft-query::prepare` accepts top-level `TRANSFORM … SELECT … GROUP BY …
PIVOT … [IN (…)]` statements. The existing CLI `query run` uses this executor over
inline, native Access or read-only SQLite sources. Parsing and preparation remain
metadata-only: they do not scan records to discover headings. Execution returns
complete buffered column names and rows under the usual query budgets.

```sh
tablecraft query run 'TRANSFORM Count(*) SELECT COLOR_ID FROM ROULETTE_TB GROUP BY COLOR_ID ORDER BY COLOR_ID PIVOT Len(NBR_VALUE) IN (2,1,3)' --database docs/examples/Access_Example_VBA.accdb
```

## Working execution contract

The SELECT supplies row headings and optional row-total aggregates. Supported
pure grouped expressions, joins, ordinary derived reads, WHERE subqueries,
parameters, HAVING and row-heading ORDER BY retain their existing retrieval
contracts. TRANSFORM must contain a supported aggregate expression; scalar
wrappers around aggregates use the existing evaluator. A nonempty GROUP BY is
required. DISTINCT/TOP/OWNERACCESS/external/action forms remain rejected.

Without IN, pivot values are discovered over the complete source relation before
WHERE, deduplicated and sorted with the explicit scalar comparison policy. WHERE
filters cell members; HAVING filters completed row groups. Their filtering does
not shrink the discovered heading domain. No source constraint hint can prune
that discovery domain. The pivot expression consequently evaluates before WHERE
in dynamic mode. With fixed headings, WHERE runs before pivot evaluation.

IN expressions use literals, parameters and supported source-free scalar calls.
They evaluate once per execution, retain written order and create missing columns;
field references, aggregates and subqueries in this list reject. A prepared plan
can be reused with different parameters and returns fresh names each time.
Values outside IN do not create columns or remove rows from row-total aggregates.

Each retained row group is partitioned by its pivot key. Member rows move into
one cell bucket without cloning payloads. Row totals and HAVING see the complete
row group. A missing intersection returns NULL without evaluating TRANSFORM;
a nonempty cell whose operands are NULL uses ordinary aggregate rules. Thus
Count over an existing all-NULL cell is zero, while a missing Count intersection
is NULL. First/Last retain cell encounter order. ORDER BY sorts row groups and
does not change encounter order within a cell. No implicit row sort is promised.

NULL pivot values do not create dynamic headings; they remain in row totals.
Fixed NULL headings are unsupported. Dynamic/fixed naming currently accepts
nonempty, control-free Text up to 255 UTF-8 bytes and exact Byte/Int16/Int32/Int64
values formatted as invariant decimal integers. Other carriers require a future
qualified naming/conversion policy. Names are retained exactly; punctuation is
not silently rewritten. Generated names colliding with a row heading or each
other under the existing ASCII-insensitive name policy fail. Fixed duplicate
keys fail under the selected SQL comparison policy; dynamic equal keys retain
the first representative. Text key comparison still needs an explicit collation.

The combined result has at most 256 columns. Owned heading/name buffers,
partitions, output rows and sort scratch share the existing byte/row/work ceilings
and cancellation controls. All inputs, cell evaluation and naming finish before
publication; errors never produce partial query results or stdout. The original
source projection pruner retains pivot-only and aggregate-only fields while
excluding unused payloads. Native selected-index failures retain no-fallback
behavior; this feature introduces no new index selection or writes.

## Limits and qualification

This working top-level profile does not reconstruct native saved crosstabs or
make them available as named/derived sources. Broader heading carriers, native
NULL/empty/punctuation naming, TRANSFORM aliases, full coercion/collation and
native optimizer/evaluation-order fidelity remain gaps. Missing-cell wrapper
behavior and discovery/evaluation order need actual Access comparisons. Explicit
contracts above must not be described as full native crosstab compatibility.

Microsoft's [TRANSFORM statement](https://learn.microsoft.com/en-us/office/client-developer/access/desktop-database-reference/transform-statement-microsoft-access-sql)
defines row/column headings and fixed values, including values with no data.
The [crosstab guide](https://support.microsoft.com/en-us/access/make-summary-data-easier-to-read-by-using-a-crosstab-query)
distinguishes criteria from fixed heading restrictions and documents native naming
and designer behavior. Its examples do not qualify every edge case above.

Eleven authored/generated tests cover dynamic/fixed headings, row totals, filtering,
NULL/missing cells, grouping expressions, parameter reuse, metadata-only binding,
name collisions, cancellation/limits and independently computed integer grids.
[compare_crosstabs.py](testing/compare_crosstabs.py) passes ten CLI controls over
the unchanged registered ACE 2007 sample using independently read Jackcess rows
and a Python grid model, through sequential and explicit Number-index sources.
This is independent row-based evidence, not native Access SQL execution.
