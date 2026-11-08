# Append-only second-table engine operation

With both `ace16-create` and `journal`, `NativeImage::plan_add_table` builds a
checked empty second-table plan against an actual captured populated source.
This is a separate operation from fresh database creation and canonical-blank
first-table initialization. It never rebuilds existing tables through the fresh
multiple-table serializer. The Access adapter exposes explicit offline staging,
apply, recovery and confirmation. Runtime and shared UI compose that service
through reviewed table creation; their changes and host acceptance are separate
from the engine qualification described here. See the [runtime contract](../../application/RUNTIME.md)
and [database-creation workflow](../../ui/DATABASE_CREATION.md). The native CLI
has no new add-table command from this change.

## Initial source and request profile

The initial source has ten 4-KiB pages and one existing local user table. Its
qualified system definitions/catalog roles, dense eight-record catalog and
23-record usage-map container retain the TableCraft-created layout. This is a
native structural qualification, not a remembered creator flag. The existing
user definition is page 7, its single DATA page is 8 and its ascending unique
Int32 primary root is 9. Complete existing row/index membership is checked to
establish those roles. Existing data, record offsets, primary history, field
properties and opaque free tails are not reserialized.

The operation refuses out-of-EOF ownership claims, extra objects, unsupported
catalog/index/allocation layouts, unavailable contiguous record capacity and
existing object body/module/extra payloads or separately owned children. These
limitations do not establish support for arbitrary imported Access applications.
Ordinary retained property bytes and unused page gaps stay unchanged. Existing
programs are never executed or skipped to admit a mutation.

The request currently uses the existing owned `FirstTableDefinition`: an empty
schema with an explicitly requested Int32 Increment primary followed by supported
scalar fields, typed constant defaults and optional Required/AllowZeroLength
policies. This does not stand in for manual Design view or schemas without a
primary key. Names use the catalog's owning comparison policy; this first profile
refuses non-ASCII table-name ambiguity rather than inventing locale equivalence.
A conflicting existing name refuses before a candidate is published.

## Exact append and independent proof

New definition, DATA and primary-root pages append at EOF as pages 10, 11 and 12.
New owned/candidate/root map records append at slots 23, 24 and 25. A new catalog
record appends at slot 8 and its declared record count advances from eight to nine.
Old page identities and references remain unchanged.

The producer modifies only these appended bodies/slots/counters, catalog count
and new pages. It preserves every other original byte, including unused catalog
and map gaps. Independent proof reconstructs the complete new definition,
empty DATA/root pages and native17 catalog row, checks the old-byte edit envelope,
and decodes actual field/index/map metadata. Defaults are projected from actual
stored expressions through the independent constant projector with no clock.
Policies retain exact presence and Boolean values. Canonical literal spelling,
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

`AddTablePlan::encode_intent` emits `TCAT0001`/profile 1, retaining the full actual
ten-page BEFORE binding and original schema. Its nested TCNT1–4 record supplies
only a closed declaration template; the nested canonical-blank header never
identifies or authorizes the actual source. Original TCNT1–4 standalone behavior
remains unchanged. An unknown action/template version or mismatched profile
refuses without decoder fallback.

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

The focused local macOS ARM64 batches pass:

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

The current append source remains exactly the initial qualified single-table
layout. It does not admit a third table, arbitrary imported catalogs, catalog
index growth, allocation-map expansion, manual/no-primary Design schemas or
broader schema alteration. Existing unrelated objects outside the supported
source profile are refused rather than regenerated or discarded.
