# ACE 16 native compatibility work

ACE 16 is the primary 100% compatibility target. Current implementation is a
bounded read profile, not complete compatibility or Microsoft Access conformance.
`ReadProfile::Ace2016` and `Ace2019` admit subversions zero and one with
header codes 5 and 6. Plain encryption-removal copies preserve subversion one.
Encoded sources require a qualified explicit decoder; raw encoded bytes and
unknown subversions remain refused. Code 3 uses the separate Ace2010 profile. Engine release/build and stored layout remain separate identities.

## Working profile

Catalogs, table/column/index metadata, existing supported scalar rows and long
values, MR2 properties, relationship declarations, explicit supported constraints
and qualified Boolean/numeric/legacy DateTime/GUID indexes use bounded native algorithms.
The Access adapter and CLI query run consume them without new host dependencies.
Typed range selection covers single Int32/Int64 keys. Text collation policy, size/work ceilings and
unsupported feature errors remain explicit. Normal ACE table edits stay blocked;
explicit [encryption rewrite copies](NATIVE_ENCRYPTION.md) are a separate
source-preserving operation.
Large Number reads use exact Int64 carriers on codes 5/6. Date/Time Extended
reads use exact civil date/time carriers on code 6. Ordinary scalar reads reject
calculated/parent-complex fields and native text index keys; profile selection does not turn
unrecognized columns into NULL.

An explicit [stored Int32 calculation reader](features/ACCESS_ADVANCED_FIELDS.md#explicit-stored-int32-calculations)
now reports qualified cached values, including stale results, without evaluating
expressions or enabling ordinary calculated query fields.

[Scoped complex storage links](features/ACCESS_ADVANCED_FIELDS.md#scoped-complex-parent-and-child-links)
now join qualified parent keys to checked raw flat children, retaining empty/NULL
memberships and encoded attachment Binary. An explicit
[stored multivalue reader](features/ACCESS_ADVANCED_FIELDS.md#bounded-stored-multivalue-collections)
now exposes typed bounded scalar collections while retaining exact values,
duplicates, NULL cells and empty membership. An explicit
[stored attachment reader](features/ACCESS_ADVANCED_FIELDS.md#typed-stored-attachment-collections)
now exposes typed nullable metadata and encoded payloads without copying them.
Its explicit decode method uses the
[bounded attachment decoder](features/ACCESS_ADVANCED_FIELDS.md#bounded-original-attachment-decoding)
to recover original bytes from qualified direct/zlib wrappers. Query expansion,
large streaming payloads and host file extraction remain separate work.

A [stored history reader](features/ACCESS_ADVANCED_FIELDS.md#stored-version-history-collections)
now resolves a visible Memo to its hidden history column and returns borrowed
nullable text/exact DATE records in storage order. Current-value selection,
default sorting, history mutation and native Access semantics remain separate.

The pinned [format reference](https://github.com/jahlborn/jackcess/blob/be0a2139b087a227d14cafd4ea231306a5a3a862/src/main/java/com/healthmarketscience/jackcess/impl/JetFormat.java)
and reflection over the reviewed Jackcess 5.0.4 artifact show shared page/row/index
geometry for these families. Compared public format constants differ in default
text sort order from the 2007 family. This does not qualify text key decoding or
Access collation; explicit ordinal query collation remains a different policy.
No third-party implementation was copied.

## Reproducible independent comparisons

Six original fixture recipes now accept V2016/V2019: Scalar, Index, Property,
Relationship, Validation and Definition. The reviewed Jackcess 5.0.4 artifact
generates fresh files; they are not retagged older files. Generation refuses
existing output directories/files. Inputs/JAR remain untracked and are not CI
requirements. [Fixture provenance and hashes](FIXTURES.md) identify observed inputs.

```sh
cargo build -p tablecraft-cli --locked
cargo build -p tablecraft-ace --examples --locked
python3 docs/testing/compare_ace16.py --jar path/to/jackcess-5.0.4.jar --cli target/debug/tablecraft --rows target/debug/examples/table_data --ranges target/debug/examples/index_range --directory new-fixture-directory --generate
```

[The runner](testing/compare_ace16.py) verifies original headers and unchanged
hashes, all five catalog fields (480 records across 12 files), exact mixed scalar
rows, 22 properties, complete relationship inventories, ten child-table integrity
reports, twelve validation-table coverage reports, 28 Int32 range cases, and
200-column chained schema/index/query comparisons on populated/empty tables.
The 2019 template contains two built-in navigation relationships; the independent
oracle uses the API that includes both user and system declarations.

Rust regression tests retag disposable copies of the registered shared-layout
example solely to exercise code paths in CI. They are not ACE 16-authored fixtures
or evidence of actual file conversion. They cover catalogs, scalar/index reads,
audits, malformed pages, cancellation/limits and continued write/codec rejection.
CLI tests exercise indexed queries and reject writes without creating output.

Native Microsoft Access comparisons remain unrun. Next: advanced field metadata,
collation/index behavior, table data macros and safe native writes with explicit
evidence.

## Modern scalar reads

Native type 0x13 requires an eight-byte fixed field and a 2016/2019 profile;
it preserves the full signed Int64 range without floating conversion. Type 0x14
requires a 42-byte fixed field and the 2019 profile. Its canonical encoding has
19 decimal day digits since 0001-01-01, a colon, 12 decimal second digits,
seven 100ns fraction digits, and the trailer `:7` followed by NUL. Every byte,
range and precision marker is checked. Jiff converts the bounded day offset to
a civil date; no timezone or host locale participates. Unknown widths/profiles,
calculated flags and malformed rows fail explicitly, including empty tables.

Same-kind extended dates compare by civil date/time, retaining all seven digits.
Mixed legacy/extended dates require explicit future conversion semantics and
currently return a type error. The CLI renders extended dates as
`#YYYY-MM-DD HH:MM:SS.fffffff#`; it does not implicitly convert literals or legacy
`date` parameters into the extended carrier.

The original [modern fixture recipe](testing/ModernScalarFixture.java) generates
V2016/V2019 inputs with signed extremes, integers beyond double precision,
NULLs, years 1–9999, leap dates and single-tick fractions. Independent exact row
comparisons cover seven and thirteen rows respectively. The runner also checks
CLI ordering, an exact BIGINT parameter filter, extended-date equality, complete
Int64 index selection/checks and rejected empty calculated columns.
Original input hashes remain unchanged. These are Jackcess comparisons,
not Microsoft Access qualification.

```sh
java -cp path/to/jackcess-5.0.4.jar docs/testing/ModernScalarFixture.java new-2016.accdb V2016
java -cp path/to/jackcess-5.0.4.jar docs/testing/ModernScalarFixture.java new-2019.accdb V2019
python3 docs/testing/compare_ace16_scalars.py --jar path/to/jackcess-5.0.4.jar --cli target/debug/tablecraft --rows target/debug/examples/table_data --ace2016 new-2016.accdb --ace2019 new-2019.accdb
```

References: [Microsoft Large Number](https://support.microsoft.com/en-us/access/using-the-large-number-data-type),
[Microsoft Date/Time Extended](https://support.microsoft.com/en-us/access/using-the-date-time-extended-data-type),
and the reviewed [ColumnImpl encoding](https://github.com/jahlborn/jackcess/blob/be0a2139b087a227d14cafd4ea231306a5a3a862/src/main/java/com/healthmarketscience/jackcess/impl/ColumnImpl.java).
No third-party implementation was copied.

## Advanced field declarations

[Inert feature inspection](features/ACCESS_ADVANCED_FIELDS.md#working-inert-metadata-slice)
now exposes calculation/result metadata, raw complex IDs and relevant field/property
markers through the library and CLI `inspect --table NAME --features`. Two original
V2016/V2019 fixtures match ten columns each with the independent reader. Complex
children, calculated row evaluation and all ACE writes remain unsupported; metadata
inspection does not silently promote those features to scalar storage.

[Complex schema resolution](features/ACCESS_ADVANCED_FIELDS.md#resolved-complex-field-schemas)
now joins checked system mappings to catalog/type/flat-table geometry through
`inspect_complex_schema` and CLI `--complex`. All three mapping rows per generated
family match independently, including raw IDs and key names. Child collections,
attachment contents, calculation and writes remain outside this metadata slice.

[Raw complex-child reads](features/ACCESS_ADVANCED_FIELDS.md#raw-complex-child-storage-reads)
now match populated multivalue, version-history and encoded attachment rows in
both qualified families. Only the child FK's proven fixed Int32 storage flag was
added to scalar decoding. Bounded original attachment decoding and explicit stored
scalar calculations now use their owning feature APIs; parent collection/query
semantics, recalculation and ACE writes remain unqualified.

[Sequential relationship audits](NATIVE_CONSTRAINTS.md) now support one-to-ten-column
integer/Currency/GUID tuples with matching paired storage types, exact partial-NULL matching
and wholly NULL exemption. Original
V2016/V2019 independent fixtures and corrupt-copy rejection cover the profile;
physical index validation remains a separate operation. Complete one-to-ten-column
numeric index traversal/checks now have [independent compound evidence](NATIVE_INDEXES.md#complete-numeric-composite-indexes),
including mixed directions and maximum-width keys. Native Access conformance,
remaining index encodings, composite seeks/mutation and all ACE writes remain
unqualified.


[Modern storage histories](testing/MODERN_SCALAR_HISTORIES.md) now independently
exercise 19,902 retained rows per 2016/2019-family input with Large Number fields,
2019 Date/Time Extended, added columns, null-mask growth, updates/deletes and
scattered Large Number index references. These extend observed read/query
evidence; other modern-field histories and native Access qualification remain open.
