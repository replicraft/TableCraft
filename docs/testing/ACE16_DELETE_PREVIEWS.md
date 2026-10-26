# ACE16 root-leaf deletion previews

With `tablecraft-ace/ace16-write-preview`,
`NativeImage::preview_ace16_deletes` prepares a complete disposable candidate for
ordinary live-record deletion. It returns `Ace16DeletePreview`, which exposes
`before_bytes()`, `candidate_bytes()` and `deleted_rows()`. It has no journal,
apply or conversion to `NativeEditPlan`. The feature is disabled by default and
adds no ordinary CLI/backend ACE write capability. Data-macro absence/execution
and native Microsoft Access save/reopen remain unqualified.

## Requests and physical scope

`RowDelete::new(row, expected)` owns the exact original record bytes for a
snapshot-local sequential live-row ordinal. The payload includes the stored
column-count header, fixed/variable regions and final NULL/Boolean mask, and
excludes the slot-directory word and page header. It is not a serialized scalar
row or a persistent record identifier. Checked ordinary-row traversal validates
the framing and compares every expected byte. Missing, duplicate, stale or
relocated requests fail the whole operation. Expected payloads larger than a
native page refuse before comparison; no payload coercion or second row decoder
is introduced.

The preview admits only plain subversion-zero ACE2016/2019 images and keeps the
existing structural, property, active-content, schema-history, constraint and
preservation gates. Every declared relationship touching the selected table is refused,
including an empty request; this slice does not implement RESTRICT/cascades or
skip required relationship actions. Ignored-NULL indexes remain write blockers,
even when their physical membership is completely readable/checkable.

Every physical index must have a checked ordinary single root leaf. Existing
write codecs are reused: Boolean, integral/Currency, finite Single/Double/legacy
DATE, GUID and Decimal, plus the existing supported mixed composite profiles.
This does not enable Text/Binary index mutation, nodes, splits/merges, allocation
or long-value reclamation. Logical aliases sharing a physical declaration receive
one edit; unrelated physical declarations sharing a root refuse.

For each selected ordinary root, the checked slot word gains both native
`tombstone` flags (`0xc000`) while retaining its payload offset. The table's live
record declaration decreases by the number of distinct requested roots. Its
location comes from the owning table-definition layout after the complete logical
definition chain is validated. Each qualified index loses the exact requested
row-reference entries, retaining encounter/tie order and repacking its existing
root leaf. Deleting all rows creates a zero-entry, zero-prefix leaf without
reclaiming its page. Stored historical index counters are not decremented as if
they were current live counts.

Record bodies, neighboring slots, Memo/OLE storage, allocation maps, file length
and all bytes outside the authorized slot/count/index spans remain unchanged.
Deleted data are not erased. Full post-edit structure and supported constraint
checks run before a candidate can be returned, including unindexed tombstones.
Cumulative bytes, row and work budgets and cancellation apply to preflight,
traversal, page encoding, candidate copying and the unchanged-byte proof. Failures
publish no candidate and never write to the source.

## Evidence boundaries

The original [delete-storage observations](DELETE_STORAGE_OBSERVATIONS.md)
establish the slot/count/empty-leaf/counter behavior through public Jackcess
operations, reopened getters and independent raw-byte checks. The indexed table
in that historical corpus includes an ignored-NULL index and remains a write
refusal; read qualification is not mutation authorization. Fresh positive writer
fixtures must use public builders for admitted profiles rather than clearing
stored flags in those originals.

Focused engine tests cover the numeric codec matrix, arbitrary Int32 delete sets,
slot/count/payload and unrelated-byte preservation, nonunique ties, NULLs,
ascending/descending/composite indexes, logical aliases, delete-all/empty roots,
no-ops, stale requests, corrupted storage, unsupported gates, quotas and
cancellation. Independent getter comparison of materialized candidates is a
separate evidence step. Synthetic tests and Jackcess comparisons do not establish
native Microsoft Access save/reopen or permit normal ACE writes.

## Focused implementation verification

The [implementation record](results/ace16-delete-preview-native-2026-10-26.json)
records 117 passing unit/integration tests: thirteen new and 104 existing writer
and page-encoding regressions. Three compile-fail documentation tests also pass,
including the new candidate/qualified-plan separation. Strict library, author and
independent targets and the working example pass; the no-default-features library
check and twenty-file formatting check pass. The author numeric matrix covers
nine carrier families in both ACE16 headers and both directions; a separate
32-case generated Int32 model checks arbitrary keys and delete sets. Independent
checks cover slot/count/payload preservation, aliases, NULLs, ties, composites,
empty roots, conflicts, gates, corruption, budgets and cancellation.

Review found that read-profile expansion had made plain subversion-one images
reachable through the old format-only preview helper. A retained synthetic probe
confirmed admission before the fix. The shared helper now enforces the existing
published plain subversion-zero boundary for all engineering previews; tests
explicitly distinguish successful read capture from deletion and existing Int32
preview refusal. Read eligibility and normal ACE write gates are unchanged.

One regression compile initially exhausted disk space before running tests; the
original log is retained and the complete relevant batch passed after cleanup.
Two initial strict runs found only example chunk-iteration style and unused shared
test-helper warnings, corrected without lint suppressions on production code or
changes to expected runtime results. Their logs and the temporary header probe
remain separate from accepted results. Public-writer candidate replay is not
counted as passing by this implementation record.
