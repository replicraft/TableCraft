# Native QueryDef construction

The engine can construct and independently check a new native constant SELECT.
This is a storage candidate, separate from normal query Save/Save As, recoverable
originals and application write authority. Those integration layers remain in
development. Existing TCQE expression replacement keeps its own admission.

## Declaration and source

With `journal` and `ace16-create`, `QueryCreationDefinition::capture` accepts a
destination name, explicit output alias, signed Int32 constant and caller-captured
`LegacyDateTime`. `NativeImage::prepare_query_creation` consumes that declaration
and a qualified immutable image. The candidate exposes only its original and
candidate bytes, captured declaration and signed native query identity.

Both new catalog date fields contain the supplied exact DATE bits. Construction
does not read a clock, execute SQL, infer result types or run database content.
The constant declaration is the first serializer profile, not an Access SQL limit.

The current source requires unencoded ACE 16/subversion-zero storage, a complete
17-column MSysObjects catalog and eight-column MSysQueries table, known system
ancestry, empty permissions/relationships and no opaque catalog bodies. Live
non-system catalog objects must be queries. Each appended system table has one
directly owned DATA page and no indexes. Fixed fields must occupy disjoint checked
spans. Full physical/map/LVAL ownership is checked independently of those pages.

Consequently this profile does not cover ordinary creator files without
MSysQueries, live user-table catalogs, indexed system tables or general query
definitions. It uses actual schema, ownership and available page geometry rather
than matching fixture file lengths or page numbers. Standard and Retained image
capture are admitted; fixed-update-purpose capture grants no such permission.

The allocator scans all live signed catalog identities across every kind and
parent, rejects duplicates and checks fragment ownership in both directions.
It chooses one above the greatest negative ID, using the signed minimum as its
seed; `-1` is a valid final allocation and prevents another creation. This follows
the retained Jackcess synthetic-ID policy recorded in the checkpoint, not an
observation of Microsoft's allocation chronology. Destination collisions use the
Name field's actual native sort declaration and comparison service.

## Construction and proof

The producer appends one type-5 catalog record and five native query fragments:
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
and checks every old byte outside the precise append/count changes. Independent
raw builders separately supply complete BEFORE/AFTER images.

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
