# Native scalar tables

Native compressed ASCII and ASCII UTF-16LE have bounded wide/bytemuck classifiers
that avoid an intermediate UTF-16 vector. Raw byte arrays are cast by value;
stored semantic types are not POD. Successful fast paths charge/reserve the retained
UTF-8 bytes; other Unicode and compressed mode switches keep strict validation.
Native long-value descriptors use borrowed little-endian zerocopy fields without
changing payload/length/chain validation. See [benchmarks](testing/NATIVE_BENCHMARKS.md).

Native index metadata is available separately; see [NATIVE_INDEXES.md](NATIVE_INDEXES.md).

The ACE library's `inspect_table_schema` and CLI `inspect <copy.accdb> --table NAME`
inspect one explicitly named local table using the bounded, unencoded Jet 4 /
ACE-code-2/5/6 profiles in [NATIVE_READING.md](NATIVE_READING.md). Linked tables and
saved queries are rejected; no external connection or active content is invoked.

`TableSchema` reports the catalog-scoped identity, definition page, declared row
count and columns in native logical column-number order. `NativeColumn` preserves
native type/flags/extended flags, fixed/variable storage and byte length (ordinary
fixed Boolean uses zero payload bytes; variable calculated Boolean retains its
declared capacity). Unknown/unsupported types remain visible in inspection.
`scalar_columns` maps supported columns into existing `tablecraft-schema::Column`
contracts and fails if any column is unsupported. No new generic schema package
or duplicate object model is needed.
Native schema name copies reserve fallibly after their existing budget charges.
Column-number sorting is in place without scratch allocation: layout decoding
already rejects duplicate numbers, so equal-key stability is unnecessary.

The scalar profile covers Boolean, Byte, Int16, Int32, Currency, Single,
Double, GUID, qualified Decimal, legacy Date/Time, bounded Long Text/opaque OLE and short Binary (1–510 declared bytes) and short Text (2–510 declared bytes,
an even UTF-16 size).
complex/attachment, calculated and unqualified extended fields remain unsupported
for scalar reads. Native Text length is in UTF-16 units; it is not converted into
a schema Unicode-scalar limit. Separate [property inspection](NATIVE_PROPERTIES.md)
now reads Required/default/validation declarations. Schema/row APIs do not load or
enforce them, so input columns conservatively permit NULL except Boolean. AutoNumber
flags describe stored metadata and never generate values during inspection.

Short Binary and Text have separately named native read limits. Both currently
accept up to 510 declared bytes; Text requires even UTF-16 width. Native system
Binary columns in MSysObjects/MSysACEs declare 510 bytes and remain readable,
matching the independent reader. Zero-length declarations remain outside this
profile. The
[pinned Jackcess type definitions](https://github.com/jahlborn/jackcess/blob/be0a2139b087a227d14cafd4ea231306a5a3a862/src/main/java/com/healthmarketscience/jackcess/DataType.java)
and an observed writer rejection limit newly created Binary fields to 255 bytes;
that authoring limit must not reject observed existing system metadata. Independent
metadata tests cover 255/256/510/511-byte Binary, Text boundaries and both actual
system schemas. TableCraft implements no native column creation yet.

The original format reader has shared internal modules for page access, usage
maps, definition metadata and Unicode decoding; catalog-specific validation stays
with the catalog consumer. Limits, cancellation, stable-copy requirements and
bounded chained table definitions follow the limits below and in NATIVE_READING.md.

On 18 October 2026, schemas from all 36 local tables (including system tables),
187 columns in the registered candidate matched Jackcess 5.0.4 for name, native
type, normalized storage length and logical order. The temporary independent
scanner opened a copy read-only and accessed column metadata only. Jackcess
reported column usage-map warnings in MSysAccessXML/MSysNameMap; this comparison
does not qualify those usage maps, table contents or validation properties.
The artifact/version/hash and license boundary are recorded in NATIVE_READING.md.
Focused library and CLI tests cover supported schemas, unsupported fields,
lookup/format failures, limits, cancellation and unchanged input bytes.
Microsoft Access comparison has not been performed.

## Bounded chained table definitions

The shared native definition reader handles one page or a chain of at most 64
pages, with a maximum 261,640 logical bytes. The first page retains its header;
continuations contribute payload after their eight-byte header. Column blocks,
UTF-16 names and physical/logical index definitions may cross those boundaries.
The one-page path retains a stack buffer without a join allocation.

The declared logical length bounds both paths. Too-short/truncated definitions,
missing/extra links, cycles, invalid page signatures and out-of-file references
fail without partial metadata. Chained allocation, page I/O and work consume the
caller budget; cancellation is checked between pages and during I/O. Unknown
properties/trailers, page free-space accounting and schema write semantics remain
unqualified; this reader does not interpret an entire definition as writable data.

The original [DefinitionFixture](testing/DefinitionFixture.java) recipe creates
Wide and EmptyWide tables with 200 Int32 columns, long non-ASCII names and
ascending/nullable-descending indexes. Each definition occupies seven pages and
28,079 logical bytes. The original [comparison runner](testing/compare_definitions.py)
matched all 800 column descriptions across the two tables/formats, eight index
definitions, and 12 complete/bounded CLI outputs against Jackcess 5.0.4. Java
stdout is explicitly UTF-8, preserving non-BMP field names. No active content runs.

```sh
java -cp path/to/jackcess-5.0.4.jar docs/testing/DefinitionFixture.java path/to/new-wide.accdb V2007
java -cp path/to/jackcess-5.0.4.jar docs/testing/DefinitionFixture.java path/to/new-wide.mdb V2000
cargo build --release --locked -p tablecraft-cli
python3 docs/testing/compare_definitions.py \
  --cli target/release/tablecraft --jar path/to/jackcess-5.0.4.jar \
  --ace path/to/new-wide.accdb --jet path/to/new-wide.mdb
```

Independent layout goldens exercise column/name/index boundaries, empty/exact/max
lengths, corruption, limits, cancellation, every I/O failure callback and 256
fixed-seed byte mutations. [Measurements](testing/NATIVE_BENCHMARKS.md#chained-table-definitions)
check ordinary-reader regressions and new wide-schema/indexed workloads.

Primary format research: [Jackcess TableImpl](https://github.com/jahlborn/jackcess/blob/be0a2139b087a227d14cafd4ea231306a5a3a862/src/main/java/com/healthmarketscience/jackcess/impl/TableImpl.java),
Apache-2.0, especially its definition-header writer, multi-page writer and complete
definition loader. No upstream implementation was copied. This is independent
writer/reader evidence; native Microsoft Access and other target execution remain
unrun. Generated inputs/JARs are not tracked or required by CI.

## Bounded scalar snapshots

`read_table` returns a completed `TableSnapshot` containing native metadata,
shared input columns and owned rows. All columns must fit the scalar profile;
unsupported columns fail even if a later query would project only supported ones.
Each call shares the caller's catalog/schema/row byte, row and step budgets.
The snapshot is bounded to 16,384 live records and the native structural ceilings;
failures return no partial snapshot. This read_table entry point performs an owned-page
scan; the separate [read_table_index](NATIVE_INDEXES.md#bounded-indexed-reads) path
uses a qualified named native index. The range API supplies storage selections;
the adapter can push safe filters through it. TOP remains in the query executor.

The decoder follows owned data pages and relocated live rows, skips deleted roots,
checks the declared live count, row offsets and masks, and handles fixed-only as
well as variable-column records. A sequential scan borrows its current page for
each live root, retaining owner/signature/slot validation and checking cancellation
and work between rows. Relocation targets still incur physical reads and their
normal owner/range/cycle checks. Only avoided I/O charges disappear; row copying
and allocation still consume the shared budget. Nothing is cached across scan
pages or separate operations. [The scan benchmark](testing/NATIVE_BENCHMARKS.md)
compares memory-backed public APIs, rather than disk or native shared-file behavior.
Older rows support the qualified dense
append-only history below; gapped/deleted histories remain unsupported. Boolean bits are values and never NULL. Other
NULLs, empty Text and empty Binary remain distinct. Numeric carriers preserve
signed ranges, Currency's scaled integer and IEEE floating bits, including signed
zero/special representations; query evaluation has its own finiteness rules.
Text is decoded strictly and checked against native UTF-16 units. Invalid wide
compression segments are rejected rather than decoded lossily.

Synthetic tests cover these distinctions, fixed/variable storage, relocation,
deleted rows, malformed data, unsupported/calculated types, shared limits and
mid-operation cancellation. A third 256-case fixed-seed native property mutates
row/definition pages. The candidate tests read ROULETTE_TB (38), TAG_GRP_TB (19),
TAG_NME_TB (933) and empty tblContacts (0) from byte copies.

A temporary external Rust consumer and Jackcess 5.0.4 matched all 1,488 rows across
ROULETTE_TB, TAG_GRP_TB, TAG_NME_TB, tblContacts, tblDefaults, DATABASE_STRUCTURE_TB
and MSysACEs exactly, including column order, physical row order and tagged values.
The normalized comparison encodes NULL, typed integers/booleans, IEEE bits,
Currency's scaled integer and hexadecimal UTF-8/Binary, preserving representational
distinctions. The SHA-256 of that normalized output was
`fa9f2863319eef2e89ed10c8ccf1507896af7cd785deabc1fcfa8fa205eb1d87`.
This candidate comparison exercises Boolean/Byte/Int32/Text/Binary; remaining
supported carriers have synthetic evidence only. No Microsoft Access oracle ran.

The original independent helpers require a separately reviewed JAR and disposable copy:

```sh
java -cp path/to/jackcess-5.0.4.jar docs/testing/TableSchemaOracle.java path/to/copy.accdb ROULETTE_TB
java -cp path/to/jackcess-5.0.4.jar docs/testing/TableDataOracle.java path/to/copy.accdb ROULETTE_TB
```

[Schema helper](testing/TableSchemaOracle.java) reports names/type/length/order;
[data helper](testing/TableDataOracle.java) reports tagged data only for explicitly
named supported tables. These engineering helpers never execute database content
and are outside the product/CI dependency closure.

Additional pinned Jackcess sources reviewed for this implementation are
[ColumnImpl](https://github.com/jahlborn/jackcess/blob/be0a2139b087a227d14cafd4ea231306a5a3a862/src/main/java/com/healthmarketscience/jackcess/impl/ColumnImpl.java)
for type payloads, Boolean mask values and extended flags, and
[TableImpl](https://github.com/jahlborn/jackcess/blob/be0a2139b087a227d14cafd4ea231306a5a3a862/src/main/java/com/healthmarketscience/jackcess/impl/TableImpl.java)
for row/fixed/variable placement and relocation. The same Apache-2.0 research-only
boundary in NATIVE_READING.md applies; no source implementation was copied.

## Legacy Date/Time

Native type 0x08 / eight bytes now decodes through LegacyDateTime's validated exact
bit carrier. Invalid/nonfinite/out-of-envelope dates fail explicitly. Core uses
Jiff civil types for checked Gregorian validation and explicit calendar conversion, replacing
the previous manual extended-calendar calculation. Stored bits and representational
Eq remain unchanged. `temporal_cmp` compares day/time fractions, so -0.25 and +0.25
are the same time and negative-day time ordering is not numeric double ordering.

SQL date comparisons/order/grouping/distinct and ISO literals use those temporal
semantics. ISO literals accept YYYY-MM-DD or YYYY-MM-DD HH:MM:SS[.fraction] (at most
nine fractional digits); ambiguous locale dates, offsets and timezone spelling fail.
CLI `--param Start=date:1986-06-01` binds DATETIME declarations. Date output is a
hash-delimited ISO calendar with nine fractional digits, explicitly rounded to the
nearest nanosecond (ties to even); it is not a lossless serialization of double bits.
The library retains exact bits; calendar-to-double conversion may round or reject
an unrepresentable endpoint. Implicit text/number/date coercion and date arithmetic
remain unsupported.

The independent Jackcess 5.0.4 comparison now covers 1,606 rows across the previous
seven tables plus CUM_VAL_TB and DECUM_VAL_TB (59 each). Calendar fields use Jackcess's
explicit LOCAL_DATE_TIME mode without a host timezone. These fixture dates have
whole-day values; synthetic tests cover negative fractional days, equivalent
representations, range endpoints and invalid date values. The tagged comparison
SHA-256 is `f7dcb2c24935ab24b8cf02a62a727be3e4e57e302d9bf2a30ba22a07cf4259ea`.
No Microsoft Access date-query comparison has been run.

Primary semantics: [Microsoft Automation DATE](https://learn.microsoft.com/en-us/dotnet/api/system.datetime.tooadate)
and the pinned ColumnImpl source above; calendar APIs use
[Jiff civil dates](https://docs.rs/jiff/0.2.38/jiff/civil/index.html). These are distinct from native
Access comparison evidence.

The [exact date-bit probes](testing/compare_date_bits.py) independently match 28
ordinary/cached cells per V2016/V2019 family, including signed zero, subnormal
serials, negative aliases and dates below millisecond precision. The read-only
oracle serializes the reviewed artifact's retained DATE carrier to an in-memory
buffer; it does not call `toDateDouble`, whose calendar conversion can change
valid bits. No database write or expression evaluation occurs. [Fixture records](FIXTURES.md#original-exact-date-precision-inputs)
retain original hashes and separate native Access qualification.
Complete [legacy DateTime indexes](NATIVE_INDEXES.md#complete-legacy-datetime-indexes)
retain those serial bits separately from SQL civil-time comparison/sorting.

## Bounded Long Text

Native MEMO (0x0c) is now a Text source: inline, single-overflow-row and chained
LVAL records are supported. Page type/LVAL signatures, row bounds/flags, exact
lengths, chain termination and duplicate references are validated. Values are
bounded to 65,536 encoded bytes and 65,536 decoded UTF-8 bytes, with at most 128
chunks; shared page/byte/step budgets and cancellation still apply. Extra payload,
truncation, deleted/relocated LVAL rows, unknown kinds and malformed chains fail
explicitly. An empty native definition is NULL; a zero-length inline definition
is empty text. No fallback fabricates truncated strings.

Long Text keeps its markup and callback/code-looking content as inert strings.
Reading USysRibbons does not parse/activate its XML or call VBA. Every scalar field
now charges a step, including NULLs, rather than charging only per row/page.

Three focused tests include all storage forms, a split compression marker across
chunks, NULL/empty distinction, cyclic/truncated/foreign/oversized/invalid input
and a 256-case fixed-seed definition mutation property. Candidate query tests cover
14 CMD_LINE_TB records and two USysRibbons records. The independent comparison now
covers 1,622 rows / eleven tables: prior nine plus CMD_LINE_TB and USysRibbons.
Text and other non-date values agree exactly; the 118 date cells also agree at
microsecond comparison precision (these are whole-day fixture values). The
normalized output with date cells elided has SHA-256
`55ba66099f881eef8be14d4cc0b43c331e9dd405ab6263af574a60e6d349a384`.
Native Access execution and application/XML behavior remain unqualified.

Format reference: pinned [Jackcess LongValueColumnImpl](https://github.com/jahlborn/jackcess/blob/be0a2139b087a227d14cafd4ea231306a5a3a862/src/main/java/com/healthmarketscience/jackcess/impl/LongValueColumnImpl.java),
reviewed under the research-only Apache-2.0 boundary. The bounded Rust codec and
synthetic fixtures are original; no upstream implementation was copied.

## GUID and binary output

Native GUID (0x0f / 16 bytes) now maps mixed-endian native integer groups to the
core canonical byte carrier through `uuid`. Nil GUID and NULL are distinct; all
128-bit patterns are retained. A fixed independent Rust golden fixture verifies
native/canonical byte order. A temporary original Jackcess V2007 creation recipe
also produced a three-row table with a known GUID, nil GUID and NULL plus binary
00ff/empty/NULL; native CLI results matched the expected canonical values. This
is independent reader/writer evidence, not a Microsoft Access oracle. Its temporary
database is not a redistributed fixture or CI dependency.

GUID ISO-style literals use Access's `{GUID {...}}` syntax. `guid:` parameters use
36-byte hyphenated spelling; parsing/output use `uuid` without RNG/generation.
GUID equality/inequality and DISTINCT use canonical bytes. Ordering operators
remain unsupported when evaluated; no native GUID index ordering is claimed.
No text/numeric coercion is introduced. CLI prints GUIDs as lowercase braced
hyphenated values and Binary as unquoted `0x` hexadecimal (empty Binary is `0x`).
Text remains quoted and NULL stays NULL. This is human-readable output rather
than a production lossless exchange protocol.

The original [GUID fixture recipe](testing/GuidFixture.java) creates only a new
explicitly named temporary database and refuses an existing output path. Its JAR
setup is the same optional engineering setup as the other helpers. No generated
database bytes were added to the repository.

## Exact native Decimal

Native NUMERIC (0x10) supports variable fields with a 17-byte storage declaration,
precision 1–28 and scale 0–precision. `NativeColumn::decimal_parameters()` exposes
the original precision/scale even for unsupported numeric layouts. Metadata bytes
11/12 are interpreted only for NUMERIC. Fixed layouts, calculated fields and the
candidate's nine-byte system-column declaration remain outside this profile.

Rows contain one sign byte and four little-endian 32-bit words in most-significant
word order. The decoder accepts canonical sign bytes 0/0x80, checks the coefficient
against the declared precision, and constructs the existing exact Decimal carrier.
No float conversion or rounding occurs. NULL stays distinct from zero; retained
scale controls CLI output. Negative zero maps to semantic zero with retained scale;
this reader does not provide byte-preserving native reserialization.

The pinned [Jackcess ColumnImpl reference](https://github.com/jahlborn/jackcess/blob/be0a2139b087a227d14cafd4ea231306a5a3a862/src/main/java/com/healthmarketscience/jackcess/impl/ColumnImpl.java)
and JetFormat offsets document the encoding. Original golden bytes test word order;
256 generated malformed inputs exercise width, sign, precision and scale limits.
Native row tests verify metadata, NULL, invalid layouts and excessive coefficients.
No dependency was added: this slice decodes a fixed integer representation into an
existing validated carrier; it introduces no decimal arithmetic implementation.

The original [Decimal fixture recipe](testing/DecimalFixture.java) creates only a
fresh disposable ACCDB through the independently reviewed Jackcess 5.0.4 JAR.
Run it like GuidFixture, with a new output path. Four rows/two Decimal columns cover
28-digit extremes, values beyond double precision, scales 4/28, trailing zeros,
negative numbers, zero and NULL. Independent Rust/Java tagged coefficient/scale
output matched exactly (SHA-256
`8e340fd9fd40ae57a2b3ad2dc5020f0515a3d15a0b728fc54e05c05df84658ae`).
CLI projection, integer comparison and ordering also succeeded. Decimal arithmetic
and implicit Decimal/float mixing remain unsupported; Microsoft Access comparison
has not been run. No generated database is tracked.

## Opaque long Binary / OLE

Native OLE (0x0b, variable field with zero declared byte length) is exposed as
opaque Binary. It shares MEMO's validated inline/single/chained long-value codec,
bounded to 65,536 bytes and 128 chunks plus caller budgets. Text decoding remains
in its own wrapper with its separate 65,536 decoded UTF-8 byte ceiling. No bytes
are interpreted as COM objects, documents, code or providers. This is stored-byte
access, not OLE activation or embedded-object authoring. Empty definitions map to
NULL; zero-length inline definitions map to empty Binary.

The pinned LongValueColumnImpl reference above uses the same storage for MEMO/OLE.
The original [Binary fixture recipe](testing/BinaryFixture.java) creates a fresh
Jackcess 5.0.4 database with four rows: invalid-Unicode raw bytes, a 12,000-byte
chained payload, empty bytes and NULL. Independent Rust/Java tagged output matched
exactly (SHA-256 `8b0795a3895a8dd998a17121cbf6c173f864ff78c0df3b9baacc64d18f60a57a`).
Synthetic native rows and CLI tests cover carrier integration and inert access.

All 146 MSysObjects rows can now be queried through the scalar source. Independent
comparison matched every non-date cell exactly, including opaque values. Its 292
fractional date cells differ by at most 313 ns: Rust displays the retained DATE
rounded to nanoseconds, while Jackcess rounds calendar conversion differently.
This is not bit-exact date agreement or native Access evidence. Date-elided tagged
output hash is `af71d043fa91602f35f9a33f1aeaad1d8b1f61b0b9f961d548b1fe2153d51263`.
The registered fixture remains unchanged; generated files stay disposable.

## Mixed-scalar Jet 4 and ACE comparison

The original [Scalar fixture recipe](testing/ScalarFixture.java) creates a new
V2000 (Jet 4 MDB) or V2007 (ACE-code-2 ACCDB) file using the reviewed Jackcess
5.0.4 JAR. It refuses existing output paths and accepts only these two formats.
Each has three rows / fourteen columns spanning every currently qualified carrier:
Boolean, Byte, Int16, Int32, Currency, Single, Double, Text, MEMO, short Binary,
opaque OLE, GUID, Decimal and legacy Date/Time. Inputs include signed-zero floats,
integer/Currency extremes, mixed Unicode/supplementary characters, a 3,000-character
MEMO, a 12,000-byte OLE chain, 28-digit Decimal, empty values, NULLs, nil GUID and
the negative Automation date 1899-12-29 06:00:00.

On 18 October 2026 an external Rust consumer and TableDataOracle matched all tagged
cells exactly, including IEEE bits, canonical GUID bytes, Decimal coefficient/scale,
raw Binary, Unicode and nanosecond calendar output. Both formats also produced the
same normalized output, SHA-256
`588a43a018a37316bfb2df9c8dff1feaa6b53f0c18d55543125f526a9d439d0a`.
CLI Jet 4 projection/order returned the expected rows and negative date. These
particular dates are exactly representable; fractional candidate dates retain the
rounding caveat above. This is independent-writer/reader evidence, not actual
Microsoft Access/provider comparison, native authoring or historical evolution.

Reproduce file generation and the independent tagged scan with reviewed tools:

```sh
java -cp path/to/jackcess-5.0.4.jar docs/testing/ScalarFixture.java path/to/new.mdb V2000
java -cp path/to/jackcess-5.0.4.jar docs/testing/ScalarFixture.java path/to/new.accdb V2007
java -cp path/to/jackcess-5.0.4.jar docs/testing/TableDataOracle.java path/to/new.mdb ScalarSamples
cargo run --locked -p tablecraft-cli -- query run --database path/to/new.mdb "SELECT ID, Amount, Stamp FROM ScalarSamples ORDER BY ID"
```

Generated files/JARs are not tracked or required by CI. Registered native fixture
bytes remain unchanged. Newer ACE codes, encoded pages, complex/calculated values,
unqualified historical row evolution, writes and native shared-file
coordination remain outside the qualified reader profile.

## Dense appended-column histories

Older rows with fewer columns now decode under a narrow dense append-only profile.
Current logical column numbers and variable indexes must be dense and agree with
the definition counts. The decoder derives fixed-data/variable-tail geometry from
the stored row count, rather than requiring payload space for newly appended
columns. Bits past that row count are ignored: absent non-Boolean fields read as
NULL and absent Boolean fields read as FALSE. This follows the reviewed Jackcess
5.0.4 NullMask behavior, confirmed by an independent generated example. It does
not evaluate or materialize current DefaultValue properties on old rows.

A fixed-only old row has no newly added variable tail. Present variable offsets,
lengths and masks retain all existing checks. Zero/future counts, gaps and
unqualified variable layouts fail explicitly. Type changes, deleted histories,
virtual property evaluation and schema mutation are not implemented. This is
read-only handling of an existing file, not ALTER TABLE support.

The original [evolution recipe](testing/EvolutionFixture.java) creates a fresh
ACE file, writes an ID-only row, appends Text/writes a second row, then appends
Int32, Boolean and MEMO/writes a third row. Int32/Boolean have defaults 999/True,
but older rows still expose NULL/FALSE. Independent tagged Rust/Java output matched
exactly, SHA-256 `92f8a7cc35cdc9e3baa31dd29d2590d52638cf9b6a1ab9f49ad30dad68f1d53b`.
Synthetic tests exercise missing integer/Boolean fields, unused high mask bits,
old fixed-only geometry, preserved variable values, gaps and invented payloads.
No native Access or deleted-column compatibility claim follows from this evidence.

[ACE 16-family evidence](NATIVE_ACE16.md) now qualifies the bounded existing read
algorithms for unencoded code-5/code-6 files. Feature/type restrictions remain.

## ACE 16 modern scalars

Codes 5/6 support fixed eight-byte Large Number (0x13) as Int64. Code 6 supports
fixed 42-byte Date/Time Extended (0x14), preserving seven fractional digits and
years 1–9999. Width/profile/calculated flags are checked before reading rows;
malformed encoded dates fail without returning partial snapshots. Complete native
single-Int64 index reads/checks now work on codes 5/6; typed Int32 range selections
still require Int32 keys. See [ACE 16 evidence](NATIVE_ACE16.md#modern-scalar-reads).

[Advanced field feature inspection](features/ACCESS_ADVANCED_FIELDS.md#working-inert-metadata-slice)
retains raw ACE complex type IDs and joins explicit typed properties without
using those declarations to enable a scalar codec. Calculated/complex snapshots
remain rejected; presentation metadata does not establish recalculation/UI semantics.

[Raw complex-child storage](features/ACCESS_ADVANCED_FIELDS.md#raw-complex-child-storage-reads)
now admits exact fixed Int32 complex-FK fields on 2016/2019 profiles. Resolved
flat child tables can feed ordinary scalar snapshots; parent complex collections
remain unsupported. Attachment FileData stays opaque encoded Binary, never
silently presented as an extracted original file.


## Bounded physical row pages

`read_table_page` accepts a validated `TablePageRequest` and returns `TablePage`,
separate from the complete `TableSnapshot` contract. It qualifies every scalar
column, enumerates live roots up to the requested range and decodes only selected
records. Deleted roots do not count toward offsets; selected relocation targets
and long values use the existing bounded readers. Larger declared totals do not
trigger the complete-snapshot row ceiling, but file size, mapped pages, physical
reads, steps and caller budgets remain bounded.

Skipped roots consume traversal work without allocating their values or spending
the selected-row allowance. Their slot geometry is checked, but their payloads,
relocation targets and long values are not interpreted. Unvisited pages are not
validated. `reached_end()` means all allocated root slots were enumerated and the
declared count matched; a full returned page may stop before that point. These
pages are extraction results, not full-table audits or transaction snapshots.

The reusable Access `NativeBrowser` owns table inventory and page orchestration;
CLI formatting stays at the application boundary. [Browsing contracts](CLI_BROWSING.md)
record pagination, limits and independent comparison scope. Query sources retain
their existing complete-snapshot behavior; this API does not imply TOP/ORDER BY
pushdown or a stable bookmark across changed files.

## Explicit cached-calculation pages

`read_stored_table_page`/`StoredTablePage` provide a distinct read-only profile
for qualified ordinary carriers mixed with stored ACE 16 calculation results.
They reuse the scalar page walker and the existing single-calculation declaration/
wrapper gates. Metadata is retained through `features()` and each column has an
explicit cached-origin accessor. No formula is evaluated or freshness promised;
ordinary scalar/query/write eligibility is unchanged. See the
[browsing contract](CLI_BROWSING.md#explicit-stored-calculations) for carrier,
paging, resource and CLI limits, and the
[advanced-field contract](features/ACCESS_ADVANCED_FIELDS.md#explicit-stored-scalar-calculations)
for exact wrappers and NULL/empty distinctions.

## Explicit multivalue pages

`read_multivalue_table_page` returns a distinct `MultivalueTablePage` whose cells
are explicitly scalar or stored collections; native complex types stay in the
original schema. No collection enters core scalar/query/edit contracts. The
existing typed collection reader qualifies templates, keys/identities and complete
flat-child references before page publication. Ordinary/cached payloads use the
same partial page walker; cached results require their separate opt-in. Optional
feature metadata and each observed parent layout/key are checked before joining
snapshots. See [browsing contracts](CLI_BROWSING.md#explicit-multivalue-cells) for
complete-parent/child limits, absent versus empty states and CLI formatting.

## ACE 2007 complex key and multivalue qualification

The existing ACE 2007 profile now qualifies fixed four-byte complex parent keys,
fixed Int32 child foreign keys carrying exactly the complex-FK extension, and
stored multivalue collections through the same raw-link/value/page readers.
Its independently observed Decimal conceptual hint remains distinct from actual
17-byte child storage. Other sizes/types/flags/hints remain unsupported; no
nine-byte Decimal record decoder or newer Large Number carrier is enabled.
[Evidence and limits](FIXTURES.md#ace-2007-complex-key-and-multivalue-inputs)
include complete reference/capacity checks and immutable original fixtures.
Cached calculations, typed attachment/history reads, query expansion and writes
retain their previous generation/semantic gates.
