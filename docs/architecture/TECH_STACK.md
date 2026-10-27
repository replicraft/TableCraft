# Technology choices

This document distinguishes implemented dependencies from selected directions.
It describes current decisions, not completed database or application features.
Exact adopted versions and reviews are recorded in
[DEPENDENCIES.md](../development/DEPENDENCIES.md).
Domain groups, current packages and progressive extraction rules are recorded in
[ARCHITECTURE.md](ARCHITECTURE.md).

The [current product decision](../decisions/0003_ACCESS_BEHAVIORAL_PARITY.md)
prioritizes faithful Access behavior. SQLite, native projects and project Python
are paused. Rendering/toolkit choices must preserve Access controls and workflows;
application policies belong in shared services as well as the future UI.

## Implemented foundation

| Component | Current implementation |
| --- | --- |
| Language and build | Rust 1.99.0, edition 2024, Cargo workspace |
| CLI | clap Parser/Subcommand derives, help/version, syntax-only query parse and bounded source-free query eval and inline query run |
| Native inspection | tablecraft-ace bounded native metadata/scalar reads; CLI inspect/query run; see NATIVE_READING.md |
| Object inventory | tablecraft-model shared scoped descriptors; native codes remain in ACE; see MODEL.md |
| Shared vocabulary | tablecraft-core values, scoped identities and resource budgets |
| Query syntax | tablecraft-sql bounded data-query parser and owned statement model |
| Retrieval | tablecraft-query bound read-only plans over explicit sources, including tablecraft-access native scalar snapshots; see QUERY_EXECUTION.md |
| Scalar schema | tablecraft-schema exact input carriers, column identity and nullability |
| Scalar semantics | tablecraft-expressions: explicit Boolean/NULL, numeric and text profile; see EXPRESSIONS.md |
| Civil calendars | Jiff under exact TableCraft date carriers; bounded ISO input, explicit year/precision rules and no implicit timezone |
| Native byte processing | zerocopy borrowed layouts; wide/bytemuck ASCII classifiers; bounded word bitmap traversal; see testing/NATIVE_BENCHMARKS.md |
| Library errors | thiserror |
| Serialization | Optional serde for scalar value exchange; constructor validation on decode |
| Generated tests | proptest; serde_json is development-only for typed round-trip tests |

Other requested library choices are recorded in
[LIBRARY_POLICY.md](../development/LIBRARY_POLICY.md). Add them with the functionality that
uses them rather than as unused dependencies.
The [complete dependency/reference review](../development/DEPENDENCY_REVIEW.md) records every
candidate from the library/Wine/RustVBA proposals, with separate keep, prototype,
study, defer and reject decisions. Existing adopted versions remain unchanged.

## Desktop typography decision

Selawik is the selected font for the TableCraft UI. This is a frontend design
decision; no font assets are bundled and no UI font loading is implemented yet.
Review asset provenance and licensing, package the required fonts, and verify
rendering, accessibility and fallback coverage when implementing the desktop UI.

## Standalone native engine direction

The native Jet/ACE implementation belongs to reusable `tablecraft-ace`
under `engines/`; working `tablecraft-access` is TableCraft's scalar Source adapter
over it. Both now exist with bounded metadata/scalar reading. Reusable query
components must depend on
backend-neutral contracts rather than importing the Access adapter or application
runtime. The engine must work for unrelated Rust applications without Microsoft's
provider. Rust is the first interface; C ABI and ODBC are later, unimplemented
interfaces. [NATIVE_ENGINE.md](NATIVE_ENGINE.md) defines scope, dependency rules,
publication requirements and compatibility evidence; only the bounded APIs
documented above are implemented.

## VBA preservation and macro direction

[VBA execution and integration are deferred](VBA_COMPATIBILITY.md), including
authoring, editor/debugger, VBA-specific DAO/ADO and Windows/COM/Wine providers.
Preserving existing code and inertly detecting dependent features remain required
for qualified native editing. Access table data macros remain in database scope;
required semantics must execute or the affected write must fail. The future macro
and automation owners remain independent of VBA. Reserve `tablecraft-vba-host` in
docs only, pending a usable independent RustVBA release. No interpreter/bridge
dependency or empty package is adopted. [Windows/Wine research](WINDOWS_COMPATIBILITY.md)
is deferred and does not prevent target-specific OS file I/O integration.

## SQL parser integration

The first query slice uses [Apache DataFusion SQLParser](https://github.com/apache/datafusion-sqlparser-rs)
(Cargo package `sqlparser` 0.63.0), with std enabled and default features disabled.
The adapter reuses its lexer and parser primitives; Access precedence and statement grammar are owned by TableCraft. Its Apache-2.0 license and
syntax-only scope were checked in the upstream documentation on 17 October 2026;
see [the source references](REFERENCES.md).

`crates/query/tablecraft-sql` contains lexical AccessDialect rules, an explicit outer
query grammar and a bounded Access expression parser. [SQL_PARSER.md](../query/SQL_PARSER.md)
records the exact supported subset, limits, corpus provenance and limitations.
This is syntax integration, without binding or execution evidence from Access.

| Component | Decision | Implementation status |
| --- | --- | --- |
| SQL lexer/parser | Apache DataFusion SQLParser (`sqlparser`) | Integrated behind private adapter |
| Access dialect | TableCraft extension and bounded parser adapter | Retrieval, declarations, crosstabs and action syntax |
| Semantic analysis | TableCraft-owned Access semantics | Initial scalar kernels and slot/parameter binding implemented; full result-type inference absent |
| Query intermediate representation | TableCraft-owned, independent of upstream AST | Owned syntax plus private bound retrieval plans |
| SQL translation | TableCraft-owned, supported subsets with semantic evidence | Not implemented |
| Native Access query evaluation | TableCraft-owned engine behavior | Not implemented |

MsSqlDialect is a SQL Server dialect, not Access SQL. Access-specific syntax must
be evaluated explicitly. The Dialect hooks return upstream Statement/Expr types;
that API is not a promise that a custom dialect can represent every Access
construct. The adapter may need specialized TableCraft nodes or upstream changes.
Keep the original SQL independently where preservation matters: upstream SQL
formatting normalizes text, and its source-span support is still incomplete.

Keep upstream AST types inside the parser adapter. Database adapters, forms,
reports and future query designers consume TableCraft definitions. Feature slices
should separate parsing, owned query definitions, expression semantics, analysis
and formatting as actual working responsibilities appear. Do not enable AST serde
merely because it is available or make upstream AST serialization a project format.

Parsing a function name does not implement IIf, Nz, DateAdd, coercion, NULL
behavior, collation, updateability or data macros. A successful syntax rewrite
does not establish equivalent SQLite results. Unrepresentable or unsupported
constructs must remain explicit failures rather than silently using a permissive
generic or SQL Server interpretation.

Full DataFusion and Arrow are not selected for this parser work. The native
engine must not require Microsoft's Jet/ACE runtime for supported cross-platform
query execution; real Access remains a separate comparison oracle.

## Evidence and further integration

The authored corpus includes supported TOP, DISTINCTROW, bracketed names and
field wildcards, PARAMETERS, TRANSFORM/PIVOT, date spelling, joins,
LIKE/function syntax and action queries, plus malformed/foreign rejection cases. See SQL_PARSER.md for
reference mapping. These documentation-derived syntax cases are not observed
Access results; query extraction from the supplied ACCDB is unimplemented.

Tests check bounded input/nesting, error locations, parse/format/parse
equivalence and explicit rejection. Original source is preserved independently
from normalized formatting. Query designer
generation, SQLite translation and differential behavior require their own later
working implementations and tests. Record exact Access/DAO builds for oracle
results, and report unavailable oracle environments as skipped.

## Native project and scripting decisions

Implementation is paused until explicitly resumed under
[the current product priority](../decisions/0003_ACCESS_BEHAVIORAL_PARITY.md).
The following choices are retained future plans and do not gate Access work.

The [accepted decision](../decisions/0001_PROJECT_FORMAT_AND_EXECUTION.md) selects a
SQLite-backed `.tcraft` container and equivalent `.tcraftproj/` workspace. Internal
schema versions/capabilities gate writes and execution; this is not implemented
by the existing ordinary SQLite reader. `tablecraft-project` will own the format,
separately from connections/transactions in `tablecraft-sqlite`.

Python is the primary and initially only general-purpose native scripting language,
superseding the earlier TypeScript/JavaScript default. CPython through PyO3, ideally
in a helper process, is the selected direction; uv-managed locked environments are
preferred. Exact runtime/ABI/distribution/isolation remain first-proof decisions.
No Python dependency is adopted now, and ordinary CLI/query/ACE builds remain
independent of it. Pyodide is a reduced browser candidate, not desktop parity.
The [scripting contract](../features/PYTHON_SCRIPTING.md) and
[execution lifecycle](APPLICATION_EXECUTION.md) own limits.

Native MDB/ACCDB continue through standalone ACE with preserved VBA and supported
Access-native automation. [Access-to-TableCraft conversion](../features/PROJECT_MIGRATIONS.md)
is a first-class future application workflow. PHP/TypeScript web generators are
later model consumers, not native scripting providers. No subjective architecture
rating or proposed crate count establishes implementation completeness.

## Agent service and MCP direction

The [editing decision](../decisions/0002_SHARED_APPLICATION_EDITING.md) selects shared
semantic edits/changesets, previews and deterministic scenarios before a large
protocol tool surface. The existing `tablecraft-builders` name is retained.
[tablecraft-edit ownership](APPLICATION_CHANGESETS.md) is prospective
and may begin as a module. No agent runtime or new crate is implemented here.

Use the [official Rust MCP SDK](https://github.com/modelcontextprotocol/rust-sdk)
(`rmcp`) for the future thin adapter, with exact release/features chosen at adoption.
The verified 2026-07-28 protocol baseline, negotiated optional Tasks/subscriptions,
owned durable handles and local-stdio-first boundary are documented in
[interfaces/MCP.md](../interfaces/MCP.md). Normal CLI/engine builds remain independent
of network/Python/MCP runtimes. Ruff/type checking/pytest are future managed project
tools, not new installed application or workspace dependencies.

## External data decisions

[Provider and interchange direction](../features/EXTERNAL_DATA.md) separates internal
backends from external adapters. Selected future candidates are Tiberius for native
SQL Server/TDS, odbc-api for generic installed ODBC, reqwest for Graph-backed
SharePoint Lists, tokio-postgres for later PostgreSQL and mysql_async for later
MySQL/MariaDB. calamine/csv/rust_xlsxwriter cover file interchange with real consumers.
ODBC still needs a system Driver Manager and vendor drivers; native protocol
clients do not require ODBC. rs-odbc is an alternative wrapper, not a replacement
Driver Manager. No new dependencies or external connectors are installed here.
See the [candidate review](../development/DEPENDENCY_REVIEW.md#external-connectivity-direction)
for review limits and adoption gates.
