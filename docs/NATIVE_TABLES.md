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
Double, short Binary and short Text. Decimal, GUID, Date/Time, Long Text, OLE,
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
