# Domain-oriented workspace

TableCraft groups libraries by the domain they serve. Package boundaries separate
working responsibilities with distinct dependencies, tests or platform needs.
Directories organize those packages; a domain directory is not itself a crate.

This adopts the grouped direction from the supplied workspace proposal. The
current layout below is implemented. The target map is an explicit direction for
future extraction, not a list of implemented features or packages to scaffold.
The private architecture pack remains under ignored `docs/temp/`.

## Current workspace

```text
apps/
└── tablecraft-cli/                 # tablecraft executable; clap host
crates/
├── foundation/
│   ├── tablecraft-core/            # exact values, scoped IDs, budgets, errors
│   ├── tablecraft-schema/          # validated scalar relation schemas
│   └── tablecraft-model/           # shared application-object catalog metadata
├── platform/
│   └── tablecraft-io/              # bounded format-independent synchronous reads
├── engines/
│   └── tablecraft-ace/             # bounded native metadata/scalar reads
├── storage/
│   └── tablecraft-access/          # native scalar Source adapter
└── query/
    ├── tablecraft-sql/             # bounded Access syntax, owned model, formatting
    ├── tablecraft-expressions/     # evaluated scalar rules over core values
    └── tablecraft-query/           # bound read-only retrieval plans/execution
```

The CLI depends on `tablecraft-sql`, core values/budgets and `tablecraft-expressions`
with `sql` enabled for scalar `query eval`; it also consumes `tablecraft-query`
for `query run` over inline sources. `tablecraft-expressions` depends on core and
optionally on `tablecraft-sql` through its `sql` tree-evaluation feature.
Libraries build independently and never depend on an application. There are ten
Cargo packages. The CLI provides `query parse`, source-free `query eval` and inline `query run`.
The CLI also consumes `tablecraft-ace` and `tablecraft-model` for native catalog
inspection. The engine consumes I/O and core/schema/model; I/O consumes core, and model consumes core/schema.
The CLI uses `tablecraft-access` to adapt native scalar snapshots to query Source;
engine/query do not import that adapter. See [NATIVE_QUERIES.md](NATIVE_QUERIES.md).
[MODEL.md](MODEL.md) distinguishes shared descriptors from native origins and
detailed application models.
Implemented scalar semantics and boundaries are in [EXPRESSIONS.md](EXPRESSIONS.md).

The original `tablecraft-query` parser package is now `tablecraft-sql`; Rust imports
use `tablecraft_sql`. Its public syntax types and parsing behavior remain in that
package. It owns the private SQLParser adapter and TableCraft's expression,
SELECT, statement and schema-definition **syntax**. These syntax nodes do not
constitute bound relational schema, expression evaluation or execution plans.
See [SQL_PARSER.md](SQL_PARSER.md) for implemented coverage and limitations.

[QUERY_EXECUTION.md](QUERY_EXECUTION.md) describes the new query package and
its explicit host source interface, supported retrieval profile and limits.

Native index definition inspection and explicitly selected single-Int32 indexed
snapshots now live within ACE. The Access adapter selects/caches the snapshot;
reusable query execution remains independent of physical index structures. See
[NATIVE_INDEXES.md](NATIVE_INDEXES.md) for the implemented profile and limits.

## Target domain map

`tablecraft-core`, `tablecraft-schema`, `tablecraft-sql`, `tablecraft-expressions`,
`tablecraft-query`, `tablecraft-model`, `tablecraft-io`, `tablecraft-access` and `tablecraft-ace` exist as library packages today. The
remaining names identify intended ownership when working consumers justify their
introduction. The map contains 36 prospective library packages across 14 groups;
the count is descriptive, not a delivery target.

| Domain | Package ownership |
| --- | --- |
| `foundation/` | `tablecraft-core`: shared values, IDs and small contracts; `tablecraft-schema`: relational schema, keys, relationships, indexes and schema differences; `tablecraft-model`: shared application-object metadata, identity and dependencies |
| `platform/` | `tablecraft-platform`: host capabilities and platform integration; `tablecraft-io`: random-access I/O, low-level locks and durability interfaces; `tablecraft-crypto`: reusable cryptographic integration when a real consumer needs it |
| `engines/` | `tablecraft-ace`: public, standalone Rust-native Jet/ACE engine; `tablecraft-ace-ffi`: later C ABI over that engine |
| `storage/` | `tablecraft-db`: backend-neutral contracts without concrete engine/adapter imports; `tablecraft-access`: TableCraft adapter over the standalone engine; `tablecraft-sqlite`: SQLite adapter and local metadata; `tablecraft-connectors`: linked and remote database adapters |
| `query/` | `tablecraft-sql`: parsing, formatting and owned syntax; `tablecraft-expressions`: Access coercion, operators, built-ins and expression evaluation; `tablecraft-query`: reusable binding, planning, execution, updateability and supported translation against lower-level contracts, independent of application services |
| `application/` | `tablecraft-forms`: control bindings, form layout, validation and events; `tablecraft-reports`: sections, grouping, pagination and export; `tablecraft-vba`: VBA language/model/interpreter and debugger primitives; `tablecraft-macros`: Access visual/embedded/table data macros; `tablecraft-automation`: reusable host contracts; `tablecraft-runtime`: application sessions, navigation, commands and host implementations |
| `compatibility/` | `tablecraft-wincompat`: API-level Windows/Automation emulation and provider integration, independent of the VBA language engine; separate from engineering oracle tooling |
| `collaboration/` | `tablecraft-crdt`: causal/conflict semantics and deterministic merge logic; `tablecraft-p2p`: transport, discovery and peer sessions; `tablecraft-sync`: replication orchestration, durable state, replay and projection |
| `presentation/` | `tablecraft-ui`: shared visual components; `tablecraft-design`: form/report/query editor behavior; `tablecraft-render`: reusable headless layout, measurement and print primitives |
| `security/` | `tablecraft-security`: trust, authorization, identity and secret-handling contracts |
| `interfaces/` | `tablecraft-mcp`: protocol adapter for approved operations |
| `extensibility/` | `tablecraft-plugins`: capability-scoped plugin hosting |
| `lifecycle/` | `tablecraft-backup`: consistent backup, restore and verification; `tablecraft-migrate`: schema changes and explicit cross-backend conversions |
| `engineering/` | `tablecraft-compat`: native Access oracle and differential checks; `tablecraft-testkit`: shared synthetic data, failpoints and deterministic helpers; `tablecraft-bench`: cross-subsystem measurement |

Application hosts remain under `apps/`: the existing `tablecraft-cli`, a future
native `tablecraft`, and a future browser `tablecraft-web`. Desktop and browser
packages are introduced with runnable functionality, not empty application shells.
UI technology, transport and other future dependency choices remain recorded in
[TECH_STACK.md](TECH_STACK.md) and [LIBRARY_POLICY.md](LIBRARY_POLICY.md); this map
does not adopt additional dependencies.

The [standalone native engine requirement](NATIVE_ENGINE.md) separates reusable
Jet/ACE implementation from TableCraft's adapter. External Rust applications must
be able to consume `tablecraft-ace` without application dependencies. Rust comes
first, a C ABI follows useful engine functionality, and an optional ODBC interface
is a later capability outside this package count. The Rust engine now has bounded metadata/scalar reading; C ABI and ODBC do not exist yet.
Visual query editor behavior remains in `tablecraft-design`; shared execution
models must not import that editor into the reusable query package.

[VBA compatibility](VBA_COMPATIBILITY.md) makes preservation, documented language
execution and native authoring separate core capability tracks. VBA and macros
have distinct planned owners; shared host contracts remain headless. None of
these language/host packages exists yet.
The [Windows compatibility direction](WINDOWS_COMPATIBILITY.md) supplies per-API
providers for qualified emulation, native bridges and explicit unsupported/blocked
behavior. It does not introduce a Windows binary emulator or a working provider.

The application-compatibility requirements are feature slices within these owners,
not additional crates: [startup/session state](features/ACCESS_APP_STARTUP.md),
[imported ribbons](features/ACCESS_RIBBON_COMPAT.md), [DAO/ADO](features/DAO_ADO.md),
[advanced fields](features/ACCESS_ADVANCED_FIELDS.md) and
[distribution/signature profiles](features/ACCESS_DISTRIBUTION.md).
The [application corpus](testing/ACCESS_APPLICATIONS.md) records the unqualified
candidate and defines separate open/preserve/execute/edit/save evidence.
The [roadmap](ROADMAP.md) maps [analysis tools](features/DATABASE_ANALYSIS.md),
[wizards](features/APP_TEMPLATES_WIZARDS.md), [expression editing](features/EXPRESSION_BUILDER.md)
and [reusable data tasks](features/IMPORT_EXPORT_TASKS.md) onto existing owners.

## Dependency and ownership rules

- Applications compose libraries; libraries never import application packages.
  Domain groups describe ownership rather than imposing an arbitrary numerical
  layering order. Each package dependency must have a concrete, acyclic purpose.
- SQLParser types stay private to `tablecraft-sql`. Future semantic consumers use
  TableCraft syntax. `tablecraft-expressions` owns evaluated Access semantics;
  `tablecraft-query` owns binding and plans. Shared syntax remains in the SQL
  package until an actual consumer establishes a better boundary.
- Shared names, scalar type descriptors and explicit schema policies now live in
  `tablecraft-schema`; see [SCHEMA.md](SCHEMA.md). Richer relational schema belongs
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
  cancellation-aware read retries and format-independent I/O errors; see [IO.md](IO.md).
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
  automation; VBA/macros and those libraries must not import the runtime back. Transaction-scoped
  data-macro integration uses a lower-level contract rather than importing the
  whole application runtime into the Access engine. Tokio remains an async
  executor choice, separate from the Access application runtime.
- Application-domain directory placement does not make pure automation contracts
  or headless macro execution an application-host dependency. Those reusable
  packages may support the standalone engine, but must not import concrete UI,
  runtime, adapter or engine implementations. SQL UDFs call a supplied lower-level
  interface, never the entire VBA/runtime implementation from query code.
- VBA must not import concrete Windows/COM providers. Hosts compose compatibility
  providers through automation contracts; wincompat must not import UI/runtime.
  Treat mechanism, coverage and authorization separately. Denied/unsupported
  operations must not be replaced by successful-looking defaults or automatic
  fallback. Replication must distinguish validated data apply from user-event
  replay and external side effects.
- UI components and designer behavior have separate owners. Shared layout/print
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
catalog consumer and scalar query adapter. `db` contracts, SQLite and compatibility boundaries
remain candidates for implementation with actual consumers. None implies native writes.

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
or a coherent public contract. Follow [DEVELOPMENT.md](DEVELOPMENT.md) for checks
and [IMPLEMENTATION.md](IMPLEMENTATION.md) for observed verification.
