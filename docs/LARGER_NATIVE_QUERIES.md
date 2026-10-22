# Explicit larger native scalar query snapshots

Sequential native scalar query loading can now exceed the default 16,384-row
snapshot ceiling under explicit caller controls. Complete-table and complete
projection caches still publish only after EOF, row-count, schema and resource
checks. Binding remains metadata-only. Query evaluation and CLI output remain
materialized/buffered; this is not streaming SQL execution or native shared-file
snapshot/locking support.

```sh
tablecraft query run 'SELECT TOP 3 ID FROM StreamRows ORDER BY ID DESC' \
  --database example.accdb --native-bytes 67108864 --native-rows 100000 \
  --native-steps 10000000 --execution-steps 10000000
```

## Independent resource controls

| Control | Default | Maximum | Scope |
| --- | --- | --- | --- |
| `--native-bytes` | 16 MiB | 256 MiB | Cumulative native reads/allocations |
| `--native-rows` | 16,384 | 1,000,000 | Cumulative native metadata/data rows and scalar snapshot retention ceiling |
| `--native-steps` | 1,000,000 | 100,000,000 | Cumulative native decoding/traversal work |
| `--execution-steps` | 1,000,000 | 100,000,000 | Shared query comparison/evaluation/discovery work |

Native budgets include catalog, query-definition and schema reads, failed work and
all tables/projections loaded during one invocation. Row counts include metadata,
so an allowance equal to a table's declared row count can still fail. Budgets never
refill between internal batches. Existing per-fetch physical read quotas remain.
Native flags require `--database` and explicitly conflict with `--sqlite`; they
cannot be silently ignored by another provider. Numeric ranges fail before I/O.

The execution-step option applies to query run over any existing source host.
It changes no database-provider permissions or source budgets. Library
`QueryLimits::default()` still uses one million steps; `DEFAULT_STEPS` preserves
that default, and `MAX_STEPS` now bounds explicit requests at one hundred million.
The same execution pool covers nested reads, named dependencies, domain operations,
schema discovery, comparisons and sorting; cancellation checks remain active.

Query allocations still have the existing 16 MiB logical allowance and 100,000-row
cumulative scanned/intermediate/output allowance. CLI output still has its 1 MiB
buffered ceiling. Larger native limits do not imply that every equally sized query
fits those independent controls. Large joins, wide values or output can still fail
without stdout. For progressive scalar export use [rows --stream](STREAMING_ROWS.md).

## Engine and adapter APIs

`tablecraft-ace::TableSnapshotLimits::new(maximum_rows, batch_rows)` validates a
positive snapshot cap up to 1,000,000 and a batch size up to 16,384, without I/O.
Defaults remain 16,384 rows and 256 rows per batch. These are consumer limits,
not native format maxima or claims that every table of that size is qualified.

A freshly opened `TableCursor` can be consumed by
`into_snapshot(limits, budget, cancelled)`. The same original controls cover
opening, fetching and retained storage. A cursor that has returned any rows fails
with `CursorAdvanced`; a previously failed cursor retains `CursorFailed` and does
not resume or call cancellation. A remaining suffix therefore cannot become a
complete-table snapshot. Default `read_table_batched` and
`read_table_projection_batched` retain their original ceilings and behavior.

`NativeSource::new_with_snapshot_limits` supplies these limits to sequential full
and projected scalar loads. The configured scalar retention ceiling also checks
complete and partial indexed snapshots before cache/visitor publication. Required
index errors retain their no-fallback contract; existing index structural ceilings
are not raised by this option. Collection/attachment/history/template readers,
index traversal expansion, editing, image retention and writes retain their
separate existing limits and qualification. No read option waives constraints,
required macros, preservation or native write qualification.

Complete caches remain available to reentrant/nested queries without holding a
mutable I/O borrow across evaluation callbacks. Failed loads publish neither
partial caches nor visitor rows; consumed budgets are not refunded. No new
library, runtime, storage format or coercion is introduced.

## Evidence

Authored 20,000-row native layouts exercise all four priority header profiles at
batch sizes 1/127/5,000, comparing exact values and one physical read per data page.
Default APIs still reject oversized tables before data reads. Focused controls
cover checked bounds, advanced/failed cursor refusal, scalar projections, count
mismatches, every observed cancellation checkpoint, metadata-only binding,
reentrant complete caches, repeated late failures and indexed retention caps.

[compare_large_queries.py](testing/compare_large_queries.py) passes 44 controls on
unchanged registered [20,000-row inputs](FIXTURES.md#original-progressive-row-inputs).
Read-only Jackcess getters supply ID rows; Python independently derives counts,
min/max, descending TOP, filters, parity groups, nested counts and UNION results.
Default snapshot/work refusals, non-refilling native row budgets, large output and
join failures retain empty stdout. Reviewed JAR/input hashes are checked. These
are independent stored-value/query-model controls, not native Access SQL, optimizer,
locale, locking or save/reopen qualification. Windows comparisons remain unrun.

```sh
python3 docs/testing/compare_large_queries.py \
  --jar path/to/reviewed/jackcess-5.0.4.jar --cli target/debug/tablecraft \
  --directory path/to/registered-stream-inputs
```
