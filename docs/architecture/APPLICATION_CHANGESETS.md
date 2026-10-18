# Staged application changesets

Accepted contract, not implemented types, CLI commands or MCP tools. A changeset
describes typed data/schema/query/form/report/action/source/resource edits against
an identified project revision. It shares the
[canonical command lifecycle](APPLICATION_EXECUTION.md) across all interfaces.

`tablecraft-edit` is the prospective owner of typed mutations, revision
preconditions, semantic diffs and validation coordination. Begin as a runtime
feature module if small; extract only for working consumers/dependency isolation.
Reusable edit definitions must not depend on GUI, MCP or concrete runtime hosts.
Runtime orchestrates approved apply through backend services; adapters/engines
still own physical transactions. This is distinct from today's DB cell-update
plans and cannot turn their restricted native write profile into general editing.

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

For a single `.tcraft` store, use appropriate SQLite transactions for consistent
schema/definitions/source/assets publication. A staged database copy is a preview,
not permission to replace a concurrently edited live file. Packed/unpacked or
external-resource publication needs an explicit namespace/recovery protocol.
For MDB/ACCDB, recheck the source under qualified native locks immediately before
apply; unsupported object writers, data macros or preservation requirements block
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
