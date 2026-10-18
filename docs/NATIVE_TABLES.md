# Native scalar tables

The ACE library's `inspect_table_schema` and CLI `inspect <copy.accdb> --table NAME`
inspect one explicitly named local table using the bounded, unencoded Jet 4 /
ACE-code-2 profile in [NATIVE_READING.md](NATIVE_READING.md). Linked tables and
saved queries are rejected; no external connection or active content is invoked.

`TableSchema` reports the catalog-scoped identity, definition page, declared row
count and columns in native logical column-number order. `NativeColumn` preserves
native type/flags/extended flags, fixed/variable storage and byte length (Boolean
uses zero payload bytes). Unknown/unsupported types remain visible in inspection.
`scalar_columns` maps supported columns into existing `tablecraft-schema::Column`
contracts and fails if any column is unsupported. No new generic schema package
or duplicate object model is needed.

The scalar profile covers Boolean, Byte, Int16, Int32, Currency, Single,
Double, GUID, qualified Decimal, legacy Date/Time, bounded Long Text/opaque OLE and short Binary (1–510 declared bytes) and short Text (2–510 declared bytes,
an even UTF-16 size).
complex/attachment, calculated and unqualified extended fields remain unsupported
for scalar reads. Native Text length is in UTF-16 units; it is not converted into
a schema Unicode-scalar limit. Required/default/validation properties are not
decoded, so input columns conservatively permit NULL except Boolean. AutoNumber
flags describe stored metadata and never generate values during inspection.

The original format reader has shared internal modules for page access, usage
maps, definition metadata and Unicode decoding; catalog-specific validation stays
with the catalog consumer. Limits, cancellation, stable-copy requirements and
unsupported chained table definitions remain as in NATIVE_READING.md.

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

## Bounded scalar snapshots

`read_table` returns a completed `TableSnapshot` containing native metadata,
shared input columns and owned rows. All columns must fit the scalar profile;
unsupported columns fail even if a later query would project only supported ones.
Each call shares the caller's catalog/schema/row byte, row and step budgets.
The snapshot is bounded to 16,384 live records and the native structural ceilings;
failures return no partial snapshot. It has no index scan or filter/TOP pushdown.

The decoder follows owned data pages and relocated live rows, skips deleted roots,
checks the declared live count, row offsets and masks, and handles fixed-only as
well as variable-column records. Rows whose stored column count differs from the
current native maximum are explicitly unsupported; historical column evolution
is not silently filled with NULL. Boolean bits are values and never NULL. Other
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
`time` for checked Gregorian validation and explicit calendar conversion, replacing
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
[time 0.3.55](https://docs.rs/time/0.3.55/time/). These are distinct from native
Access comparison evidence.

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
