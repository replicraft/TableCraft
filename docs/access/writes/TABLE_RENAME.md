# Source-bound persisted table rename

Table rename now has a connected native planner, original intent, offline adapter,
runtime review and shared Navigation Pane workflow. This is a restricted schema
operation, not general schema editing or dependency repair. The local batched
acceptance checks pass within this profile; native Microsoft Access save/reopen
and timestamp behavior remain unqualified.

Current GUI-created databases do not yet enter this profile: creators retain the
missing/ON Name AutoCorrect default and seven original system roles; the usual
indexed ID and optional field properties also require broader qualification.
This is not complete usable table CRUD for the creator baseline. Existing frozen
construction/recovery profiles must not be silently changed to force admission.

## Native source admission

`NativeImage::plan_table_rename` admits the unencoded, subversion-zero native
`Ace2016` and `Ace2019` storage profiles under standard complete-image capture.
Those stored profiles do not identify an Access UI release. Jet 3/4 and older ACE
profiles remain outside this operation, regardless of their separate read or
row-edit support.

The current source must contain exactly one ordinary local user table and the
closed native system roles: DatabaseRoot, Tables, Databases, Relationships,
MSysObjects, MSysRelationships, MSysACEs and MSysDb. Native parent/type/system
identities must agree. The source must establish all of the following:

- The role-qualified MSysDb has one unnamed default property group containing
  exactly `Track Name AutoCorrect Info` and `Perform Name AutoCorrect`, each a
  native LONG with the DDL flag and value zero. Missing properties do not mean
  disabled: missing, enabled or malformed settings refuse the operation.
- Catalog field numbers and visible variable indices exhaust their stored
  geometry. Retired slots cannot conceal unvisited payloads. Ordinary layout
  parsing separately rejects duplicate variable indices.
- Catalog variable payloads are absent apart from names and the checked inline
  database properties. NameMaps, saved queries, forms/reports, macros, links,
  unqualified properties or other application dependencies are refused rather
  than rewritten or executed. Other system tables must have no records.
- All local tables have no physical or logical indexes. User fields have ordinary
  scalar storage without calculated/complex or Memo/OLE payloads. This profile
  does not qualify catalog index maintenance or broader object graphs.
- Definition, DATA, map-storage and represented allocation roles agree. Free
  candidate pages must belong to that same table's owned DATA membership. The
  catalog DATA page cannot alias another table's definition or map storage.
  Every local table has an ordinary unflagged positive native ID matching its
  actual definition page; masked-ID aliases are not admitted. Unknown definition
  tails and unqualified LVAL declarations also refuse.

The proposed spelling obeys the native identifier restrictions and the
64-UTF-16-unit limit. Namespace collisions are checked against other native
objects using the catalog Name column's stored, supported General comparison
policy; an unsupported comparison policy refuses. Original table selection
supports exact spelling with ASCII-insensitive lookup. It does not claim arbitrary
locale-insensitive lookup aliases.

## Exact mutation and original intent

An additional explicit `NativeImage::plan_indexed_table_rename` planner now
constructs unpublished candidates with unchanged user-table single-Int32 indexes.
It retains the closed single-table catalog, explicit disabled Name AutoCorrect
and unindexed system-table requirements. The retained reader must check complete
key/row populations and physical owned membership; shared member-role proof
excludes INDEX/DATA/TDEF/map/LVAL aliases, and all index map storage and members
must be globally allocated. Generated primary keys, logical aliases of one tree,
retained trees and inactive tails remain unchanged by the catalog rename.
Original INDEX write-stamp bytes are preserved too; the zero-stamp restriction
for rewriting an INDEX page does not apply to this catalog-only mutation.

The original `plan_table_rename`, recoverable planner and frozen `TCRN0001`
reconstruction remain unindexed. Four focused tests pass, including 42 complete independent
BEFORE/AFTER pairs across both stored headers, and key/membership/dependency,
cancellation, budget and frozen-profile refusals. The shared ownership filter
passes one active case; three historical pinned-original cases remain ignored.
The journal-only, no-default-features engine check also passes, exercising its
alternate LVAL inventory implementation.

The first 22-test rename batch passed 21 cases and exposed an incorrect refusal
expectation for the unchanged INDEX write stamp. Reclassifying it as explicit
opaque-byte preservation passes the four affected tests; the other 18 unchanged
rename/intent cases were not repeated. This is focused engine evidence, not a
whole-workspace gate. Indexed SDK getter observations, native Access comparisons,
and the new code's strict lint remain unrun. Ordinary creators still refuse.

The separate engine `plan_recoverable_indexed_table_rename` API now seals the
indexed planner purpose as `TCRN0002`, profile 2. The original API still seals
`TCRN0001`, profile 1, with unchanged framing and unindexed source admission.
Both use the same 84-byte request header and checked expected-image V1 journal;
the request version does not change the journal language. Capture binds the
actual native planner purpose, even when both planner routes produce identical
images. Fresh reconstruction selects only the retained request's exact profile.
Crossed version/profile pairs and unknown versions refuse without another decoder
or planner attempt. Indexed source admission still requires positive disabled
Name AutoCorrect and all original closed-catalog restrictions.

Five new focused intent tests and independent V2 wire controls are authored;
their execution remains pending. They cover purpose binding, full-image fresh
requalification, pending/committed/torn recovery, legacy replay refusal on indexed
sources, stale originals and altered journal AFTER images. Existing adapter
family decoding uses the engine's checked request decoder; creating/reviewing
indexed adapter/frontend saves is not connected by this engine checkpoint.

The planner changes the actual catalog Name carrier using its native column
encoding. It adjusts the necessary variable offsets, DATA slot offsets and free
counter within the existing catalog page. The page must have enough capacity;
there is no allocation, relocation, table rebuild or dependency repair.

Logical catalog slots, native object/parent identities, user table definition and
data remain unchanged. Other catalog record bodies and bytes outside the exact
packing scope are preserved. Shrinking a name retains vacated source bytes.
The candidate is decoded independently to check the resulting name and catalog
identities. Case-only requests retain the requested spelling. Exact spelling
no-ops still undergo complete source and dependency admission.

The current operation preserves original timestamps, including DateUpdate.
Microsoft Access timestamp behavior during rename has not been compared; this is
an explicit behavioral gap, not established parity.

With `journal`, `plan_recoverable_table_rename` seals the native plan together with
`NativeTableRenameIntent`. Its closed `TCRN0001` record contains the original and
proposed spellings, native object/parent/definition/catalog-page/slot identities,
and the complete BEFORE length and digest. Decoding validates framing and name
invariants before copying names. It neither qualifies a database nor grants write
authority. Unknown versions, profiles and malformed records refuse; a rejected
rename record is never retried as an UPDATE or another action family.

Before staging a reviewed operation, the adapter captures fresh bytes through the
same retained exclusive offline target. The native sealed plan checks the whole
original and its retained binding, reconstructs the rename using the original
request, verifies identity/spellings and compares the complete reconstructed AFTER
and changed fact with the review. Native work and source/image proof use separate
cumulative budgets. No stored patch, ordinal, SQL text or arbitrary AFTER is
replayed as authority.

## Persistence and retained recovery

`stage_reviewed_table_rename_with_budgets` produces a consuming schema stage;
there is no journal until explicit apply. Refusal retains the same session and
its state/resources. Apply uses the existing fixed-length V1 expected-image
journal, with host-owned exclusive pairing and durable namespace obligations.

A successful rename receipt has `SchemaChange::TableRenamed` and no matched or
changed row counts. Exact no-ops have no schema-change marker and still have no
row counts. Names are not inserted into allocation-free completion receipts.

Both pending recovery and settled confirmation decode the exact TCRN family,
verify its complete BEFORE binding and reconstruct the whole native candidate
before consuming the same checked V1 session. Pending or partial-marker recovery
restores the original; a committed journal requires the complete replacement.
Settled confirmation checks bytes/barriers without repair writes. Failures retain
recovery/finalization ownership rather than erasing a known outcome or silently
starting another operation. See [offline transactions](NATIVE_TRANSACTIONS.md).

## Runtime and frontend workflow

The runtime accepts `ReviewTableRename` with the document revision, original table
and proposed spelling. It uses the immutable opened snapshot without holding a
native lease while the user enters a name. A successful response contains review
facts and a separate consuming `ReviewedTableRename` capability. The existing save
worker opens the exclusive target and performs fresh adapter qualification; the
presentation DTO cannot authorize a save.

The shared UI offers Rename in the Navigation Pane context menu and F2 for its
selected eligible table. It currently requires a local non-system table and all
object tabs closed. Busy/stale requests and unsupported host capabilities refuse.
Submitting the naming editor initiates runtime review; only its matching result
can start the save worker. Exact no-op results close the editor without a save.

Active saves and retained outcomes keep their owner across cancellation and
recovery/confirmation. After acknowledgment of the appropriate committed or
replacement-verified schema receipt, the host requests `OpenTableRenamed` using
the runtime-issued expected-content witness. The reopen compares the complete
fresh file against the expected candidate and refreshes the catalog/revision.
The renamed entry is selected but remains closed; no table-opening side effect is
invented. A changed source or stale callback cannot install an assumed renamed
catalog. A resource-free Released outcome can also initiate this guarded read
when its current known result establishes the replacement, or its completed
last receipt qualifies this same schema action. This does not fabricate a missing
receipt or authorize another write. A current rollback/original result takes
precedence over an older completed receipt.

A retained reserved journal still blocks reopening. The host preserves the known
terminal result alongside that failure and drops the resource-free witness;
it does not bypass the read guard. Rename-specific automatic restart recovery
is not connected. Existing explicit normal-open recovery remains separate and
does not promise this workflow's guarded AFTER/closed-selection behavior.
This change adds no CLI rename command.

## Evidence and visual limits

The independent raw `canonical_catalog_names::rename_pair` fixtures provide
separately authored complete BEFORE/AFTER images, explicit Name AutoCorrect OFF,
retained native identities and empty/populated user rows. Intent, recovery,
adapter, runtime and headless presentation checks pass. The focused coverage
includes 18 native planner/intent tests, five adapter cases, four runtime lifecycle
cases plus typed outcome coverage, three navigation tests and four connected
desktop cases. The broader local gates pass 136 adapter units, 184 runtime tests
plus its diagnostic integration, 138 desktop tests, nine testkit tests and 511 UI
tests. The native unit batch passed 1,323 cases with one invalid Jet password
fixture; correcting that fixture passes its targeted rerun (1,324 covered cases,
260 existing ignored cases). No unchanged whole suite was repeated for that fix.

[Independent reader observations](../../testing/results/ace16-table-rename-independent-reader-2026-11-08.json)
cover 14 original BEFORE/AFTER pairs through 28 immutable Jackcess getter runs.
The maintained `table_rename_fixtures` example reconstructs native plans and
exports matching complete image hashes. This is read observation, not a Jackcess
write, SDK save or native Microsoft Access comparison.

The navigation references opened for this work are the private Home/datasheet
capture `Bildschirmfoto_2026-10-28_12-07-35.png` (1920 × 1141) and Table Design
capture `Bildschirmfoto_2026-10-28_12-07-50.png` (1920 × 1165). They show Windows
Access inside Oracle VirtualBox with Windows 11 desktop chrome; the outer host OS
and effective Windows DPI/display scale are not attested. The
[visual reference record](../../ui/VISUAL_REFERENCES.md) separately identifies
Microsoft 365 Apps version 2610, build 20527.20088 on the Account screen; the
Access 2016 database title is not an application-version guarantee.

These images show the compact object pane and matching table tab. They contain
no rename context menu, name editor, duplicate-name diagnosis, rename progress or
keyboard interaction; no dark rename capture is recorded either. The naming
modal is therefore a current presentation adaptation, not a verified native
rename-dialog match. Verification remains headless; no Windows Access interaction
or cross-platform native GUI observation is claimed.

Two normal-host offscreen naming frames were rendered and inspected on macOS
ARM64 with Windows input emulation at 1×: en-US/light 1280 × 820 and en-US/dark
900 × 580. The input fills the dialog width and initially selects the complete
name. These are presentation observations, not Windows runtime qualification.
