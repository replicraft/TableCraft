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
required. Positive TOP counts/percentages now work; DISTINCT/OWNERACCESS/external/
action forms and TOP 0 remain rejected.

## TOP row groups

TOP limits completed row groups, after WHERE/HAVING, row/cell evaluation and final
row ordering. Counts retain the requested number of groups; percentages use the
ceiling of the percentage of surviving groups. A count above the result size or
100 PERCENT keeps all groups. TOP 0/0 PERCENT and percentages above 100 remain
unsupported. DISTINCT/TOP combinations retain their existing crosstab refusal.

When ORDER BY exists, every group tied at the cutoff across the complete sort-key
tuple survives. Row-heading expressions and row-total aliases can supply keys;
pivot-generated cell names do not become new static sort fields. Without ORDER BY,
the current executor keeps encounter groups, without promising native ordering.
The same helper now implements ordinary SELECT and crosstab TOP.

Heading discovery/fixed columns finish independently of TOP. Limiting groups never
shrinks the heading domain, bypasses limits or suppresses errors in omitted cells,
row totals, HAVING or sort keys. All groups/cells are evaluated before limiting;
this is not source/TOP pushdown. Tie comparisons retain provider errors and check
cancellation, including after the last boundary comparison and before publication.
TOP allocates no extra result buffer. These are explicit current rules, with native
Access crosstab options, ties and error ordering still awaiting Windows comparison.

## Heading and cell behavior

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

This working top-level profile now also receives [native saved crosstabs](NATIVE_SAVED_CROSSTABS.md)
through explicit retrieval. [Named crosstabs](NAMED_CROSSTABS.md) now expose literal
fixed headings to ordinary SELECT/UNION/nested consumers without binding scans.
Dynamic/computed named schemas and inline TRANSFORM sources remain unsupported.
Broader heading carriers, native
NULL/empty/punctuation naming, wider TRANSFORM alias binding, full coercion/collation and
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

Parent-level multivalue criteria remain unsupported in crosstabs. Explicit
flattened `.Value` inputs retain their existing behavior and dynamic pre-WHERE
heading domain; parent matching is not silently moved ahead of heading discovery.

Aggregate AS aliases now round-trip in owned syntax without replacing pivot-generated
column names. A trailing owner-access flag is retained in syntax and rejected by
execution. Native stored positive TOP now executes rather than being silently
omitted; the native adapter's evidence and remaining metadata gaps are
recorded separately. These additions do not qualify native dynamic heading discovery
or its WHERE/HAVING interactions.

Four focused/generated TOP tests cover counts, percentages, complete-key/NULL ties,
heading retention, omitted-cell errors and provider failure/final cancellation.
[compare_crosstab_top.py](testing/compare_crosstab_top.py) passes 782 independent
Python nullable-rank/refusal controls. Stored flags/percentages have additional
[original native inputs](FIXTURES.md#crosstab-top-inputs) and independent comparisons;
these remain model/reader evidence, not Microsoft Access SQL execution.
