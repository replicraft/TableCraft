# Staged application changesets

Accepted changeset contract, updated 9 November 2026. General staged changeset
types, digest-bound apply and multi-object orchestration are not implemented.
Current typed consuming reviews and native save services are implemented; they
are the starting point, not a completed changeset engine. The intended logical
changeset describes typed data/schema/query/form/report/action/source/resource
edits against an identified source revision and follows the
[canonical command lifecycle](APPLICATION_EXECUTION.md).

`tablecraft-edit` is the prospective owner of typed mutations, revision
preconditions, semantic diffs and validation coordination. Begin as a runtime
feature module if small; extract only for working consumers/dependency isolation.
Reusable edit definitions must not depend on GUI, MCP or concrete runtime hosts.
Runtime orchestrates approved apply through backend services; adapters/engines
still own physical transactions. Today's runtime already owns typed cell, row,
table, field, saved-query and database-option reviews, native source qualification,
consuming save actors and operation-specific outcome facts. These bounded native
services do not become general editing merely by grouping requests.

## Current services and the remaining boundary

| Responsibility | Current implementation | Remaining changeset work |
| --- | --- | --- |
| Typed requests | Runtime commands and sealed consuming reviews; JSON cannot construct native authority | A reusable staged operation representation with stable source/object preconditions |
| Compatibility validation | Engine/adapter admission for each actual operation and source; fresh reconstruction before native apply | Unified validation/impact reporting across staged operations and every affected object |
| Results and recovery | Typed terminal outcomes, retained owners, original action intents and complete expected-image reopening | Coordinated multi-step results, partial application and reconciliation without claiming cross-operation atomicity |
| Semantic diff | Individual reviews expose operation-specific before/after facts | A complete machine-readable change/impact record with canonical digest binding |
| Dependencies | Operation-specific native closed-source proofs and bounded query metadata | An application dependency graph that distinguishes proven absence, known references and unknown bodies |
| Agent staging | MCP owns bounded opaque handles to actual runtime reviews | Multi-object staging, diff validation, immutable approval scope and digest-bound apply |

A session-local review ID is not a durable changeset or an approval digest.
Likewise, a native journal's complete BEFORE binding and original request prove
source/action reconstruction; they do not establish that a principal approved a
canonical application diff. The current save worker remains the sole persistence
owner while this additional application contract is developed.

P0 is to keep typed operations, compatibility-aware admission and honest terminal
results shared across hosts. These responsibilities already have working runtime
consumers, although their format/object coverage remains bounded. P1 adds the
single-operation semantic diff, impact validation and staging before expanding to
multiple objects. Persistent histories and cross-project application remain later
work. Do not replace functioning native operations with an empty general framework.

`tablecraft-model` currently owns inert catalog descriptors and object categories.
A Form/Report/Macro category is not a decoded form/report definition or executable
application model. Those future subsystems must supply actual definition and
reference contracts before changesets can edit them.
`tablecraft-compat` remains a proposed engineering boundary; current reusable
fixture and observation machinery lives in `tablecraft-testkit` and the engineering
harnesses under `tests/`. Neither proposed crate exists merely because it appears
in an architecture diagram.

## Next vertical slice: one field rename

Extend the working runtime `editing/field_rename` feature first. Add a real impact
and semantic-diff result alongside its existing source inspection and consuming
review; introduce a shared `editing/changesets` module only when working common
staging behavior needs it. No new `tablecraft-edit` crate is necessary simply
because desktop and MCP are two consumers of the same runtime today. Extract a
crate when working reusable planning has a useful independent dependency/testing
boundary, with actual consumers and no runtime/engine cycle.

The first slice should complete these steps without widening native admission:

1. Capture the exact document revision, native table/field identity and original
   spelling from current schema/source inspection. Native number, stored creation
   ID, displayed column position and Caption remain distinct. Runtime owns the
   operation state; protocol clients supply intent, not fabricated native IDs.
2. Report the requested old/new name and all known effects as a semantic diff.
   Track dependency coverage explicitly: a proven empty dependency set differs
   from an unsupported or opaque object whose references have not been inspected.
   Do not call a partial query-reference scan a complete application graph.
3. Reuse the [current native rename admission](../access/writes/FIELD_RENAME.md):
   qualified empty ACE16 field-history sources, explicit Name AutoCorrect OFF,
   selected non-key Number Long or qualified Short Text, and same-width ASCII
   names with an entirely admitted comparison namespace. Native identifier,
   duplicate-name, index, property, payload and preservation checks still apply.
   Other formats and source shapes need independent admission/evidence.
4. Block apply for known unsupported dependencies and for unknown dependency
   coverage. The current closed-source proof refuses query dependencies, NameMaps,
   macros, broader object graphs and unknown residual bytes; it does not repair
   those references. Future form/report/VBA bodies cannot be reported as safe
   merely because their catalog descriptors were enumerated. The first positive
   impact record can prove no dependencies only for the actual qualified closed
   source; it must not invent dependency support to make the preview succeed.
5. Bind an immutable canonical operation/diff digest to the source preconditions,
   validation coverage, principal/grant scope and applicable authorization. Define
   its encoding and lifetime before implementing digest-based apply. Changing the
   proposed name or validation context invalidates the prior reviewed digest.
6. Consume the approved operation through the existing native review/save path,
   recapturing/requalifying source bytes under the actual exclusive offline owner.
   No native lease is held while the user or agent reviews the diff. Reopen only
   through the same complete expected-image witness after acknowledged outcome.

Tests must connect the real diff/impact result to the actual consuming operation:
changed name/digest/grants, stale bytes despite an unchanged host revision, wrong
native field identity, unknown object dependencies, unsupported profiles,
case-only/no-op requests, cancellation and retained/uncertain outcomes. A
source-free DTO test alone cannot qualify this slice. No-op requests retain native
admission and follow the existing no-save witness workflow; do not fabricate an
applied receipt for them.

The slice does not add a mandatory changeset dialog or approval screen to normal
Access GUI operations. Desktop keeps Access's actual save/confirmation workflow;
agent staging is another interaction policy over the same application services.

## Lifecycle and identity

1. Create an owned changeset handle for a principal, project and explicit base
   revision. Capture relevant container/capability/schema-epoch requirements.
2. Stage typed operations in a disposable copy or isolated workspace. Reject
   invalid identifiers/carriers and record dependency effects. Source remains
   unchanged; no native locks remain held while waiting on an agent or review.
3. Produce a machine-readable semantic diff: object IDs, before/after revisions,
   schema/data/source/resource changes, full affected counts or explicit unknowns,
   exact numeric rules, preserved content and potential effects. Samples may be
   bounded/redacted but cannot stand in for complete mutation scope.
4. Validate schema, bindings/references, constraints, version/capabilities and
   format/writer support. Run authorized disposable scenarios and render previews
   against the staged revision. Mark every check performed/failed/skipped/unrun.
   Missing or failed required validation blocks apply; a skipped check cannot
   become success through an agent's narrative.
5. Finalize a canonical digest binding base revision, exact operations/payloads,
   resource/source hashes and relevant validation context. Applicable approval
   binds that digest, project, revision and authorized scope; staging more edits
   invalidates it. No approval-by-handle-name or approval of one diff followed by
   application of another. Specify canonical serialization before implementation.
6. Apply through shared commands after checking ownership, current permissions,
   approvals, versions and source revision again under the backend's transaction/
   locking preconditions. Stale validation requires revalidation and renewed
   approval where applicable, or rejection. Publish a consistent application
   revision; persist outcome/audit information with changes where feasible.
7. Discard an unapplied changeset without touching the source. Confirmed apply is
   terminal; undo requires a new compensating change, not erasing committed history.

Stage, diff, validate, preview, apply and discard are semantic operations, not a
frozen RPC naming scheme. Handles need project/principal scoping, expiration,
bounded storage and retention/cleanup policy. Idempotency keys bind caller,
operation and digest: repeat identical applies returns the recorded outcome;
reuse with different content fails. Persist enough to reconcile interrupted or
uncertain outcomes before retry. A durable handle requires implemented durable
storage, not simply an SDK task ID.

## Backend and failure boundaries

The following `.tcraft` publication direction is retained planning, paused with
SQLite/native-project implementation until explicit reprioritization. For a
single `.tcraft` store, use appropriate SQLite transactions for consistent
schema/definitions/source/assets publication. A staged database copy is a preview,
not permission to replace a concurrently edited live file. Packed/unpacked or
external-resource publication needs an explicit namespace/recovery protocol.
For active MDB/ACCDB work, a host's local document revision is not a universal
monotonic database revision. Recheck exact relevant source/object preconditions
under qualified native locks immediately before apply; unsupported object writers, data macros or preservation requirements block
the edit. Never hold native locks for human/LLM latency.

A logical changeset does not promise atomic commits across independent databases,
files or external Python effects. Reject unsupported required atomic scope, or
use an explicitly authorized staged workflow with partial-outcome reporting and
compensation/recovery. Cancellation before commit requests abort; cancellation
after commit cannot claim rollback. Worker failure, uncertain commit and partial
external publication remain distinct outcomes, following
[transaction rules](TRANSACTIONS.md). Do not blindly retry unknown outcomes.

Required tests include changed digests/permissions after approval, stale revisions,
cross-project handles, duplicate/idempotency-conflicting calls, interrupted apply,
concurrent GUI/agent edits, budgets/cancellation and native unsupported profiles.
