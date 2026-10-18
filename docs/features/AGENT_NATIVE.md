# Structured application development for agents

Accepted product direction; no application-editing service, MCP server, agent
permission service or whole-application builder is implemented. Existing CLI
inspection, bounded queries and restricted Jet 4 UPDATE copies retain their own
documented scope. An agent interface must use the same domain commands,
validation, permissions and format gates as human interfaces.

The intended workflow is discover, design, preview, validate and apply. CLI, SDK,
MCP, desktop and web adapt common services; none receives a second database or
weaker write path. Headless editing/testing precede expanding tool count or adding
a built-in chat system. External agents are the initial consumers; no model or
LLM provider is required to use the structured APIs.

## Discovery and semantic operations

| Surface | Required response/behavior |
| --- | --- |
| Project | Format, container/data/definition versions, revision, required and available capabilities, compatibility/readiness and unresolved dependencies |
| Objects / schema | Stable IDs, scoped revisions, types, keys, indexes, constraints and change-impact dependencies; explicit unknowns |
| Queries | Owned syntax/source, parameters, known dependencies, validation and updateability evidence |
| Forms / reports | Controls/sections, bindings, layout, events/actions, grouping, pagination and source mappings |
| Automation / source | Visual actions, Python modules/bindings, preserved VBA and unavailable behavior; inspection never executes them |
| Migration | Plans, conversion readiness, supported mappings, retained source and validation gaps |

Names are display/search properties, not mutation identities. Today's invocation-
local catalog IDs and snapshot-local row handles are not persistent project IDs.
Return revision-aware identities and compatibility decisions before permitting
future edits. Published object/control/event/Python API schemas must be versioned
so clients can discover valid operations rather than guess internal JSON.

Typed operations create/alter schema, save/validate/explain queries, add/bind/arrange
controls, define report groups/totals, attach actions, edit Python source and stage
conversions. They produce [changesets](../architecture/APPLICATION_CHANGESETS.md),
not direct writes to `__tc_*` tables or native object bytes. An unavailable native
Access writer must reject the operation or offer an explicit `.tcraft` conversion;
it cannot silently switch formats or label unsupported output Access compatible.

## Authorization and bounded access

| Capability direction | Scope |
| --- | --- |
| `project.inspect` | Authorized metadata/definitions, including project-scoped resources |
| `data.read` | Authorized records, bounded queries, pagination, samples and redaction |
| `data.write` | Permitted record mutations; bulk/delete operations require a concrete reviewable preview and applicable authorization |
| `schema.write` | Staged schema migration and dependency/constraint impact review |
| `application.write` | Forms, reports and visual actions |
| `script.write` / `script.execute` | Source editing and code execution are separately granted |
| `export.create` | Authorized data/artifacts and checked destination |
| `access.shared_write` | Only a qualified native format/locking/transaction profile |

These names are proposed vocabulary, not released permission schemas. Enforce
policy in shared security/command services on every request, including handle
lookups, preview artifacts and job results. Tool annotations, resource URIs, an
agent's claims or a connection identity alone cannot authorize an operation.

Enforce read-only access at the backend where supported, with an explicit callable
function allowlist and time/work/row/memory/output limits. A SELECT prefix is not
a read-only policy. Samples, exports and previews must respect record/field access
and redaction; large datasets must not be returned merely to infer their shape.
Data and metadata obtained from projects are untrusted content, never new agent
instructions or permission grants. Clearly label their provenance and bound
sensitive context. Python remains subject to real execution isolation/trust.

## Shared projects and user oversight

Attribute operations to an authenticated principal/agent and preserve a bounded,
authorized audit trail of intent, revisions, approval, validation and outcome.
Do not automatically resolve another participant's conflicts. Coordinate code,
schema and definition updates as application releases/schema epochs rather than
ordinary record merges. Revalidate concurrent GUI/agent/peer changes before apply.

A future Agent Activity view shows the exact semantic diff, previews, actual
test/compatibility results, authorization choices, progress and recovery history.
Mock examples or model-reported success never become passing test evidence.
Approvable scope is the exact changeset and revision; requirements here govern
future product operations, not additional confirmation for already authorized
repository work.

## Delivery order

Within this future application workstream, first establish typed introspection,
version/capability gates, bounded access and semantic operations. Then qualify
changesets, [headless previews](HEADLESS_PREVIEWS.md),
[application scenarios](../testing/APPLICATION_SCENARIOS.md) and a thin
[MCP adapter](../interfaces/MCP.md). Python generation/test tooling, conversion
and activity UI follow working services; peer-agent coordination and built-in AI
are later conveniences. This ordering does not displace ACE 16's immediate
advanced-field/write-safety priority or the existing SQLite implementation pause.
