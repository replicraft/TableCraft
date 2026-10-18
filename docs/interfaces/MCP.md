# MCP adapter direction

Selected protocol direction; no MCP library/server/CLI command is implemented and
`rmcp` is not in Cargo.toml/Cargo.lock. Use the official Rust SDK when a working
adapter consumer exists; review its exact release/features/target closure then.
`interfaces/tablecraft-mcp` translates shared services; it must not own project
state, independent permission rules, changeset logic or physical transactions.
Do not create a separate application package or empty library now.

## Service mapping

| MCP surface | TableCraft direction |
| --- | --- |
| Tools | Typed discovery/design, bounded queries, changesets, validation, previews and migrations |
| Resources | Project-scoped manifests, schemas/capabilities, dependencies, definitions and supported API/schema documentation |
| Prompts | Discoverable guided CRUD/report/schema-review/Access-modernization workflows, independent of LLM provider |
| Structured output | Typed results, stable IDs/revisions, semantic diffs, actual check results, compatibility and recovery status |
| Optional Tasks extension | Long-running conversion/import/test/render jobs mapped to owned durable application jobs |
| Negotiated subscriptions | Authorized resource/job changes with bounded delivery and revocation behavior |

Capability/resource/tool names in the proposal are illustrative. Define versioned
schemas with the actual service implementation. A resource URI or job/changeset
handle is not authorization; check principal/project ownership and current policy
on every use. Avoid names as durable object identity. Include compatibility status
in open/operation results so every host obeys the same write/execution gates.

Start with an explicitly invoked local stdio host. The suggested
`tablecraft mcp serve --project ...` spelling is planned, not runnable. Keep normal
CLI/query/engine builds free of MCP network/runtime dependencies through an
explicit future hosting feature/boundary. Remote HTTP follows later authentication,
origin/transport review and project authorization; local access is not permission
to run arbitrary scripts or export all data. Reserve stdout for protocol data.

## Verified protocol baseline

Reviewed 18 October 2026: the official [2026-07-28 specification](https://modelcontextprotocol.io/specification/2026-07-28)
describes stateless requests, per-request capability negotiation, tools/resources/
prompts and opt-in extensions. The [Rust SDK](https://github.com/modelcontextprotocol/rust-sdk)
documents support for this version and prior versions. Negotiate actual support;
do not assume every peer implements the same version or extension.

The [Tasks extension](https://tasks.extensions.modelcontextprotocol.io/) provides
job polling/input/progress/cancellation mechanisms; persistence/recovery of an
application job remains TableCraft's responsibility. Cancellation acknowledges
intent and is cooperative: the job may still commit or finish. Return its actual
terminal/uncertain outcome rather than asserting rollback. Keep explicit owned
handles across requests; connection reuse does not establish user/project identity.
TableCraft's policy requires authorization beyond possessing an opaque handle.

Tool annotations and prompt text do not enforce security. Apply
[shared capabilities](../features/AGENT_NATIVE.md#authorization-and-bounded-access)
to tools, resources, prompts, artifacts and job results. Data remains untrusted
content, separate from instructions and permission grants. Validate with
[deterministic service scenarios](../testing/APPLICATION_SCENARIOS.md) before
claiming tool-level application authoring, not by increasing a tool count.
