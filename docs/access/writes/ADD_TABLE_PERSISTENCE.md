# Append-only table creation and recovery

With both `ace16-create` and `journal`, `NativeImage::plan_add_table` builds a
checked empty table plan against an actual captured source with existing tables.
This is a separate operation from fresh database creation and canonical-blank
first-table initialization. It never rebuilds existing tables through the fresh
multiple-table serializer. The Access adapter exposes explicit offline staging,
apply, recovery and confirmation. Runtime and shared UI compose that service
through reviewed table creation; their changes and host acceptance are separate
from the engine qualification described here. See the [runtime contract](../../application/RUNTIME.md)
and [database-creation workflow](../../ui/DATABASE_CREATION.md). The native CLI
has no new add-table command from this change.

## Retained allocation and deleted-table histories

The current retained source family derives physical storage from the complete
captured catalog and allocation inventory. It supports appending after deleting
the last user table and after further table additions, without treating retired
pages or catalog tombstones as reusable free space. Live catalog counts remain
separate from physical record slots. Fresh table identities and definition/DATA
pages start at the actual EOF; a primary root is added only for a keyed target.
The existing system/database-property dialect selects this family before source
qualification. A failed retained proof never retries a legacy source family.

`AddTableSourceProfile::Retained` requires a complete source-bound inventory,
including live table/index membership and retained DATA, INDEX and LVAL roles.
Protected orphaned or tombstoned storage is preserved; recognizing its ownership
does not grant permission to overwrite it. Existing object programs, unsupported
properties, dependency roles and required validation remain independent refusals.
The newly authored declaration still uses the existing checked first-table
constructor profile; this does not imply a complete Access table designer.

Owned/candidate/primary usage maps use the actual page's bitmap window rather
than assuming every page lies below page 64. The global inline allocation bitmap
can grow into proven adjacent free record capacity. Indirect-map promotion and
adding catalog/map container pages are not implemented; exhausted actual carrier
capacity refuses before saving. These are implementation gaps, not Access
page-count or table-count limits. Retained capture accepts the native database
size bound with fallible allocation, cancellation and cumulative caller budgets;
it does not inherit the older 64 MiB capture ceiling.

`TCAT0006` carries this exact source family and a closed TCNT1–6 declaration.
`NativeAddTableIntent::new_retained` binds request data only. Preparation uses the
V3 image journal, and `qualify_retained_recovery` requires the same checked V3
session before fresh native reconstruction and complete BEFORE/AFTER comparison.
TCAT1–5 retain their original V2 grammars and source admission. Relabeling an older
request or journal does not acquire retained-source authority.

Local macOS ARM64 checks pass 29 creation tests, 16 add-table request/recovery
tests and six capture tests. Three headless GUI journeys pass replacement-table
and subsequent-table saves, including independent whole-image comparisons and
fresh reopening. That initial first-record continuation exposed a separate INSERT
source refusal. The subsequent [TCNA8 route](NORMAL_ACE_INSERT.md#retained-tables-and-original-presence)
now passes real first-row review/save/reopen, alongside the complete 37-test
headless table-creation family. Table creation itself grants no row mutation
authority. Native Windows execution and Microsoft Access save/reopen comparison remain
unqualified. The following sections retain the earlier source-family contracts
and their historical evidence; their geometry ceilings do not constrain TCAT6.

## Source and request profiles

The two older source languages remain distinct. The retained generated-table language has
`n` existing local tables, for `1 <= n <= 18`, exactly `7 + 3n` 4-KiB pages,
`7 + n` catalog records and `20 + 3n` inline usage-map records. Its old source
qualification is retained for TCAT1–4 replay.

The mixed language derives an ordered ledger from the actual catalog, table
definitions and indexes. Each existing table has a definition followed by one
DATA page and, only when it has a primary key, one primary-root page. The first
definition is page 7; subsequent definitions follow the preceding table's actual
two- or three-page extent. The ledger must end exactly at EOF. For `p` original
pages and `n` tables, catalog/map counts are `7 + n` and `20 + (p - 7)`.
Length is only a framing bound: matching a legacy length does not turn a manual
or keyless source into a legacy generated source. Complete existing row/index
membership, schema roles and ownership are checked for every table.

The first table retains owned/candidate map slots 3/7 and optional primary-map
slot 22. A later definition at page `d` uses owned/candidate slots
`20 + (d - 7)` and the following slot, with one further slot only for a primary
root. A mixed source may combine generated Int32 Increment primaries, ordinary
single-field Int32/Int64/Currency primaries at their actual field position, and
keyless tables. Existing records, physical references, primary history, field
properties and opaque free tails are not reserialized.

The original one-table generated source contract remains unchanged. Its retained
empty Long Text/OLE declaration exception uses the same source checks for both
generated and manual targets. This preserves existing source bytes; it does not
author new LVAL fields or broaden repeated-source admission. After an append,
another append still refuses if a table exposes those unsupported storage roles.
Repeated generated sources and sources with mixed manual/keyless roles require
ordinary in-row scalar storage with no extended field flags. Memo/OLE LVAL,
complex references, Date/Time Extended and unsupported schema/storage roles
refuse rather than being projected away. Composite, descending and broader
primary-key families remain outside this append profile.

The operation refuses out-of-EOF ownership claims, extra objects, unsupported
catalog/index/allocation layouts, unavailable contiguous record capacity and
existing object body/module/extra payloads or separately owned children. These
limitations do not establish support for arbitrary imported Access applications.
Ordinary retained property bytes and unused page gaps stay unchanged. Existing
programs are never executed or skipped to admit a mutation.

The request uses the owned `FirstTableDefinition`. `capture` retains the generated
Int32 Increment-primary declaration. `capture_manual` supplies an ordinary schema
with no generated field and either no key or one explicit Int32/Int64/Currency
primary at its actual field position. Both retain supported scalar fields, typed
constant defaults and optional Required/AllowZeroLength policies. Optional Description is retained independently on ordinary fields and
the generated primary, including the difference between absent and present-empty.
It is inert text, with no trimming, normalization or expression evaluation. The
constructive profile conservatively counts up to 255 UTF-16 units against
Microsoft's [255-character Description contract](https://learn.microsoft.com/en-us/office/vba/access/concepts/miscellaneous/description-property-access).
MR2 stores exact UTF-16 text with flags 1 and native Text type 10. Aggregate
inline-property and catalog-page capacity still refuse independently; several
individually valid descriptions need not fit one candidate catalog page.
No hidden ID or primary root is invented for a keyless request. A primary key's
non-NULL/unique restriction remains separate from an ordinary field's explicit
Required policy. Names use the catalog's owning comparison policy; this profile
refuses non-ASCII table-name ambiguity rather than inventing locale equivalence.
A conflicting existing name refuses before a candidate is published.

## Exact append and independent proof

New definition and DATA pages append at the original EOF and its next page.
A keyed request adds its primary root on the following page; a keyless request
adds exactly two pages and no root/map placeholder. Owned/candidate maps append,
with a third map only for a keyed target. The new catalog record appends at slot
`7 + n`, and the declared count advances to `8 + n`. The retained generated
one-table transition remains pages 10/11/12, map slots 23/24/25 and catalog slot 8
exactly. No original page, record offset or reference is relocated.

The base-zero eight-byte inline/global FREE-map profile covers at most 64 pages.
A qualified 61-page source can fit either target shape; a 62-page source can fit
only a two-page keyless target. An Increment or ordinary manual-keyed request
against that same 62-page source refuses without a write. Table counts depend on
the actual keyed/keyless extents, and inline map/catalog record capacity can
refuse earlier. The old generated-only eighteen→nineteen transition remains
61→64 pages. Neither nineteen tables nor this bitmap ceiling is a native Access
format or table-count limit. Global bitmap/map-container and catalog capacity
growth remain separate work.

The producer modifies only these appended bodies/slots/counters, catalog count
and new pages. It preserves every other original byte, including unused catalog
and map gaps. Independent proof reconstructs the complete new definition,
empty DATA and optional root pages and native17 catalog row, checks the old-byte edit envelope,
and decodes actual field/index/map metadata. Defaults are projected from actual
stored expressions through the independent constant projector with no clock.
Policies retain exact presence and Boolean values. Description is checked as exact
inert text with its own presence, carrier and declaration flags. Canonical literal spelling,
strict property framing and independently computed record lengths prevent padding
from enlarging the permitted edit range into an old opaque gap.

Independent raw fixtures cover a populated existing row, unchanged old primary
root and three nonzero opaque-tail/gap sentinels. The candidate tests include
same-name and allocation refusals, forged metadata and out-of-scope bytes, a
semantically valid overlong catalog record, atomic capacity refusal and measured
allocation/row/work/cancellation admission.

`add_table_source_capabilities(captured, scope, budget, cancelled)` supplies
separate `can_add_keyless()` and `can_add_keyed()` advisory facts without a dummy
declaration or output plan. `add_table_source_available` retains the keyed-target
advisory meaning. Unsupported page counts refuse before capture/allocation;
complete native structural qualification follows. Facts include the two/three-page
bitmap requirement and inline map-record capacity, while actual name/schema and
catalog-record fit remain consuming-plan checks. Structural write refusals can
make the facts unavailable while ordinary browsing still works; they are not a
database-health claim. Cancellation, I/O, allocation, resources and
canonical-construction failures remain errors. Actual planning recaptures and
qualifies the source and checks the real requested declaration independently.

## Original intent and V2 journal

`AddTablePlan::encode_intent` retains the complete actual BEFORE binding and
original schema using five disjoint closed action versions:

| Action | Profile | Original image length | Declaration template |
| --- | ---: | --- | --- |
| `TCAT0001` | 1 | Exactly ten pages; original source semantics retained | TCNT1–4 only |
| `TCAT0002` | 2 | Thirteen through 61 pages, in three-page steps | TCNT1–4 only |
| `TCAT0003` | 3 | Exactly ten pages | TCNT5 only; at least one Description |
| `TCAT0004` | 4 | Thirteen through 61 pages, in three-page steps | TCNT5 only; at least one Description |
| `TCAT0005` | 5 | Page-aligned 9–61 pages for a keyed target; 9–62 for a keyless target | Closed TCNT1–6, with actual manual key position or keyless sentinel only in TCNT6 |

For TCAT1–4, Description presence and the exact legacy source partition select
the version. TCAT5 is selected from the freshly qualified mixed source/target
operation: a manual target, a nonlegacy source, or both. Generated TCNT1–5 targets
can therefore follow a manual/keyless source without changing their declaration
language. Decoded intents retain their typed outer identity; re-encoding a TCAT5
record cannot downgrade it merely because its source length matches an older
partition. `NativeAddTableIntent::new_mixed` is explicitly data-only binding,
not source-role qualification or permission to save.

A version/profile/length mismatch refuses before decoded ownership allocation.
TCAT1/2 retain their TCNT1–4 grammar and original record ceilings; TCAT3/4 retain
TCNT5 and their Description allowance. All four still reject nested TCNT6.
TCAT5 independently pins TCNT1–6 magic/profile pairs, rather than delegating
future version admission to a broader decoder. TCNT6 alone carries the actual
optional manual key; it never invents an Increment field. The nested canonical
blank header is only declaration-template data and never identifies the actual
AddTable source. Unknown versions and mismatched profiles refuse without decoder
fallback. Standalone TCNT1–6 behavior remains separate; see
[first-table framing](FIRST_TABLE_PERSISTENCE.md#typed-completion-and-recovery).

`AddTablePlan::prepare` consumes the checked plan through
`prepare_resizable_expected`, comparing the complete expected BEFORE and keeping
the existing V2 journal semantics. `PreparedAddTable` commits or rolls back without
inventing row counts. The caller owns exclusive target/journal association,
durable namespace setup and subsequent retirement; preparation creates no files.

`qualify_native_add_table_recovery` verifies the original actual-source binding,
captures the checked journal's BEFORE and reconstructs only the selected native
operation. TCAT1–4 use the retained legacy-only source/target planner; they cannot
acquire mixed-ledger admission even when the page count matches. TCAT5 uses
structural planning and additionally requires its resulting typed plan to be
mixed. A legacy generated-source/generated-target operation relabeled TCAT5
refuses too. Both routes compare complete reconstructed BEFORE/AFTER images with
that same checked V2 session. Only then does
`QualifiedAddTableRecovery` permit explicit recovery or settled-image confirmation.
It does not reopen the journal, retry another decoder, clean resources or infer
historical completion from current bytes. Refusal performs no target writes.
These engine APIs do not own filesystem paths or cleanup. The adapter's
`OfflineWriteSession::plan_add_table` and `_if_original` capture from the same
retained exclusive target; their consuming `StagedAddTable::apply` uses the
existing journal, retirement and cleanup lifecycle. `SchemaChange::TableAdded`
reports the added field count with no matched/changed record count. Recovery and
confirmation report it only for committed or verified Replacement images.

## Observed verification and limits

### Initial second-table checkpoint

The earlier local macOS ARM64 checkpoint recorded these passing batches:

| Boundary | Passing tests | Scope |
| --- | ---: | --- |
| Engine candidate and advisory source fact | 12 | Seven candidate tests and five advisory tests, including readable unsupported/corrupt primary facts and the 64 MiB zero-allocation preflight |
| TCAT original-intent codec | 3 | Independent framing, actual-source binding, closed nested profiles and resource/cancellation behavior |
| Existing standalone TCNT codec | 9 | Retained first-table/default/policy/wider-default behavior and original wire checks |
| Public native V2 journal/recovery | 4 | Actual commit, pending/committed/interrupted recovery, complete-image/request/source tampering and refusal without target writes |
| Offline adapter selection | 18 | Addition and existing offline lifecycle regressions, including same-file addition, subsequent generated insertion, retained/cold recovery and selected-decoder refusal |

These are separate boundary-specific tests. Repeated focused reruns are not added
to their totals, and this is not a full-workspace regression or native Access
save/reopen qualification. Filesystem receipts retain the actual platform
mechanism; local macOS execution does not establish Windows or Linux behavior.

[Jackcess 5.0.4 independently read the retained BEFORE/AFTER fixture images](../../../tests/results/reading/ace16-add-table-independent-reader-2026-11-07.json).
It reported V2016, retained `Items` row `(Id=1, Score=17)` at physical row `8:0`,
and enumerated the added empty `More` table with LONG AutoNumber `Id`, TEXT(20)
`Name` and primary-index metadata. This check did not perform an indexed lookup,
execute the first insertion or journal recovery, run Microsoft Access, or test
Windows/Linux filesystem durability. Independent-reader metadata/read agreement
must not be presented as native Access mutation parity.

### Accepted repeated-addition follow-up

The subsequent local macOS ARM64 batches pass:

| Boundary | Passing tests | Scope |
| --- | ---: | --- |
| Engine candidate and advisory fact | 19 | The twelve earlier tests plus seven repeated-profile checks, including complete independent AFTER comparisons for every source with two through eighteen tables |
| TCAT original-intent codec | 5 | Both disjoint action versions, all four pinned declaration templates, actual-source binding, framing, allocation and cancellation |
| Public native V2 journal/recovery | 6 | Retained ten-page behavior plus third-table commit/interrupted-growth/cold recovery and complete-image tamper refusal |
| Offline adapter | 19 | Same-file additions, retained/cold TCAT1 and TCAT2 recovery and the existing offline lifecycle |
| Runtime / desktop | 149 / 111 | Captured source profile, consuming review, repeated Create → Table, duplicate-name correction, separate record saves and fresh queries |

Strict all-feature library/test lint also passes for the six affected packages.
Windows x64 and ARM64 runtime/desktop all-target/all-feature compilation checks
pass with their dependencies. These checks are not native Windows filesystem or
GUI execution, nor a complete twenty-package Windows/CLI/SQLite build.

[Jackcess independently read the repeated-addition fixture images](../../../tests/results/reading/ace16-repeated-table-independent-reader-2026-11-07.json).
The result covers transitions two→three, three→four and eighteen→nineteen tables,
with both original rows retained at physical positions `8:0` and `11:0`:
`Items(Id=1, Score=17)` and `More(Id=1, Name=Ada)`. New tables were empty, with
checked field, AutoNumber and primary-index metadata. Six exported files contain
five distinct images: the two-table AFTER equals the three-table BEFORE, and the
receipt marks that duplicate explicitly. These observations concern independently
authored fixture bytes, not production mutation correctness, indexed lookups,
insertion, recovery, native Access or platform durability. The producer's
whole-image comparisons and journal tests remain separate evidence.

At that generated-only checkpoint, manual/keyless append sources remained
unavailable and a nineteen-table generated source required allocation work before
another append. Those counts and reader observations remain historical evidence.

The current mixed-role source and TCAT5 implementation extends structural
admission as described above. Its new codec and cold-recovery checks are not
included in those earlier passing counts. Arbitrary imported catalogs, indirect
allocation/global bitmap expansion, unsupported catalog/index growth and broader
schema alteration remain refused. Unrelated objects outside the admitted lineage
are refused rather than regenerated or discarded. Native Access save/reopen and
Windows/Linux execution qualification remain separate from these source and
protocol contracts.
