# Stored ACCDT table data

The Access backend now discovers template object identities and reads a bounded
inline-XSD table-data profile. Names and categories come from related Object
Metadata, not filenames or guessed application bodies. Generic Object parts with
Type Table and one explicit Table Data relationship can supply stored snapshots.
No template is instantiated and no startup content or external provider runs.

```sh
tablecraft inspect example.accdt --catalog
tablecraft inspect example.accdt --table 'Sample Table'
tablecraft tables example.accdt --count
tablecraft rows example.accdt 'Sample Table' --limit 100
tablecraft rows example.accdt --all
tablecraft query run 'SELECT ID FROM [Sample Table] ORDER BY ID' --database example.accdt
```

This is partial template support. Access-produced template qualification, broader
schemas and native application behavior remain required. In particular,
[SharePoint List Definition tables](ACCESS_TEMPLATE_LISTS.md) now admit qualified
stored declaration sequences that agree with inline XSD. Missing declarations,
extra fields, unsupported types and default-dependent schemas remain refused;
a data part alone cannot bypass agreement. Missing data parts remain
unavailable; they are never interpreted as empty tables. Linked objects are never
retrieved. Renaming a native database to `.accdt` still uses its actual header.

## Backend contracts

`TemplatePackage::objects` returns shared SchemaName/ObjectKind metadata plus
source-specific part references and linked/list-definition markers. It follows
recognized Object, ListInstanceDefinition, Link, Query, Form, Report and Macro
relationships. Each selected object requires exactly one related Object Metadata
part. Metadata Type/Name order, supported kinds, duplicate identities, relationship
kind agreement and data-part cardinality are checked. Unknown future type names
remain visible as Unknown with their original spelling, without becoming table or
query bindings. NameMap is inert and ignored
as directed by the specification; application bodies and properties stay opaque.
This is not complete validation of every object's application schema.

`table_columns` reads only the selected inline schema prefix with its namespace
context. It requests no data-instance event. `read_table` explicitly parses the
complete stored table and validates every row before returning a TemplateTable.
Both APIs accept cumulative budgets and cancellation. `TemplateSource` bridges
these contracts to the existing read-only query evaluator: columns are cached per
immutable source, and scans parse complete snapshots under the same host budget.
Repeated scans consume cumulative resources. No selected/partial snapshot is
cached as a full table. Known named query bodies remain unsupported, and conflicting
table/query identities fail rather than choosing one.

Preparation never decodes record values. Thus a valid schema can bind despite a
later malformed record; execution fails before supplying any row from that table.
Visitor rejection stops delivery. Cancellation/allocation/quota failures during
schema discovery are not cached as permanent schema errors; an explicit later
attempt retains the spent cumulative budget. Callbacks retain typed limit/cancellation errors;
reentrant schema callbacks fail without panicking or poisoning metadata. Full
query results and CLI output stay buffered on later failures. No native/index
predicate pushdown, writable session or implicit active-content execution is added.

## Inline schema and value profile

The data part has one root containing inline XSD followed by the data instance.
The supported schema has a no-target-namespace data-root declaration and one row
element, with an unbounded optional row reference and an ordered scalar sequence.
There are at most 256 columns. Explicit field types and inline simple restrictions
are supported; imports/includes, named types, choices, repeated cells, arbitrary
attributes, defaults/fixed values, identity constraints and unsupported facets fail.
Office-data schema annotations are ignored; other unknown schema constructs fail.
Namespace prefixes resolve by URI, including type/base/ref QName values.

| XSD type | Shared materialized carrier |
| --- | --- |
| string | Text, with optional maxLength in Unicode scalar values |
| boolean | Boolean; true/false and 1/0 lexical forms |
| byte / short | Int16, with the declared signed range checked |
| unsignedByte | Byte |
| unsignedShort / int | Int32, with the declared range checked |
| unsignedInt / long | Int64, with the declared range checked |
| float / double | IEEE Single / Double, including exact INF, -INF and NaN spellings |
| decimal | Checked 96-bit Decimal with retained scale up to 28 |
| dateTime | Unzoned legacy DATE under the bounded spelling below |
| base64Binary / hexBinary | Binary, with optional decoded-byte maxLength |

These are inline XSD types. Office-data annotations do not silently turn decimal
into native Currency, string into GUID, or a field into an executable calculation.
Native Access field-type/semantic fidelity and object/list-schema agreement remain
separate qualification work. Integer transport rejects fractional/exponent input;
unsigned negative zero denotes zero. Decimal parsing uses existing rust_decimal
`from_str_exact`, without rounding; totalDigits/fractionDigits constrain the
normalized value rather than forcing a retained scale. Values outside the carrier
or lexical size profile fail explicitly. Unsupported types are not exposed as NULL.

String whitespace and XML character references are retained after XML decoding;
CDATA and predefined entities work. General entities, DTDs and external resources
are refused. Numeric/binary syntax uses XSD whitespace rules, not machine locale
or SQL NumberFormat. Numeric spellings are capped at 128 bytes. Finite float
spellings use the standard IEEE parser, including its rounding/range behavior.

DateTime currently requires four-digit-year YYYY-MM-DDThh:mm:ss, optionally with
one to nine fractional digits, no offset/timezone and the legacy DATE range.
Jiff validates civil components; the existing legacy representation can round
fractional precision. No timezone or host clock is inferred or discarded. Wider
XSD datetime years/precision/zones and native extended-date typing remain open.
The optional declared data-root generated timestamp is syntax checked and inert.

Missing minOccurs=0 cells become NULL; required cells cannot disappear. Explicit
xsi:nil=true requires nillable=true and empty content. Empty Text/Binary remain
empty values. Cell order, unknown/duplicate cells, invalid values and supported
facet failures are rejected. XML `_xHHHH_` UTF-16 names decode once, including
paired surrogates and escaped underscores; invalid surrogates fail. The bounded
NCName profile remains narrower than all Unicode XML names. Decoded field names
must remain distinct under the shared ASCII-insensitive name policy.

## Limits, CLI and evidence

[Package limits](ACCESS_TEMPLATES.md) still apply. Metadata/data trees are UTF-8,
at most 8 MiB, 64 levels and 100,000 nodes, with 64 attributes per element. XML
bytes, estimated parser/tree work, copied namespace identities, schemas, row cells
and decoded payload storage share caller quotas. Table reads have a 100,000-row
hard ceiling; host row/byte/step allowances can be lower. The CLI retains its
16 MiB browsing allowance and per-command row/work limits. Native query byte
controls also apply to template query input. No cancellation/limit failure silently
retries, switches sources or publishes a partial result.

`tables --count` counts fully validated stored rows; ACCDT has no native declared
row-count field in this path. Linked/missing stored data show N/A. Template Kind
is reported as Template, with no invented native system-table flag; --show-system
does not infer a flag from names. `rows` validates the full stored table before
slicing the displayed page, unlike native partial-payload paging. `--all` retains
named sections and explicit unopened statuses for linked/missing data. Collection/
cached-calculation controls, native indexes, saved template query execution and
`--collation database` are refused; callers can select an existing explicit text
policy. Stored template locale properties do not activate an unqualified comparer.

Nine feature tests cover exact values/types/names, schema-only binding despite
bad records, reusable joins/filters/aggregates, nullable/order/facet failures,
linked/missing/List Definition refusals, reentrancy, quotas and every observed
cancellation checkpoint. The original [Python XML/ZIP runner](testing/compare_template_tables.py)
passes 48 typed-template CLI/query controls, including 15 scalar domains. Existing
package controls are updated for catalog inspection. All fixtures are original
inert data generated in memory/temporary directories; bytes remain unchanged.
These are independent engineering controls, not native Access qualification.

References: Microsoft's [Object Metadata](https://learn.microsoft.com/en-us/openspecs/sharepoint_protocols/ms-accdt/7e388f86-af30-4720-908e-f2745934278a),
[List Data](https://learn.microsoft.com/en-us/openspecs/sharepoint_protocols/ms-accdt/606146d3-514f-4555-9d7e-039be4b3f40c)
and [MS-ACCDT specification, sections 2.3–2.5](https://officeprotocoldoc.z19.web.core.windows.net/files/MS-ACCDT/%5BMS-ACCDT%5D-181001.pdf),
plus [W3C XSD datatypes](https://www.w3.org/TR/xmlschema-2/). The implementation and
fixtures are original; no specification code/schema/example is copied.


Qualified [List Definition agreement](ACCESS_TEMPLATE_LISTS.md) now runs before
columns or rows are returned for that part type. Request metadata stays within the
Access backend; the shared evaluator and CLI do not acquire a SOAP client or an
application runtime. Earlier blanket List Definition refusals remain historical
package-stage evidence, not the current qualified profile.
