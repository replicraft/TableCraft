# Saved-table catalog tombstone observations

The [35-image record](../../results/reading/table-drop-catalog-observations-2026-11-09.json)
compares independently authored first-party raw AFTER images with public Jackcess
5.2.0 system-catalog row deletion. Separate experiments execute full UCanAccess
5.2.0 SQL `DROP TABLE` under two native cursor preconditions. No Microsoft Access
execution or normal TableCraft save is qualified by these probes alone.

The raw sources cover headers 5/6, empty/populated unindexed tables, seven retained
Int32 index families, and nine creator variants with explicit Name AutoCorrect
OFF. Native Id 7 identifies the sole user table. Catalog slot 8 is used by the
inline-OFF layouts; slot 7 precedes the creator's later database-property record.

Every SDK-written image exactly matches the raw model: catalog TDEF2's live count
changes from nine to eight, and the selected DATA4 slot gains both deletion and
overflow bits (`0xc000`). Exactly two bytes differ. Physical catalog rows, free
space, all user definition/data/index/property bytes, every usage/allocation map
and EOF remain unchanged. Fresh SDK reopening reports no user tables and eight
live catalog records. This is logical removal with retained historical storage,
not page retirement, compaction or reusable-space qualification.

The initial strict-name attempt completed four ASCII/Unicode writes, then refused
a BOM source before modification. The remaining 31 distinct inputs used exact
native-Id selection; no completed write was repeated. Four BOM sources have an
SDK name-getter disagreement. A separate readonly observation returns UTF-16 units
`feff fffd 0074 fffd 006d fffd` for the raw escaped name intended as FEFF + `Items`.
This is not evidence that the SDK merely strips a leading BOM. Whole-image row
deletion remains exact, while positive name-getter qualification remains absent.

## Actual SQL DROP and native cursor state

The ordinary UCanAccess SQL path executes `DROP TABLE`, explicitly commits and
freshly reopens 31 non-BOM source copies. Opening alone leaves every input byte
unchanged. All 31 AFTERs exactly match the catalog-only model, including populated
sources. This initially surprising result is not general populated-DROP evidence.

UCanAccess `Persist2Jet.dropTable` source and bytecode contain additional user-row,
ACE and relationship deletion steps. A distinct four-case probe establishes that
ordinary loading leaves the native table's default cursor after-last while
Hibernate support is false. Explicitly resetting that cursor before SQL DROP
changes the resulting native operation:

| Source | Original user rows | Changed bytes after reset | Catalog-only model |
| --- | --- | --- | --- |
| Empty unindexed | 0 | 2 | Exact match |
| Populated unindexed | 1 | 4 | Additional user live-count and DATA tombstone changes |
| Retained indexed tree | 2 | 25 | Additional DATA, INDEX contraction and allocation changes |
| Creator primary index | 1 | 6 | Additional user live-count, DATA and INDEX changes |

All four reset cases reopen with no user table and eight live catalog rows. Their
different cursor state is an explicit independent precondition, not a repeated
identical success counted twice. Populated catalog-only outcomes are therefore
cursor-dependent and cannot authorize a general populated table writer. The raw
catalog-only model is suitable only for independently admitted empty-table work
until complete row/index/event composition receives its own expected images.

Jackcess page-deallocation source was reviewed separately and is not applied by
the raw catalog model. No Access comparison, required-program execution, general
allocation reclamation or multi-table behavior follows from these experiments.

First-party raw builders live in
[`canonical_table_drop.rs`](../../../crates/engineering/tablecraft-testkit/src/fixtures/access/canonical_table_drop.rs).
Expected bytes never come from the production mutation engine. SDK jars and source
archives remain ignored reference artifacts with retained hashes/license review;
no third-party code or database fixture was copied into the repository. The record
pins all BEFORE/expected/actual image hashes, observation source hashes, failed
attempts and the exact current raw-builder sources. Its new Rust unit tests still
require the coordinated root verification batch.
