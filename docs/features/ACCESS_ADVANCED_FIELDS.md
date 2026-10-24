# Advanced Access field behavior

Status: inert field-feature metadata, raw child scalar storage and explicit stored
scalar calculation inspection are implemented. Scoped raw parent/child links and
bounded original attachment decoding are also available. Explicit mixed
scalar/multivalue pages and optionally cached results are available through the
[browsing API](../CLI_BROWSING.md#explicit-multivalue-cells). Full collection/query
semantics, calculated evaluation and editing
remain future requirements. Current core values represent
scalars. [Backend-neutral scalar multivalue query expansion](../MULTIVALUE_QUERIES.md)
now binds and executes explicit `.Value` projections/grouping over host collection
views. Qualified native stored values now connect through explicit
`query run --database PATH --multivalues`; a field editor remains unimplemented;
SQL member syntax alone does not establish native `.Value` compatibility.

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

An explicit [row-local calculation preview](../CALCULATION_PREVIEWS.md) can now
prepare/evaluate admitted stored formulas against ordinary same-table fields.
It returns separately named recomputed results without replacing cached values,
coercing to their declared carrier or establishing freshness/native compatibility.
Ordinary query eligibility and the inert cached browsing path below remain unchanged.

The explicit [cached browsing profile](../CLI_BROWSING.md#explicit-stored-calculations)
now composes these qualified stored results with ordinary scalar fields in one
physical page. `StoredTablePage` retains the inert declarations and per-column
cached markers; `rows --stored-calculations` labels headers `[cached]`.
It shares the decoder/gates below without enabling formula execution, ordinary
query eligibility or writes. A selected bad cache fails the entire page, while
skipped/unvisited payloads remain outside partial-read validation.


`read_stored_calculation`/`StoredCalculation` now extend the explicit path to
Boolean, Double, Currency, legacy DateTime, Decimal and Short Text alongside Int32. The
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
partial result. Extended-date calculations and error caches remain unqualified.
[Cached Long Text calculations](../CACHED_MEMO_CALCULATIONS.md) now resolve observed
MEMO long-value storage before unwrapping and decoding the cached Text.
[Cached Decimal calculations](../CACHED_DECIMAL_CALCULATIONS.md) now admit the
observed Numeric wrapper separately, with self-described effective scale and
strict geometry independent of ordinary Decimal declarations.

The original [mixed scalar recipe](../testing/CalculatedScalarFixture.java)
explicitly disables evaluation and creates six stored projections plus empty
declarations in each family. [Independent comparisons](../testing/compare_calculated_scalars.py)
match 30 cells/profile, compressed/uncompressed Unicode and empty Text, Boolean
NULL/false, signed-zero/subnormal/maximum Double, full-range Currency and dates.
The original [date-bit oracle](../testing/StoredDateBitsOracle.java) uses the
reviewed artifact's Date carrier and in-memory `ColumnImpl.write` serialization
to retain serial bits, with strict calculated-wrapper validation. `toDateDouble`
can round through milliseconds and is no longer used as a bit oracle. Independent
[ordinary/cached precision probes](../testing/compare_date_bits.py) now cover signed
zero, negative aliases, subnormals and sub-millisecond dates. For example the cached serial
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

Encoded inputs are capped at 2 MiB; decoded bodies at 1 MiB plus 4 KiB, headers at 4 KiB,
file bytes at 1 MiB, and type text retains the existing bounded native UTF-16
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

## Bounded stored multivalue collections

`read_multivalue_column` now composes validated complex links into an owned
`MultivalueColumn`, with a shared scalar `value_column()` and borrowing `values(parent)`
iterators. Values retain flat storage order, duplicates, NULL cells and exact
carriers. An existing empty parent has an empty iterator; an absent ordinal returns
None. `links()` retains the raw schema, child identities and scoped parent keys.
Keys/ordinals remain snapshot-local, and no nested core Value variant is added.

The ACE 2007/ACE 16 profiles admit a single conceptual Byte/Int16/Int32/Single/
Double/GUID/Decimal/short-Text value field with qualified fixed/updatable/extended
flags; Int64/Large Number stays ACE 16-only.
Paired scalar types, fixed/variable representation and Decimal precision/scale
must agree; ordinary native field gates remain. Each parent is capped at 100
stored values, including NULL cells and duplicates. This follows Microsoft's
[documented product ceiling](https://support.microsoft.com/en-us/access/using-lookup-and-multivalued-fields-in-queries)
as an explicit bounded-reader policy. Raw complex links retain their separate
whole-table ceiling and can inspect the independent reader's 101-item control.
All validation completes before publishing a collection; existing fallible
allocation, parent/child integrity checks, budgets and cancellation remain shared.

The reviewed Jackcess 5.0.4 `ComplexColumnSupport` recognizes single-field scalar
templates. Its ACE 2007/ACE 16 Decimal conceptual template declares length 9, precision
18/scale 0, while `ComplexColumnCreator` writes ordinary 17-byte child fields.
The resolver now accepts only this empty, variable, updatable, unextended template
hint with matching qualified child metadata. It does not introduce a nine-byte
record decoder or relax the ordinary Decimal scalar profile. Unknown hints,
nonempty conceptual templates and incompatible children fail explicitly.

ACE 2007 now has independent [raw-link and exact-collection evidence](../FIXTURES.md#ace-2007-complex-key-and-multivalue-inputs),
including the same conceptual hint and fixed Int32 child foreign-key layout.
Typed attachments/history now have separate [ACE 2007 qualification evidence](../FIXTURES.md#ace-2007-stored-attachment-and-history-inputs); cached
calculations retain their ACE 16 gates. The raw primitive by itself does not
qualify additional semantics.
Ordinary nine-byte Decimal storage, Large Number fields and `.Value` SQL semantics
are not inferred from successful legacy collection reads.

The original [recipe](../testing/MultivalueFixture.java) supplies eight native
scalar collection types, exact signed/subnormal/non-finite IEEE bits, GUIDs,
Decimal neighbors beyond f64 precision, Unicode/control text, duplicates, stored
NULLs, empty parents, 100-item and separate 101-item controls. Inert row-source
properties meet the reference builder's requirements; no lookup is executed.
[The read-only oracle](../testing/MultivalueOracle.java) uses public collection
retrieval and exported implementation metadata with expression evaluation disabled.
[Comparison](../testing/compare_multivalues.py) matches 32 parent collections and
137 stored values per V2016/V2019 family, rejects 12 corrupt disposable copies
without output, and rechecks the earlier Text/attachment/history inputs. Original
hashes remain unchanged and recorded in [provenance](../FIXTURES.md#original-ace-16-multivalue-collection-inputs).

Seven CI tests add exact Large Number collections, typed template/profile guards,
strict Decimal hints, 100/101 boundaries, immutable source bytes, resource/
cancellation failures and a 128-case nullable-sequence/multiplicity model. The
engineering `table_data --multivalues COLUMN` mode supplies tagged comparisons;
it conflicts with stored-calculation/index modes. The production CLI and query
source have no new collection mode. Lookup display translation, `.Value` SQL
expansion, Access collection sorting/deduplication rules, mutation, native Access
and application save/reopen remain unqualified. All ACE writes remain blocked.

```sh
java -cp path/to/jackcess-5.0.4.jar docs/testing/MultivalueFixture.java new-2016.accdb V2016
java -cp path/to/jackcess-5.0.4.jar docs/testing/MultivalueFixture.java new-2019.accdb V2019
python3 docs/testing/compare_multivalues.py --jar path/to/jackcess-5.0.4.jar \
  --cli target/release/tablecraft --rows target/release/examples/table_data \
  --ace2016 new-2016.accdb --ace2019 new-2019.accdb \
  --complex2016 complex-2016.accdb --complex2019 complex-2019.accdb
```

## Typed stored attachment collections

`read_attachment_column` now returns a complete `AttachmentColumn` over checked
complex links. `attachments(parent)` borrows typed `StoredAttachment` records in
flat snapshot order, retaining positive scoped child identities and nullable
FileURL, FileName, FileType, wrapped FileData, exact legacy FileTimeStamp and raw
Int32 FileFlags. Empty parent membership remains distinct from an absent ordinal.
Accessors use typed errors without unchecked casts; strings/bytes borrow the
existing snapshot without an additional payload copy.

The current ACE 2007/ACE 16 profiles require exactly six named conceptual fields and
their matching flat child fields, the observed native types/widths and qualified
fixed/updatable flags. Empty conceptual templates admit zero or the observed
0x10 extended marker; flat data fields require ordinary supported scalar flags.
Unknown/extra/missing fields, nonempty templates, other collection types and
inconsistent references fail before publication. Metadata/record validation
charges bounded work and checks cancellation; existing fallible native reads and
whole-table ceilings remain. No multivalue-specific 100-item cap is applied here.

`StoredAttachment::decode` explicitly invokes the existing bounded wrapper
decoder. NULL FileData returns None; a present invalid wrapper returns an error,
not an empty file. NULL metadata, empty strings and empty encoded byte values
remain distinct. Metadata inspection does not validate or inflate the wrapper.
Stored FileType and the wrapper's decoded type label remain separate. No string
becomes a filesystem destination or resolved URL, and no file content executes.

The existing [attachment recipe](../testing/AttachmentFixture.java) now has a
read-only `--metadata` oracle using exported native metadata/public attachment
accessors. Its ordinary DATE writer retains exact timestamp serial bits with
UTC/DateExt settings and expression evaluation disabled; it never writes the
database. [The comparison](../testing/compare_attachment_columns.py) matches all
six original metadata/payload records per V2016/V2019 family. Six disposable
mutations cover all-NULL metadata, late missing payload/checksum failure, unknown
template flags/names and orphan references. Valid NULL/corrupt-wrapper metadata
still matches the independent getter; explicit decoding fails without stdout,
including after earlier files were decoded. Other collection types are rejected.
Original hashes remain unchanged in [provenance](../FIXTURES.md#original-ace-16-attachment-payload-fixtures).
Both typed collection comparisons discover the generated hidden version-history
column through resolved declarations and reject that actual collection as well
as the visible ordinary Notes Memo field; the two are not interchangeable.

Four CI tests add template reordering, Unicode/NUL/path-like labels as inert data,
exact negative DATE bits/raw flags, NULL/empty distinctions, independent zlib
bytes, field/wrapper label differences, strict metadata and resource/cancellation
checks. The engineering `attachment_data` example now uses the typed reader;
its `--metadata` mode emits encoded data, while its default decoded output still
matches all 65,810 original bytes/profile. Native Access, collection mutation,
large streaming payloads, full native query expansion and host extraction remain unqualified.
Explicit TableCraft [decoded query views](../NATIVE_QUERIES.md#explicit-decoded-attachment-values)
now compose those original bytes with aligned metadata under a separate opt-in;
they do not qualify native SQL FileData representation.
All ACE writes stay blocked.

```sh
python3 docs/testing/compare_attachment_columns.py --jar path/to/jackcess-5.0.4.jar \
  --cli target/release/tablecraft --example target/release/examples/attachment_data \
  --ace2016 attachments-2016.accdb --ace2019 attachments-2019.accdb \
  --complex2016 complex-2016.accdb --complex2019 complex-2019.accdb
```

## Stored version-history collections

`read_version_history_column` accepts a visible Memo name and resolves its hidden
history storage through the conceptual value-field name and observed 0x20 parent
key marker. Generated names are not guessed. The qualified ACE 2007/ACE 16 profiles require
an ordinary visible Memo, an empty two-field Memo/legacy-DATE template, matching
flat fields and checked scoped parent/child references. Missing, incompatible or
ambiguous bindings fail explicitly. A Memo without history is Unsupported;
an absent visible column is UnknownColumn.

The returned `VersionHistoryColumn` exposes the associated visible `memo_column()`,
raw hidden-column `links()` and borrowing `stored_versions(parent)` records.
Each `StoredVersion` retains its positive child identity, nullable text and exact
nullable DATE serial. Stored flat-row order, duplicate text, Unicode/NUL content,
NULL/empty text, empty membership and absent ordinals remain distinct. No current
Memo value, timestamp, newest-first order or latest-value selection is synthesized.
Typed accessors avoid unchecked casts and additional text copies.

Resolved complex schemas now feed an internal shared link reader directly,
avoiding a second schema resolution for the visible-to-hidden binding. Existing
fallible storage reads, sort/search/reference checks, work/allocation budgets and
cancellation remain shared; history record checks charge bounded work before
publication. Earlier multivalue/attachment/link readers use the same checked path.

The reviewed Jackcess 5.0.4 [history implementation](https://github.com/jahlborn/jackcess/blob/be0a2139b087a227d14cafd4ea231306a5a3a862/src/main/java/com/healthmarketscience/jackcess/impl/complex/VersionHistoryColumnInfoImpl.java) binds its value-field
name to the visible Memo. Its public history API sorts by modified time and then
descending child identity for ties; that reference observation is separate from
this deliberately stored-order view. [The original recipe](../testing/VersionHistoryFixture.java)
uses two tables to exercise Notes/Details bindings and independently scoped keys.
The reviewed builder permits only one append-only Memo per table; that generator
restriction is not a Microsoft Access compatibility claim. Explicit DateExt/Date
overloads preserve signed zero, subnormal, negative-alias, sub-millisecond and
calendar-endpoint DATE bits. No lookup, formula or application content executes.

[The read-only oracle](../testing/VersionHistoryOracle.java) iterates flat storage
and separately exposes public API order, with expression evaluation disabled.
[Comparison](../testing/compare_version_history.py) matches six parent collections/
18 versions per V2016/V2019 family, all 13 literal DATE bit probes, empty/Unicode/
NUL text and nullable values/times. Public API membership matches; a separate
equal-time control observes its reversed identity order without assigning that
policy to TableCraft. Six corrupt disposable copies fail without stdout for
reference, template, hidden-marker and Memo-name binding violations. An additional
NULL timestamp mutation still matches raw reference storage. Original populated
history controls and hashes remain unchanged in [provenance](../FIXTURES.md#original-ace-16-stored-version-history-inputs).

Five CI tests add strict source/profile/template guards, template reordering,
NULL parent references, orphan rejection, exact representation/storage order,
resource/cancellation limits and a 128-case nullable Unicode/DATE sequence model.
The synthetic UTF-16 generator keeps its first bytes outside the reserved native
compressed-text header, while retaining arbitrary Unicode suffixes; its seed is
kept as a fixture-encoding guard. The engineering `table_data --version-history MEMO`
mode accepts the visible name and conflicts with index/calculation/multivalue
modes. Explicit [stored-history query members](../STORED_HISTORY_QUERIES.md) now
reuse these checked records and their actual hidden/value/modified field names.
Current-value reconciliation, default chronological history
ordering, append/update/delete semantics, actual Access and application save/reopen
remain unqualified. All ACE writes stay blocked.

```sh
java -Dfile.encoding=UTF-8 -cp path/to/jackcess-5.0.4.jar docs/testing/VersionHistoryFixture.java new-2016.accdb V2016
java -Dfile.encoding=UTF-8 -cp path/to/jackcess-5.0.4.jar docs/testing/VersionHistoryFixture.java new-2019.accdb V2019
python3 docs/testing/compare_version_history.py --jar path/to/jackcess-5.0.4.jar \
  --cli target/release/tablecraft --rows target/release/examples/table_data \
  --ace2016 new-2016.accdb --ace2019 new-2019.accdb \
  --complex2016 complex-2016.accdb --complex2019 complex-2019.accdb
```


## Shared browsing and CLI stored-content views

The Access backend now exposes typed attachment/history columns through
`NativeBrowser::attachment_column` and `version_history_column` under shared
cumulative controls. `inspect --table NAME --attachments COLUMN` reports inert
metadata and encoded lengths; `--version-history MEMO` reports exact stored
versions. Both retain complete checked snapshots and show scoped membership plus
raw legacy DATE bits. Neither activates content, decodes attachments implicitly,
selects latest history, or changes ordinary scalar query/row profiles.
See the [CLI contract](../CLI_BROWSING.md#stored-attachments-and-memo-history).
The existing independent attachment/history runners check these CLI views too.


## Mixed typed stored-collection pages

`read_stored_collection_table_page` combines ordinary scalar cells, multivalue
lists, attachment metadata/encoded bytes and stored Memo history in one bounded
parent page. Complete raw links feed the existing strict typed validators;
conceptual field count only routes validation and never qualifies a type alone.
Unknown templates fail without decoder fallback. The old multivalue-only page
API now wraps the same page engine under its existing restricted policy.
History still binds a qualified visible Memo and hidden key, and attachment
wrappers remain encoded. No SQL scalar coercion, implicit decoding or latest-value
selection is introduced. Shared browsing and `rows --collections` consume this
[typed page contract](../CLI_BROWSING.md#mixed-scalar-and-stored-collection-pages).

## Attachment metadata SQL

Explicit native `query run --collections` now binds stored `FileURL`, `FileName`,
`FileType`, `FileTimeStamp` and `FileFlags` members through shared aligned child
views. One attachment contributes its matching fields together; no cross-product
is taken within that child record. Metadata binding reads no user records;
execution uses complete bounded attachment snapshots and retains encoded payloads.
`FileData` also exposes exact stored Binary under this opt-in, preserving the
encoded envelope. [Stored attachment data](../STORED_ATTACHMENT_DATA.md) documents
the physical declaration/getter evidence and explicit decoded alternative. Native
SQL/DAO byte equivalence, implicit extraction/decoding and native Access semantic
qualification remain pending. See the
[query contract](../NATIVE_QUERIES.md#attachment-metadata-member-queries).

## Explicit stored-history SQL views

`query run --collections` now also exposes qualified stored-history members under
their actual resolved hidden field names. Text and modified-time references stay
aligned to one complete checked snapshot; the visible current Memo remains an
independent scalar. Binding reads metadata only, Text policies come from the flat
member, and unsupported/malformed shapes retain the strict history guards.
See [stored-history query contracts](../STORED_HISTORY_QUERIES.md) for discovery,
examples, resource/cancellation and cache boundaries. This engineering view does
not establish native Access SQL equivalence or latest-value semantics.
