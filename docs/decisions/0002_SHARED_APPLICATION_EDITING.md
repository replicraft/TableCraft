# Shared structured editing for humans and agents

Accepted direction, 18 October 2026; current-service clarification added
9 November 2026. General application changesets remain future functionality;
working typed native editing services are described below. This extends the
[native-project decision](0001_PROJECT_FORMAT_AND_EXECUTION.md).

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
coordination; keep it module-first until working reusable functionality justifies
an independent boundary. Runtime owns actual apply lifecycle, engines own physical
transactions, and owning presentation/testing subsystems supply actual previews
and scenarios. Designers remain `tablecraft-builders`, not `tablecraft-design`.
The local stdio MCP executable now uses the official Rust SDK and the same runtime
services as desktop. Its session owns protocol handles, grants and actual runtime
capabilities; it is not another native edit engine. This decision does not create
`tablecraft-edit`, a desktop-attached bridge, form/report editors or a compatibility
crate merely to match a proposed package chart.

## Current implementation and next application slice

Runtime's typed commands and sealed consuming reviews already support bounded
native cell/row/table/field/query/settings operations. Source inspection is inert;
review construction holds no file lease while waiting for a host decision. Actual
apply acquires the retained exclusive offline resource, reconstructs the original
native request against fresh bytes, and preserves explicit recovery ownership.
Operation-specific outcome facts are shared by desktop and MCP; complete expected
image witnesses still govern post-save schema publication.

This is the P0 foundation for shared structured editing. It is not multi-object
staging, a complete semantic dependency graph, canonical application diffs or
digest-bound approval/apply. An opaque review handle and an engine's BEFORE-bound
journal intent are not substitutes for those missing application contracts.
Multiple independent successful saves do not form one atomic changeset.

The next P1 slice is a single field rename with real impact/semantic-diff facts,
extending runtime's existing `editing/field_rename` feature before extracting a
new crate. Reuse its actual native source restrictions and complete-image review;
retain exact names/identities and refuse known unsupported or unknown dependency
coverage. Current Name AutoCorrect OFF/closed-object qualification proves only
its admitted absence of dependencies; it does not update saved queries, forms,
reports, macros or VBA references. Define immutable digest encoding and permission/
source binding before implementing digest-based apply. Keep Access's existing
visible GUI save behavior while agents can explicitly inspect staged changes.
See the [one-operation acceptance plan](../architecture/APPLICATION_CHANGESETS.md#next-vertical-slice-one-field-rename).

`tablecraft-model` currently supplies catalog metadata and object categories, not
form/report definitions. `tablecraft-compat` is prospective; current independent
fixtures and observations remain in `tablecraft-testkit` and `tests/`. Future
object edits require real owning subsystem contracts and native writers first.

Prioritize working shared editing, headless visual/semantic previews and
deterministic application scenarios over expanding protocol tool counts. External
agents precede a built-in assistant. Preserve independent admission across the
requested Access 97–2019 baseline (Jet 3/4 and ACE 12/14/15/16), version/permission
gates, native preservation and VBA execution/Windows-emulation deferrals. ACE 16
remains the behavioral reference, not proof that another profile supports an edit.

Contracts: [agent services](../features/AGENT_NATIVE.md),
[changesets](../architecture/APPLICATION_CHANGESETS.md),
[previews](../features/HEADLESS_PREVIEWS.md),
[scenarios](../../tests/reports/reading/APPLICATION_SCENARIOS.md) and [MCP](../interfaces/MCP.md).
