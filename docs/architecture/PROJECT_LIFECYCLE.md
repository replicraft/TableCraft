# Project lifecycle

Target application contract; project sessions and this lifecycle are not implemented.
`tablecraft-project` classifies format compatibility before `tablecraft-runtime`
can create an editable/executable application session. All hosts share this path.

1. Open inertly under bounded I/O: identify document/backend, inspect bootstrap
   versions and required capabilities without Python, plugins, dependency installs
   or automatic external connections.
2. Validate compatibility, metadata, resources and references. Return an explicit
   [open outcome](../formats/SCHEMA_VERSIONING.md#open-outcomes); expose available
   inspection, recovery and migration choices rather than a writable connection.
3. If an upgrade is chosen, prepare/authorize a concrete plan, verify a consistent
   backup, apply deterministic transitions and revalidate. A failed upgrade returns
   a recoverable state, never an ambiguous normal editing session.
4. Create a session only for its qualified capabilities. Resolve external sources
   through authorized connection services. Negotiate execution environment, script
   trust and permissions separately from file readability; data/design inspection
   must remain useful when Python cannot run.
5. Execute commands through [application services](APPLICATION_EXECUTION.md).
   Query/data sources remain lazy, bounded and cancellable. UI caches/render state
   are derived and never substitutes for canonical definitions or preserved bytes.
6. Save through the owning backend/project writer, applying preservation and
   version gates. Pack/export/backup through consistent publication, not a raw
   copy of a live SQLite main file. Close/drain workers, report cleanup failures
   and retain recovery information for pending/uncertain operations.

The future project health view consolidates unsupported native objects,
unavailable event handlers, migration warnings, broken links, environment/trust
issues, version incompatibilities and integrity failures. Diagnostics identify the
affected object/operation with recovery choices through authorized host output;
payload-free library tracing remains unchanged.

Application releases and TableCraft executable upgrades have independent lifecycle
decisions. A script/form update is executable distribution with its own permissions,
compatibility and data-migration plan, not ordinary record synchronization.
