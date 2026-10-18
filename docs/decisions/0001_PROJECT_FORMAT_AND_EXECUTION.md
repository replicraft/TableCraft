# Native projects, scripting and transaction ownership

Accepted direction, 18 October 2026. This records product and ownership decisions;
the native project format, Python host and migration system are not implemented.

- New projects default to a SQLite-backed `.tcraft` application document. An
  unpacked `.tcraftproj/` workspace represents the same versioned model.
- MDB/ACCDB remain first-class native Access formats, using standalone
  `tablecraft-ace`. The primary implementation target remains 100% ACE 16
  compatibility, subject to the explicit VBA/Windows deferrals and actual evidence.
- Python is the primary and initially only general-purpose language for native
  projects. This supersedes the earlier TypeScript/JavaScript-default proposal.
  Rust owns database behavior and application services; visual actions remain
  language independent. CPython/PyO3 is the selected implementation direction,
  preferably hosted in a helper process; no interpreter dependency is adopted yet.
- `tablecraft-project` will own format identity, serialization, compatibility and
  internal migrations. The SQLite adapter owns connections and SQLite behavior.
  `tablecraft-model` will own shared versioned definitions as real consumers emerge;
  detailed form/report behavior stays with its domain owner.
- Every released native document must carry a container version and required
  capabilities, checked before writes or script execution. Internal migrations,
  developer data migrations and application releases are separate operations.
- Transactions are implemented by their engines and adapted through shared
  `tablecraft-db` contracts when working consumers justify them. There is no new
  `tablecraft-transactions` crate. A later cross-backend coordinator needs a
  separate guarantee/failure model; CRDT replication is not atomic coordination.
- Access-to-`.tcraft` application conversion is a first-class future workflow,
  ahead of native-project export back to Access. Preserve unresolved source and
  disclose unavailable behavior. PHP/TypeScript code generation is later work,
  independent of the native scripting language; optional AI is not a prerequisite.

These choices do not resume broad SQLite implementation ahead of the current
ACE 16 work. No empty project, Python, transaction or code-generation packages
are introduced. Proofs must precede release claims; architectural scores in the
supplied proposals are not implementation metrics.

Owning contracts: [format](../formats/TCRAFT_FORMAT.md),
[versioning](../formats/SCHEMA_VERSIONING.md),
[execution](../architecture/APPLICATION_EXECUTION.md),
[transactions](../architecture/TRANSACTIONS.md),
[Python](../features/PYTHON_SCRIPTING.md) and
[conversion](../features/PROJECT_MIGRATIONS.md).
