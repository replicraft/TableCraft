# Explicit cached calculation queries

`query run --database PATH --stored-calculations` explicitly exposes qualified
stored calculation results to the normal read-only query executor:

```sh
tablecraft query run 'SELECT ID,Amount FROM CalculatedDecimals' \
  --database example.accdb --stored-calculations
```

These are cached values, potentially stale. The flag does not evaluate formulas,
prove freshness, save data or enable calculated columns in ordinary sources.
Output aliases retain SQL names; the explicit command option identifies the
chosen cached-value policy. Saved/named queries use the same opted-in source.
The flag requires a native database and is unavailable for SQLite and templates.

Library hosts select `NativeSource::with_stored_calculations()` before discovering
any table schema. Switching an already discovered source is rejected.
`is_stored_calculation(table, ordinal)` reports metadata-only provenance for
exposed scalar columns. Preparation validates the existing typed ResultType,
Expression and physical layout without reading cached values or executing code.
Unsupported declarations remain unavailable while unrelated ordinary fields can
still be queried. Cached Boolean fields remain nullable, and calculated Decimal
raw precision/scale bytes do not become ordinary Decimal field constraints.

The engine's explicit `TableCursor::open_stored_projection` reuses its existing
source-bound traversal and complete materialization. Columns retain native logical
order. Selected cached fields use their qualified stored decoders, including
Byte, Int16, Int64, Single, GUID, Decimal and Memo; ordinary-only and zero-width projections do not decode caches.
Complete caches publish only after EOF, count/schema and cumulative control checks.
A failed/advanced cursor suffix cannot become a complete table. TOP and WHERE do
not suppress malformed selected cache values later in a complete sequential or
full-index snapshot. Qualified typed key selections have the separate partial
read scope below.

Stored Single/Double caches may be inspected or browsed with non-finite IEEE
values, while query snapshots retain their existing finite-number admission
policy. A selected non-finite source value therefore refuses the whole query
snapshot before output; this is separate from malformed cached storage.

An explicitly required ordinary native index can return cached scalar outputs
through `read_stored_table_index_with_request`, or through
`read_stored_table_index_with_references` when logical ROW identity is needed.
Both readers accept the existing qualified typed ordinary-key selections as well
as complete `All` traversals. They bind fresh qualified ResultType/Expression
declarations to the indexed table schema, decode stored results without executing
formulas, and validate every physical key against its corresponding record,
including unprojected keys. Missing, unsupported or corrupt required indexes
never fall back to sequential reads.

Only existing wholly safe source constraints select cached query candidates;
WHERE remains in the evaluator. Selected reads cap returned rows rather than the
whole table declaration, and validate traversed pages and returned records.
Skipped subtrees and unselected payloads remain outside that partial coverage.
Conservative duplicate-key boundaries can require visiting a neighboring subtree
even when its returned rows do not match. An empty selection still qualifies
selected cached declarations before publishing a result.

Full and projected query snapshots retain their schema and complete-cache checks.
Partial selections remain temporary and never become complete table/projection
caches. A later unconstrained read must load the complete relation. Automatic
index hints use `read_stored_table_index_in_table_order` to preserve original
logical page/slot order; required indexes preserve their configured key order.
Neither path executes formulas or adds progressive indexed execution.
Cached scalars may also accompany [stored collection members](../queries/INDEXED_COLLECTION_QUERIES.md)
through checked logical parent references. Per-field text
identities and explicit host comparers retain the existing comparison contract.
Recomputed previews continue rejecting calculated dependencies even on this
source, and remain separate from cached results.

The selected index must use ordinary eligible key fields. Microsoft documents
that users cannot index Calculated, OLE Object or Attachment fields in
[Create and use an index to improve performance](https://support.microsoft.com/en-us/access/create-and-use-an-index-to-improve-performance).
Refusing a calculated key follows that Access restriction; reading a cached
output through an ordinary key does not create an index on the cached field.

The focused synthetic checks cover metadata-only opening/binding, intentionally
stale values, NULL, one-row batches and complete cache reuse, ordinary projection
pruning over corrupt caches, late failure, required/automatic index boundaries,
unsupported declarations, quotas, cancellation and buffered CLI publication.
These checks establish the explicit stored-read profile, not native Access query
recalculation, application outcomes or freshness. Windows qualification remains
separate from independent storage/getter evidence.


The original [cached-query replay](../../testing/compare_cached_queries.py) passes 92
getter/query controls against unchanged Decimal and Memo inputs in both ACE
2016/2019 profiles. Exact coefficients/scales, Unicode/NULL/empty values, length,
count, filtering, empty tables, ordinary refusal and corrupted-copy pruning are
checked through a fixed CLI. The [report](../../testing/results/cached-calculation-queries-2026-10-24.json)
retains input, source, oracle and binary hashes. These are independent stored-value
checks; only TableCraft executes SQL. No native Access query qualification is
implied, and original fixture bytes are unchanged.
