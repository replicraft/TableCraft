# Advanced Access field behavior

Status: inert field-feature metadata, raw child scalar storage and explicit stored
scalar calculation inspection are implemented. Scoped raw parent/child links and
bounded original attachment decoding are also available. Full collection/query
semantics, calculated evaluation and editing
remain future requirements. Current core values represent
scalars. No collection query binding or field editor is implemented. SQL member
syntax does not establish `.Value` semantics.

## Contracts beyond scalar storage

| Feature | Behavior that must survive |
| --- | --- |
| Lookup metadata | Stored bound value versus displayed value, source definitions, column bindings and single/multiple selection |
| Multivalued fields | Child identities/values, null/empty distinctions, `.Value` flattening, criteria, grouping and supported mutation rules |
| Attachments | Parent/child records, names/types, original payload bytes, metadata and bounded streaming |
| Rich text | Original markup and formatting metadata, plain-text projection as an explicit conversion |
| Append-only Long Text | Current content and historical entries with qualified update/history access |
| Hyperlinks | Display/address/subaddress/screen-tip components and round-trip encoding |
| Input masks | Display/input rules, literal-storage options and interaction with underlying stored type |
| Calculated fields | Expression/type metadata, allowed functions, dependencies, errors and native recalculation rules |

Keep structured fields out of ordinary scalar coercion. An attachment collection
is not a Binary value and a multivalue is not delimited Text. Native child records
and metadata belong to the engine; schema describes field capabilities; query
binding/evaluation implements access and row expansion; DAO exposes child cursors;
forms/designers own presentation. Avoid a single universal Value variant that
imports these whole subsystems into core.

Input masks are not a substitute for engine validation. Qualify which host applies
which rule rather than assuming every DAO/SQL write follows a UI mask. Calculated
field expressions need their own restricted context, not the full set of future
VBA/UDF capabilities. Required validation and data macros must run or writes fail.

Unsupported conversion must identify loss before modifying data: flattening
multivalues, dropping history, stripping markup or extracting only attachment
names cannot be an implicit native save. SQLite conversion needs explicit child
schema and metadata policies; SQLite alone supplies no Access application runtime.

## Acceptance evidence

Use recipe-generated native fixtures for each feature and combinations, then
application copies. Compare Access query row expansion/criteria, DAO children,
attachment hashes, history and metadata before/after supported edits and reopen.
Test malformed children, size limits, failed updates and preservation during
unrelated edits. No advanced-field compatibility is qualified today; record
results in the [application corpus](../testing/ACCESS_APPLICATIONS.md).

## References

Microsoft documents [lookup/multivalue query behavior](https://support.microsoft.com/en-us/access/using-lookup-and-multivalued-fields-in-queries),
[Recordset2 complex children](https://learn.microsoft.com/en-us/office/client-developer/access/desktop-database-reference/recordset2-object-dao)
and [Field2.AppendOnly](https://learn.microsoft.com/en-us/office/client-developer/access/desktop-database-reference/field2-appendonly-property-dao).
See also [attachment behavior](https://support.microsoft.com/en-us/access/attach-files-and-graphics-to-the-records-in-your-database),
[field types/properties](https://support.microsoft.com/en-us/access/introduction-to-data-types-and-field-properties)
and [input masks](https://support.microsoft.com/en-us/access/control-data-entry-formats-with-input-masks).
Each field profile needs property-level references and native evidence when
implemented; this inventory does not assert a complete native specification.

## Working inert metadata slice

`inspect_table_features` joins bounded native schema and original property maps on
stable/offline input. `ColumnFeatures` exposes calculation flags/expression/result
native tag, complex-child FK marker, AutoNumber/Hyperlink flags and explicit
AppendOnly/TextFormat declarations. `NativeColumn::complex_type_id` preserves the
raw ACE type identifier without pretending it resolves children; Jet 4 does not
interpret that overlapping definition slot. Missing properties remain unknown,
wrong typed/width declarations fail on interpretation, and unknown bytes/properties
remain retained. No expression, handler, import mask or active content executes.

CLI: `tablecraft inspect COPY.accdb --table NAME --features`. Results are buffered;
malformed typed declarations return no partial stdout. Ordinary scalar reads
continue rejecting calculated/complex layouts, including empty tables.

The original [advanced fixture](../testing/AdvancedFieldFixture.java) generates
V2016/V2019 calculated, multivalue, attachment, append-only/history, rich-text,
hyperlink and input-mask declarations. Each has ten columns including the hidden
history field. [Independent comparison](../testing/compare_advanced_fields.py)
matches all exposed semantic declarations, compares complex presence and verifies
three distinct raw type IDs. Exact numeric IDs have separate synthetic golden
coverage; the independent public API does not expose their raw numbers. The
property stream retains input-mask/lookup and other declarations without claiming
UI or write semantics. Input hashes remain unchanged; Microsoft Access remains
unrun. Structured child references/values and native recalculation remain next.

Encoding references: [ColumnImpl flags](https://github.com/jahlborn/jackcess/blob/be0a2139b087a227d14cafd4ea231306a5a3a862/src/main/java/com/healthmarketscience/jackcess/impl/ColumnImpl.java),
[JetFormat complex-ID slot](https://github.com/jahlborn/jackcess/blob/be0a2139b087a227d14cafd4ea231306a5a3a862/src/main/java/com/healthmarketscience/jackcess/impl/JetFormat.java),
and [DAO Field2.Expression](https://learn.microsoft.com/en-us/office/client-developer/access/desktop-database-reference/field2-expression-property-dao).
No implementation was copied; no third-party fixture was added.

## Resolved complex-field schemas

`inspect_complex_schema` now resolves each ACE complex type ID through
MSysComplexColumns, verifying conceptual owner/name, unique mapping, catalog-local
value-template and flat-table references, nonaliasing definitions and child shape.
It checks matching value-column native types/widths plus distinct fixed Int32
foreign-key and AutoNumber identity columns. Unsupported shapes fail rather than
becoming collections. A scalar table requires no complex system-table lookup.

`ComplexTableSchema`/`ComplexColumnSchema` retain checked native schemas/references;
`inspect COPY --table NAME --complex` exposes them without reading parent keys,
child values or attachment bytes. It does not qualify calculated evaluation,
`.Value` query expansion, child mutations, attachment decompression or native saves.
The same stable/offline-source contract, cancellation and resource limits apply.

The existing original V2016/V2019 advanced fixtures independently match all three
complex mappings each (multivalue, attachment and history), including raw IDs,
type/flat table names, key column names and declared child counts. The oracle uses
public complex-column APIs plus original native system declarations. Synthetic
checks reject missing/duplicate/foreign-owner/self/dangling references, wrong keys
and mismatched value geometry; CLI errors remain buffered. No Microsoft Access
oracle has run. Next is bounded native child data, preserving raw payload semantics.

## Raw complex-child storage reads

Fixed four-byte native Int32 fields carrying exactly the complex-FK extended bit
0x08 now have an Int32 storage codec on the qualified 2016/2019 profiles. Ordinary
scalar snapshots/query sources can read the resolved flat child tables. Unknown
extra bits, different types/widths and older unqualified profiles remain rejected.
Parent complex fields (0x12) remain outside scalar Value, preventing accidental
flattening of whole collections into an integer or delimited string.

[ComplexRowsFixture.java](../testing/ComplexRowsFixture.java) generates three
multivalue, three attachment-storage and three version-history rows per family,
with Unicode, empty values, fixed timestamps and a parent with empty collections.
[Exact raw comparison](../testing/compare_complex_rows.py) matches all eighteen
child rows across the two families through the native library and CLI query count.
It preserves the encoded attachment FileData bytes as opaque Binary; this is not
file extraction/decompression or proof of parent collection/query `.Value`
semantics. Original input hashes stay unchanged; native Access remains unrun.
Common flag facts now have one owner in native layout rather than separate masks
in schema, feature inspection and complex resolution. No dependency was added.

## Explicit stored Int32 calculations

`read_stored_int32_calculation` inspects one calculated column's stored results on
a stable/offline 2016/2019-family source. CLI:
`inspect COPY.accdb --table NAME --stored-calculation COLUMN`. Output explicitly
labels stored values and uses zero-based snapshot-local row ordinals. No
expression is parsed/evaluated and no freshness or recalculation guarantee is
implied. Ordinary scalar snapshots/query reads still reject calculated fields.

The bounded profile requires native Int32, variable storage, capacity 39, exact
updatable/calculated flags, explicit matching BYTE ResultType and nonempty text
Expression declarations, including on empty tables. Qualified wrappers have a
zero sixteen-byte prefix, little-endian length at 16, data at 20 and three zero
trailing bytes. Int32 results have exactly four data bytes; an absent field or a
qualified zero-length wrapper means stored NULL. Unknown prefixes, lengths,
trailers and declaration variants fail without a partial result. The broader
stored-scalar profile below extends explicit inspection; historical layouts
remain unqualified. Shared record geometry, owned-page/count
checks, fallible allocation, budgets and cancellation still apply.

The original [stored-calculation recipe](../testing/CalculatedStorageFixture.java)
explicitly disables expression evaluation and writes known caches, including a
deliberately stale result, NULL and Int32 extremes. The
[comparison runner](../testing/compare_calculated_storage.py) matches seven exact
stored results and an empty table per family, rejects ordinary query reads and
three unknown/malformed disposable copies/profile, and verifies unchanged original
hashes. Synthetic CI tests also cover matching declarations, profile/width/flag
gates, malformed typed properties, late CLI failures and resource/cancellation
limits. No Access application code executes.

Wrapper facts come from the reviewed Jackcess 5.0.4 CalculatedColumnUtil artifact,
Apache-2.0; [its source reference](https://github.com/jahlborn/jackcess/blob/be0a2139b087a227d14cafd4ea231306a5a3a862/src/main/java/com/healthmarketscience/jackcess/impl/CalculatedColumnUtil.java)
is format research only. No implementation was copied or linked. Native Access
comparison, error-state variants, remaining result types, dependencies, recalculation
and all ACE writes remain separate work.

## Explicit stored scalar calculations

`read_stored_calculation`/`StoredCalculation` now extend the explicit path to
Boolean, Double, Currency, legacy DateTime and Short Text alongside Int32. The
strict Int32 API remains available. Generic values retain exact core carriers;
ordinary scalar query snapshots still reject calculated fields. CLI
`--stored-calculation` uses this profile, labels the stored carrier and buffers
formatted/escaped output before publication. Formulas can be stale and never run.

The same code-5/code-6, matching typed ResultType, nonempty Expression, variable
storage and exact flag/wrapper gates apply even to empty tables. Fixed carriers
require declared capacity 39. Calculated Boolean's variable capacity is retained
in metadata; ordinary fixed Boolean still uses its mask bit. Present canonical
Boolean payloads are 0x00/0xff, a qualified empty wrapper is false, and absent
storage is NULL. Other Boolean encodings fail explicitly.

Double retains all stored bits, including signed zero and subnormals; Currency
retains the signed scaled integer. Legacy DateTime uses the existing finite,
bounded exact-serial constructor. Short Text declares the 23 wrapper bytes plus
an even capacity through 510 UTF-16 bytes. Both observed compression flags are
qualified; decoding validates UTF-16/mode switches and decoded length in UTF-16
units. A present empty Text wrapper yields empty Text, separately from NULL.
Leading FF FE is a native compression marker, not an unescaped literal U+FEFF;
an explicit wide segment represents that character. The original recipe's
read-only text probe checks both encodings against the reviewed decoder, and
synthetic CI retains the valid escaped form and arbitrary Unicode after an
ordinary uncompressed prefix.
Unknown types, flags, capacities, wrappers, encodings and lengths fail without a
partial result. Long Text/Decimal/extended-date calculations and error caches
remain unqualified.

The original [mixed scalar recipe](../testing/CalculatedScalarFixture.java)
explicitly disables evaluation and creates six stored projections plus empty
declarations in each family. [Independent comparisons](../testing/compare_calculated_scalars.py)
match 30 cells/profile, compressed/uncompressed Unicode and empty Text, Boolean
NULL/false, signed-zero/subnormal/maximum Double, full-range Currency and dates.
The original [date-bit oracle](../testing/StoredDateBitsOracle.java) uses the
reviewed artifact's Date carrier and `ColumnImpl.toDateDouble` to retain serial
bits; a calendar round trip would lose precision. For example the cached serial
renders 03:04:05.000000065 through TableCraft while Jackcess's calendar renderer
reports milliseconds. This is an explicit precision-policy difference, not native
Access conformance. Input hashes remain unchanged and a corrupted disposable
Boolean cache produces no stdout. Synthetic tests cover strict metadata, empty
tables, malformed values, exact bit/Unicode models, budgets and late cancellation.

Microsoft documents [Expression and Result Type properties](https://support.microsoft.com/en-us/access/introduction-to-data-types-and-field-properties)
for calculated fields; binary eligibility follows the reviewed CalculatedColumnUtil
artifact/reference above. No third-party implementation was copied. Recalculation,
query eligibility, required macro semantics and all ACE writes remain separate.

## Scoped complex parent and child links

`read_complex_column_links` resolves a selected complex column, projects its stored
parent keys and reads the complete resolved flat child snapshot. It returns
snapshot-local parent links and borrowing child-row iterators in flat snapshot
order. Keys are scoped to that selected column; they are not global identities.
Missing parent storage retains NULL/empty membership. An existing empty parent
returns an empty iterator; an invalid ordinal returns None. Attachment Binary
stays encoded; no extraction or decompression occurs.

The profile admits code-5/code-6 fixed four-byte complex parent keys with exact
fixed/updatable/AutoNumber flags and either zero extended flags or the observed
0x20 history-parent variant. Assigned parent keys and flat FK/identity values
must be positive. Parent keys and child identities must be unique in their own
scopes; every child FK must bind a parent in the selected column. Typed
`ComplexReferenceViolation` errors distinguish ambiguous parents, missing parents,
duplicate child identities and NULL child references. Other layouts/values remain
unsupported. No partial storage view is published on failure.

CLI `inspect COPY.accdb --table NAME --complex-links COLUMN` buffers zero-based
parent ordinals, stored keys and raw child counts. It reads only explicit local
storage, with shared geometry/count/ownership checks, fallible allocation, bounded
sorting/search work and cancellation. Flat index trees, `.Value` query expansion,
Access attachment/history semantics, child mutations and native saves remain
separate work. This view is not a write authorization or database integrity claim.

The existing [complex recipe](../testing/ComplexRowsFixture.java) now has a read-only
`--links` oracle using its public key/count APIs. The
[comparison runner](../testing/compare_complex_links.py) matches nine scoped parent
links per family across multivalue/attachment/history columns, including empty
memberships. Twelve corrupted disposable copies/profile check missing/duplicate/
NULL references with no partial stdout. Existing raw child-row comparisons remain
separate evidence. Synthetic CI tests verify exact child memberships/ordering,
NULL/empty distinctions, strict profiles, resource/cancellation limits and CLI
atomicity. Original input hashes remain unchanged; native Access remains unrun.

## Bounded original attachment decoding

`decode_attachment` explicitly decodes native FileData wrappers into original
file bytes and a preserved wrapper type label. It neither opens the file nor
uses metadata as a path. Ordinary native scalar/complex-link readers still return
encoded Binary; decoding is a separate operation. The pure Rust flate2 backend
handles zlib and verifies its checksum. Native wrapper/header parsing stays in
ACE. No archive, Office document, image, script or executable content is interpreted.

The current profile admits direct (tag 0) or one complete zlib (tag 1) body. The
outer little-endian length must match the exact decoded body. The inner header
has a checked length, tag 1 and exact UTF-16 type-unit count/terminator. Type text
retains embedded Unicode/NUL content and is not a MIME/path validation result.
Unknown tags/dictionary variants fail as Unsupported; inconsistent lengths,
invalid Unicode/checksums, excess output and trailing streams publish no payload.

Encoded inputs are capped at 64 KiB; decoded bodies at 68 KiB, headers at 4 KiB,
file bytes at 64 KiB, and type text retains the existing bounded native UTF-16
policy. Uncompressed wrappers also have to fit the encoded ceiling. Fallible
project buffers, exact codec progress/end checks and a one-byte overflow sentinel
prevent trusting an advertised size alone. Copy/inflate work checks budgets and
cancellation between at most 4 KiB input/output blocks. A conservative 128 KiB
logical codec workspace is charged; upstream internal allocations are not
intercepted. Other encodings, large streaming files and attachment mutation remain
unimplemented. File extraction/publication policy belongs to a future host.

The original [attachment recipe](../testing/AttachmentFixture.java) generates
fresh V2016/V2019 files with both wrapper modes, an empty file, all byte values,
Unicode bytes, inert PNG/ZIP signature bytes, a 64 KiB compressible file and an
empty parent. [Independent comparison](../testing/compare_attachments.py) matches
six original files/type labels per family through Jackcess's public decoder and
the `attachment_data` engineering example; original input hashes remain unchanged.
CI tests add an independent Python-zlib golden, truncation/checksum/metadata/
Unicode/overflow/trailing-stream cases, resource/cancellation checks and 256
fixed-seed generated files. No native Access oracle has run.

References: the reviewed Jackcess 5.0.4 AttachmentColumnInfoImpl artifact,
[its source reference](https://github.com/jahlborn/jackcess/blob/be0a2139b087a227d14cafd4ea231306a5a3a862/src/main/java/com/healthmarketscience/jackcess/impl/complex/AttachmentColumnInfoImpl.java),
and the [flate2 streaming API](https://docs.rs/flate2/1.1.10/flate2/struct.Decompress.html).
No upstream implementation was copied. [Dependency records](../DEPENDENCIES.md#native-attachment-compression)
and [native benchmarks](../testing/NATIVE_BENCHMARKS.md) own adoption/measurement.
