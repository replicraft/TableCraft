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

Every physical index must have a checked supported existing topology. Requests
remove entries within their original leaf partitions without redistributing
retained entries. Empty nonroot leaves and branches can be retired when their
existing owned and global-free bitmap cells qualify. Every nonempty node retains
its original height and bounded-final-child or tail-child form; surviving leaf
links are repaired in encounter order. Qualified internal chains are repaired
separately at each original height. An entirely empty tree retains its root
page as a height-zero leaf. Logical aliases sharing a physical declaration receive
one edit; unrelated physical declarations sharing a root refuse.

Existing write codecs are reused: Boolean, integral/Currency, finite
Single/Double/legacy DATE, GUID and Decimal, plus the supported mixed composite
profiles. DELETE additionally preserves original keys for qualified single Short
Text and Short Binary indexes, as described below; UPDATE retains its earlier
codec boundaries. Page allocation, map growth/window shifts, nonempty root
promotion and long-value reclamation remain unsupported. These
limits do not prevent qualified existing-page contraction, including a retained
single-child chain at its original heights.

For each selected ordinary root, the checked slot word gains both native
`tombstone` flags (`0xc000`) while retaining its payload offset. The table's live
record declaration decreases by the number of distinct requested roots. Its
location comes from the owning table-definition layout after the complete logical
definition chain is validated. Each qualified index loses the exact requested
row-reference entries, retaining encounter/tie order and repacking only existing
pages. Multi-level deletion consumes each leaf's original population before
filtering and recomputes every ancestor separator from the complete retained
key/row maximum of its child subtree. The optional tail child contributes its
subtree maximum without adding a stored separator. An ancestor requiring a split
refuses the entire candidate, even when other
indexes could complete. Deleting all rows converts each original index root to a
zero-entry, zero-prefix height-zero leaf and retires its descendants.
Stored historical index counters are not decremented as if
they were current live counts.

Record bodies, neighboring slots, Memo/OLE storage, file length and all bytes
outside the authorized slot/count/index/map spans remain unchanged. Retirement
clears owned bits, sets global-free bits and zeros only the retired page's first
four bytes; its remaining bytes stay intact.
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

The nonempty-leaf record above describes its historical implementation boundary.
Existing-page contraction is described below; it does not supply an allocator or
enable INSERT, ordinary ACE saves or required data-macro execution.

## Existing-page contraction

When deletion empties an original leaf or branch, the preview removes that child
from its original parent and recomputes complete retained key/row maxima. A
nonempty root or internal node keeps its height, including a bounded single-child
chain. The original tail/bounded representation is retained from actual metadata;
page counts alone never choose a node form. An empty complete tree reuses its
original root as an empty leaf. Required separator growth that cannot fit an
existing page refuses the complete operation.

Retirement uses existing represented cells in inline or indirect owned and
global-free maps. It does not enumerate future global bits as owned pages,
allocate bitmap pages, change pointers, slide an inline window or grow a row.
Map containers qualify by role: owned-map containers have owner zero, and the
fixed global page-one/row-zero container has owner one. Missing representation,
map/data/index overlap, cross-index ownership overlap or an already-free live
page refuses the candidate. Global header/map storage, selected table-definition
and data-owned pages, and index-owned pages must be represented as allocated.
This qualification concerns the touched table and supporting metadata; it is not
a database-wide allocator-coherence proof.

Edits sharing a bitmap byte are merged before candidate construction; conflicting
set/clear requests fail. Root/data/map storage cannot be retired. Full structural
and supported constraint checks run after edits, followed by an independent audit
of exact owned-set removal, global-byte additions, retained roots/storage and
retired headers/residual bytes. Every other bit, including beyond-EOF global
free bits, remains unchanged. Operations with no retirement retain their earlier
allocation behavior and do not start requiring global-map metadata.

Review identified two safety gaps before acceptance. The first draft trusted a
map container's own owner declaration; it now requires the explicit role owners.
A causal synthetic probe also showed that an allocator-free map container could
produce a candidate even though ordinary table checks passed. Reserved, map,
table-definition and target-data allocation checks now reject that input. The
unsafe synthetic source/candidate and failed probe are retained separately from
accepted results. Ordinary table checks are not presented as global allocation
qualification.

The [contraction implementation record](results/ace16-delete-contraction-native-2026-10-26.json)
records 138 passing engine unit/integration functions: twelve new contraction
controls, twenty-two earlier DELETE controls, ninety-eight writer regressions and
six private encoding controls. Three compile-fail documentation tests also pass.
The separately committed request-file consumer contributes four passing tests;
these are reported separately from engine totals. Selected library/test/example
strict lint and the no-default-features library check pass. Original public-writer
candidate replay is a separate evidence step and is not counted here.

The earlier exhaustion-negative fixtures have no global allocation metadata.
Their refusal now reports malformed allocation metadata instead of unsupported
tree contraction; their original bytes, no-candidate expectation and no-retirement
positive controls remain intact. A diagnostic records the exact changed error.
One expanded lint run found the resulting unused test import; it was removed
without changing an expected result. Superseded green checks, failed causal probes
and diagnostic capture setup failures remain identified in the record.

## Preserved Short Text and Short Binary keys

Deletion admits single native Short Text keys with the existing qualified General
sort metadata, and single native Short Binary keys with their checked storage
length. Both directions use the original semantic read profile for comparisons.
Every index must pass complete before/after checks. The physical mutation captures
the exact reconstructed key bytes supplied to the checked decoder, after shared
prefix reconstruction and before the separately checked row/child trailers. It
copies retained bytes rather than generating new Text weights or re-encoding a
semantic Binary value. Parent bounds use the retained child's complete original
key and row reference.

Captured semantic keys and wire bytes are owned, fallibly allocated and charged.
Retained ancestor copies re-decode the original bytes under cumulative work and
allocation controls. The existing page grammar is unchanged. A separate deletion
transport shares checked page repacking and contraction; it implements no UPDATE
operation, column inclusion or dummy mutation profile. Full before/after checks
still encode row values for semantic agreement, so this is not an encoder-free
operation.

Nonunique Short Text overflow/digest entries can qualify through the existing
complete check; unique overflow/digest ambiguity remains refused. Memo/Long Text,
Text/Binary composite mutation, unsupported sort families and ignored-NULL indexes
remain outside this profile. The normal mutation gate still refuses indexed
Binary UPDATE, including empty update requests. Deletion's explicit admission
changes only that early blocker; property, history, relationship, active-content,
constraint, preservation and macro qualification boundaries remain unchanged.
Existing topology and mutation safeguards remain in force; internal sibling
chains must qualify as described below.

The [encoded-key implementation record](results/ace16-delete-encoded-native-2026-10-26.json)
records 178 passing engine functions, including ten new controls: two private
preserved-wire/resource controls and eight independent public-contract tests.
Root-leaf Short Text General families and Binary cases run in both ACE16 headers;
node/contraction cases cover both existing child forms. Three compile-fail docs,
strict selected library/test/example lint, minimal standalone compilation and
sixteen-file formatting pass. No original public-writer candidate replay is
counted by this record.

The first independent Binary fixture declared fixed storage while constructing
variable records. An ordinary reader/check diagnostic established that failure
before deletion; correcting only its native variable flag made both directions
readable and checkable. Successful wire/value goldens stayed unchanged. Exact
Memo/composite/ignored-NULL refusal variants and the existing special descending
Binary empty-value order were also corrected in newly authored expectations.
Initial attempts and diagnostic sources remain preserved separately. No production
correction followed these test-authoring failures.

## Internal sibling chains during contraction

Before emitting contraction patches, the preview captures every original internal
node in depth-first child order, separately for each original height. Each level
must have either all previous/next pointers zero, preserving the earlier unlinked
profile, or the complete reciprocal chain for that exact sequence. Partial,
asymmetric, cyclic, cross-level and root sibling links refuse. The root is the
sole node at its height and must have zero siblings. This additional preflight
applies to contraction; existing no-op and no-retirement behavior is unchanged.

For a linked level, removing branches filters that original sequence and repairs
only surviving nodes' previous/next links to their retained neighbors. Unlinked
levels stay zero; retired pages retain their original residual bytes. Nonempty
nodes keep their original heights and actual bounded/tail child form. A wholly
empty index keeps its original root as an empty height-zero leaf and retires every
original nonroot page. Its final owned set must contain only that root.

A separate final audit checks the actual candidate after ordinary structure,
constraint, map and preservation checks. It independently walks the original node
sequence to verify retained reciprocal neighbors, node kinds/heights/owners and
zero endpoints. For a proven-empty root it checks leaf kind, zero height,
links/tail/prefix/entry bitmap and full free-entry capacity, together with complete
original nonroot retirement. A private causal test calls this audit directly:
valid survivor/empty-root states pass, while forged dangling links, header changes
and omitted retirement fail even without invoking the repair producer.

Captured levels/page IDs, survivor vectors, patches and final scans use the same
cumulative budget and cancellation contract. This does not change UPDATE,
original encoded-key transport, allocation-map roles, macro eligibility or normal
ACE write capability. The preserved public-writer baseline describes an older
frozen binary's internal-chain refusals; a repaired original-file candidate replay
requires a new frozen artifact and separate acceptance record.

The [internal-chain implementation record](results/ace16-delete-internal-chains-native-2026-10-26.json)
records 187 passing engine functions, including nine new controls: two author
integration tests, six independent integration tests and one direct after-audit
causal test. The remaining 178 functions retain prior DELETE, encoded-key,
read/check and writer coverage. Three compile-fail docs, strict selected
library/test/example lint, minimal standalone compilation and fifteen-file
formatting pass. Both ACE16 headers, linked and all-zero levels, first/middle/last
branch retirement, mixed child forms, singleton/root-empty states, exact residual
bytes, malformed links, cumulative quotas/cancellation and retry are covered.
Original-file replay against a repaired frozen executable is separate and is not
counted by this engine implementation record.

The first library compilation exposed only method visibility and an unused
import. Two initial strict attempts found unused shared-helper re-exports in the
new author and independent tests; each test now uses the original helper through
its shared wrapper. Logs remain retained. Runtime tests and their independent
expected values required no correction.

## Original-file internal-chain acceptance

The [repaired frozen-tool replay](results/ace16-delete-internal-chains-originals-2026-10-26.json)
passes all 32 controls: twenty candidates and twelve typed refusals over 96
commands. All source/request pins remain unchanged, and request bytes match the
initial refusal baseline. The sixteen previously unmet positives now pass with
their original selections and budgets. This is separate from the implementation
record above and does not count repeated historical runs as new controls.

Independent getters and raw graph/map checks verify retained rows, exact
key/reference boundaries, per-height sibling chains and unchanged unrelated
bytes. The [owning observations](DELETE_CONTRACTION_STORAGE_OBSERVATIONS.md#accepted-original-replay-with-internal-sibling-chains)
record valid candidate tail-only singleton internals versus bounded reference
internals; the comparison establishes semantic/preservation agreement rather
than identical reference bytes or native Access save/reopen qualification.
