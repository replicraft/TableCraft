# Convert to TableCraft and application upgrades

Accepted feature requirements; conversion and project migrations are unimplemented.
`tablecraft-migrate` will own analysis, schema/data/object transforms, developer
data migrations, source mappings and reports. `tablecraft-project` owns internal
container migrations and packed/unpacked serialization. Neither needs a separate
schema-migration crate now. Keep conversion headless and callable by all hosts.

## Access application conversion

ACCDB/MDB to `.tcraft` is a first-class future workflow, ahead of native-project
export to Access. Create a fresh destination; keep the original bytes unchanged.
Analyze objects/dependencies before selecting conversions, then convert supported
schema/records, queries, forms/subforms, reports and automation, and validate the
result. A form is ready only when its sources, controls and required behavior are
qualified. Optional Windows SaveAsText extraction is engineering tooling; the
long-term native extractor must not require an Access installation.

| Source | Required treatment |
| --- | --- |
| Types, keys, constraints, defaults, AutoNumber, relationships | Explicit semantic mappings and validation; retain exact Currency/Decimal/Int64/date precision and NULL distinctions |
| Attachments / multivalued fields | Structured/normalized child data with source identity and UI semantics, not guessed scalar columns |
| Saved queries / expressions | Shared owned query model and qualified translation; syntax rewriting alone is not equivalent behavior |
| Forms / controls | Preserve layout, sizes, fonts, tab order, bindings, hierarchy and source mappings; expose unsupported ActiveX/control placeholders |
| Reports | Preserve printable geometry, grouping, totals and pagination separately from responsive screen layout |
| Visual / table data macros | Translate supported semantics; unresolved required write rules block affected operations |
| VBA, compiled content, references | Preserve original obtainable source/opaque bytes and dependency metadata; do not promise decompilation or execution |
| Event bindings / external resources | Keep unresolved dependencies visible; a button with unavailable code must report that condition |
| Linked tables | Explicit relink/import plan, credential separation and declared availability/capabilities |

Offer full conversion and later frontend-only conversion with a linked Access
backend. The latter depends on qualified native shared-file writes and does not
inherit CRDT semantics. During staged cutover declare one authoritative store;
two writable database copies do not synchronize automatically.

Every conversion emits per-object source IDs/provenance, converted/preserved/
unsupported state, dependency blockers and performed/unrun validation. Classify
the resulting application as fully functional only for verified required
workflows; otherwise report partially functional, data-only or analysis-only.
Archiving VBA never establishes behavioral equivalence. Retain unresolved source
so future RustVBA/manual modernization does not require repeating the migration.

Conversion must be repeatable and resumable from a recorded source revision.
Selective regeneration detects user edits and conflicts before overwriting them.
Faithful layouts and explicit modernization are separate choices. Future code
generation in `tooling/tablecraft-codegen` consumes the shared model, not another
independent Access parser. Python is the native logic target; web TypeScript/PHP
targets come later. Generated sources need dependencies, migrations, tests and
source mappings. Deterministic supported transformations precede optional reviewed
AI assistance; unknown logic must never become plausible invented behavior.

## Application releases

A developer application upgrade bundles versioned definitions/source/resources
and explicit data migrations. It is separate from a TableCraft runtime update and
from internal container migration. Plan compatibility, authorization, backup,
transaction/recovery and [peer schema epochs](COLLABORATIVE_SCHEMA_EVOLUTION.md)
before activation. Do not overwrite records when deploying new forms/scripts.

Phase acceptance proceeds through relational data, saved queries, forms, reports,
macro/event behavior and linked-backend/application diagnostics. Every phase has
a feature matrix; no phase title proves all objects supported. Check exact values,
rows, keys/relationships, query results, validation, events and user workflows;
compare report layout/pagination against recorded native output where available.
VBA translation remains later work, independent of the preservation requirement.

Reference: [Access SaveAsText](https://learn.microsoft.com/en-us/office/vba/api/access.application.saveastext).
[AccessClone](https://github.com/kentyler/accessclone) is a modernization design
reference with project-described capabilities, not verified complete conversion
or a production dependency; audit exact licenses before reuse.

Agent conversion planning/validation uses the same inert analysis, version gates,
source mappings and disposable verification. Expose unresolved VBA as unavailable
behavior; optional suggested Python replacements are changes to review/test, not
verified conversion. All agent apply requests follow the shared
[changeset lifecycle](../architecture/APPLICATION_CHANGESETS.md).
