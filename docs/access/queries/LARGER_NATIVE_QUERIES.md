# Native scalar query snapshots and explicit budgets

Native scalar query loading has no implicit 16,384-row snapshot ceiling.
Complete-table and projection caches publish only after successful EOF, schema,
row-count and resource checks. Binding remains metadata-only. Buffered retrieval
and explicit progressive execution retain their separate publication contracts.

```sh
tablecraft query run 'SELECT TOP 3 ID FROM Items ORDER BY ID DESC' \
  --database example.accdb
```

## Optional resource controls

`--native-bytes`, `--native-rows` and `--native-steps` select cumulative native
budgets. `--execution-bytes`, `--execution-rows` and `--execution-steps` select
independent cumulative query budgets. Omitting a control adds no product quota;
explicit values are honored without silently clamping them to former maxima.
Individual snapshots use the existing unsigned 32-bit row API representation.
Cumulative native and query counters use unsigned 64-bit representations, so
one row pool may span multiple tables and scans.

Native accounting includes metadata, failed work and every selected source.
Query accounting includes intermediate rows, dependencies, domains, comparisons
and sorting. Counters do not refill between batches. Explicit limits can stop an
operation with a resource error; they are neither Access format limits nor
up-front allocation reservations. Cancellation, fallible allocation and checked
counter arithmetic remain active.

Per-value encoded/native text policies use actual wire and representation bounds;
see [encoded values](../storage/LARGER_ENCODED_VALUES.md) and
[decoded Memos](../storage/LARGER_MEMOS.md). Individual query Text and final CLI
output likewise have no implicit product-size quota. Explicit `--text-bytes` and
`--output-bytes` still select separate caller budgets. These controls do not
waive native declaration validation or permit unsupported query semantics.

## Engine and adapter APIs

`TableSnapshotLimits::new(maximum_rows, batch_rows)` accepts a positive unsigned
32-bit retained-row budget. Its default uses that representation bound; it does
not allocate that many slots. Batch sizes continue to select fetch granularity.
Cursor materialization grows only for actual rows and publishes a complete cache
only after EOF. An advanced or failed cursor cannot publish its suffix as a
complete relation.

Explicit index requests retain selected-read failures and no-fallback semantics.
Source geometry, key/owner validation and revision checks remain mandatory.
Some legacy traversal and implementation restrictions are still separate work;
removing an arbitrary snapshot cap does not establish full native compatibility.
Mutation qualification, constraints, required programs and preservation remain
independent of read resource controls.

## Historical verification

The following records describe their captured implementations and selected
budgets. Their former product ceilings are superseded by the policy above.

### Recorded evidence

Authored 20,000-row native layouts exercise all four priority header profiles at
batch sizes 1/127/5,000, comparing exact values and one physical read per data page.
Default APIs still reject oversized tables before data reads. Focused controls
cover checked bounds, advanced/failed cursor refusal, scalar projections, count
mismatches, every observed cancellation checkpoint, metadata-only binding,
reentrant complete caches, repeated late failures and indexed retention caps.

[compare_large_queries.py](../../../tests/access/reading/tables/compare_large_queries.py) passes 44 controls on
unchanged registered [20,000-row inputs](../../../tests/reports/reading/FIXTURES.md#original-progressive-row-inputs).
Read-only Jackcess getters supply ID rows; Python independently derives counts,
min/max, descending TOP, filters, parity groups, nested counts and UNION results.
Default snapshot/work refusals, non-refilling native row budgets, large output and
join failures retain empty stdout. Reviewed JAR/input hashes are checked. These
are independent stored-value/query-model controls, not native Access SQL, optimizer,
locale, locking or save/reopen qualification. Windows comparisons remain unrun.

```sh
python3 tests/run.py tests/access/reading/tables/compare_large_queries.py \
  --jar path/to/reviewed/jackcess-5.0.4.jar --cli target/debug/tablecraft \
  --directory path/to/registered-stream-inputs
```

Focused indexed-request controls cover invalid limits before I/O, exact full and
zero-width retention, partial match caps, and full-count refusal after a partial
seek. The oversized-declaration test is deliberately inconsistent metadata; it
checks seek scope and does not qualify a genuinely large index. Existing projected,
GUID and adapter Int64/retention tests cover unchanged key and cache boundaries.

## Larger indexed evidence

[compare_large_indexes.py](../../../tests/access/indexes/native/compare_large_indexes.py) passes 288 controls
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
corruption. [Provenance](../../../tests/reports/reading/FIXTURES.md#original-large-index-inputs) records generation
and hashes; no generated database is tracked.

These inputs establish the measured 20,000-row profile, not arbitrary key
cardinality, size, depth or locality. Physical-read quotas remain in force;
widely scattered keys can still exceed them. This is neither a performance claim
nor native Access SQL, locale, concurrent-read or save/reopen qualification.

```sh
python3 tests/run.py tests/access/indexes/native/compare_large_indexes.py \
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
shared query allocation pool. See [larger payloads](../storage/LARGE_PAYLOADS.md).

## Larger scalar histories (23 October 2026)

The original [scale/history recipe](../../../tests/access/reading/tables/ScaleHistoryFixture.java) performs
100,000 inserts in scattered key order, adds a nullable Long column halfway
through, grows selected Memo values, deletes every 101st original identity and
appends 100 rows. Each ACE 16, ACE 2007 and Jet 4 input retains 99,110 rows.
The [independent runner](../../../tests/access/reading/tables/compare_scale_histories.py) checks every stored
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

Registered sizes/hashes and reproduction are in [FIXTURES.md](../../../tests/reports/reading/FIXTURES.md#larger-scalar-history-inputs).
Originals remain unchanged. This establishes those sequential histories, not
arbitrary million-row, collection/index, native Access SQL or live-file coverage.

## Mixed scalar storage histories

A separate [original corpus](../../../tests/reports/rows/MIXED_SCALAR_HISTORIES.md) now checks 19,902
retained rows per ACE 16, ACE 2007 and Jet 4 file across shared scalar types.
Schema extension crosses the sixteen-field null-mask boundary; updates grow
Memo/OLE values and populate previously absent late fields. Forty-five controls
compare complete/projected cursors, CLI streams, pages and bounded queries with
independent Python models/read-only getters, retaining original hashes. This
adds storage-history evidence without changing limits or qualifying arbitrary
type evolution, live-file snapshots, native query behavior or writes.
