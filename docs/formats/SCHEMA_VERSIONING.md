# Project versions and compatibility gates

Required from the first `.tcraft` release; not implemented. `tablecraft-project`
owns container compatibility and internal version transitions. Each TableCraft
release must publish its readable versions, writable version(s), supported
definition/Python/protocol versions and required-capability policy. A minimum
reader version alone cannot prove compatibility.

| Version | Authority and meaning |
| --- | --- |
| Container schema | `user_version`; internal metadata/resource representation, controlled by TableCraft |
| User data schema | Developer-controlled relational schema; never advanced by a container-only upgrade |
| Application definitions | Shared versioned form/report/query/action representation |
| Python host API | Callable contract and behavior, separate from CPython runtime version |
| Collaboration protocol / schema epoch | Wire/operation semantics and the schema an operation targets |
| Application release | Developer's distributed application, independent of TableCraft executable releases |

Check identity and bounded bootstrap metadata before decoding arbitrary definitions,
loading scripts, returning a writable session or scheduling upgrades. Unknown
required capabilities block execution/writes even when the container version is
recognized. Optional unknown objects may be inertly inspected/preserved only if
the profile explicitly guarantees that behavior; do not silently discard them.

## Open outcomes

| Outcome | Permitted next step |
| --- | --- |
| Current and supported | Validated read/edit session, with execution separately gated by trust/runtime capabilities |
| Readable older version | Read-only access or an explicit available migration plan; no automatic mutation |
| Upgrade required | Show deterministic complete migration path and backup/recovery requirements before authorization |
| Newer / unsupported required feature | Reject editing/execution; bounded inspection only if an explicit compatible reader exists |
| Invalid identity, metadata or history | Fail with an actionable diagnostic; no guessed version or writable fallback |

No silent downgrade. An older version is writable only under an explicitly
qualified writer profile. A writable session must carry the compatibility decision
so CLI, desktop, Python and future web/MCP entry points cannot bypass it.

## Migration invariants

Before an internal upgrade, obtain explicit authorization for the concrete plan,
create and verify a recoverable SQLite backup, and validate preconditions/current
versions. Use deterministic ordered transitions with stable migration IDs and
checksums. Apply schema/metadata/version/history changes atomically where SQLite
permits; perform separately published filesystem work through an explicit recovery
protocol. Check SQLite integrity, foreign keys and application invariants before
success. A checksum mismatch, missing step, cancellation, disk failure or failed
invariant cannot advance the advertised version. Retain a recoverable original.

Do not execute project Python to discover the version or select internal upgrades.
Developer-authored data migrations have a separate approved execution/trust path
in `tablecraft-migrate`; container upgrades never silently change business tables.
Opening/upgrade authorization is a future product interaction, not a requirement
to reconfirm already authorized repository development work.

Header/manifest checks, migration history, resource hashes and required-feature
invariants must agree. Read compatibility and write compatibility are distinct.
See [project lifecycle](../architecture/PROJECT_LIFECYCLE.md) and
[schema epochs](../features/COLLABORATIVE_SCHEMA_EVOLUTION.md).

Primary references: [user_version](https://www.sqlite.org/pragma.html#pragma_user_version),
[schema_version warning](https://www.sqlite.org/pragma.html#pragma_schema_version),
[integrity_check](https://www.sqlite.org/pragma.html#pragma_integrity_check),
[foreign_key_check](https://www.sqlite.org/pragma.html#pragma_foreign_key_check).
