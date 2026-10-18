# Compatibility-driven roadmap

Priority decision updated 18 October 2026. The native Jet/ACE engine, Access
interoperability, visual application builder and SQLite lead implementation.
This orders work without imposing a strict sequence; UI and compatibility testing
can progress alongside storage. [IMPLEMENTATION.md](IMPLEMENTATION.md) records
working behavior: bounded native metadata/scalar/indexed reads, owned Access SQL,
read-only retrieval, exact values, generic I/O and a headless CLI. Writes, native
shared-file coordination, SQLite, forms/reports and UI hosts remain unimplemented.

## Next implementation and measurement

Next: qualify native index constraint semantics against independent fixtures,
keeping native Access conformance separate. Native schema allocation paths are
fallible and sorting avoids scratch allocation. Read-only consistency reports combine
sequential rows and eligible indexes with explicit coverage gaps. Usage-map ordering
and chained definitions have bounded corruption/budget and Jet 4/ACE evidence.
Writes still require preservation, required macro semantics, transactions and
recovery qualification; successful reads do not supply those guarantees.
Native Int32 seeks and [safe predicate pushdown](QUERY_EXECUTION.md#safe-source-constraints)
now work through explicitly selected indexes. Schema discovery is separate from
row loading. Physical traversal stays in ACE, source adaptation in Access and
backend-neutral planning in query. Automatic index choice and TOP pushdown remain
future work.

Continue original minimal fixture recipes and independent-reader comparisons;
add real Microsoft Access comparisons as the oracle environment becomes available.
The [application corpus](testing/ACCESS_APPLICATIONS.md) still has one candidate
and no qualified application open/preserve/edit/save-and-reopen baseline. A Jackcess
comparison is independent evidence, not Microsoft Access conformance.

## Implementation priorities

| Order | Direction | Scope |
| --- | --- | --- |
| 1 | Native Jet/ACE engine | MDB/ACCDB reading, writing, indexes, transactions, unknown-object and existing VBA preservation |
| 2 | Microsoft Access collaboration | Native shared-file locking, concurrent writes, durability and recovery; actual mixed-client tests |
| 3 | SQLite and query engine | Tables, Access SQL, relationships, expression semantics and explicitly qualified translation |
| 4 | Custom Office-style desktop UI | Ribbon, navigation pane, datasheets and visual query builder; accessibility from the first custom controls |
| 5 | Forms and reports | Designers, binding, headless layout/rendering and native object interoperability |
| 6 | P2P / CRDT collaboration | Offline-first replication, conflict handling, deterministic merges and constrained relational apply |
| 7 | Broader tooling | CLI expansion, web frontend, plugins, MCP, migrations and diagnostics with real consumers |

The current CLI and diagnostics continue supporting engine work. Preserve useful
working tools while following this order. Feature contracts cover [advanced fields](features/ACCESS_ADVANCED_FIELDS.md),
[distribution/signatures](features/ACCESS_DISTRIBUTION.md), [startup/session state](features/ACCESS_APP_STARTUP.md),
[imported ribbons](features/ACCESS_RIBBON_COMPAT.md), [expression editing](features/EXPRESSION_BUILDER.md),
[linked/saved data tasks](features/IMPORT_EXPORT_TASKS.md), [analysis tools](features/DATABASE_ANALYSIS.md)
and [wizards](features/APP_TEMPLATES_WIZARDS.md). They are requirements, not completed features.

## Native format expansion

Retain Jet 4/ACE 12 as the current bounded read baseline and explicitly target
ACE 14, ACE 15, ACE 16 and Jet 3 compatibility. Qualify newer unencoded ACE reads
alongside field/relationship metadata work, then extend legacy Jet 3 reading.
Acquire original minimal fixtures with recorded provenance and compare catalog,
scalar, text and index behavior before enabling each profile. Access engine
releases do not map one-to-one to file layouts; the
[format target matrix](NATIVE_READING.md#compatibility-targets) records this
distinction and current limitations.

Jet 2 is an optional read-only feasibility study after the main read targets.
Establish fixtures, layout references and a comparison environment first; retain
it as deferred research if reliable extraction requires substantial work. It does
not gate the main engine, safe writes for qualified profiles or the UI.

Writes require separate preservation, constraints, macro, transaction and recovery
evidence for each profile. Encryption and advanced fields are qualified separately;
neither newer headers nor successful scalar reads imply support for every feature.

## VBA deferral

| Capability | Decision |
| --- | --- |
| VBA parser/interpreter, execution, new/edited module authoring, editor and debugger | Deferred to future standalone RustVBA work |
| DAO/ADO interfaces specifically for VBA | Deferred; backend-neutral data/cursor work remains useful independently |
| Windows API emulation, COM and Wine compatibility | Deferred |
| RustVBA integration | Deferred until a usable standalone project release exists |
| Preserving existing VBA modules/projects through unrelated edits | Required before qualifying native saves |
| Detecting VBA-dependent buttons, events, startup, ribbon and query features | Required; report unavailable actions and unresolved dependencies |
| Access table data macros | Retained database compatibility scope; implement required semantics or block affected writes |

[VBA_COMPATIBILITY.md](VBA_COMPATIBILITY.md) owns preservation/detection and deferred
integration. [WINDOWS_COMPATIBILITY.md](WINDOWS_COMPATIBILITY.md) retains provider
research only. Remove `tablecraft-vba`, `tablecraft-wincompat` and `tablecraft-wine`
from active implementation; reserve `tablecraft-vba-host` in docs only. Keep
`tablecraft-macros` and `tablecraft-automation` for Access-native macros, event dispatch
and non-VBA actions. No empty crates or speculative dependencies are added.

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
