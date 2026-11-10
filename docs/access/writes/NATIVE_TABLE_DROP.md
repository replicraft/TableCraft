# Native table deletion

Saved-table deletion uses shared runtime inspection, consuming review and the
offline save/recovery service. The desktop closes native object tabs before
requesting it; SQL drafts can remain open. See the
[confirmation workflow](../../ui/TABLE_DROP.md).

## Current populated tables

The current ACE16 route handles qualified populated tables and retained histories
after field DROP, ADD, rename, row INSERT and row DELETE. It also handles an
emptied table whose generated counters and old physical records remain. Metadata
selects this source before native qualification; failed admission is never retried
through an older table-deletion source.

Admission requires the qualified eleven-page, unencoded ACE16/subversion-zero
creation geometry and user definition page seven. This is a current implementation
boundary, not an Access file-size or table-size restriction. Complete current
declarations, explicit Name AutoCorrect OFF, program absence, object dependencies,
row ownership and qualified Int32 indexes are checked again from the original
image. Required programs, relationships, unsupported index families and unknown
ownership refuse deletion.

The actual MSysRelationships and MSysACEs definitions and inline maps must prove
empty storage. Their candidate maps are checked as subsets of owned maps, then
owned maps and declared membership must be empty. Catalog names or zero stored
row counts alone are insufficient. This route performs no permission or
relationship cleanup when such records exist; it refuses that source.

The logical deletion composes all original user-row tombstones and index member
removals, followed by the selected catalog root's tombstone and catalog live-count
decrement. It resolves forwarded roots while retaining hidden terminal payloads.
User TDEF/DATA/INDEX pages remain allocated. Their bytes outside the proven
row/index actions, plus maps, generated counter history, inactive bytes and old
property snapshots, are preserved. It does not compact, reclaim or erase storage.

A private table-DROP row family handles historical records even when current
field numbers are contiguous. It repeats the existing independent row/index
image proof without widening TCND1 or TCND2 row-deletion admission. A separate
catalog proof resolves the original owner and compares the complete byte
complement and fresh catalog identity delta.

The logical-storage model follows the reviewed UCanAccess
`Persist2Jet.dropTable` and Jackcess `TableImpl.deleteRow` behavior; these remain
[reference implementations](../../architecture/REFERENCES.md), not native Access
save/reopen evidence.

## Request and recovery boundaries

Current requests use `TCTD0002`, with an original native identity, table spelling
and complete BEFORE binding. `NativeTableDropIntent::decode_current` accepts only
that family. TCTD0001 retains its original two empty-table source profiles and its
original decoder. Neither family stores SQL, final patches or write permission.

Fresh review reconstruction and cold recovery retain the captured source family,
repeat native admission, and compare both complete journal images before effects.
The existing checked V1 journal retains its exclusive owner through recovery;
settled confirmation performs no target writes. A decoded intent or valid journal
digest cannot authorize a forged AFTER or a different native source.

Broader storage geometry, nonempty permission/relationship cleanup, arbitrary
application dependencies and Windows/Access comparison remain unfinished.
