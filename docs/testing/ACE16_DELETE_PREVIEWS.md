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
preservation gates. The single-table and explicit batch services require the closed
cross-table enforced RESTRICT profile below. The separate acyclic cascade service
has its own action proof; unsupported actions are never silently skipped, even for
empty requests. Ignored-NULL indexes remain UPDATE blockers;
DELETE uses the separately qualified physical-membership subset described below.

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

## Explicit multi-table deletion batches

`TableRowDeletes::new(table, requests)` borrows one table name and its original
`RowDelete` requests. `NativeImage::preview_ace16_batch_deletes` accepts these groups
and returns the same disposable `Ace16DeletePreview`; `deleted_rows()` is the total
number of removed roots across all groups. The single-table entry point uses this
same planner. Neither entry point writes a source or grants journal/normal-save
permission.

All table and relationship gates run against the original image. Every requested
ordinal and raw payload is checked before index planning. Duplicate table groups,
including case-insensitive spellings, refuse rather than merge; repeated or stale
row requests still fail. Empty named groups retain their complete table, index,
relationship and raw-traversal checks. An empty group list is an exact candidate
copy under the ordinary format and resource gates. The public ceilings are
`MAX_DELETE_BATCH_TABLES` (256 groups) and `MAX_DELETE_BATCH_ROWS` (16,384 cumulative
requests), with existing per-table and shared resource ceilings also applying.

All requested records and indexes are staged from that one original image before
one complete candidate is constructed. Every selected table then passes the
existing structural and relationship checks against the final image. Explicitly
removing children and their parent can therefore succeed together when no final
reference is broken; a surviving child still causes `MissingParent`. This is a
physical final-image batch contract, not a claim about SQL RESTRICT action order
or the behavior of successive public-reader delete calls. The same closed
cross-table enforced RESTRICT profile applies even to empty groups. Self
relationships, delete cascades, SET NULL, unenforced/unknown rules and unsupported
mutation-key carriers remain refused. No automatically discovered row is deleted.

Retirement map changes are composed once for the operation. Checked per-table
retirement intents identify actual original physical pages and their owned maps;
the composer merges only typed owned-bit clears and global-free-bit sets with
identical original bytes and disjoint masks. It emits one patch for each shared
bitmap byte. Generic index or record patches are never combined through a bitwise
OR. Selected tables' complete original data and distinct physical index-owned
populations must not overlap one another or a selected table definition.

The final global audit requires exactly the union of retired-page bits in every
touched global byte and exact preservation of each retired page after its
four-byte invalidation prefix. Separate per-table audits retain original owned-map
membership, root identity, empty-root ownership and internal sibling-chain proofs.
An index can become empty while live NULL-key table rows remain. Tables with no
retirements emit no allocation edits and retain the existing nonretirement gates;
this does not qualify unrelated allocator metadata. Budgets and cancellation cover
the whole operation, and no candidate is published after any failed proof.

The [verification record](results/ace16-batch-delete-native-2026-10-26.json)
records 239 passing engine functions, including nine new controls, and three
compile-fail documentation checks. Focused tests cover complete final-image
child/parent removal, partial-NULL composites, multiple incoming edges, exact
cross-owner shared-bitmap composition, protected record/page bytes, ownership
conflicts, stale requests and typed resource/cancellation failures. Private causal
checks reject missing/extra/already-free retirement bits and changed retired page
headers or residuals. Lint, minimal compilation and scoped formatting pass.
Original-file multi-table comparisons and native Access action/save qualification
remain separate evidence work.

## Separate acyclic cascading deletions

`NativeImage::preview_ace16_cascading_deletes` accepts the same borrowed
`TableRowDeletes` groups and returns the same non-journalable `Ace16DeletePreview`.
It is a separate action-expanding service. The existing single-table and explicit
batch entrypoints retain their RESTRICT-only admission, including empty requests.
No normal CLI/backend DELETE or native save is enabled.

The new service checks every explicit seed against its original ordinary root and
complete expected record before closure deduplication. It follows only enforced,
known cross-table Cascade edges reachable from the explicitly named tables; every
reached table passes the existing complete before-image gates even when no row
matches. For every outgoing Cascade lookup that is expanded, parent columns must be
exactly a checked genuine primary key in its declared order, with requiredness
and uniqueness. Those parent components must all be non-NULL. Deleting only child
rows through an incoming Cascade edge fires no parent lookup; existing full FK
and final-membership checks apply without imposing this additional primary-key
requirement on the unmodified parent. Corresponding lookup columns use matching
existing Byte, Int16, Int32, Int64, Currency, GUID or Decimal declarations;
Decimal precision and scale must agree. Matching is exact, without floating conversion or key re-encoding.
All-NULL child tuples have no parent association. Unsupported actions, unknown or
unenforced rules, self references and reachable directed Cascade cycles refuse;
unrelated relationship components are not activated.

Associations and ordinary physical root references are captured from the original
image. A bounded iterative queue expands selected rows, processing a physical root
once even when several seeds or diamond paths reach it. Explicit duplicate/stale
requests still fail before this deduplication. Generated requests copy checked
original raw records through the existing row walker. All captured rows must have
qualified ordinary framing. No pointer into a temporary page or reconstructed
scalar record becomes an expected payload. The current association service retains
a complete second scalar snapshot after each full before-check, including
unindexed Memo/OLE payloads. This bounded materialization is charged and can
exhaust the allowance earlier than an explicit raw-row batch; matching is not
payload-pruned.

The resulting complete groups enter the existing batch planner through a private
validated action proof. The same table/row ceilings apply to the expanded set,
not just explicit seeds, and all capture, association, queue, key comparison,
allocation and replay work shares one resource budget and cancellation callback.
Restriction edges are validated against the complete final image, just as in the
explicit batch service. This is an engineering final-image model; it does not
claim the order or error behavior of SQL or successive mixed-action DELETE calls.
All index, typed map-composition, global union and per-table root/ownership/sibling
proofs remain required before candidate publication.

Separate fresh public-builder and read-only getter captures cover both ACE16
profiles, a chain, a diamond, overlapping/reversed seeds and all seven declared
carrier families; their source and original-file receipts remain a separate
evidence promotion. Twenty-four acyclic public Jackcess actions matched frozen
original-association models, complete typed rows and logical index orders; NULL
child sentinels remained. These are independent-reader observations, not native
Microsoft Access qualification. Separate bounded self probes are contrary evidence:
both self-tree actions returned success while leaving a dangling child; all six
loop/cycle actions raised `StackOverflowError` under a 1 MiB JVM stack. Their
partial copies were preserved and reopened; the cyclic failures left the original
bytes unchanged. These cases remain refused rather than being qualified merely
because the engineering work queue could terminate.

The [implementation verification record](results/ace16-cascading-delete-native-2026-10-26.json)
records 255 distinct passing unit/integration functions, including 16 new controls,
and three compile-fail documentation checks. Independent tests cover frozen
association expansion, NULL exemption, explicit/generated overlap, valid stored
cycle refusal, directional primary qualification, final-image Restrict semantics,
shared-byte two-owner contraction, exact record/page preservation, stale inputs,
unsupported rules and typed budget/cancellation failures. The prior source and
logs for two test-construction compile errors and an incorrect unknown-rule read
coverage assertion remain preserved. No production failure or row-answer change
was needed. Strict lint, minimal compilation and scoped formatting pass. These
checks do not qualify a native Access action or save.

## Evidence boundaries

The original [delete-storage observations](DELETE_STORAGE_OBSERVATIONS.md)
establish the slot/count/empty-leaf/counter behavior through public Jackcess
operations, reopened getters and independent raw-byte checks. The indexed table
in that historical corpus includes an ignored-NULL index. Earlier deletion
milestones refused it; the explicit deletion extension below retains that stored
flag and requires complete eligible-root checks. Read qualification alone is not
mutation authorization. Fresh positive writer fixtures must use public builders
for admitted profiles rather than clearing stored flags in those originals.

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
Text/Binary composite mutation and unsupported sort families remain outside this
profile. Ignored-NULL deletion follows the additional contract below. The normal
mutation gate still refuses indexed
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
Memo/composite/then-existing ignored-NULL refusal variants and the special descending
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

## Original-file Short Text and Short Binary acceptance

The separate [encoded-key original replay](results/ace16-delete-encoded-internal-chains-originals-2026-10-26.json)
passes 48 controls on the same committed tool: 36 candidates and twelve
unsupported-profile refusals. Both previously refused Modern Text height-two
delete-all selections now succeed. The original files and selectors are unchanged.

The [reference recipes and reproduction tools](ENCODED_DELETE_OBSERVATIONS.md)
cover General legacy/2010, both directions, NULL/empty values, ties, Binary segment
boundaries and supported nonunique Text overflow digests. Independent getters,
exact retained encoded keys/references and whole-file preservation checks cover
4,374 removed rows, 8,676 surviving target rows and 158 retired pages. Earlier
refusals and source versions remain recorded separately. These engineering
comparisons do not enable normal ACE writes or qualify native Access saves.

## DELETE with ignored-NULL physical membership

An ignored-NULL index omits a root only when all its key components are NULL;
partially NULL composites remain indexed. Deletion now admits that behavior for
the existing qualified DELETE codec families. It requires a complete checked
before-image: sequential roots establish the eligible population independently,
and complete physical traversal proves distinct live roots, key/record agreement
and exact eligible membership. Read support alone or a selected range is
insufficient. Every UPDATE entry point still refuses ignored indexes, including
empty updates.

For each physical index, the preview captures an effective request subset by
intersecting requested physical row references with the complete original decoded
leaf-key stream. Logical aliases share that physical operation. Each deletion
algorithm still requires exactly `removed == effective_requests.len()`; ordinary
indexes retain the original complete request slice. Record identity, tombstones
and the table count use every original request, including omitted roots. No
missing index entry is invented and no second decoder interprets request payloads
to guess NULL membership.

An empty effective subset qualifies the original supported profile/topology and
emits no index rewriting, repacking or retirement. NULL-only deletion therefore
preserves affected index pages byte-for-byte. Empty Text/Binary values are indexed
values and differ from NULL. Unsupported Memo, variable composite, sort, digest,
relationship or active-content profiles still refuse even an empty-effective
request. Stored index flags and historical counters are preserved.

Deleting every eligible entry can empty an index while NULL table rows remain
alive. After complete post-edit membership checks, the ownership audit reads the
actual physical root under the shared allowance. An empty root must be a
height-zero leaf with zero sibling/tail/prefix/entry bitmap and full free-entry
capacity, and its owned set must contain only that root. Table row count alone
does not establish physical-index emptiness. Other indexes independently remove
their own effective subsets, with existing chain/map/preservation checks before
any candidate is exposed.

Fixed-tuple deletion explicitly uses physical-membership constructor policy with
original metadata, followed by the existing mutation-codec checks. Default
complete-table constructors and all UPDATE gates are unchanged; no stored flag
is cleared to borrow a different profile. Captured request vectors and membership
scans use fallible allocation, cancellation and cumulative budgets. This remains
a separate non-journalable engineering candidate, not ordinary ACE DELETE or
native Access save qualification.

The [ignored-index DELETE implementation record](results/ace16-delete-ignored-native-2026-10-26.json)
records 221 passing engine functions, including nine new integration controls:
two author profile matrices and seven independent multi-index/constraint controls.
The remaining functions include prior writer/DELETE coverage and twenty-five
direct ignored-index read/check regressions. Three compile-fail docs, strict
selected library/test/example lint, minimal standalone compilation and
fourteen-file formatting pass. Original public-writer replay against a frozen
artifact is separate and is not counted here.

The independent fixture initially inherited a passive Column Score property while
naming that column A. The unchanged property gate correctly refused it. Only its
authored column name and matching no-op update name changed; index/value goldens
stayed intact. Two older ignored-index DELETE negatives now check valid deletion
and unchanged empty/nonempty UPDATE refusal; their original failing outcomes and
source snapshots are retained. A missing test import during this migration was
corrected after preserving both compile logs. No production behavior was weakened
to address these fixture or test-authoring failures.

## Original-file ignore-NULL acceptance

The [frozen-tool replay](results/ace16-delete-ignored-original-replay-2026-10-26.json)
passes all fifty positive selections on six unchanged registered originals.
Independent checks cover per-index eligible subsets, exact retained encoded
keys/references, original leaf partitions and separator maxima, maps, roots and
all bytes outside authorized changes. Thirty-two zero-effective index cases keep
their complete index pages unchanged; twenty-four physical indexes become empty
while NULL table rows remain.

The [reference and candidate observations](IGNORED_DELETE_OBSERVATIONS.md) distinguish
sequential reference allocation/repacking from the atomic batch contract. Two
reference files grow six pages and add bitmap/index pages. Their candidates keep
the original file length, nonempty heights and map representations, reaching the
same final values and keys. Reference-retired residuals can reflect intermediate
deletions; candidates preserve original retired-page residual bytes. No selector,
request or budget was relaxed, and native Access qualification remains separate.

## Cross-table enforced RESTRICT relationships

Deletion now admits known, enforced cross-table relationships whose declared
delete action is RESTRICT. Known update actions may be RESTRICT or Cascade;
deletion does not rewrite surviving key values or dispatch update actions. Delete
Cascade, SetNull, unknown/inherited semantics, unenforced declarations and
self-relationships remain refused, including empty requests. This profile makes
no SQL statement-timing or self-co-deletion claim.

Before rule admission, the existing selected-table checks still validate complete
index structure, reciprocal native FK/catalog metadata, stored membership and the
separate mutation-carrier profile. Supported relationship keys remain exact paired
Byte, Int16/32/64, Currency, GUID or Decimal fields, with matching Decimal precision
and scale. One-to-one and one-to-many checks are retained. Read qualification
alone does not expand that mutation-carrier allowlist.

Child deletion and unreferenced parent deletion use the existing raw-root,
table-count and index machinery. The final image must pass complete membership
for every touching relationship again. A removed parent with any surviving
dependent therefore returns the typed `ConstraintViolation::MissingParent` error
and exposes no candidate.
All-NULL child tuples remain exempt; partially NULL tuples require their exact
parent. Every touching edge is checked, so a second incoming edge can block an
otherwise unreferenced parent removal.

There is no new foreign-key encoder, key-matching recipe or cascade dispatcher.
Unmodified related tables and relationship/index declarations remain covered by
the unchanged-byte proof. Properties, history, active content, existing UPDATE
and ignored-index contracts, cumulative budgets/cancellation and non-journalable
candidate isolation remain. Normal ACE/backend SQL DELETE, macro eligibility and
native Access save/reopen are not enabled by this extension.

The [RESTRICT implementation record](results/ace16-delete-restrict-native-2026-10-26.json)
records 230 passing engine functions: nine new assertion functions and 221
existing DELETE/read/writer/private regressions. Two author tests reuse the
peer-authored synthetic cross-table fixture with their own expected rows and
preservation assertions; seven peer tests cover multiple edges, partial NULLs,
metadata/action refusals and failure atomicity. This is shared synthetic fixture
coverage, separate from future original public-writer storage evidence.

Three compile-fail docs, strict selected library/test/example lint, minimal
standalone compilation and seven-file formatting pass. A causal healthy
unenforced fixture first passes ordinary structure and NotEnforced read checks,
then verifies exact DELETE refusal for empty and nonempty requests. Runtime
expectations required no corrections. Initial unused-import/shared-helper warnings
were resolved without production lint suppression; the final helper spacing
change is cosmetic. Self-reference timing and repaired original-file candidate
replay remain outside this implementation record.
