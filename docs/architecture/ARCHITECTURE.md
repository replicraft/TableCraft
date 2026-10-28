# Domain-oriented workspace

TableCraft groups libraries by the domain they serve. Package boundaries separate
working responsibilities with distinct dependencies, tests or platform needs.
Directories organize those packages; a domain directory is not itself a crate.

This adopts the grouped direction from the supplied workspace proposal. The
current layout below is implemented. The target map is an explicit direction for
future extraction, not a list of implemented features or packages to scaffold.
The private architecture pack remains under ignored `docs/temp/`.

## Active product scope

The [Access behavioral-parity decision](../decisions/0003_ACCESS_BEHAVIORAL_PARITY.md)
sets the current direction: native Access compatibility, with ACE 16 primary.
SQLite and native `.tcraft` projects are paused, including Python, conversion and
native-project P2P infrastructure. Their existing packages and future architecture
plans remain documented without becoming prerequisites for the backend or UI.

Shared services enforce Access behavior across CLI, future GUI and agent hosts.
Future UI policies must distinguish stored file capabilities, target Access
application version and database settings. Do not introduce generalized project
infrastructure or placeholder profile packages for the paused plans.

## Current workspace

```text
apps/
└── tablecraft-cli/                 # tablecraft executable; clap host
crates/
├── foundation/
│   ├── tablecraft-core/            # exact values, scoped IDs, budgets, errors
│   ├── tablecraft-schema/          # validated scalar relation schemas
│   ├── tablecraft-model/           # shared application-object catalog metadata
│   └── tablecraft-db/              # shared snapshots and consuming edit contracts
├── platform/
│   ├── tablecraft-io/              # bounded format-independent synchronous reads
│   └── tablecraft-crypto/          # checked cipher/hash and secret-buffer integration
├── formats/
│   └── tablecraft-formats/         # inert ZIP/OPC/XML/template and CAB codecs
├── engineering/
│   └── tablecraft-testkit/         # independent raw fixtures; development only
├── engines/
│   └── tablecraft-ace/             # native storage, reads, checks and edit previews
├── backends/
│   ├── tablecraft-access/          # native scalar Source and edit adapters
│   └── tablecraft-sqlite/          # read-only SQLite Source adapter
└── query/
    ├── tablecraft-sql/             # bounded Access syntax, owned model, formatting
    ├── tablecraft-expressions/     # evaluated scalar rules over core values
    └── tablecraft-query/           # retrieval and backend-neutral edit planning
```

The CLI depends on `tablecraft-sql`, core values/budgets and `tablecraft-expressions`
with `sql` enabled for scalar `query eval`; it also consumes `tablecraft-query`
for `query run` over inline sources. `tablecraft-expressions` depends on core and
optionally on `tablecraft-sql` through its `sql` tree-evaluation feature.
Libraries build independently and never depend on an application. There are fifteen
Cargo packages. The CLI provides `query parse`, source-free `query eval` and inline `query run`.
The CLI also consumes `tablecraft-ace` and `tablecraft-model` for native catalog
inspection. The engine consumes I/O and core/schema/model; I/O consumes core, and model consumes core/schema.
The CLI uses `tablecraft-access` to adapt native scalar snapshots to query Source;
engine/query do not import that adapter. See [NATIVE_QUERIES.md](../access/queries/NATIVE_QUERIES.md).
`tablecraft-access::NativeBrowser` owns reusable table discovery and page operations
for CLI and future UI hosts. The engine owns physical catalog/slot/value decoding;
its separate `TablePage` result makes partial row validation explicit. The CLI
opens the requested file, passes host limits and formats the result; it does not
own table classification or pagination semantics. No new crate is needed for this
boundary. Its `TableDescription` reports scalar-column support from engine metadata;
`TableEntry` exposes captured storage classifications without opening linked
sources. Neither promises readable records or a complete integrity check.
The distinct `StoredTablePage`/`stored_page` opt-in exposes qualified cached
calculation results with retained inert declarations and per-column origins.
It shares physical paging while keeping ordinary scalar/query eligibility intact.
`MultivalueTablePage` composes explicit scalar/collection cells through the same
browser, keeping collection types outside core scalar/query contracts. Complete
parent-key/child qualification remains in the native engine; the CLI only renders
stored lists. Its optional feature metadata enables independently opted-in cached
results. See [browsing contracts](../cli/CLI_BROWSING.md).
Access also maps explicitly read native saved SELECT fragments into SQL's owned
syntax, retaining original metadata; ACE does not own that adaptation and saved
query sources are not automatically expanded. See [saved queries](../access/queries/NATIVE_SAVED_QUERIES.md).
[MODEL.md](../foundation/MODEL.md) distinguishes shared descriptors from native origins and
detailed application models.
Implemented scalar semantics and boundaries are in [EXPRESSIONS.md](../expressions/EXPRESSIONS.md).

The original `tablecraft-query` parser package is now `tablecraft-sql`; Rust imports
use `tablecraft_sql`. Its public syntax types and parsing behavior remain in that
package. It owns the private SQLParser adapter and TableCraft's expression,
SELECT, statement and schema-definition **syntax**. These syntax nodes do not
constitute bound relational schema, expression evaluation or execution plans.
See [SQL_PARSER.md](../query/SQL_PARSER.md) for implemented coverage and limitations.

[QUERY_EXECUTION.md](../query/QUERY_EXECUTION.md) describes the new query package and
its explicit host source interface, supported retrieval profile and limits.

Native index definition inspection and explicitly selected single-Int32 indexed
snapshots now live within ACE. The Access adapter selects/caches the snapshot;
reusable query execution remains independent of physical index structures. See
[NATIVE_INDEXES.md](../access/indexes/NATIVE_INDEXES.md) for the implemented profile and limits.

## Target domain map

`tablecraft-core`, `tablecraft-schema`, `tablecraft-sql`, `tablecraft-expressions`,
`tablecraft-query`, `tablecraft-model`, `tablecraft-db`, `tablecraft-io`,
`tablecraft-access`, `tablecraft-sqlite`, `tablecraft-ace`, `tablecraft-crypto`,
`tablecraft-formats` and `tablecraft-testkit` exist as library packages today. The
remaining names identify intended ownership when working consumers justify their
introduction. The target map includes implemented and prospective owners across
16 groups; it is not a package-count delivery target.

| Domain | Package ownership |
| --- | --- |
| `foundation/` | `tablecraft-core`: shared values, IDs and small contracts; `tablecraft-schema`: relational schema, keys, relationships, indexes and schema differences; `tablecraft-model`: shared application-object metadata, identity and dependencies; `tablecraft-db`: backend-neutral snapshots/edit contracts; future canonical versioned application definitions with real consumers |
| `platform/` | `tablecraft-platform`: host capabilities and platform integration; `tablecraft-io`: random-access I/O, low-level locks and durability interfaces; `tablecraft-crypto`: reusable cryptographic integration when a real consumer needs it |
| `engines/` | `tablecraft-ace`: public, standalone Rust-native Jet/ACE engine; `tablecraft-ace-ffi`: later C ABI over that engine |
| `backends/` | `tablecraft-access`: built-in integration over standalone ACE; `tablecraft-sqlite`: built-in SQLite integration |
| `adapters/` | Future external providers: `tablecraft-access-msace`, `tablecraft-odbc`, `tablecraft-mssql`, `tablecraft-sharepoint`; later `tablecraft-postgres` and `tablecraft-mysql` |
| `query/` | `tablecraft-sql`: parsing, formatting and owned syntax; `tablecraft-expressions`: Access coercion, operators, built-ins and expression evaluation; `tablecraft-query`: reusable binding, planning, execution, updateability and supported translation against lower-level contracts, independent of application services |
| `formats/` | `tablecraft-formats`: working inert Access package/template codecs; future `tablecraft-project`: paused native `.tcraft` serialization and internal migrations |
| `application/` | `tablecraft-forms`: control bindings, form layout, validation and events; `tablecraft-reports`: sections, grouping, pagination and export; `tablecraft-macros`: Access visual/embedded/table data macros; `tablecraft-automation`: reusable macro/action/event host contracts independent of VBA; `tablecraft-python`: CPython/PyO3 host, versioned Python API, project environments and lifecycle; `tablecraft-edit`: future reusable typed application mutations, changesets, semantic diffs and validation coordination; `tablecraft-runtime`: application sessions, navigation, canonical commands/events and host implementations |
| `collaboration/` | `tablecraft-crdt`: causal/conflict semantics and deterministic merge logic; `tablecraft-p2p`: transport, discovery and peer sessions; `tablecraft-sync`: replication orchestration, durable state, replay and projection |
| `presentation/` | `tablecraft-ui`: shared widgets, styling and accessibility; `tablecraft-builders`: visual form/report/query builders and editing workflows; `tablecraft-render`: reusable headless layout, measurement and print primitives |
| `security/` | `tablecraft-security`: trust, authorization, identity and secret-handling contracts |
| `interfaces/` | `tablecraft-mcp`: thin rmcp-based protocol adapter over authorized shared services; application state stays with its owners |
| `extensibility/` | `tablecraft-plugins`: capability-scoped plugin hosting |
| `lifecycle/` | `tablecraft-backup`: consistent backup, restore and verification; `tablecraft-migrate`: schema changes and explicit cross-backend conversions |
| `tooling/` | `tablecraft-codegen`: later maintainable-source generation from the shared application model, independent of native scripting |
| `engineering/` | `tablecraft-compat`: native Access oracle and differential checks; `tablecraft-testkit`: shared synthetic data, failpoints and deterministic helpers; `tablecraft-bench`: cross-subsystem measurement |

Application hosts remain under `apps/`: the existing `tablecraft-cli`, a future
native `tablecraft`, and a future browser `tablecraft-web`. Desktop and browser
packages are introduced with runnable functionality, not empty application shells.
UI technology, transport and other future dependency choices remain recorded in
[TECH_STACK.md](TECH_STACK.md) and [LIBRARY_POLICY.md](../development/LIBRARY_POLICY.md); this map
does not adopt additional dependencies.

The [standalone native engine requirement](NATIVE_ENGINE.md) separates reusable
Jet/ACE implementation from TableCraft's adapter. External Rust applications must
be able to consume `tablecraft-ace` without application dependencies. Rust comes
first, a C ABI follows useful engine functionality, and an optional ODBC interface
is a later capability outside this package count. The Rust engine now has bounded metadata/scalar reading; C ABI and ODBC do not exist yet.
`tablecraft-ui` owns reusable presentation controls. `tablecraft-builders` composes
those controls into visual query builders and form/report designers, including
their editing workflows. Builders consume domain models and services; form/report
runtime behavior stays with its application-domain owners.
Visual query editor behavior belongs in `tablecraft-builders`; shared execution
models must not import that editor into the reusable query package.

[VBA scope](VBA_COMPATIBILITY.md) retains existing-project preservation and inert
dependency detection. VBA execution, authoring, editor/debugger, VBA-specific
DAO/ADO and Windows/COM/Wine providers are deferred. `tablecraft-vba`,
`tablecraft-wincompat` and `tablecraft-wine` are outside the active target map.
Reserve `application/tablecraft-vba-host` only as a deferred integration boundary
for a future independent RustVBA release; it is excluded from the active package
count and has no manifest. RustVBA's internal crates belong to its own project.
Access macros and non-VBA automation remain in scope with headless contracts.
[Windows/Wine research](WINDOWS_COMPATIBILITY.md) records limits for reconsideration,
not an active provider implementation track.

The application-compatibility requirements are feature slices within these owners,
not additional crates: [startup/session state](../features/ACCESS_APP_STARTUP.md),
[imported ribbons](../features/ACCESS_RIBBON_COMPAT.md), [DAO/ADO](../features/DAO_ADO.md),
[advanced fields](../features/ACCESS_ADVANCED_FIELDS.md) and
[distribution/signature profiles](../features/ACCESS_DISTRIBUTION.md).
The [application corpus](../testing/ACCESS_APPLICATIONS.md) records the unqualified
candidate and defines separate open/preserve/execute/edit/save evidence.
The [roadmap](../ROADMAP.md) maps [analysis tools](../features/DATABASE_ANALYSIS.md),
[wizards](../features/APP_TEMPLATES_WIZARDS.md), [expression editing](../features/EXPRESSION_BUILDER.md)
and [reusable data tasks](../features/IMPORT_EXPORT_TASKS.md) onto existing owners.

## Ownership review — 27 October 2026

The pre-extraction workspace broadly followed the adopted domain chart. The dependency
guard passed for its twelve packages; inspected query and foundation code retained
its engine/host separation. This verifies declared edges, not every source-level
ownership decision. The older private pack and prospective package counts are
planning history, not a requirement to create all listed packages.

Before extraction, tracked Rust files under `src/` contained 61,020 lines in ACE, 62,354 in
expressions, 28,635 in query and 15,314 in Access. These figures include tests and
exclude integration-test directories and unregistered INSERT work. File size alone
does not establish an extraction boundary. ACE's mutation and index slices account
for 38,953 of those lines; its encryption slice accounts for 3,654. Much of the
engine's size therefore remains native storage work.

The following table retains the review findings. The implemented follow-up below
records their current status.

| Priority | Change | Boundary and concrete benefit |
| --- | --- | --- |
| First crate candidate | `platform/tablecraft-crypto` | Extract useful checked cipher/digest integration and secret-buffer handling from ACE's Agile/legacy/SID consumers. Give it engine-neutral errors and explicit inputs; it must not import ACE. Retain native descriptor/profile interpretation, password policy, encoding keys/page-IV policy, header rewriting and source/preservation orchestration in ACE. Do not merely wrap individual RustCrypto calls or move the complete encryption tree wholesale. |
| First small existing-crate moves | Query row views into `tablecraft-db` | `source/projection.rs` contains `ProjectedRow` and projection validation; `source/multivalue.rs` contains the borrowed `MultivalueList` view. They have real Access, SQLite and query consumers. Use lower-level validation errors and adapt them to `QueryError`; query-specific visitors remain in query. Preserve the paused SQLite scope. |
| First API cleanup | Owned syntax constructors in `tablecraft-sql` | Saved-query/calculation consumers currently parse synthetic `SELECT 0` or skeleton queries to obtain owned names and statement shapes. Add bounded constructors in the existing syntax owner, then replace those construction workarounds without widening saved-query admission or exposing upstream parser ASTs. |
| Engineering reuse | `tablecraft-testkit` and `tablecraft-compat`, progressively | Start with genuinely shared raw fixture builders, I/O failpoints and process/pin/report plumbing. ACE currently imports integration fixtures through `src/test_support.rs` path attributes; Access tests repeat reader probes. Keep private proof tests and independent goldens with their owners. Engine-free testkit code must not introduce testkit-to-ACE cycles or foreign-trait implementation problems. Consolidating Python/Java laboratory tools does not by itself require a Rust crate. |
| Next independent format boundary | Access package readers under `formats/` | Access `template/` owns ZIP/OPC/XML/IRI parsing and `input/deployment*` owns CAB extraction. A working package-format library could isolate those dependencies for package-only consumers. Package errors must stop borrowing ACE's `InspectError`. Keep `TemplateSource`, native input routing, `NativeReadSource` integration and query adaptation in Access. Choose a package name with the extraction; this is not the paused `.tcraft` implementation. |
| Module cleanup | Routed-source forwarding within Access | Linked, external and recomputed sources repeat capability delegation. Share the mechanical forwarding while keeping namespace ownership, selected-handle checks, source identity, cancellation and budgets explicit. This does not justify a provider or runtime crate. |

Crypto extraction must preserve independent vectors, all supported encryption
profiles, zeroization, budget/cancellation behavior and atomic buffer publication.
Moving code supplies neither ciphertext authentication nor signer verification.
`tablecraft-security` remains a distinct future policy/trust boundary. Current
deployment signatures are unverified; there is no implemented signature verifier
to relocate. The generic image journal's SHA-256 framing stays in I/O.

Keep native pages, allocation, physical index keys, record layouts, constraints,
preservation and backend transactions together in ACE. Keep Access expression
semantics in expressions and relational planning/execution in query. The query
`Source` contract includes named SQL, query errors, policy provenance and residual
constraints; moving it wholesale into DB would invert the intended dependencies.
Small core/schema/model/DB packages are not missing responsibilities merely because
they are small; native physical types do not become shared schema types by moving
files.

CLI arguments, password prompting, paths, temporary files, output publication and
table formatting remain host duties. A second writable host can justify extracting
the shared edit workflow from composition into Access services; do not create an
empty runtime/edit package in anticipation. Optional locale/error catalogs and
evaluated-operand kernels can remain feature slices within expressions for now.

Each accepted extraction needs a working consumer, preserved public behavior,
focused checks and updated workspace/dependency rules. INSERT source files remain
pending and are not moved by this review.

### Implemented follow-up

The user-directed refactor implements `tablecraft-crypto`, `tablecraft-formats`
and development-only `tablecraft-testkit`, with real consumers and no new registry
versions. Package readers no longer depend on ACE's error type. CLI package
inspection imports formats directly; Access retains native qualification and
`TemplateSource`. The shared DB row/collection views are imported directly from
DB, without query compatibility exports. SQL owns bounded name constructors;
Access and query consumers no longer parse dummy carriers. Access shares fourteen
mechanical forwarding methods while keeping each route and namespace policy
explicit. Engineering artifact readers share a Python module; no empty Rust
compatibility package was introduced.

See [crypto](../platform/CRYPTO.md), [package formats](../formats/ACCESS_PACKAGES.md),
[DB views](../foundation/DB.md), [SQL construction](../query/SQL_PARSER.md) and
[testkit](../testing/TESTKIT.md) for implemented boundaries. Normal native saves,
signature verification and paused project/runtime work retain their prior status.

## Dependency and ownership rules

- Applications compose libraries; libraries never import application packages.
  Domain groups describe ownership rather than imposing an arbitrary numerical
  layering order. Each package dependency must have a concrete, acyclic purpose.
- SQLParser types stay private to `tablecraft-sql`. Future semantic consumers use
  TableCraft syntax. `tablecraft-expressions` owns evaluated Access semantics;
  `tablecraft-query` owns binding and plans. Shared syntax remains in the SQL
  package until an actual consumer establishes a better boundary.
- Shared names, scalar type descriptors and explicit schema policies now live in
  `tablecraft-schema`; see [SCHEMA.md](../foundation/SCHEMA.md). Richer relational schema belongs
  there with working consumers. DDL syntax
  stays in `tablecraft-sql`; conversion to schema requires explicit validation.
  Shared application-object metadata belongs to `tablecraft-model`, while forms,
  reports and automation own their domain behavior. Avoid duplicate definitions
  or a central model package that imports the entire application stack.
- Keep the native engine in `tablecraft-ace` together initially, with internal modules for format,
  pages, allocation, catalog, records, indexes, long values, objects, locking,
  transactions, recovery and format-specific crypto as they become real. Extract
  packages only after cache, transaction and mutation ownership is understood.
- `tablecraft-io` now owns generic bounded exact/read-at-offset operations,
  cancellation-aware read retries and format-independent I/O errors; see [IO.md](../foundation/IO.md).
  ACE adapts those errors and owns page geometry, allocation, records and native
  format policy. I/O imports core budgets, never engines, adapters or query code.
  Low-level file-lock primitives may later belong to `tablecraft-io`; Jet/ACE shared-file
  coordination belongs to `tablecraft-ace`, not the application adapter. Peer replication has separate
  semantics and belongs to the collaboration domain.
- Keep CRDT logic testable without networking. Transport owns connections; sync
  composes transport, replicated operations and durable storage. These packages
  do not implement native Access shared-file locking.
- `tablecraft-db` owns backend-neutral contracts, never concrete adapter/engine
  construction. The native engine may use `tablecraft-query`; that reusable query
  package must not import `tablecraft-access`, application services or runtime.
  Hosts compose engine adapters, avoiding engine/query/database/adapter cycles.
- The application runtime implements narrow interfaces consumed by forms and
  automation; macros and those libraries must not import the runtime back. Transaction-scoped
  data-macro integration uses a lower-level contract rather than importing the
  whole application runtime into the Access engine. Tokio remains an async
  executor choice, separate from the Access application runtime.
- Application-domain directory placement does not make pure automation contracts
  or headless macro execution an application-host dependency. Those reusable
  packages may support the standalone engine, but must not import concrete UI,
  runtime, adapter or engine implementations. SQL UDFs call a supplied lower-level
  interface, never the entire VBA/runtime implementation from query code.
- Preserve existing VBA and report dependent actions as unavailable while execution
  is deferred. Do not introduce VBA/Windows/Wine providers in TableCraft. Required
  data macros remain an engine write obligation. Replication must distinguish
  validated data apply from user-event replay and external side effects.
- Shared UI components and visual builder workflows have separate owners. Shared layout/print
  primitives should work headlessly; native/browser integration stays explicit.
- Access/DAO oracle dependencies stay in engineering tooling, outside production
  library and CLI dependency closures. Native compatibility requires observed
  Access results, not merely successful parser or cross-platform builds.

## Progressive extraction

The first step is complete: relocate core under `foundation/`, relocate and rename
the parser under `query/`, and update its CLI consumer, CI and documentation.

Next, introduce schema and semantic packages with their first working behavior:
schema contracts with real consumers, broader expression semantics beyond the
initial checked scalar evaluator, and query binding/planning against those contracts.
The scalar schema and bounded read-only retrieval slice now implement that step;
bounded native scalar sources are also implemented; full field/schema semantics remain future work.
Native format research and comparison tooling can proceed alongside that work.
The standalone `ace` engine and shared object model now have a working read-only
catalog consumer and scalar query adapters, including [SQLite](../backends/SQLITE.md). `db` snapshot/edit contracts now have working native consumers. Further
compatibility boundaries require actual consumers; these reads do not qualify ACE writes.

The platform I/O boundary now has a working native-engine consumer. Generic
read helpers and focused I/O tests live there; native fault-injection/snapshot
integration tests remain with ACE and its adapter. Locks, durability, buffered
caches and async I/O are not implemented by this extraction.

Application, presentation, collaboration and lifecycle packages follow their
actual consumers. A small desktop milestone may introduce working UI components
early; native multiuser compatibility experiments also need not wait for peer
replication. Stage order guides extraction and does not gate unrelated research.

Use modules for feature slices within a package. Promote a slice to a crate when
it has useful dependency isolation, independent consumers, platform/testing needs
or a coherent public contract. Follow [DEVELOPMENT.md](../development/DEVELOPMENT.md) for checks
and [IMPLEMENTATION.md](../IMPLEMENTATION.md) for observed verification.

[Database contracts](../foundation/DB.md) now have working owned snapshots and edit planning,
consumed by the Access adapter. This does not scaffold connection/cursor/session
APIs or change SQLite behavior. The standalone engine remains independent of DB
and the adapter; generic query editing can depend on DB without importing either.

[Query editing](../access/writes/QUERY_EDITING.md) now consumes DB contracts without native imports.
The CLI composes that planner with NativeEditor and fresh-copy host I/O. Optional
engine journaling remains separate from the CLI copy lifecycle.

## Native projects and execution direction

Implementation of this retained direction is paused by
[the Access behavioral-parity decision](../decisions/0003_ACCESS_BEHAVIORAL_PARITY.md).
It is not an active backend/UI prerequisite. Shared Access services enforce
native behavior across all hosts; future project modernization must not redefine
that behavior. File capabilities, target Access version and database settings
remain distinct application-policy inputs, not merely storage magic bytes.

The [accepted decision](../decisions/0001_PROJECT_FORMAT_AND_EXECUTION.md) selects
SQLite-backed `.tcraft`, equivalent unpacked `.tcraftproj/` workspaces and Python
as the initial general-purpose scripting language. This supersedes the earlier
JS/TypeScript default; modern web/PHP generators remain later independent work.
Native MDB/ACCDB interoperability remains first-class and ACE 16 remains the
immediate implementation focus. Project/Python/codegen names above have no
manifests and must not be scaffolded without working functionality.

[Project format](../formats/TCRAFT_FORMAT.md), [version gates](../formats/SCHEMA_VERSIONING.md),
[project lifecycle](PROJECT_LIFECYCLE.md),
[application execution](APPLICATION_EXECUTION.md),
[transaction ownership](TRANSACTIONS.md),
[Python hosting](../features/PYTHON_SCRIPTING.md),
[application conversion](../features/PROJECT_MIGRATIONS.md) and
[peer schema evolution](../features/COLLABORATIVE_SCHEMA_EVOLUTION.md) define the
accepted contracts and remaining decisions. They are not working APIs or release
claims. [Qualification](../testing/PROJECT_COMPATIBILITY.md) requires executable proofs.

[Dependency boundaries](DEPENDENCY_BOUNDARIES.md) now have a runnable
local-package guard and regression tests. New packages need an explicit policy
review; a growing target map does not relax existing engine/query independence.

## Shared application editing and agent services

The [editing decision](../decisions/0002_SHARED_APPLICATION_EDITING.md) reserves one
reusable semantic-editing boundary for humans and agents, with changesets,
revision preconditions, exact diffs, validation and approval scope. The prospective
edit package may start as a feature module; it has no manifest or implementation.
It consumes models/lower contracts without runtime, GUI or protocol imports.
Runtime composes approved apply; MCP/CLI/designers adapt the same services.
`tablecraft-builders` retains its name despite the old design spelling in the proposal.

[Agent discovery/design](../features/AGENT_NATIVE.md),
[changesets](APPLICATION_CHANGESETS.md),
[headless previews](../features/HEADLESS_PREVIEWS.md),
[deterministic scenarios](../testing/APPLICATION_SCENARIOS.md) and
[MCP direction](../interfaces/MCP.md) are requirements, not implemented tools.
Existing native edit plans remain a distinct bounded capability. No agent path
may bypass version, authorization, native format or preservation restrictions.
The actual workspace has fifteen packages and no MCP runtime dependency.

## Built-in backends and external data

[External Data](../features/EXTERNAL_DATA.md) defines the accepted connection and
interchange direction. Engines implement storage independently; built-in backends
integrate locally managed storage; external adapters use independent providers,
protocols or service APIs. File location does not decide ownership: a shared ACCDB
opened through our native engine still uses the built-in Access backend.

DB remains in foundation and imports no concrete backend or adapter. Hosts compose
providers; shared services own connection profiles, link management and saved data
tasks. External adapters are prospective, with no empty directories or manifests.
The earlier catch-all `tablecraft-connectors` proposal is superseded by these
specific owners. Future SQL Server connectivity must qualify against Microsoft
first; Irongate is a later compatibility target. SharePoint means a live Lists connector, not a hosted clone.

## Storage ownership checkpoint

The former `storage/` grouping has no remaining responsibility or manifest.
Shared snapshot/edit contracts belong to foundation DB; native images, physical
transactions, pages, indexes and format-specific checks stay in ACE. Moving them
into DB would introduce a native dependency into the neutral contract layer and
weaken the standalone engine boundary. Backend-specific native-to-shared conversion
and saved-query adaptation remain in Access; SQLite integration stays in its
backend. Generic bounded I/O and image journaling already have their platform owner.
No further extraction is justified by current consumers. Future task/session/edit
services should move only when their implementation establishes a real boundary.
