# Compatibility-driven roadmap

Adopted priority direction from the 18 October 2026 application review; this is not
implemented functionality or a release schedule. Current behavior is in
[IMPLEMENTATION.md](IMPLEMENTATION.md): core values, bounded Access syntax, initial
scalar evaluation and a headless CLI. Native databases, VBA/macros, forms/reports,
collaboration and UI hosts remain unimplemented.

## Next implementation and measurement

Prioritize measured application compatibility over speculative package expansion.
The next native slice is the [standalone engine's bounded read-only format/header
inspection](NATIVE_ENGINE.md#integration-order-and-acceptance), with synthetic
malformed/limit tests and inert inspection of disposable fixture copies. It must
work outside TableCraft and cannot imply catalog reading or native write support.
Query/expression work can advance independently with real schema/binding consumers.

The [application corpus](testing/ACCESS_APPLICATIONS.md) has one reviewed MIT
candidate and no native results. Add reproducible minimal fixtures, then
representative applications, recording open/preserve/execute/edit/save-and-reopen
separately. Object inventory and a runnable native comparison harness should drive
subsequent work. Architecture coverage is not a readiness score.

## Priorities

These order risk/value within working slices. P0 does not require every object
model to be complete before a limited read-only milestone can ship.

| Priority | Requirements and specifications |
| --- | --- |
| P0: compatibility correctness | [DAO/ADO](features/DAO_ADO.md), [advanced fields](features/ACCESS_ADVANCED_FIELDS.md), [distribution/signatures/version profiles](features/ACCESS_DISTRIBUTION.md), [startup](features/ACCESS_APP_STARTUP.md), required validation/data macros under [VBA](VBA_COMPATIBILITY.md) |
| P1: faithful application runtime | [Imported ribbons](features/ACCESS_RIBBON_COMPAT.md), AutoKeys/session state, [Expression Builder](features/EXPRESSION_BUILDER.md), form events, reports and [linked sources](features/IMPORT_EXPORT_TASKS.md) |
| P2: developer workflows | [Dependencies, Compare, Documenter, search and application tests](features/DATABASE_ANALYSIS.md), [wizards/application parts](features/APP_TEMPLATES_WIZARDS.md), saved import/export jobs |
| P3: collaboration experience | Invite/peer lifecycle, offline status, conflicts/resolution history, consistent backup/restore, history views and qualified collaborative design |
| P4: expansion | Additional formats, external integrations, wider API providers, application/web publishing and agent-driven authoring |

Documenter, Compare, search and application tests lead developer tooling. Native
mutation/authoring require stronger evidence than inspection. Unsupported behavior
must fail or be reported at its actual capability boundary.

## Deeper qualification of existing directions

| Direction | Evidence needed as implementation arrives |
| --- | --- |
| Public engine | Rust API/version/features, connection options, standalone examples/tests; C ABI then independent ODBC milestone |
| VBA/macros | Language conformance, reference resolution, compile/runtime diagnostics, event/error/transaction traces and native authoring |
| Windows providers | Registry/coverage manifest, qualified signatures/dispatch, virtual environments, denied/unsupported outcomes |
| Native shared-file | Named SMB implementations, mixed Access/ACE versions, contention, crash/recovery and reconnect laboratory runs |
| Forms | Control/property/event matrix, binding, focus, cancellation, subforms and version-specific behavior |
| Reports | Printer metrics, paper/labels, fonts/fallback, grouping/pagination and build-specific design limits |
| Linked sources | SharePoint/ODBC/server/driver profiles, authentication, relinking, credential migration and offline errors |
| Browser | Storage/permission/offline limits and feature qualification; no initial direct simultaneous ACCDB editing promise |

[Native engine](NATIVE_ENGINE.md), [VBA](VBA_COMPATIBILITY.md) and
[Windows compatibility](WINDOWS_COMPATIBILITY.md) remain the canonical owners;
do not duplicate them under alternate proposal filenames. Feature slices fit the
existing [domain map](ARCHITECTURE.md). This review adds no package/dependency names
or empty scaffolding.

## Collaboration and browser boundaries

For TableCraft-native P2P projects, show invite authorization, peer/device identity,
revocation, connection/offline state, sync progress, concurrent values, resolution
history and backup/restore state. Define which changes peers acknowledged; a
disconnected peer cannot justify a global "all synchronized" claim. Preserve
conflicting values until explicit resolution. Resolving data conflicts does not
undo external actions. Collaborative design needs object/dependency merge rules.

Native Access shared-file mode shows connections, locks/contention and recovery.
It uses engine locking/transactions, not CRDT conflict semantics. P2P apply cannot
replay external effects or bypass required database validation.

Before browser delivery, specify supported file/replica workflows, permission
denial/revocation, persistence/quota/eviction, backup/export, offline edits and
reconnect/conflicts. Do not assume native locks, COM/DLL access, unrestricted
background tasks or durable browser storage. Web publishing is a later deployment
capability with an explicitly supported application subset.

## Scope limits

Keep Excel/Power Query reimplementation, a general spreadsheet editor, a full
Windows/Wine emulator, an unrelated new relational storage engine, a cloud database
service and a general website builder outside this critical path. The Rust-native
Jet/ACE implementation remains required. Arbitrary third-party ActiveX is long-term
research. AI assistance/agent authoring follows reliable CLI/MCP operations,
capability checks and reviewable changes; an MCP tool list cannot prove native
authoring works.

Adopt data adapters progressively under [import/export profiles](features/IMPORT_EXPORT_TASKS.md).
Historical formats, compiled projects and native signing retain separate fidelity
levels. Public progress claims must cite observed results and limitations.
