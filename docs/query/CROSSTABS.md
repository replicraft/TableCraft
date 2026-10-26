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
values formatted as invariant decimal integers. Boolean uses `True`/`False`.
Single, Double, Currency and Decimal use the explicit CStr NumberFormat; legacy
and extended Date/Time use its explicit DateTextFormat. Missing required policies
fail even for fixed headings over empty tables. Binary and Guid remain unsupported.
These are explicit provisional names, not qualified native Access spelling.
Keys remain typed for sorting, matching and cell membership: different keys whose
renderings collide fail rather than merging cells. Every candidate is validated,
including duplicates, and formatting allocations share the execution budget. Names are retained exactly; punctuation is
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

This working top-level profile now also receives [native saved crosstabs](../access/queries/NATIVE_SAVED_CROSSTABS.md)
through explicit retrieval. [Named crosstabs](NAMED_CROSSTABS.md) now expose literal
fixed headings to ordinary SELECT/UNION/nested consumers without binding scans.
Qualified dynamic/computed named schemas use explicit
[execution-phase discovery](NAMED_SCHEMA_DISCOVERY.md); metadata-only preparation
still refuses contextual headings. Boolean fixed headings can bind without a host.
Inline TRANSFORM sources remain unsupported. Native scalar heading spelling,
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
[compare_crosstabs.py](../testing/compare_crosstabs.py) passes ten CLI controls over
the unchanged registered ACE 2007 sample using independently read Jackcess rows
and a Python grid model, through sequential and explicit Number-index sources.
This is independent row-based evidence, not native Access SQL execution.

## Collection membership criteria

```sql
TRANSFORM Count(*)
SELECT Id
FROM Items
WHERE Tags.Value = 7
GROUP BY Id
PIVOT Category;
```

Crosstab WHERE clauses now support the explicit
[parent-membership and mixed-query rules](collections/MULTIVALUE_QUERIES.md#parent-membership-criteria).
Wholly hidden members fold once per parent/pair without multiplying cells by
matching item count. Collections referenced by row headings, GROUP BY, TRANSFORM
or **PIVOT** expand into aligned children; their hidden siblings remain scalar
inputs for that same child. A pivot-only `.Value` therefore retains ordinary
flattened filtering, while another wholly hidden collection uses membership.
Joined aliases, absent/empty/stored-NULL distinctions, negative atoms, parameters
and normal grouping/HAVING/TOP retain their existing explicit contracts.

Binding waits until the pivot dependencies are known before classifying hidden
members. It includes the pivot in the shared temporary pruning keys, then removes
it from executable row grouping. Preparation still reads metadata only. Dynamic
execution evaluates each source-row pivot once **before** membership, discovers
its heading, and privately carries that value through matching without rescanning
or reevaluation. Rejected parents still supply headings and can fail discovery
or its width limit. Fixed IN mode evaluates membership before each retained-row
pivot, preserving the existing phase distinction. The new private callback adds
no public source contract or core-value identity, and shares all query budgets.

Fixed literal named crosstabs use the same membership execution without changing
their metadata-only schema. An outer filter/projection/TOP cannot hide an inner
cell or decoder error. Native collection opt-in and selected-index refusals still
apply; decoded FileData requires the separate explicit decoder option. Hidden
collection predicates in HAVING/ORDER BY remain unsupported.
[Multi-member WHERE atoms](collections/MULTIVALUE_QUERIES.md#several-hidden-members-in-one-atom)
now retain same-child alignment or independent actual-item products. [ON members](collections/MULTIVALUE_QUERIES.md#collection-members-in-join-conditions)
now drive child expansion independently of output; WHERE siblings then test that
same child, while other hidden collections retain parent membership. [Subqueries inside membership atoms](collections/MULTIVALUE_QUERIES.md#subqueries-inside-membership-criteria)
now retain explicit captured-item scopes, including nested and named retrieval.

Eight new multivalue tests and one aligned named-member test cover fixed/dynamic
headings, pivot-only expansion, mixed/joined/outer/named scopes, independent
nullable grids, discovery-once observations, pre-WHERE errors/width limits,
parameters, malformed providers, metadata changes, budgets and every measured
execution cancellation checkpoint. Native/CLI regressions retain complete caches
and buffered errors across the current ACE families. The independent parent and
attachment recipes now pass 225 and 162 controls respectively, including
subqueries and ON child joins, with unchanged original ACE 2007/2016/2019 hashes. Microsoft documents crosstab WHERE criteria, fixed
headings and multivalued query concepts in the references above and the
[multivalued query guide](https://support.microsoft.com/en-us/access/using-lookup-and-multivalued-fields-in-queries).
Those references and independent getter/model tests do not qualify this complete
collection/crosstab profile in Microsoft Access; native SQL, NULL/negation,
error-order and decoded-binary representation remain unrun.

Aggregate AS aliases now round-trip in owned syntax without replacing pivot-generated
column names. A trailing owner-access flag is retained in syntax and rejected by
execution. Native stored positive TOP now executes rather than being silently
omitted; the native adapter's evidence and remaining metadata gaps are
recorded separately. These additions do not qualify native dynamic heading discovery
or its WHERE/HAVING interactions.

Four focused/generated TOP tests cover counts, percentages, complete-key/NULL ties,
heading retention, omitted-cell errors and provider failure/final cancellation.
[compare_crosstab_top.py](../testing/compare_crosstab_top.py) passes 782 independent
Python nullable-rank/refusal controls. Stored flags/percentages have additional
[original native inputs](../testing/FIXTURES.md#crosstab-top-inputs) and independent comparisons;
these remain model/reader evidence, not Microsoft Access SQL execution.

## Scalar heading verification

Eight focused tests cover all newly admitted carriers, per-execution formats,
metadata-only Boolean binding, contextual named discovery, collisions, eager
validation and every observed cancellation checkpoint. The original MIT
[scalar pivot runner](../testing/compare_scalar_pivots.py) passes 68 independent
controls. It derives names/counts using Python and read-only Jackcess stored-value
getters across four unchanged 20,000-row inputs: Jet 4, ACE 2007 and ACE 16
V2016/V2019. It also checks fixed missing columns, missing policies, overlong
names and collision refusals. Native Access SQL is not executed; exact native
heading spelling, sorting and formatting remain unqualified.
