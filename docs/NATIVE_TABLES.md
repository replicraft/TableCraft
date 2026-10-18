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

The initial carrier profile covers Boolean, Byte, Int16, Int32, Currency, Single,
Double, short Binary (1–510 declared bytes) and short Text (2–510 declared bytes,
an even UTF-16 size). Decimal, GUID, Long Text, OLE,
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
