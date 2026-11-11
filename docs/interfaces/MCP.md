# Local MCP host and retained direction

`apps/tablecraft-mcp` is a local stdio application using the official `rmcp` SDK,
Tokio transport and Schemars-derived input schemas. It adapts the same
`tablecraft-runtime` document, builder, review, save and recovery services as the
desktop. It imports neither desktop nor egui and implements no native write
planner. Its format/operation limits remain those of the shared runtime.

```sh
cargo run -p tablecraft-mcp -- --root /absolute/path/to/databases
```

This invocation is read-only. Grant only the write families needed by a trusted
local client at process startup:

| Flag | New authority |
| --- | --- |
| `--allow-data-write` | Apply reviewed row edits and INSERT/UPDATE/DELETE queries |
| `--allow-schema-write` | Create a database; apply reviewed table, field and database-option edits |
| `--allow-application-write` | Apply reviewed saved-query SQL or literal-expression edits |
| `--allow-maintenance` | Start checked recovery from a reserved native journal |

Tool discovery filters physical write operations using the same immutable grants
checked by the session actor. Reviews remain inert and may be inspected without a
write grant; a review handle is not permission to apply it. Tool arguments cannot
enable a grant. Cancel/recover/release/acknowledge and read-only continuations still
require their actual retained operation owner. These local process grants do not
implement remote principals, persistent changeset authorization or desktop IPC.

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
and a separate 8 MiB tool-argument limit. Logical result JSON is capped at 4 MiB
before the SDK allocates its escaped text fallback; oversized results return an
explicit output-limit error without truncated records or payload diagnostics.
Each process owns one document session.

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

### Per-run read-query allowances

`run_query` and `run_saved_query` accept an optional `options` object. Missing or
null options, blocks and fields retain the shared runtime defaults. Overrides
are captured by that one read command; they do not change later requests,
inspected query/object identities, source permissions or native write limits.
Action-query review tools reject this read-only `options` field.

The object can override `snapshot_rows`, `batch_rows`, and the independent
`native`, `execution`, `comparison` and `presentation` pools. Each pool can supply
`bytes`, `rows` and `steps`. These are cumulative allocation/work allowances,
not resident-memory measurements. Runtime constructors validate every override
before database dispatch; invalid limits are refused rather than clamped.

| Pool | Default bytes | Default rows | Default steps |
| --- | ---: | ---: | ---: |
| Native | `u64::MAX` | `u64::MAX` | `u64::MAX` |
| Execution | `u64::MAX` | `u64::MAX` | `u64::MAX` |
| Comparison | `u64::MAX` | 0 | `u64::MAX` |
| Presentation | `u64::MAX` | 0 | `u64::MAX` |

The complete-table snapshot defaults to `u32::MAX` rows with 256-row fetches.
Explicit snapshot and batch row allowances accept positive `u32` values. Pool
bytes and steps accept positive `u64` values; native/execution rows must be
positive and comparison/presentation rows must remain zero. These are request
and accounting representations. Actual validated rows grow under the selected
cumulative allowances; a large requested row count does not preallocate that
many rows. Increasing one allowance does not increase another or qualify unsupported
SQL, native types, indexes or file formats. The MCP's separate 4 MiB logical
result cap still applies.

For a larger supported read, an existing valid run request can include:

```json
{
  "options": {
    "snapshot_rows": 200000,
    "native": {"bytes": 268435456, "rows": 200000, "steps": 100000000},
    "execution": {"bytes": 268435456, "rows": 1000000, "steps": 100000000},
    "comparison": {"bytes": 268435456, "steps": 100000000}
  }
}
```

The run still requires its actual current document revision and inspected query
ID, or the exact published saved-query object ID and name. No source path,
collation substitution, capability grant or caller-authored plan can be supplied
through these controls.

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

After an acknowledged INSERT failure, `retry_insert` can only verify the retained
complete original image and issue a fresh insertion draft. It never replays old
inputs or a consumed review. `discard_insert_retry` drops that advisory context.
Unknown source images remain refresh-required; an actual uncertain or observed
committed save chain cannot gain original-retry permission.

`inspect_field_rename` returns `name_profile` as either
`{"kind":"same_width_ascii","original_units":N}` or
`{"kind":"fitted_ascii","original_units":N}`, or `null` when unsupported.
The retained `same_width_ascii_utf16_units` field is non-null only for the actual
same-width policy; fitted support does not silently reinterpret that restriction.
Fitted proposals still require a fresh consuming review, available native space,
unchanged source revision and the same schema-write grant at Apply.

An exact unchanged field-rename review follows desktop's no-save path. Apply
returns an `operation_id` and checked read-only reopen facts, without a journal,
`save_id` or fabricated saved receipt. Failed/cancelled verification retains the
same witness for `retry_unchanged_field_rename` or explicit
`discard_unchanged_field_rename`. Case-only changes still use the actual native
save path. The initial apply requires the schema grant in either case.

The remaining sections describe future agent services, not capabilities supplied
by this host merely because tools are discoverable.

## Architecture proposal: current status

The broader agent-development design remains the target. Existing one-operation
reviews are not a dependency-aware multi-object changeset implementation.

| Milestone | Implemented boundary | Remaining work |
| --- | --- | --- |
| MCP-0: local host | Official SDK, stdio, tool discovery, structured session status and cooperative read cancellation | Desktop attachment is a separate mode |
| MCP-1: inspection | Local table/field metadata, stored collections and supported saved queries | Broader object definitions, relationships, indexes and dependency discovery through shared services |
| MCP-2: reads | Bounded shared-runtime query execution; structured display cells with independent NULL/Boolean facts | A lossless typed scalar export contract; display text is not a stored value or edit input |
| MCP-3: capability checks | Actual source-specific inspection and native admission for implemented operations | A complete application compatibility/dependency report |
| MCP-4: editing | Consuming single-operation reviews, literal inputs, original-image insertion continuation and retained save/recovery owners | Canonical semantic diffs, digest-bound authorization and dependency-aware multi-object staging |
| MCP-5: application objects | No form/report authoring or previews supplied by the tool list | Working definition, rendering and application-scenario services |
| MCP-6: desktop attachment | Not implemented | Authorized local IPC sharing the desktop's real session, pending edits and owners |
| MCP-7: remote/project work | Not implemented | Authenticated remote transport; SQLite/native-project implementation remains paused |

The proposed `tablecraft-edit` boundary stays module-first in runtime until
working reusable planning justifies extraction. Physical transactions, locks and
recovery remain in ACE; shared editing does not create cross-object atomicity.
The [changeset contract](../architecture/APPLICATION_CHANGESETS.md) identifies a
concrete field-rename impact/diff slice and its unsupported-dependency refusals.

The executable remains at the requested `apps/tablecraft-mcp` location. A reusable
`crates/interfaces/` adapter is a possible later extraction for an actual second
host, such as desktop-attached IPC. Creating that package now would not supply
attachment, authorization or shared session ownership. The existing separate
app already keeps SDK and transport dependencies out of normal desktop/CLI builds.

Desktop attachment must use the desktop's authorized application session; opening
a second standalone writer to emulate attachment is not equivalent. Future
resources/prompts, Tasks, subscriptions, persistent handles and remote principals
need their own ownership, expiry, permission and lifecycle contracts. SDK support
for those primitives does not mean this host advertises or implements them.

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
