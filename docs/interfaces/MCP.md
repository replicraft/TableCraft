# Local MCP host and retained direction

`apps/tablecraft-mcp` is a local stdio application using the official `rmcp` SDK,
Tokio transport and Schemars-derived input schemas. It adapts the same
`tablecraft-runtime` document, builder, review, save and recovery services as the
desktop. It imports neither desktop nor egui and implements no native write
planner. Its format/operation limits remain those of the shared runtime.

```sh
cargo run -p tablecraft-mcp -- --root /absolute/path/to/databases
```

Configure an MCP client to launch that executable with these arguments. Standard
output contains only MCP protocol messages. This is an explicitly invoked local
host, without HTTP transport, remote authentication, project scripting or VBA.
The required root scopes filesystem arguments; they must be absolute and their
canonical target (or existing parent for new files) must be inside it. Canonical
paths are passed to the runtime. This is not an operating-system sandbox against
another process concurrently replacing filesystem ancestors; native publication
and exclusive-session checks still apply.

## Current service workflow

`tools/list` advertises exact input schemas generated from typed Rust requests.
The same request types validate arguments before dispatch, borrowing input text.
Unknown fields, wrong types and fabricated capability payloads are refused.
One dedicated session thread serializes calls; blocking native services do not
run on Tokio's transport thread. Input is bounded by the SDK's 16 MiB line limit
and a separate 8 MiB tool-argument limit. Each process owns one document session.

1. `open_database` takes `path` and optional `password`. Its catalog contains the
   actual document revision, local/system table facts and saved-query identities.
2. `read_rows`/`describe_table` use that revision. `inspect_query` returns an opaque
   inspected query ID for read execution or reviewed INSERT/UPDATE/DELETE.
3. `begin_edit`/`begin_insert` return native drafts. Review literal inputs with
   `review_edit`/`review_insert`; deletion uses `review_delete`. NULL and omitted
   insertion fields remain distinct. Display text never becomes automatic input.
4. Each review returns a `review_id` and matching `apply_operation`. Apply consumes
   the retained sealed capability; JSON cannot supply a native plan. A failed
   worker start retains the capability. The runtime rechecks source drift and
   actual source eligibility before writes. The journal location derives from
   the reviewed source; callers cannot redirect it through an ignored argument.
5. Poll `save_status` with its returned `save_id`. Cancellation is cooperative.
   Recovery/release use the retained owner and return a new actor ID. Acknowledge
   the actual terminal report; schema/query/settings saves reopen using their
   retained complete-image witness before publishing a replacement catalog.
   A failed guarded reopen retains that same witness for explicit retry.

The host exposes qualified table creation/rename/deletion, field
addition/rename/deletion, saved-query SQL and literal-expression editing, Name
AutoCorrect options, action queries, blank database creation and checked restart
recovery. These are bounded implementations, not arbitrary DDL or general Access
compatibility. Table deletion currently requires a qualified empty source.
Find/Replace retains an actual runtime occurrence under an opaque `find_id`;
replacement creates an ordinary edit draft and follows the same review/apply flow.
Filtered/sorted Find retains its real view for continuation and replacement.
Formatted Find requires explicit locale/date/time/currency options; it does not
infer query collation or edit syntax from display preferences. Filter, sort and
totals tools use actual runtime handles. Visual query tools import/build/rebuild
the shared checked model against native metadata; execution remains explicit.

Blank creation uses `create_database`, `creation_status`,
`acknowledge_creation`, then `open_created_database` with the real creation ID.
Existing destinations are never overwritten. `review_table_creation` consumes a
checked headless table draft and retains the native capability only after the
runtime qualifies it.

Save and creation reports remain owned until acknowledged. Losing a client or a
response does not prove rollback. Unknown source outcomes block further edits.
Review/query/find handles are session-local, revocable and superseded by current
state; they are not durable identifiers or permission grants.

`session_status` discovers real save/creation owner IDs after a lost response.
SDK request cancellation revokes queued work or active read publication, and an
interrupted read requires explicit reopening. It never silently cancels an
accepted native save or creation; resolve those through their actual status and
explicit cancellation/recovery tools. Cancelled guarded reopening retains the
same accepted witness and report for retry.

The remaining sections describe future agent services, not capabilities supplied
by this host merely because tools are discoverable.

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

The suggested `tablecraft mcp serve --project ...` spelling remains planned; use
the separate local executable above. Normal CLI/query/engine builds remain free
of MCP transport dependencies. Remote HTTP follows later authentication,
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
[deterministic service scenarios](../../tests/reports/reading/APPLICATION_SCENARIOS.md) before
claiming tool-level application authoring, not by increasing a tool count.
