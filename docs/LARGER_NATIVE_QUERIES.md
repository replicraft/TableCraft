# Explicit larger native scalar query snapshots

Native scalar query loading can now exceed the default 16,384-row
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
| `--native-rows` | 16,384 | 1,000,000 | Cumulative native metadata/data rows and scalar/collection-table retention ceiling |
| `--native-steps` | 1,000,000 | 100,000,000 | Cumulative native decoding/traversal work |
| `--execution-bytes` | 16 MiB | 256 MiB | Shared cumulative query allocations |
| `--execution-rows` | 100,000 | 1,000,000 | Shared scanned/intermediate/output rows |
| `--execution-steps` | 1,000,000 | 100,000,000 | Shared query comparison/evaluation/discovery work |

Native budgets include catalog, query-definition and schema reads, failed work and
all tables/projections loaded during one invocation. Row counts include metadata,
so an allowance equal to a table's declared row count can still fail. Budgets never
refill between internal batches. Existing per-fetch physical read quotas remain.
Native flags require `--database` and explicitly conflict with `--sqlite`; they
cannot be silently ignored by another provider. Numeric ranges fail before I/O.

The three execution options apply to query run over any existing source host.
It changes no database-provider permissions or source budgets. Library
`QueryLimits::default()` still uses one million steps; `DEFAULT_STEPS` preserves
that default, and `MAX_STEPS` now bounds explicit requests at one hundred million.
The same execution pool covers nested reads, named dependencies, domain operations,
schema discovery, comparisons and sorting; cancellation checks remain active.

Query allocations retain the default 16 MiB logical allowance and 100,000-row
cumulative scanned/intermediate/output allowance. Explicit `--execution-bytes`
and `--execution-rows` can raise those separate pools to 256 MiB and 1,000,000.
Library DEFAULT_BYTES/DEFAULT_ROWS retain the original defaults; MAX_BYTES/MAX_ROWS
clamp explicit library requests to the same maxima. Budgets are cumulative
accounting, not up-front reservations or a process-wide heap quota. CLI output defaults to its 1 MiB
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

`NativeSource::new_with_snapshot_limits` supplies these limits to sequential and
indexed full/projected scalar loads and [complete collection tables](LARGER_NATIVE_COLLECTIONS.md). Indexed requests use
`read_table_index_with_request` and `IndexReadRequest`: optional scalar column
names, a typed selection and a positive retained-row ceiling up to 1,000,000.
Complete index reads require the declared count to fit and agree with the result.
Partial seeks cap matching rows rather than the declared table size; a small point
lookup may therefore succeed in a table larger than the default snapshot ceiling.
Only visited index pages and selected roots are checked by a partial seek, which
never certifies a complete table or populates a complete-table cache.

Legacy engine index entry points retain their original 16,384-row table guard.
Required index errors retain their no-fallback contract. Index depth, mapped-page,
physical-read and resource quotas remain independent; index traversal materializes
its selection rather than using the sequential batch size. Independent 20,000-row
indexed comparisons now cover the profiles below; the accepted ceiling is not
evidence that all tables up to 1,000,000 rows can be read.
Legacy collection/attachment/history entry points, template readers, editing,
image retention and writes retain separate default limits and qualification.
Explicit larger collection APIs are described [separately](LARGER_NATIVE_COLLECTIONS.md).
No read option waives constraints, required macros, preservation or native write
qualification.

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

Focused indexed-request controls cover invalid limits before I/O, exact full and
zero-width retention, partial match caps, and full-count refusal after a partial
seek. The oversized-declaration test is deliberately inconsistent metadata; it
checks seek scope and does not qualify a genuinely large index. Existing projected,
GUID and adapter Int64/retention tests cover unchanged key and cache boundaries.

## Larger indexed evidence

[compare_large_indexes.py](testing/compare_large_indexes.py) passes 288 controls
on four original 20,000-row files: Jet 4, ACE 2007, ACE 2016 and ACE 2019. Each
file has seventeen indexes; the ACE 16-family files add a Large Number index.
Read-only Jackcess cursor/getter order supplies expected IDs, while Python checks
complete unique ID coverage and the original nullable integer pattern. Every
index is exercised with a complete ID projection, zero-output-field Count and a
second scalar output projection. Families include ascending/descending Int32,
nullable duplicate keys, mixed-direction integer tuples, Byte/Int16/Boolean,
Currency, Single/Double, legacy DateTime, GUID, Short Binary, General Short Text,
Decimal and ACE 16 Large Number. Hidden key fields still undergo row/key checks.

Default-budget Int32 point/range/miss, duplicate integer/NULL, GUID equality/NULL
and exact Large Number range controls agree with independent cursor filters.
Nested complete counts after selected outer reads preserve the full table.
Default complete-read refusal, explicit small row budgets, missing mandatory
indexes and eager expression failures retain empty stdout. Original hashes are
verified before and after reads. Synthetic Rust 20,000-row trees additionally
cover full/projected/zero-width values, both directions across four priority
headers, exact ceilings, unchanged legacy API refusal and selected hidden-key
corruption. [Provenance](FIXTURES.md#original-large-index-inputs) records generation
and hashes; no generated database is tracked.

These inputs establish the measured 20,000-row profile, not arbitrary key
cardinality, size, depth or locality. Physical-read quotas remain in force;
widely scattered keys can still exceed them. This is neither a performance claim
nor native Access SQL, locale, concurrent-read or save/reopen qualification.

```sh
python3 docs/testing/compare_large_indexes.py \
  --jar path/to/reviewed/jackcess-5.0.4.jar --cli target/debug/tablecraft \
  --directory path/to/registered-large-index-inputs
```

With `--generate`, the runner creates fresh files from the original recipe,
prints their hashes and checks the same outcomes and unchanged bytes. Existing
outputs are refused. Fresh database metadata can change hashes; replay without
`--generate` requires the registered inputs exactly. Generation disables per-row
autosync for fixture construction and explicitly flushes/closes; it provides no
transaction/durability evidence.


Explicit `query run --text-bytes` and `--output-bytes` permit larger individual
Text values and final output under separate ceilings. They do not increase the
shared query allocation pool. See [larger payloads](LARGE_PAYLOADS.md).

## Larger scalar histories (23 October 2026)

The original [scale/history recipe](testing/ScaleHistoryFixture.java) performs
100,000 inserts in scattered key order, adds a nullable Long column halfway
through, grows selected Memo values, deletes every 101st original identity and
appends 100 rows. Each ACE 16, ACE 2007 and Jet 4 input retains 99,110 rows.
The [independent runner](testing/compare_scale_histories.py) checks every stored
field against a separate Python model and read-only Jackcess getters. Twenty-four
CLI controls verify aggregates, Unicode Memo lengths, sorting, deleted identities,
complete streamed values and distinct default native/query byte/row refusals.

For equivalent larger queries, request both native and query resources explicitly:

```sh
tablecraft query run 'SELECT Count(*),Sum(ID),Count(Later) FROM ScaleRows' \
  --database example.accdb --native-bytes 268435456 --native-rows 1000000 \
  --native-steps 100000000 --execution-bytes 268435456 \
  --execution-rows 1000000 --execution-steps 100000000
```

Registered sizes/hashes and reproduction are in [FIXTURES.md](FIXTURES.md#larger-scalar-history-inputs).
Originals remain unchanged. This establishes those sequential histories, not
arbitrary million-row, collection/index, native Access SQL or live-file coverage.
