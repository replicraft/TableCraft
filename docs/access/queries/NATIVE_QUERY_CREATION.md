# Native QueryDef construction

The engine can construct and independently check a new native constant SELECT.
The Access adapter and Runtime now compose that constructor with a distinct
recoverable original and reviewed save service. The desktop now connects
[authored Save/Save As and genuine saved Design](../../application/SAVED_QUERY_EDITING.md)
to that service. The source-pinned
[GUI lifecycle record](../../../tests/results/queries/querydef-gui-lifecycle-2026-11-13.json)
records local qualification and explicit reuse of earlier unchanged-owner checks.
Existing TCQE expression replacement keeps its own admission.

## Declaration and source

With `journal` and `ace16-create`, `QueryCreationDefinition::capture` accepts a
destination name, explicit output alias, signed Int32 constant and caller-captured
`LegacyDateTime`. `NativeImage::prepare_query_creation` consumes that declaration
and a qualified immutable image. The candidate exposes its original and candidate bytes, captured declaration,
signed native query identity and nominal existing/initialized storage effect.
These inert observations grant no save or publication authority.

Both new catalog date fields contain the supplied exact DATE bits. Construction
does not read a clock, execute SQL, infer result types or run database content.
The constant declaration is the first serializer profile, not an Access SQL limit.

The current source requires unencoded ACE 16/subversion-zero storage, a complete
17-column MSysObjects catalog, known system ancestry, empty permissions/relationships
and no active or opaque catalog bodies. Protected ordinary local tables may coexist
with saved queries. Their records, indexes, generated state and declarations remain
unchanged; retained LvProp storage passes the existing property/LVAL ledger. Known
database settings retain explicit Name AutoCorrect OFF checks and qualified optional
query mode; unknown settings and payloads grant no source authority.

An existing eight-column MSysQueries role is reused. If that role is absent, a
separately qualified source branch constructs its actual TDEF and DATA pages at
checked EOF, four ownership-map records and one exact local/system catalog entry.
Its native ID and parent come from the real allocation and Tables role. Both catalog
DATE fields use the same caller-captured value as the new QueryDef. Independent
proof checks this initialization before the ordinary fragment append completes.

Each appended system table must have one directly owned DATA page and no indexes.
Fixed fields occupy disjoint checked spans. Initialization additionally needs actual
catalog/map append room and a representable complete allocation plan. Full physical,
map and LVAL ownership is checked independently of those target pages. Insufficient
page geometry refuses; it is an implementation profile rather than an Access row
or value limit. Standard and Retained image capture are admitted; fixed-update-purpose
capture grants no such permission.

Indexed or unsupported system storage, foreign/active/opaque objects, broader query
serializers and other Jet/ACE write profiles remain outside this creation route.
The separate local-table field/wildcard serializer is not active and is not qualified
by the constant-query checkpoint. Admission uses actual schema, ancestry, ownership
and available geometry, without matching fixture lengths or page numbers.

The allocator scans all live signed catalog identities across every kind and
parent, rejects duplicates and checks fragment ownership in both directions.
It chooses one above the greatest negative ID, using the signed minimum as its
seed; `-1` is a valid final allocation and prevents another creation. This follows
the retained Jackcess synthetic-ID policy recorded in the checkpoint, not an
observation of Microsoft's allocation chronology. Destination collisions use the
Name field's actual native sort declaration and comparison service.

## Construction and proof

Once query storage is qualified, the producer appends one type-5 catalog record
and five native query fragments:
Start, Type, Flags, Column and End. It updates actual DATA directory/free-space
framing and both live counts without relocating existing records. Unsupported
capacity refuses; it is not treated as an Access database limit.

Names and the stack literal remain borrowed during encoding. Existing row
framing and text representation owners have real shared consumers; ordinary
writer flags and typed refusal categories remain unchanged. Short Text retains
its declared representation, including the leading-marker escape. Memo expression
bytes use their separate inline uncompressed UTF16 representation.

Fresh native capture repeats source qualification, then independently decodes
every new cell, checks complete offsets, NULL spans, fixed padding, unused mask
bits, preferred Short Text and exact inline Memo scope. It rereads the new query
and checks old bytes outside the authorized append/count/map/allocation spans.
Initialization's added EOF pages are independently checked as well. Independent
raw builders separately supply complete BEFORE/AFTER images.

## Recovery and application services

`NativeImage::plan_recoverable_query_creation` returns a sealed plan containing
the complete image pair and a `TCQC0001` semantic original. The original retains
the destination, alias, literal, captured DATE, allocated identity and complete
source binding. Its decoder validates the entire frame before allocating names.
Fresh recovery reconstructs the candidate from the original source and compares
the complete AFTER; it does not replay SQL or consult a clock. TCQC0001 framing is
unchanged: complete BEFORE and its persisted semantic fields deterministically
select existing versus missing storage and rederive the nominal effect. Older
implementations refuse missing-role sources. A new nonreconstructible fact would
require a new version; there is no failed-plan retry or decoder fallback.

This operation selects the checked V3 image protocol. The existing host journal
bundle durably pairs the encoded original with that image journal; a raw V3
journal does not itself contain the semantic original. Journal-only builds
recognize its data but refuse unavailable native construction. Other action
families and versions are never fallback decoders.

The adapter's `prepare_saved_query_creation` accepts the owned SQL model and
explicit destination/date facts. It admits the constant declaration above,
formats through the shared SQL owner and reconstructs the complete native model
before returning a plan. Exclusive offline staging repeats source qualification
and full-image proof. Successful publication reports `QueryChange::Created`,
without schema changes or affected-row counts.

Runtime separates `ReviewQueryCreation`, which saves supplied authored SQL, from
`ReviewStoredQueryCopy`, which copies the actual stored definition. Authored
requests may bind an exact saved-query source identity and draft revision; dirty
Save As must use that route. Review captures the injected clock once. The save
worker consumes the nominal capability, and `OpenQueryCreated` checks the complete
saved image and actual new query identity before publishing a fresh catalog.
For initialized storage, the private witness also binds exact prior table membership
and the actual system-role identity/TDEF. Guarded reopen checks all of those facts.
Desktop forwards only this nominal verified effect to the renderer-neutral catalog
comparator, which allows exactly the technical local/system addition while retaining
prior table fields/order and query identities. The user receipt remains Created,
without a schema or affected-row claim. Save, recovery and reopen do not resample
the clock or execute the query.

The [current qualification record](../../../tests/results/queries/querydef-gui-lifecycle-2026-11-13.json)
records source-extension and GUI lifecycle checks with exact source/input pins,
failed attempts and explicit scope reuse. See the
[verification summary](../../application/SAVED_QUERY_EDITING.md#verification-status)
for current UI/Desktop counts and separately reused native/adapter/Runtime checks.
The historical service/construction checkpoints below retain their earlier scope;
they do not independently qualify these later source branches or GUI workflows.

The [service checkpoint](../../../tests/results/queries/querydef-persistence-services-2026-11-13.json)
records 671 native, 5 saved-reader, 343 adapter, 493 Runtime, 17 selected Desktop
and 80 MCP passes. Separate journal-only and adapter-without-creation runs pass
four and one tests respectively. The batch retains 51 explicit fixture-dependent
or qualification ignores. Strict lint across eleven packages, separate engine
feature combinations, minimal Runtime and Rustdoc pass. These checks qualify the
documented service profile, not GUI persistence or native Windows/Access behavior.

## Local qualification

The [construction checkpoint](../../../tests/results/queries/querydef-construction-2026-11-13.json)
records seven native feature tests, including Standard/Retained captures,
Unicode/leading-marker names, signed constants, repeated creation, all-kind ID
selection, the last negative ID, collisions, malformed ownership/overlap,
cancellation and resource refusal. Broader regression scopes pass 664 native,
5 saved-reader, 334 adapter, 490 Runtime and 17 selected Desktop tests. The stated
suites retain 51 explicit fixture/qualification ignores.

Strict lint, Rustdoc and separate no-default, create-only, journal-only and
combined engine library builds pass. These are local construction, preservation
and regression checks. They establish neither normal GUI persistence nor native
Windows/Access behavior, timestamps/events parity or the other requested profiles.

The retained implementation reference is
[Jackcess DatabaseImpl](https://github.com/jahlborn/jackcess/blob/be0a2139b087a227d14cafd4ea231306a5a3a862/src/main/java/com/healthmarketscience/jackcess/impl/DatabaseImpl.java),
specifically `getNextFreeSyntheticId` and its indexed/full-scan maximum selection.
