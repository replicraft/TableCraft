# Inert native linked-table definitions

`tablecraft-ace::read_linked_table_definition` reads a selected linked-table
catalog declaration from an explicitly supplied stable native source. It returns
the caller-scoped local object ID, signed native ID, local alias, native linked
storage kind and the separate `Database`, `ForeignName` and `Connect` fields.
This engine API remains inert. The separate [explicit routing service](NATIVE_LINKED_QUERIES.md)
and CLI `query run --link ALIAS PATH` can consume host-authorized targets; ordinary
queries continue to refuse unresolved linked sources.

`NativeLinkText` distinguishes an absent catalog column, stored NULL, decoded
Text and an unsupported declaration. An unsupported optional field remains
visible without decoding its payload. Supported selected Text payloads must
decode completely; malformed payloads fail the operation. No Debug or Serde
implementation implicitly publishes connection strings or paths.

`access_target()` borrows opaque target identifiers only when storage is
non-ODBC linked, Database and ForeignName are nonempty Text and Connect is
exactly NULL or empty Text. Whitespace is not trimmed. Nonempty/unknown provider
strings, ODBC storage and missing or unsupported fields refuse interpretation.
The raw getters remain available. A successful interpretation grants no authority
to open a file, use credentials, resolve a path or execute ForeignName as SQL.

## Read and failure boundaries

The reader inventories the local catalog, resolves one local table alias and
validates its parent and native origin. It reads the MSysObjects schema, then
rereads the selected catalog record and checks Id, ParentId, Flags, Name and Type
against that origin. Duplicate identities, unresolved parents or mismatching
rereads fail. Local tables are outside this API. The source must remain stable;
these checks do not create a concurrent cross-file snapshot.

Only the identity and three target metadata fields are projected. Qualified
optional fields use the observed system variable-Text declaration: Database and
Connect are Memo, ForeignName is ShortText, flags 0x12 and extended flags zero.
No target source, saved query, provider or user-table body is activated. The only
trace span is payload-free. Returning metadata requires complete successful reads.

Catalog scans, schema reads, selected record/Text decoding, allocations and work
consume the caller's existing cumulative native budget. Reusing that budget
across calls does not refill it. Cancellation remains checked through the existing
read/decode paths and selected-field processing. Query, comparison and output
budgets remain distinct from native work. The Access adapter now provides an explicit
[shared native allowance](NATIVE_SOURCE_BUDGETS.md) for already supplied sources.
The separate [routing facade](NATIVE_LINKED_QUERIES.md) uses that aggregate pool
and validates selected target identities and metadata before exposing linked rows.

## Contract sources and independent evidence

Microsoft documents the separate [TableDef Connect property](https://learn.microsoft.com/en-us/office/client-developer/access/desktop-database-reference/tabledef-connect-property-dao)
and [SourceTableName property](https://learn.microsoft.com/en-us/office/client-developer/access/desktop-database-reference/tabledef-sourcetablename-property-dao),
including an Access link example with DATABASE and a separate remote table name.
These public DAO contracts do not establish a physical MSysObjects layout.
That layout is checked independently using the pinned Jackcess public
[createLinkedTable](https://jackcess.sourceforge.io/apidocs/com/healthmarketscience/jackcess/Database.html)
writer and reopened [TableMetaData getters](https://jackcess.sourceforge.io/apidocs/com/healthmarketscience/jackcess/TableMetaData.html).
The getter oracle also compares the physical catalog fields and installs a
throwing target resolver: no target-resolution calls are permitted.

The [original recipe](testing/LinkedTableFixture.java),
[getter oracle](testing/LinkedTableOracle.java),
[replay runner](testing/compare_linked_tables.py) and
[accepted report](testing/results/linked-metadata-2026-10-24.json) retain sixteen
passing comparisons across Jet 4, ACE 2007, ACE 2016 and ACE 2019 files. Each
frontend has four links, covering a real local backend basename, exact Unicode,
opaque SQL-looking/whitespace identifiers and an external Memo-sized target
string. Both frontend and backend originals remain unchanged. The report pins
inputs, recipe/reader sources, lockfile, reference artifact and frozen executable.
Fixture provenance is in [FIXTURES.md](FIXTURES.md#original-inert-linked-table-metadata).

Eight focused engine tests additionally cover missing/unsupported declarations,
ODBC and unknown connection text, malformed selected payloads, identity drift,
invalid parents/duplicates, every observed cancellation checkpoint and cumulative
byte/row/step exhaustion. Strict scoped library/test/example Clippy passes.
These are independent stored-metadata and engineering safety checks. Native
Access link execution, live shared-file coherence, authentication, linked queries,
linked writes and automatic filesystem/provider resolution remain unqualified.
