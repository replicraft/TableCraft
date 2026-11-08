# Explicit database Name AutoCorrect options

`NativeImage::plan_disable_name_auto_correct` now constructs an unpublished native
candidate for an explicit request to disable Track/Perform Name AutoCorrect.
This does not change constructor defaults, imply that missing metadata is zero,
or provide a normal save capability. The new candidate's focused tests remain
unrun at this implementation checkpoint.

The first source language is the existing canonical seven-system-role creator
ledger with exactly one user table and no MSysDb. Source admission reuses the
complete append-table creator qualification, including native roles, original
catalog records, header/system pages, schema/row/index checks and exact bounded
allocation memberships. Ordinary manual/keyless and generated-primary originals
must independently enter that existing language. The reserved native MSysDb
identity and name must be unused. Existing database properties, eight-role
catalogs, imported application dependencies and broader allocation layouts
remain unsupported by this first operation.

The explicit request appends a real MSysDb row under the Databases container and
one 152-byte unnamed MR2 map containing the two exact LONG/DDL zero properties.
Creation/update dates and the owner carrier are absent rather than invented.
The map uses one external LVAL row on a new EOF page. The existing distinct,
empty LvProp owned/candidate map records must represent that page below their
64-page boundary. The operation sets only those two allocation bits and updates
the catalog definition's count plus its checked dense DATA append scope. It
does not relocate original catalog rows, reuse free pages, expand maps, rebuild
tables/indexes or author NameMaps.

The shared native17 catalog serializer owns the new database-record variant;
the checked dense catalog-page helper and existing DATA-page serializer own
physical append/LVAL framing. Candidate verification checks the precise original
byte complement, unchanged identities, new native containment role and exact
database-property decoding. All original user TDEF/DATA/INDEX bytes, physical
row slots, counters, properties and opaque tails remain unchanged.

The [SDK reference and independent full-image model](../../testing/ACE16_DATABASE_OPTIONS_SOURCES.md)
establish the external representation on two current creator originals. Separate
raw fixtures author six empty/populated keyless/manual/generated-primary pairs
with retained sentinels for the native candidate tests. Authored tests and SDK
write observations remain distinct from executed native planner checks.

This candidate is outside all existing creation and rename intent languages.
No journal, adapter/runtime/GUI option save, Access comparison or Windows
execution is supplied by this checkpoint. Existing add-table source admission
still assumes the original seven-role ledger. Rename V1/V2 still requires its
existing inline OFF source grammar, so this external option candidate does not
yet establish creator-compatible rename or general CRUD composition. Those
operations need explicit new source/profile qualification and original-intent
recovery rather than relaxed historical guards.
