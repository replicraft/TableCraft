# Normal ACE row deletion

Normal DELETE is a separate source-bound operation from UPDATE and the
[engineering deletion copies](SQL_DELETE_COPIES.md). The engine, typed Access
editor, retained offline session and `query journal-delete` use the same normal
planner and original-request journal lifecycle.

```sh
tablecraft query journal-delete 'DELETE FROM Sentinel WHERE [ID]=23' \
  --database offline.accdb --journal fresh-delete-journal
```

## Admission

The initial route accepts the qualified native seventeen-field catalog family
in unencoded ACE2016/2019 subversion-zero images. It freshly derives the
[target-local program certificate](NATIVE_TARGET_PROGRAM_ABSENCE.md), strict
selected declarations and full normal Source protection. Supported passive
properties and readonly Memo/OLE siblings retain their existing qualification.
Unknown code/property carriers, linked targets and incomplete ownership refuse.
The initial fifteen-field global data-only family is unchanged; its DATA grammar
does not admit these deletion tombstones.

The selected table must have no indexes and no relationship involving it.
Unrelated relationships and application objects remain protected; they do not
authorize cascades or code execution. Required data programs are not executed by
this route. The positive BeforeChange UPDATE route does not grant DELETE
admission. Empty selections still perform the complete action and source checks.

## Original roots and preservation

`NativeImage::prepare_ordinary_row_deletes` captures exact original resolved
record bytes for snapshot-local ordinals. Duplicates, missing rows, stale raw
witnesses and ambiguous terminal ownership refuse. Preparation grants no save
permission. The recoverable planner repeats all source and witness checks.

Direct and qualified forwarded roots retain their logical identity. Forwarding
is resolved through the existing bounded native reader and complete owned-page
root census. Only the original logical root slot receives the deleted and
relocated bits; the table definition's live-row count decreases once per removed
root. The forwarding pointer/capsule, resolved hidden record, readonly long-value
storage, other slots, page free counts, maps and file length remain unchanged.
There is no payload erasure, compaction, LVAL reclamation, page retirement,
allocation or index repair.

A fresh independent proof captures BEFORE again, resolves the original roots,
binds the complete raw witnesses and reconstructs only the slot/count changes.
It compares the entire AFTER image and recaptures complete final Source and
constraints with the reduced row count. It does not trust staged slot choices or
use UPDATE's equal-row-count validator. The independently authored forwarded
witness is the one-hop EOF layout produced by the separately qualified growth
source; it is not evidence for every historical forwarding layout.

## Journal and shared consumers

`NativeDeleteIntent` uses separate `TCND0001` framing. It retains the original
table, ordered row ordinals, full raw expected records and complete BEFORE
binding. It stores neither SQL nor final patches nor an absence permission.
TCNI1–7 UPDATE records retain their existing formats and dispatch.

`plan_recoverable_ordinary_row_deletes` returns a distinct normal deletion plan.
Its V1 preparation uses the existing fixed-length durable journal. Recovery
consumes the same checked journal, verifies the original binding, freshly
replans DELETE and compares both complete journal images before any target write.
Settled confirmation verifies current bytes without replay. A decoded intent or
stored AFTER alone never authorizes recovery.

`NativeJournalEditor` and `OfflineTableEditor` implement the shared
`DeletableTable` contract. Handles belong to their issuing Snapshot; raw witnesses
come from that same immutable image. `StagedDelete` exposes selected/deleted counts
before journal creation and uses the existing apply/receipt/retirement/cleanup
path. The refreshed snapshot after deletion issues new handles because live
ordinals may change. Planning failure, interruption and finalization failure
retain the same session resources and existing known-outcome rules.

`OfflineWriteSession::plan_delete` uses the existing shared SQL predicate planner.
All predicates evaluate against the original snapshot before the backend is
consumed once, including zero matches. Parameters, external sources, joins and
hidden providers refuse. The CLI adds no native mutation logic. Its success
report contains Transaction, Matched rows, Deleted rows and Journal namespace;
report allocation precedes mutation and publication follows release.

The existing `recover-update` and `finalize-update` commands also recognize TCND
through the host's closed action dispatch. They never retry an invalid DELETE
record as UPDATE and never replay SQL. Offline exclusivity, budgets, platform and
namespace obligations remain in [offline sessions](OFFLINE_WRITE_SESSIONS.md).

## Qualification boundary

Independent direct-root and forwarded-root source models precede their producer
calls. Each source tranche passes four native read views and four independent
SDK observations with original inputs unchanged. The forwarded witness preserves
the separately authored EOF hidden record and complete map bytes.

Five direct-root and four forwarded-root native functions pass. Four same-session typed/SQL workflows
perform eight exact growth/deletion commits; two CLI workflows perform four exact
commits and four row readbacks. Every complete image matches its frozen expected
bytes. Source qualification is recorded separately from these producer results.
See the [forwarded source record](../../testing/results/ace16-forwarded-delete-sources-2026-11-01.json).
The combined batch passes 1,114 ACE units, 79 adapter units, 98 CLI units and
83 recovery regressions. Strict all-feature, journal-only, preview-only and
minimal builds pass. Prior EOF session/CLI and recovery checks also pass. See the
[native write record](../../testing/results/ace16-normal-delete-writes-2026-11-01.json)
and [retained host record](../../testing/results/ace16-retained-normal-delete-2026-11-01.json)
for the exact scope and preserved first failures.

Microsoft Access save/reopen and Windows locking/durability conformance remain
unrun. Broader INSERT, indexed DELETE, cascades, reclamation and general
program-bearing DELETE remain separate work.

The separate [normal explicit-row INSERT route](NORMAL_ACE_INSERT.md) now uses
the same session lifecycle with distinct original typed requests.
