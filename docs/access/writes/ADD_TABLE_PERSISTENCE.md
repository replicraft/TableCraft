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

## Source and request profiles

A qualified source has `n` existing local user tables, for `1 <= n <= 18`, and
exactly `7 + 3n` 4-KiB pages. Its qualified system/header roles, dense `7 + n`
catalog records and `20 + 3n` inline usage-map records retain the admitted native
lineage. This is structural qualification, not a remembered creator flag or
permission inferred from file length. Complete existing row/index membership and
ownership are checked for every table.

For zero-based existing table `i`, definition/DATA/primary-root pages are
`7 + 3i`, `8 + 3i` and `9 + 3i`. The first table keeps owned/candidate/root map
slots 3/7/22. Later tables use slots `20 + 3i`, `21 + 3i` and `22 + 3i`. Each has
one DATA page and one qualified unique Int32 primary root. Existing records,
physical references, primary history, field properties and opaque free tails are
not reserialized.

The original one-table source contract remains unchanged. The repeated profile
with two or more existing tables additionally requires ordinary in-row scalar
storage with no extended field flags. Memo/OLE LVAL, complex references,
Date/Time Extended and other unsupported schema/storage roles refuse; they are
not projected away to make a source eligible.

The operation refuses out-of-EOF ownership claims, extra objects, unsupported
catalog/index/allocation layouts, unavailable contiguous record capacity and
existing object body/module/extra payloads or separately owned children. These
limitations do not establish support for arbitrary imported Access applications.
Ordinary retained property bytes and unused page gaps stay unchanged. Existing
programs are never executed or skipped to admit a mutation.

The request currently uses the existing owned `FirstTableDefinition`: an empty
schema with an explicitly requested Int32 Increment primary followed by supported
scalar fields, typed constant defaults and optional Required/AllowZeroLength
policies. Optional Description is retained independently on ordinary fields and
the generated primary, including the difference between absent and present-empty.
It is inert text, with no trimming, normalization or expression evaluation. The
constructive profile conservatively counts up to 255 UTF-16 units against
Microsoft's [255-character Description contract](https://learn.microsoft.com/en-us/office/vba/access/concepts/miscellaneous/description-property-access).
MR2 stores exact UTF-16 text with flags 1 and native Text type 10. Aggregate
inline-property and catalog-page capacity still refuse independently; several
individually valid descriptions need not fit one candidate catalog page.
This does not stand in for manual Design view or schemas without a
primary key. Names use the catalog's owning comparison policy; this first profile
refuses non-ASCII table-name ambiguity rather than inventing locale equivalence.
A conflicting existing name refuses before a candidate is published.

## Exact append and independent proof

New definition, DATA and primary-root pages append at the original EOF and its
next two pages. Owned/candidate/root maps append at slots `20 + 3n` through
`22 + 3n`. The new catalog record appends at slot `7 + n`, and the declared count
advances from `7 + n` to `8 + n`. For the retained one-table source these remain
pages 10/11/12, map slots 23/24/25 and catalog slot 8 exactly. No original page,
record offset or reference is relocated.

The largest admitted source has eighteen tables and 61 pages. Its addition
produces nineteen tables and 64 pages. That ceiling comes from the current
base-zero eight-byte inline/global FREE-map bitmap profile; it is **not a native
Access format or table-count limit**. Global bitmap/map-container and catalog
capacity growth remain separate implementation work.

The producer modifies only these appended bodies/slots/counters, catalog count
and new pages. It preserves every other original byte, including unused catalog
and map gaps. Independent proof reconstructs the complete new definition,
empty DATA/root pages and native17 catalog row, checks the old-byte edit envelope,
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

`add_table_source_available(captured, scope, budget, cancelled)` supplies an
advisory source fact without a dummy declaration or output plan. It rejects
unsupported input lengths before capture or allocation, then uses the same
complete source qualification as actual planning. Structural write refusals can
make this fact false while ordinary catalog/sequential browsing still works;
false is not a database-health claim. Cancellation, I/O, allocation, resource and
canonical-construction failures remain errors. Actual planning captures and
qualifies the source again and checks the real requested name/schema.

## Original intent and V2 journal

`AddTablePlan::encode_intent` retains the complete actual BEFORE binding and
original schema using four disjoint closed action versions:

| Action | Profile | Original image length | Declaration template |
| --- | ---: | --- | --- |
| `TCAT0001` | 1 | Exactly ten pages; original source semantics retained | TCNT1–4 only |
| `TCAT0002` | 2 | Thirteen through 61 pages, in three-page steps | TCNT1–4 only |
| `TCAT0003` | 3 | Exactly ten pages | TCNT5 only; at least one Description |
| `TCAT0004` | 4 | Thirteen through 61 pages, in three-page steps | TCNT5 only; at least one Description |

Capture chooses the version from Description presence and exact source length; that is wire framing,
not native source authority. A version/profile/length mismatch refuses before
decoded ownership allocation. Neither action can be downgraded or retried as the
other. TCAT1/2 preserve their original nested TCNT1–4 grammar and original record
ceilings. Description-bearing TCAT3/4 pin nested TCNT5 instead; a new action
cannot disguise an older template, nor can an older action acquire Description
or its larger record allowance. The nested record is only a declaration template:
its canonical-blank header never identifies or authorizes the actual source.
Standalone TCNT1–4 behavior remains unchanged. TCNT5 retains exact optional
Description bytes, including present-empty, through original declaration ownership
and fresh native reconstruction; see [first-table intent framing](FIRST_TABLE_PERSISTENCE.md#typed-completion-and-recovery).
Unknown action/template versions and mismatched profiles refuse without decoder
fallback. The new description actions use the same source partitions and native
qualification; they do not expand allocation, catalog or table-count capacity.

`AddTablePlan::prepare` consumes the checked plan through
`prepare_resizable_expected`, comparing the complete expected BEFORE and keeping
the existing V2 journal semantics. `PreparedAddTable` commits or rolls back without
inventing row counts. The caller owns exclusive target/journal association,
durable namespace setup and subsequent retirement; preparation creates no files.

`qualify_native_add_table_recovery` verifies the original actual-source binding,
captures the checked journal's BEFORE, reruns `plan_add_table` and compares both
complete reconstructed images with that same checked V2 session. Only then does
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

[Jackcess 5.0.4 independently read the retained BEFORE/AFTER fixture images](../../testing/results/ace16-add-table-independent-reader-2026-11-07.json).
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

[Jackcess independently read the repeated-addition fixture images](../../testing/results/ace16-repeated-table-independent-reader-2026-11-07.json).
The result covers transitions two→three, three→four and eighteen→nineteen tables,
with both original rows retained at physical positions `8:0` and `11:0`:
`Items(Id=1, Score=17)` and `More(Id=1, Name=Ada)`. New tables were empty, with
checked field, AutoNumber and primary-index metadata. Six exported files contain
five distinct images: the two-table AFTER equals the three-table BEFORE, and the
receipt marks that duplicate explicitly. These observations concern independently
authored fixture bytes, not production mutation correctness, indexed lookups,
insertion, recovery, native Access or platform durability. The producer's
whole-image comparisons and journal tests remain separate evidence.

Current source qualification still refuses arbitrary imported catalogs, indirect
allocation or global bitmap expansion, unsupported catalog/index growth,
manual/no-primary Design schemas and broader schema alteration. Existing
unrelated objects outside the admitted lineage are refused rather than regenerated
or discarded. Nineteen existing tables require further allocation work before
another append; they remain readable through the existing browser/query paths.
