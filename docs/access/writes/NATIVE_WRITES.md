# Bounded offline native edits

An opt-in [ACE 16 engineering preview](../../testing/ACE16_WRITE_PREVIEWS.md) now creates
physical fixed scalar and qualified Text/Binary candidates for disposable-copy
comparisons. It returns a separate,
non-journalable type and does not enable the normal ACE save/commit path. Data-macro
semantics and actual native Access qualification remain pending. The optional [declared update-cascade planner](../../testing/ACE16_WRITE_PREVIEWS.md#declared-update-cascade-batches)
expands bounded integral/Currency/GUID actions inside that engineering boundary. The qualified
Jet 4 planner follows the bounded native contract below.

The separate feature-gated [existing-page delete preview](../../testing/ACE16_DELETE_PREVIEWS.md)
now prepares ordinary ACE16 tombstones, live-count changes and exact existing-leaf
entry removal. Multi-level trees preserve surviving leaf partitions and exact
key/row ancestor bounds. Qualified empty leaves and branches retire through
existing owned/global-free map cells; a fully empty tree retains its original
root as a height-zero leaf. It returns `Ace16DeletePreview`, never a journalable
plan, and preserves all normal ACE write/data-macro gates. Qualified single Short
Text and Short Binary indexes now retain checked original key bytes during
deletion. A separate [slot-update service](../../testing/ACE16_WRITE_PREVIEWS.md#existing-width-short-text-and-binary-slots)
now re-encodes changed single/mixed Text/Binary keys, preserving actual field
spans and index topology with full final constraints. It does not resize records
or enable ordinary saves. Allocation, Text/Binary
composite deletion and ordinary SQL/backend DELETE remain unimplemented. Engine checks
and original-file candidate replay have separate evidence records in the preview
contract; neither establishes native Access save/reopen.
The separate [slot-update original corpus](../../testing/SLOT_UPDATE_OBSERVATIONS.md)
passes 124 complete candidates and 38 atomic refusals on the frozen committed
consumer, with exact retained keys, ancestor bounds and unchanged-byte proofs.
These results retain their distinction from public-writer canonicalization and
native Access save semantics.
The separate [same-page update service](../../testing/ACE16_PAGE_UPDATES.md)
can also reuse fitted record capacity or append a hidden target on the original
logical root's page. It retains logical row/index references, exclusive final
target ownership and independent opaque-byte/map audits. Slot, physical-space
and candidate-hint limits refuse without allocating another page. This remains
an engineering candidate, with ordinary ACE saves and data-macro qualification
still unavailable.
Its [fixed original-file comparison](../../testing/PAGE_UPDATE_OBSERVATIONS.md)
passes 156 candidates and 24 typed refusals, including sixteen hidden appends
and eighteen byte-identical no-ops. Earlier exact-span results retain their
original selectors, tool and counts.
The separate [variable-update service](../../testing/ACE16_VARIABLE_UPDATES.md)
adds alternate owned-page placement and EOF allocation where all required map
cells already exist. It preserves logical roots and derives exact map changes
through independent event replay. The grown image receives fresh header/catalog
and complete constraint checks through a non-journalable finalizer. General map
growth, arbitrary free-page reuse and production ACE saves remain unavailable.
Its [eight original-file candidates](../../testing/VARIABLE_UPDATE_OBSERVATIONS.md)
now pass exact image, map, value and index checks, with each shrink starting
from the corresponding generated growth candidate. The earlier PAGE corpus
retains its separate implementation and outcome counts.
The same frozen variable consumer separately passes
[eight Short Binary allocation candidates](../../testing/BINARY_VARIABLE_UPDATE_OBSERVATIONS.md),
including new EOF pages, alternate-owned reuse and fitted shrinks from generated
candidates. Its unindexed Binary field and retained primary index have complete
byte/value/map/index proofs; public padding canonicalization remains distinct.

The separate [Memo/OLE service](../../testing/ACE16_LONG_VALUE_UPDATES.md) now
replaces qualified ordinary Memo/OLE fields using column maps. Memo Unicode
compression (exact type 12, flag 2, extended flag 1) now has a measured encoding
rule and independent request-byte audit; unknown extended fields stay refused.
LVAL chunks and ordinary parent forwarding share one EOF/map sequence, with
immutable request authority and independent final audits. Old chunks remain
allocated; unknown ownership, broader field/map profiles and normal saves remain
unqualified. Its [revised frozen-consumer replay](../../testing/LONG_VALUE_UPDATE_ACCEPTANCE_OBSERVATIONS.md)
now passes all 60 unchanged raw Memo/OLE controls with complete independent image,
getter and map proofs. The initial zero-of-60 atomic census refusals and subsequent
narrow correction remain separately recorded; normal saves stay unavailable.
The compressed extension's 68 scoped runtime checks pass, including default
storage checks and reader-valid alternate-encoding forgeries. Its original-file
candidate acceptance remains a separate committed-consumer qualification.

The separate [Int32 leaf-split service](../../testing/ACE16_INDEX_SPLITS.md) now
supports root-leaf and non-root leaf growth through represented EOF index pages.
Its [root](../../testing/ROOT_SPLIT_UPDATE_OBSERVATIONS.md) and
[non-root](../../testing/NONROOT_SPLIT_UPDATE_OBSERVATIONS.md) two-file candidate
comparisons pass complete independent image proofs. The subsequent
[recursive implementation](../../testing/ACE16_INDEX_SPLITS.md) now propagates
internal-parent overflow and retained-root growth with independent full key/ROW,
capacity, framing, sibling and map audits. Its 108 scoped runtime checks pass;
original-file recursive candidate acceptance remains a separate frozen-consumer
gate. Other codecs, map growth and production saves retain their separate gaps.
The [parent-overflow reference](../../testing/PARENT_INDEX_SPLIT_OBSERVATIONS.md)
now measures two successful public-writer root-growth histories on unchanged
sparse inputs, alongside two atomic refusals from the existing kernel. Its
active tree/map geometry guides recursive implementation; public scratch bytes
are not byte goldens or native Access qualification.

Contraction now qualifies complete internal-node sibling chains by height and
repairs retained neighbors. Entirely unlinked levels retain their existing
profile; partial or malformed chains refuse. The
[deep reference corpus](../../testing/DELETE_CONTRACTION_STORAGE_OBSERVATIONS.md)
now has a separate passing 32-control candidate replay; its initial refusal
baseline remains unchanged. These comparisons do not qualify native Access saves.

Qualified ignore-NULL indexes use separate checked per-index deletion subsets:
all requested rows are tombstoned, but only present entries are removed from each
index. Partial-NULL tuples remain indexed. A physically empty index retains
root-only ownership even when NULL table rows remain, and zero-effective requests
leave its bytes unchanged. Every UPDATE blocker and complete validation step
remains in force; see the [delete-only contract](../../testing/ACE16_DELETE_PREVIEWS.md).
The [original-file comparison](../../testing/IGNORED_DELETE_OBSERVATIONS.md) passes
fifty positive candidates while keeping reference sequencing/allocation changes
separate from atomic candidate preservation.

The deletion preview also admits qualified cross-table enforced RESTRICT
relationships. Complete before/after membership checks permit child deletion and
unreferenced parent deletion, while a surviving dependent rejects the candidate
with `ConstraintViolation::MissingParent`. Exact paired integral, Currency, GUID
and matching Decimal keys retain their existing mutation qualification. All-NULL
child tuples are exempt; partially NULL tuples require their exact parent.
Self relationships, unenforced declarations and other delete actions remain
refused even for empty requests. See the
[closed relationship profile](../../testing/ACE16_DELETE_PREVIEWS.md#cross-table-enforced-restrict-relationships).
This does not enable normal saves or establish native Access action timing.
The [84-control original-file comparison](../../testing/RESTRICT_DELETE_OBSERVATIONS.md)
passes the explicit final-image model but retains six public per-row action
disagreements for NULL parent keys with all-NULL children. This is not full
RESTRICT action parity; native Access qualification remains pending.

Explicit `TableRowDeletes` groups now stage several tables against one original
image and validate one complete final image. Shared global-free bitmap bytes are
composed from typed retirement intents, with an exact union audit plus per-table
ownership and sibling proofs. Child/parent removals may therefore be explicitly
batched without an intermediate broken image. This does not dispatch cascades or
claim SQL RESTRICT action timing; the same engineering-only save boundary applies.
The [original-file batch replay](../../testing/BATCH_DELETE_OBSERVATIONS.md)
passes 28 controls, including height-two trees on both owners and exact shared
bitmap-byte retirement unions. All eight table-order pairs are byte-identical;
this remains separate from native Access action/save qualification.

The separate `preview_ace16_cascading_deletes` engineering API now expands
qualified acyclic cross-table Cascade associations using original physical roots.
Outgoing lookups require required-primary parent keys; nullable all-NULL children
remain exempt. Explicit seeds validate before deduplication, RESTRICT edges retain
final-image checks, and the common complete index/map preservation audits still
apply. Self/cyclic cascades and SET NULL remain refused. This does not enable
ordinary ACE saves or change the explicit RESTRICT-only batch API. Matching uses
[projected fixed-key facts](../../testing/CASCADE_KEY_FACTS.md); full before/final
validation still decodes unrelated Memo/OLE under unchanged limits. See
[the closed cascade contract](../../testing/ACE16_DELETE_PREVIEWS.md#separate-acyclic-cascading-deletions).
Its [original-file replay](../../testing/CASCADE_DELETE_OBSERVATIONS.md) now passes
24 supported acyclic candidates and eight closed-profile refusals. Original
record bodies, complete surviving keys, typed values and protected bytes are
checked independently. The public reference's self-tree and stack-overflow
counterexamples remain separate from those supported action comparisons.

Existing LONG AutoNumber deletion now has an
[original-file preservation comparison](../../testing/AUTONUMBER_DELETE_OBSERVATIONS.md):
sixteen candidates retain counter 260, including highest/all-row deletion, and
sixteen disposable public-writer append descendants allocate 261. This is
engineering preservation evidence, not native INSERT support or AutoNumber UPDATE
permission; GUID/complex identity and native Access saves remain separate.

`NativeImage::plan_int32_updates` produces complete before/after images without
writing to its source. The current engineering profile is unencoded Jet 4 only:
existing non-NULL fixed Int32 fields in unflagged local tables, with the native
updatable flag and no AutoNumber/extended flags. Assignments use sequential
snapshot-local row ordinals plus expected original values. They are not persistent
row identities. Duplicate, absent or stale assignments fail the entire plan.

Affected indexes must fit the existing-tree Int32 profile below; indexes on unchanged
fields must have complete supported checks. Unknown constraints, properties and property
scopes block planning. Selected table Lv/LvModule/LvExtra payloads must be
explicitly absent, and direct catalog children block writes even when these local
payloads are NULL. `NativeImage::inspect_object_children` reports bounded direct
containment for a known captured identity; an empty result does not establish
absence of streams, callbacks or indirect dependencies. Opaque or uninspected
content blocks writes. Required/AllowZeroLength and supported field/table rules
and associated qualified relationships must pass before and after edits. Defaults
remain inert because this profile changes existing records. No cascades execute.
Only Required, AllowZeroLength, ValidationRule/Text, DefaultValue, Caption and
Description property keys are admitted. Missing declarations remain absent.

The physical writer rejects selected relocated/historical rows and overlapping
fixed payloads. It changes four field bytes per changed row plus qualified
index-entry spans, without altering null masks, variable tails, row lengths,
index headers/bitmaps/padding, page allocation or file length.
A bounded independent comparison proves every byte outside these spans remains
identical. Whole-image copying and comparisons charge shared allocation/work
budgets and check cancellation between 64 KiB units. Failures publish no plan.

All ACE writes remain blocked: header recognition does not establish absence of
table data macros. Microsoft describes [data macros](https://support.microsoft.com/en-us/access/add-data-macros-in-a-desktop-database)
and [cross-version read-only behavior](https://www.microsoft.com/en-us/copilot/blog/2010/01/13/cross-version-compatibility-between-access-2010-and-access-2007/).
No unavailable macro semantics may be skipped. This Jet 4 profile does not qualify
ACE application save/reopen or VBA preservation through actual Access edits.
The [ACE 16 data-macro evidence plan](../../testing/DATA_MACROS.md) records the current
negative observations, missing positive native fixtures and acceptance before
absence detection can permit writes. XML/service documentation is not a native
ACCDB storage map, and ordinary macro enumeration does not cover table events.

## Verification and laboratory harness

Independent synthetic golden files exercise all Int32 replacements with a
256-case fixed-seed row model, exact unchanged-byte comparisons, no-op/stale/NULL
updates, AutoNumber/ACE rejection, post-edit rule failures, unknown properties,
separately owned catalog objects and cancellation. These tests require neither
Java nor extra binary fixtures.

The original [UpdateFixture.java](../../testing/UpdateFixture.java) recipe creates a
Jet 4 table with an Int32 primary index, a non-indexed range-validated Score,
Unicode memo and 12 KiB binary payloads. The engineering example writes only a
fresh disposable output:

```sh
cargo build -p tablecraft-ace --example update_int32 --locked
java -Dstdout.encoding=UTF-8 -cp path/to/jackcess-5.0.4.jar docs/testing/UpdateFixture.java new.mdb V2000
python3 docs/testing/compare_updates.py --example target/debug/examples/update_int32 --cli target/debug/tablecraft --jar path/to/jackcess-5.0.4.jar --jet new.mdb
```

The comparison confirms Score 20 becomes 27, exactly one byte differs, memo/binary
lengths survive, and Jackcess plus the native index checker reopen the output.
It additionally changes indexed Id 2 to 27, reorders its root-leaf entries and
independently reopens index order 1/3/27. Every byte outside the four-byte Id field
and existing entry-data span remains identical. CLI copies match native plans.
Rule violations, stale values and duplicate keys create no output.
Original files remain unchanged. A failed harness output write can leave an
incomplete copy; this example is not an in-place transaction. Optional journal integration is described below. Allocation, broader index mutation,
INSERT/DELETE, native shared-file
coordination and Microsoft Access comparison remain separate work.

## Existing-tree Int32 index maintenance

This fixed non-NULL profile retains byte layout. The optional ACE 16
[indexed NULL preview](../../testing/ACE16_WRITE_PREVIEWS.md#indexed-int32-null-transitions)
has a separate within-page repacking contract; it does not widen the journalable API.

Existing keys may change when every affected index is a qualified ordinary/unique/primary
single-Int32 index with a retained nonempty tree topology. Both directions and unchanged
NULL entries in ordinary indexes are supported. The original shared prefix,
entry counts and every encoded entry width must remain usable. Nonzero write stamps,
levels that differ from subtree heights, composite/text indexes, different physical owners
sharing one root and unowned physical declarations remain unsupported. Logical
aliases sharing the same physical definition receive one patch.

The writer binds changes to validated physical root references and expected keys,
resorts key/reference pairs and rechecks unique/required rules. Sorted keys are
partitioned into leaves using their original counts; node separators become the
complete maximum key/row reference of each non-tail subtree. The writer encodes
only existing entry-data regions. Page ownership, child/tail/sibling links, levels,
prefix declarations, entry-boundary bitmaps, free
space, key counts, allocation and unknown padding remain byte-identical. Existing
nonnull key replacements retain cardinality. Changes requiring a different
prefix, entry width, split or merge are rejected rather than silently rebuilt.

Physical-definition ownership is checked even when no index-key patch is needed:
unrelated/nonindexed target changes, equal-value updates and empty assignment
requests cannot bypass it. An unowned physical declaration may hide an affected
column from the logical index inventory. Normal Jet 4 plans and optional ACE 16
previews refuse such tables with `IndexedColumn` before publishing a candidate.
Fully owned logical aliases and index-free tables retain their existing permits.
Focused malformed-copy regressions cover complete and partial loss of logical owners,
normal/preview/no-op requests, cumulative work and late cancellation. These are
corrupt-metadata refusal controls, not valid-layout or native Access qualification.

The candidate receives a complete structural/index check as well as the existing
field/rule/relationship audit before publication. No-op assignments leave the
image unchanged; duplicate keys in unique indexes or later field-rule failures
return no plan. Non-unique ties retain ascending physical row-reference order
in both key directions, including ties spanning leaves and node separators.
Synthetic tests cover aliases, both directions, nullable unique keys, nested nodes,
zero-separator tail subtrees, swaps, exact opaque byte preservation,
prefix/header/height rejection and CLI output atomicity. A separate
256-case fixed-seed tree model checks both directions, NULL, arbitrary Int32
replacements and two tree heights. Empty non-root leaves and unbalanced source
levels remain unsupported.
This is an offline Jet 4 engineering profile. All ACE writes remain blocked.

Complete Boolean/numeric/GUID composite index reads/checks now work, but an update touching
any composite key remains blocked with IndexedColumn. A synthetic Jet 4 regression
confirms that an otherwise eligible update to a non-key Int32 field preserves the
entire composite index and every byte outside the exact field. Read coverage does
not supply composite key mutation, splits/recompression or ACE write permission.
Single-GUID read/check coverage now also permits an otherwise qualified unrelated
Jet 4 Int32 edit, with an exact unchanged-GUID-index/field proof. GUID targets
remain outside the Int32 editor; all ACE writes remain blocked.
Boolean index reads/checks likewise permit otherwise qualified unrelated Jet 4
Int32 edits, with every byte outside the field preserved. Boolean targets remain
outside that editor. This supplies no Boolean key mutation or ACE write permission.
Single/Double read/check qualification has the same unrelated-Int32 byte proof;
it enables no floating target edits, key mutation or ACE writes. Physical IEEE
index checks do not qualify native Access non-finite write acceptance.
Legacy DateTime indexes likewise remain byte-identical through unrelated eligible
Jet 4 Int32 edits; DateTime target/key mutation and every ACE write remain blocked.
ACE 16-family single/composite Decimal index reads/checks do not enable any ACE edit or
Decimal target/key mutation. Older numeric index variants remain unqualified.
Nodes that represent their final child with an explicit upper-bound entry and
zero tail pointer read/check correctly. The Jet 4 key writer still rejects that
topology with IndexedColumn. ACE 16 engineering previews now traverse only actual
children and rewrite every explicit bound, including the final child; no page-zero
traversal or patch is created. This extension has authored regression coverage,
not independent-reader or native Access qualification; see
[preview evidence](../../testing/ACE16_WRITE_PREVIEWS.md#bounded-final-child-nodes).

ACE 16 fixed-scalar previews also extend ordinary/unique existing-tree key
maintenance to Byte, Int16, Int64 and exact scaled Currency. Their non-NULL
replacements keep the fixed layout; eligible NULL changes use existing-page
repacking. This does not expand the public Jet 4 Int32 planner or enable normal
ACE saves. [The integral preview contract](../../testing/ACE16_WRITE_PREVIEWS.md#integral-and-currency-index-updates)
owns coverage and remaining non-integral/composite/allocation gaps. Checked
foreign-index previews have their separate contract below.

The original [multi-page fixture](../../testing/IndexUpdateFixture.java) creates 1,000
manual Int32 keys with ascending primary and descending unique indexes. Each tree
has a level-1 root and three level-0 leaves. Unicode memo and binary fields include
long external payloads. The [comparison runner](../../testing/compare_index_updates.py)
matches three cross-leaf moves and a complete 1,000-key swap against independent
row/index cursors. It compares memo/binary SHA-256 and exact bytes outside original
field/entry spans, checks both trees, matches native/CLI copies and rejects a
cross-leaf duplicate before output creation. Original input hashes remain unchanged.

```sh
java -cp path/to/jackcess-5.0.4.jar docs/testing/IndexUpdateFixture.java new-tree.mdb
python3 docs/testing/compare_index_updates.py --jar path/to/jackcess-5.0.4.jar \
  --jet new-tree.mdb --cli target/release/tablecraft --example target/release/examples/update_int32
```

The reviewed Jackcess 5.0.4 artifact and its
[IndexData reference](https://raw.githubusercontent.com/jahlborn/jackcess/be0a2139b087a227d14cafd4ea231306a5a3a862/src/main/java/com/healthmarketscience/jackcess/impl/IndexData.java)
inform page headers, encoded references, prefixes and entry-end bitmaps. These
original recipes and independent-reader comparisons do not establish Microsoft
Access write compatibility.

## Optional journaled persistence

With `tablecraft-ace/journal`, `NativeEditPlan::prepare` binds the validated before
image to `tablecraft-io`'s `prepare_expected`. The complete target must match
before any journal write. Preparation syncs the journal without changing the
target; `PreparedNativeEdit::commit` rechecks, writes/syncs the candidate and
syncs its marker. Every interrupted/failed commit requires explicit recovery
before ordinary access. Success returns changed rows and leaves the journal.
`rollback` restores the original; `recover_native_edit` performs checked generic
recovery and captures the resulting native image. Recovery is repeatable.

Hosts must supply distinct correctly paired exclusive offline resources, establish
the journal namespace durably before preparation, retain interrupted journals and
perform durable cleanup explicitly. The engine never creates/deletes/discovers
journals or implements Access shared-file locking. See [journal preconditions](../../foundation/IMAGE_JOURNAL.md).
OS locks coordinate cooperating handles, not live Microsoft Access clients.

Native integration tests cover stale targets, retained/reused journals,
interruption after a partial payload side effect, rollback/repeated recovery and
real locked-file commit/close/reopen. Generic I/O fault/corruption tests remain
separate. These are software fault and local filesystem observations, not power
loss, network filesystem or native Microsoft Access durability qualification.

The indexed-plan integration interrupts commits after the data field, after the
new root separator and between the two leaves. Inconsistent intermediate images
fail a table check; recovery restores all pages together. Successful indexed commits
and no-op multi-page images reopen/recover with the expected complete bytes.

Column fixed/updatable masks follow the pinned
[ColumnImpl reference](https://raw.githubusercontent.com/jahlborn/jackcess/be0a2139b087a227d14cafd4ea231306a5a3a862/src/main/java/com/healthmarketscience/jackcess/impl/ColumnImpl.java).
Only the property DDL flag bit is admitted, confirmed against the reviewed
Jackcess 5.0.4 PropertyMapImpl constants; other flag bits block mutation.

Aliased table definition pages are rejected before mutation, including distinct
unflagged catalog IDs sharing the same physical definition.

## Non-unique Int32 replacements

The existing-tree writer also admits ordinary non-unique single-Int32 indexes.
It applies exact expected-value edits, sorts by native key direction and then
ascending physical row reference, and rebuilds separators from complete key/row
pairs. Duplicate keys can span leaves and aliases share one physical patch. Unique/
primary and required declarations still enforce their rules, including a separate
unique index on the same field. Unchanged NULL neighbors are retained; selected
NULL transitions remain a separate writer profile. Existing prefixes, topology,
entry widths, counts, bitmaps, free space and every other byte remain fixed.

Four focused CI cases include literal leaf/node/deep/alias/descending/nullable
controls, duplicate batches, exact no-ops and prefix refusal, ACE 16 candidate
equivalence and a 128-case independent multirow key/reference model. The original
unique-index duplicate/header/rule regressions remain in place.

The inert [NonUniqueUpdateFixture recipe](../../testing/NonUniqueUpdateFixture.java)
creates fresh V2016/V2019 inputs and a Jet 4 control with duplicate and NULL keys,
non-monotonic row identities, ascending/descending indexes, shared aliases,
1,000-row trees, Unicode Memo/OLE values and unique/rule guards.
[The runner](../../testing/compare_nonunique_updates.py) accepts `--jar`, `--cli`, `--rows`,
`--preview`, `--native`, `--ace2016`, `--ace2019` and `--jet4`. It matches seven
candidates per family (including one exact no-op) against complete independent
rows/index cursors, proves unchanged bytes outside original field/entry spans,
checks every candidate, retains existing outputs and rejects unique/rule/stale/
missing/NULL requests before file creation. Jet 4 CLI copies match engine output;
ordinary ACE CLI editing remains blocked. [Recorded hashes](../../testing/FIXTURES.md#non-unique-index-update-inputs)
qualify these independent copies, not actual Access saves or broader mutations.

## Mask and foreign-index engineering boundaries

The opt-in [mask preview](../../testing/ACE16_WRITE_PREVIEWS.md#boolean-and-null-mask-transitions)
now handles Boolean bits and non-indexed nullable fixed fields, retaining payload
bytes when NULL clears presence. This is not a production save capability. Native
enforced relationships create type-2 foreign-key indexes. ACE 16 previews now
admit their checked single-column and two-to-ten-column integral mutation profiles; normal saves remain
blocked.
[Reciprocal metadata inspection](../indexes/NATIVE_INDEXES.md#reciprocal-foreign-index-links)
checks pointers and relationship-catalog agreement. Qualified
[foreign physical reads/checks](../indexes/NATIVE_INDEXES.md#foreign-physical-traversal-and-checks)
validate trees and declarations separately. The Jet 4 writer retains its explicit
foreign-index blocker. The [ACE preview contract](../../testing/ACE16_WRITE_PREVIEWS.md#checked-foreign-index-updates)
also requires complete catalog coverage and before/after membership checks;
removed/disguised foreign declarations cannot qualify a preview. Do not remove
or ignore indexes to obtain candidates. Non-integral foreign mutation,
required cascades and production saves remain implementation work.
Eligible ACE 16 indexed Int32 presence changes now have a separate
[repacking preview](../../testing/ACE16_WRITE_PREVIEWS.md#indexed-int32-null-transitions),
with unchanged page allocation and complete post-checks. A required split rejects
the whole candidate before publication.

Integral composite changes have a separate
[ACE preview contract](../../testing/ACE16_WRITE_PREVIEWS.md#integral-composite-index-updates).
Each component retains its own direction, exact carrier and NULL state; complete
tuple/row pairs drive sorting and node bounds. Composite changes may repack
existing pages even for a non-NULL replacement. This does not enable Jet 4
composite mutation, allocation or production ACE saves. Explicit multi-column/
table candidate preparation now has the separate
[batch contract](../../testing/ACE16_WRITE_PREVIEWS.md#explicit-fixed-field-batches),
with original-image expectations and complete final validation. It neither
discovers automatic actions nor establishes native transaction/durability behavior.
