# ACE16 deletion previews within existing pages

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

Every physical index must have a checked supported existing topology. A root leaf
can become empty. In a multi-level tree, every original non-root leaf must remain
nonempty; requests never redistribute entries across original leaf partitions.
Both bounded-final-child and tail-child node forms retain their exact child and
sibling pointers, heights, root identities and ownership maps. Existing write
codecs are reused: Boolean, integral/Currency, finite Single/Double/legacy
DATE, GUID and Decimal, plus the existing supported mixed composite profiles.
This does not enable Text/Binary index mutation, empty-branch/root contraction,
splits/merges, allocation or long-value reclamation. Logical aliases sharing a physical declaration receive
one edit; unrelated physical declarations sharing a root refuse.

For each selected ordinary root, the checked slot word gains both native
`tombstone` flags (`0xc000`) while retaining its payload offset. The table's live
record declaration decreases by the number of distinct requested roots. Its
location comes from the owning table-definition layout after the complete logical
definition chain is validated. Each qualified index loses the exact requested
row-reference entries, retaining encounter/tie order and repacking only existing
pages. Multi-level deletion consumes each leaf's original population before
filtering and recomputes every ancestor separator from the complete retained
key/row maximum of its child subtree. The optional tail child contributes its
subtree maximum without adding a stored separator. A leaf emptied by a batch or
an ancestor requiring a split refuses the entire candidate, even when other
indexes could complete. Deleting all rows from an existing root leaf creates a
zero-entry, zero-prefix leaf without reclaiming its page; delete-all from a node
tree remains refused. Stored historical index counters are not decremented as if
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

The initial root-leaf [implementation record](results/ace16-delete-preview-native-2026-10-26.json)
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

## Nonempty-leaf tree extension

The [tree-extension record](results/ace16-delete-trees-native-2026-10-26.json)
records 126 passing unit/integration tests, including nine new tree controls,
and three retained compile-fail documentation tests. Strict selected library,
test and example lint, minimal library compilation and twelve-file formatting
pass. Two initial lint attempts found only unused shared test helpers and
iterator style in independent test models; their logs remain preserved, with
unchanged expected results. No runtime test or production correction was needed.

This record
separates this bounded operation from general DELETE. The old equal-cardinality
update rewrite remains unchanged. The original root-leaf path also retains its
allocation behavior; only node trees use the separate original-partition
traversal. A no-op still qualifies every tree page but emits no patches, avoiding
canonicalization of unchanged valid encodings.

New independent and author tests cover both ACE16 headers, ascending/descending
keys, both child forms, two/three-level trees, logical aliases, nullable/tied
maxima including row-reference-only boundary changes, mixed-direction composite
keys, immutable maps/links/heights and original leaf membership. A numeric matrix
covers nine existing writer families. A separate checked 302-child fixture proves
that removing one maximum can reduce the shared separator prefix and require
3,625 bytes where only 3,616 are available: the operation refuses despite every
leaf remaining nonempty. Batch exhaustion, malformed topology, cumulative quotas
and cancellation remain atomic failures. Public-writer node input/candidate replay
is separate from these engine checks and is not native Access qualification.

True empty-branch/root contraction still requires qualified child removal,
sibling repairs, root-identity/height changes and page ownership/free-map behavior.
This extension neither supplies an allocator nor enables INSERT, ordinary ACE
saves or required data-macro execution.
