# Native TableCraft project format

Accepted requirements; no `.tcraft` writer/reader or released schema exists yet.
Ordinary read-only SQLite snapshots do not implement this format. The exact
SQLite application ID, DDL, encoding limits and first version must be reviewed
with the first working format slice; example numbers in proposals are not assigned.

## Identity and contents

`.tcraft` is a SQLite database containing relational data, versioned application
definitions, original Python source, event bindings, project/dependency manifests,
resources and optional replication metadata. Reserve `__tc_*` for internal tables;
validate reserved-name collisions during creation/import. Use `application_id`
for a selected unique document ID and `user_version` for the authoritative
container version. Never use SQLite's internal `schema_version` as our version.
An extension or table prefix alone is insufficient identification.

The logical manifest must identify a persistent project UUID, application release,
data-schema and definition versions, required Python host API/capabilities,
supported execution environments and optional collaboration protocol/schema epoch.
Record resource hashes, source-object provenance, external connection declarations
and migration history. Validate agreement with header identity/version; refuse
writes on inconsistency. Do not store credentials, installed environments, runtime
binaries, caches or machine-specific paths in the portable document.

Embedded data/resources make a self-contained profile. A project linked to an
external Access/backend database must declare that dependency and cannot claim
that the document alone contains all required records. Project format and data
backend remain separate concepts. Ordinary external `.sqlite` databases can still
be opened without becoming TableCraft projects. Python remains outside MDB/ACCDB;
Access cannot execute native TableCraft handlers.

## Packed and unpacked equivalence

`.tcraftproj/` is a developer workspace containing a manifest, dependency lock,
SQLite data, Python/SQL source, definitions and resources as regular files. Exact
layout is unreleased. Both representations serialize one canonical versioned
application model through `tablecraft-project`, preserving stable object IDs,
source bytes, versions, dependencies, opaque archives and resource hashes.
Detailed domain definitions remain owned by their subsystems.

Pack/unpack must have deterministic semantic ordering and text encodings, with
round-trip verification. Deterministic SQLite file bytes are not assumed. Validate
paths, size limits, duplicate IDs, hashes and references before publishing an
output. Refuse traversal, escaping symlinks and ambiguous duplicate resources.
Packing a stale workspace cannot overwrite a newer document without explicit
reconciliation against its recorded base revision. Never execute scripts or
install dependencies as a side effect of packing, unpacking or inspection.

## Consistent publication

A portable document is conceptually one file; active SQLite journaling can involve
sidecars. Export/backup must use a consistent SQLite snapshot and verify it before
publishing; copying an active main database file alone is not the backup protocol.
Define atomic publication, durable namespace operations and interrupted-export
cleanup for each supported platform. Embedded resources and definitions should
commit with their associated metadata/data changes when they share one database.
Do not default to a ZIP containing a live SQLite database.

Acceptance requires [version gates](SCHEMA_VERSIONING.md),
[lifecycle checks](../architecture/PROJECT_LIFECYCLE.md), bounded/lazy loading and
[cross-version proofs](../testing/PROJECT_COMPATIBILITY.md). File-open must not
read every table, parse every Python module or render all forms eagerly.

Primary references: [SQLite application files](https://www.sqlite.org/appfileformat.html),
[application ID](https://www.sqlite.org/pragma.html#pragma_application_id),
[WAL](https://www.sqlite.org/wal.html),
[consistent backup](https://www.sqlite.org/backup.html).
