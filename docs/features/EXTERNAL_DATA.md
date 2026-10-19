# External Data and provider ownership

Status: accepted direction, 19 October 2026. The package relocation is implemented;
external connectors, file interchange, saved task execution and ribbon UI are not.
Existing bounded Access/SQLite reads and narrow Jet 4 edits retain their documented
limits. This work does not enable ACE writes or change the ACE 16 priority.

## Ownership

| Domain | Responsibility |
| --- | --- |
| `foundation/tablecraft-db` | Existing shared snapshot/edit contracts; future connection/cursor/capability contracts with real consumers |
| `engines/tablecraft-ace` | Independently reusable native storage, indexes, transactions and recovery |
| `backends/tablecraft-access` | Built-in integration over our native engine |
| `backends/tablecraft-sqlite` | Built-in integration over SQLite; SQLite remains a third-party engine |
| `adapters/` | Future external provider integrations, including locally installed Microsoft ACE |
| `formats/tablecraft-project` | Future native document format; not a connector |

Hosts select concrete providers. DB and reusable query code do not import them.
Shared headless application/lifecycle services own connection profiles, linked-table
management, saved tasks, validation and conversion. Ribbon, CLI, Python and MCP
entry points invoke those services under the same permissions and write gates.
Do not create a catch-all connector crate or empty packages to mirror this table.

## Operations and UI direction

The planned External Data ribbon is a first-class tab with Import & Link, Manage
and Export groups. Commands cover Excel, CSV/text, Access, SQL Server, ODBC and
SharePoint, linked tables, saved imports/exports, data tasks, document merge and
report/PDF export. Command search uses the shared command registry and help;
`tablecraft-builders` owns visual workflows and `tablecraft-ui` presentation.

| Operation | Meaning |
| --- | --- |
| Import | Copy into a new local table |
| Append | Add records to an existing table after validation |
| Link | Refer to live external records; writable only when explicitly supported |
| Export | Produce a destination artifact or authorized destination writes |
| Refresh/relink | Inspect schema/status or change a link target with review |
| Remove link | Remove the local definition without deleting source records |

A `.tcraft` frontend may eventually link to an existing Access backend without
migration. Link management must cover changed paths, missing sources, schema drift,
offline status and credentials without opening embedded links during inspection.
Application modernization from ACCDB to `.tcraft` remains a separate conversion
workflow with preservation/loss reports, rather than a table-only import.
[Saved tasks](IMPORT_EXPORT_TASKS.md) distinguish Access specifications from richer
TableCraft tasks. Upsert and reusable relink operations are separately labeled
enhancements; neither is implied by native saved-import compatibility.

## File interchange and default applications

Use direct file handling: calamine for spreadsheet reading, rust_xlsxwriter for new
XLSX exports and csv for delimited interchange when those consumers are implemented.
Do not execute workbook macros or assume cached formula values are recalculated.
Qualify dates, types, range selection, NULL/empty strings, formula injection,
encoding and bounded decompression. Live workbook links need their own read-only
profile and refresh/concurrency evidence; a one-time importer does not supply it.

Generate document/report output, then offer an explicit open action using the
system's registered application. No installed Excel, Word or ArtCraft application
is required to create the file. Native template mail merge requires field mapping,
escaping, output limits and fidelity tests; launching a word processor alone does
not implement merge. DOCX/RTF support is prospective, without a selected new crate.
PDF/report pagination remains with reports/render, not database adapters.

## Connection implementation order

These are priorities within future connectivity work, after the current native
milestones; they do not restart SQLite expansion or replace roadmap steps 1–9.

| Priority | Prospective adapter | Direction |
| --- | --- | --- |
| First | `tablecraft-mssql` | Native TDS through Tiberius; qualify against real Microsoft SQL Server first |
| Next | `tablecraft-odbc` | odbc-api with installed Driver Manager and vendor drivers |
| Next | `tablecraft-sharepoint` | Microsoft Graph for live SharePoint/Microsoft Lists |
| Later | `tablecraft-postgres` | tokio-postgres native protocol |
| Later | `tablecraft-mysql` | mysql_async; qualify MySQL and MariaDB independently |
| Optional | `tablecraft-access-msace` | Installed Windows provider; existing VBA/COM deferrals remain in force |

Irongate may reuse TDS only after authentication, TLS, types, metadata, parameters
and transaction tests establish actual compatibility. A common wire protocol is
not a compatibility guarantee. Native Rust clients avoid ODBC deployment, but
their selected TLS/authentication features can still add platform dependencies.

ODBC is an API, not a universal network protocol. Support DSNs, DSN-less profiles,
driver discovery, prepared parameters, bounded retrieval, schema/key discovery,
capability negotiation and explicit read/write modes. Use system ODBC on Windows
and a qualified unixODBC installation on Linux/macOS. Explain missing managers,
drivers and process/driver bitness mismatches; provide official installation
guidance without silently installing drivers. rs-odbc also needs a Driver Manager
and is not selected as a replacement. Writing a new manager is out of scope.

Keep blocking driver calls on bounded workers, with host-owned environment
lifecycle and documented timeout/cancellation limits. Future pooling needs a
separate lifecycle/thread-affinity review. An architecture-specific helper process
is optional later isolation, not an implemented capability. Browsers require a
trusted server or local companion for native ODBC; they cannot load drivers.

## SharePoint scope

Connect to genuine lists through OAuth/Entra sign-in, site/list/column discovery,
bounded paginated reads and supported item CRUD. Advertise filtering/sorting only
where supported; Graph's indexed-field/filter restrictions prevent general SQL
pushdown. Use ETag conditional writes and surface conflicts, not blind overwrites.
Respect service permissions and throttling; bound Retry-After handling by host
deadlines/cancellation. Never automatically replay an uncertain mutation.

Attachments, lookup/person/managed metadata types and delta refresh require later
profiles. Targeted SharePoint REST may complement Graph where needed; do not claim
Graph covers every attachment or permission operation. Document libraries are
separate later scope. Hosted Lists replacements, sites/pages, server-side SharePoint
migration platforms, Power Apps and Power Automate emulation are out of scope.
CSV remains a snapshot fallback with explicit losses: attachments, relationships,
permissions, workflows and version history are not preserved by ordinary CSV.

## Contracts, security and acceptance

Negotiate read/write, paging, stable keys, concurrency tokens, transactions, schema
changes and predicate/ordering capabilities per source. Do not promise remote SQL,
cross-source joins, cross-backend atomicity or multirow SharePoint transactions.
Avoid fetching an entire remote table just to show its first page. TableCraft's
eager-error semantics must still hold when deciding safe predicate pushdown.

Project connection definitions contain non-secret configuration and secret
references. Tokens, passwords and connection-string secrets stay in a qualified OS
store or authorized secret provider, outside shareable documents and diagnostics.
Server authorization remains authoritative. UI permissions cannot protect a file
against another application opening it directly; legacy MDB user-level security
must not be attributed to modern ACCDB. No content inspection signs in, connects
to providers, schedules tasks or opens exported files implicitly.

Acceptance needs disposable real-server/driver/tenant tests for authentication/TLS,
exact carriers, keys, NULLs, pagination, restricted pushdown, conflicts, schema drift,
timeouts, cancellation, uncertain outcomes and permission denial. Qualify platform
and driver bitness separately. File tests need independent readers and preservation/
loss reports. Saved task tests include repeated append, partial completion and
explicit resume/idempotency. No such external qualification has run yet.

## References

- [Microsoft import/link/export concepts](https://support.microsoft.com/en-us/access/introduction-to-importing-linking-and-exporting-data-in-access)
- [Microsoft user-level security scope](https://support.microsoft.com/en-us/access/what-happened-to-user-level-security)
- [ODBC environment, driver and DSN discovery](https://docs.rs/odbc-api/latest/odbc_api/struct.Environment.html)
- [Tiberius](https://docs.rs/tiberius/latest/tiberius/), [tokio-postgres](https://docs.rs/tokio-postgres/latest/tokio_postgres/), [mysql_async](https://docs.rs/mysql_async/latest/mysql_async/)
- [calamine](https://docs.rs/calamine/latest/calamine/) and [rust_xlsxwriter](https://docs.rs/rust_xlsxwriter/latest/rust_xlsxwriter/)
- [Graph list reads and filtering](https://learn.microsoft.com/en-us/graph/api/listitem-list?view=graph-rest-1.0), [conditional item updates](https://learn.microsoft.com/en-us/graph/api/listitem-update?view=graph-rest-1.0), [throttling](https://learn.microsoft.com/en-us/graph/throttling)

The supplied twelve-feature article is a product checklist, not normative storage
or security evidence. Its features map to the following existing or future owners;
this is coverage planning, not a declaration of implemented compatibility.

| Feature | Owner / remaining scope |
| --- | --- |
| Data organization | Schema, DB, native/SQLite backends; visual table/relationship/index editing remains future work |
| Import/export | Shared tasks and lifecycle services; format handlers and external adapters |
| Queries | SQL, expressions and query; current bounded profiles do not establish parity |
| Macros | Macros/automation; required table data macros remain write blockers |
| Reports | Reports/render and builders; pagination/printing remain future work |
| Modules | Preserve native VBA; execution deferred; future native-project Python |
| Controls | Forms/model/builders; full property and behavior compatibility catalog required |
| Tell Me | UI command search over shared command registry/help |
| Attachments | ACE complex fields and future project/migration services; extraction is not full edit/migration support |
| Forms | Forms/runtime/builders; binding, validation and event behavior remain future work |
| Validation/integrity | Schema/expressions/DB and backend-owned enforcement; current narrow checks remain explicit |
| Security/access control | Security/platform plus actual backend/service enforcement; qualification per backend |

Compact/repair and verified backup remain distinct native maintenance and lifecycle
operations, with separate concurrency and recovery requirements.
