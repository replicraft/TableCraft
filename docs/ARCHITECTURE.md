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
│   └── tablecraft-core/            # exact values, scoped IDs, budgets, errors
└── query/
    └── tablecraft-sql/             # bounded Access syntax, owned model, formatting
```

The CLI depends on `tablecraft-sql`. Both libraries build independently; neither
depends on an application or on the other library. There are three Cargo
packages. This reorganization adds no third-party dependencies or runtime
features. The CLI command remains `tablecraft query parse "<SQL>"`.

The former `tablecraft-query` parser package is now `tablecraft-sql`; Rust imports
use `tablecraft_sql`. Its public syntax types and parsing behavior remain in that
package. It owns the private SQLParser adapter and TableCraft's expression,
SELECT, statement and schema-definition **syntax**. These syntax nodes do not
constitute bound relational schema, expression evaluation or execution plans.
See [SQL_PARSER.md](SQL_PARSER.md) for implemented coverage and limitations.

## Target domain map

Only `tablecraft-core` and `tablecraft-sql` exist as library packages today. The
remaining names identify intended ownership when working consumers justify their
introduction. The map contains 31 prospective library packages across 12 groups;
the count is descriptive, not a delivery target.

| Domain | Package ownership |
| --- | --- |
| `foundation/` | `tablecraft-core`: shared values, IDs and small contracts; `tablecraft-schema`: relational schema, keys, relationships, indexes and schema differences; `tablecraft-model`: shared application-object metadata, identity and dependencies |
| `platform/` | `tablecraft-platform`: host capabilities and platform integration; `tablecraft-io`: random-access I/O, low-level locks and durability interfaces; `tablecraft-crypto`: reusable cryptographic integration when a real consumer needs it |
| `storage/` | `tablecraft-db`: backend connection, transaction, cursor and capability contracts; `tablecraft-access`: native MDB/ACCDB engine; `tablecraft-sqlite`: SQLite adapter and local metadata; `tablecraft-connectors`: linked and remote database adapters |
| `query/` | `tablecraft-sql`: parsing, formatting and owned syntax; `tablecraft-expressions`: Access coercion, operators, built-ins and expression evaluation; `tablecraft-query`: binding, planning, execution coordination, updateability, designer query representation and supported translation |
| `application/` | `tablecraft-forms`: control bindings, form layout, validation and events; `tablecraft-reports`: sections, grouping, pagination and export; `tablecraft-automation`: macro, scripting and VBA compatibility behavior; `tablecraft-runtime`: application sessions, navigation, commands and event services |
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

## Dependency and ownership rules

- Applications compose libraries; libraries never import application packages.
  Domain groups describe ownership rather than imposing an arbitrary numerical
  layering order. Each package dependency must have a concrete, acyclic purpose.
- SQLParser types stay private to `tablecraft-sql`. Future semantic consumers use
  TableCraft syntax. `tablecraft-expressions` owns evaluated Access semantics;
  `tablecraft-query` owns binding and plans. Shared syntax remains in the SQL
  package until an actual consumer establishes a better boundary.
- Relational schema belongs to `tablecraft-schema` when introduced. DDL syntax
  stays in `tablecraft-sql`; conversion to schema requires explicit validation.
  Shared application-object metadata belongs to `tablecraft-model`, while forms,
  reports and automation own their domain behavior. Avoid duplicate definitions
  or a central model package that imports the entire application stack.
- Keep the Access engine together initially, with internal modules for format,
  pages, allocation, catalog, records, indexes, long values, objects, locking,
  transactions, recovery and format-specific crypto as they become real. Extract
  packages only after cache, transaction and mutation ownership is understood.
- Low-level file-lock primitives may belong to `tablecraft-io`; Jet/ACE shared-file
  coordination belongs to `tablecraft-access`. Peer replication has separate
  semantics and belongs to the collaboration domain.
- Keep CRDT logic testable without networking. Transport owns connections; sync
  composes transport, replicated operations and durable storage. These packages
  do not implement native Access shared-file locking.
- The application runtime implements narrow interfaces consumed by forms and
  automation; those libraries must not import the runtime back. Transaction-scoped
  data-macro integration uses a lower-level contract rather than importing the
  whole application runtime into the Access engine. Tokio remains an async
  executor choice, separate from the Access application runtime.
- UI components and designer behavior have separate owners. Shared layout/print
  primitives should work headlessly; native/browser integration stays explicit.
- Access/DAO oracle dependencies stay in engineering tooling, outside production
  library and CLI dependency closures. Native compatibility requires observed
  Access results, not merely successful parser or cross-platform builds.

## Progressive extraction

The first step is complete: relocate core under `foundation/`, relocate and rename
the parser under `query/`, and update its CLI consumer, CI and documentation.

Next, introduce schema and semantic packages with their first working behavior:
schema contracts with real consumers, expression evaluation with independently
tested Access semantics, and query binding/planning against those contracts.
Native format research and comparison tooling can proceed alongside that work.
The `db`, Access, SQLite, model and compatibility boundaries are candidates for
early implementation, not mandatory empty packages or promises of native writes.

Application, presentation, collaboration and lifecycle packages follow their
actual consumers. A small desktop milestone may introduce working UI components
early; native multiuser compatibility experiments also need not wait for peer
replication. Stage order guides extraction and does not gate unrelated research.

Use modules for feature slices within a package. Promote a slice to a crate when
it has useful dependency isolation, independent consumers, platform/testing needs
or a coherent public contract. Follow [DEVELOPMENT.md](DEVELOPMENT.md) for checks
and [IMPLEMENTATION.md](IMPLEMENTATION.md) for observed verification.
