# Bounded offline native edits

`NativeImage::plan_int32_updates` produces complete before/after images without
writing to its source. The current engineering profile is unencoded Jet 4 only:
existing non-NULL fixed Int32 fields in unflagged local tables, with the native
updatable flag and no AutoNumber/extended flags. Assignments use sequential
snapshot-local row ordinals plus expected original values. They are not persistent
row identities. Duplicate, absent or stale assignments fail the entire plan.

Affected indexes must fit the unique existing-tree profile below; indexes on unchanged
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

## Verification and laboratory harness

Independent synthetic golden files exercise all Int32 replacements with a
256-case fixed-seed row model, exact unchanged-byte comparisons, no-op/stale/NULL
updates, AutoNumber/ACE rejection, post-edit rule failures, unknown properties,
separately owned catalog objects and cancellation. These tests require neither
Java nor extra binary fixtures.

The original [UpdateFixture.java](testing/UpdateFixture.java) recipe creates a
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

## Unique existing-tree Int32 index maintenance

Existing keys may change when every affected index is a qualified unique/primary
single-Int32 index with a retained nonempty tree topology. Both directions and unchanged
NULL entries in ordinary unique indexes are supported. The original shared prefix,
entry counts and every encoded entry width must remain usable. Nonzero write stamps,
levels that differ from subtree heights, nonunique/composite/text indexes, different physical owners
sharing one root and unowned physical declarations remain unsupported. Logical
aliases sharing the same physical definition receive one patch.

The writer binds changes to validated physical root references and expected keys,
resorts key/reference pairs and rechecks unique/required rules. Sorted keys are
partitioned into leaves using their original counts; node separators become the
complete maximum key/row reference of each non-tail subtree. The writer encodes
only existing entry-data regions. Page ownership, child/tail/sibling links, levels,
prefix declarations, entry-boundary bitmaps, free
space, key counts, allocation and unknown padding remain byte-identical. Existing
nonnull unique key replacements retain cardinality. Changes requiring a different
prefix, entry width, split or merge are rejected rather than silently rebuilt.

The candidate receives a complete structural/index check as well as the existing
field/rule/relationship audit before publication. No-op assignments leave the
image unchanged; duplicate keys or later field-rule failures return no plan.
Synthetic tests cover aliases, both directions, nullable unique keys, nested nodes,
zero-separator tail subtrees, swaps, exact opaque byte preservation,
prefix/nonunique/header/height rejection and CLI output atomicity. A separate
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
ACE 16-family single-Decimal index reads/checks do not enable any ACE edit or
Decimal target/key mutation. Older numeric index variants remain unqualified.
Nodes that represent their final child with an explicit upper-bound entry and
zero tail pointer now read/check correctly. Key mutation still rejects that valid
topology with IndexedColumn; it cannot fall through to a page-zero rewrite.

The original [multi-page fixture](testing/IndexUpdateFixture.java) creates 1,000
manual Int32 keys with ascending primary and descending unique indexes. Each tree
has a level-1 root and three level-0 leaves. Unicode memo and binary fields include
long external payloads. The [comparison runner](testing/compare_index_updates.py)
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
journals or implements Access shared-file locking. See [journal preconditions](IMAGE_JOURNAL.md).
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
