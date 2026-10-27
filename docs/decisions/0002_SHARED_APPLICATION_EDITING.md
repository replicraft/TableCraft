# Shared structured editing for humans and agents

Accepted direction, 18 October 2026; future application functionality, not a
released API. This extends the [native-project decision](0001_PROJECT_FORMAT_AND_EXECUTION.md).

The [Access behavioral-parity decision](0003_ACCESS_BEHAVIORAL_PARITY.md) now
governs current native-file commands across all hosts. Shared editing services
must preserve Access restrictions and observable operation behavior. Native-project
implementation remains paused; these contracts do not resume it.

All hosts use common semantic operations with discover/design/preview/validate/
apply support. Stage multi-object changes against explicit revisions; bind
applicable authorization to the exact diff/digest and scope. Recheck permissions,
source revision and format capabilities before apply. Idempotency, audit and
recovery are service concerns, independent of a protocol connection or LLM.

Reserve `tablecraft-edit` for reusable mutations, changesets and validation
coordination; begin as a module if sufficient. Runtime owns actual apply lifecycle,
engines own transactions, render owns headless previews and test services verify
workflows. Designers remain `tablecraft-builders`, not `tablecraft-design`.
MCP uses the official Rust SDK as a thin future adapter with stdio first; it does
not own editing state. No new crate, dependency or server is introduced now.

Prioritize working shared editing, headless visual/semantic previews and
deterministic application scenarios over expanding protocol tool counts. External
agents precede a built-in assistant. Keep ACE 16's current implementation priority,
version/permission gates, native preservation and VBA/Windows deferrals intact.

Contracts: [agent services](../features/AGENT_NATIVE.md),
[changesets](../architecture/APPLICATION_CHANGESETS.md),
[previews](../features/HEADLESS_PREVIEWS.md),
[scenarios](../testing/APPLICATION_SCENARIOS.md) and [MCP](../interfaces/MCP.md).
