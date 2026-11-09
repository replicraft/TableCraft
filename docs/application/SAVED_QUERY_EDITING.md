# Guarded existing saved-query SQL edits

The runtime owns SQL qualification and native save authority. Shared UI owns
exact text, revisions, parked sessions and save presentation; the desktop binds
these through one consuming review/save/reopen lifecycle. Hosts explicitly opt
in to editing. Unavailable editing never starts a save: retained SQL remains
read-only, while a source without qualified SQL shows the unavailable placeholder.

## Current native boundary

The complete-SQL route reviews existing projection and WHERE expressions in a
qualified local SELECT against actual saved fragments from the same immutable
ACE 16 image. Multiple saved definitions and MSysQueries DATA pages can coexist.
Each edit binds the target's signed native identity, attribute, order and exact
original text. Existing fragment membership/order, aliases, parameters, sources,
flags and all other model fields must remain unchanged. Only changed expression
ASTs are serialized; unchanged stored spelling remains byte-exact. The complete
candidate is reconstructed and must equal the full proposed SQL model.

The native source remains a closed, unencoded ACE 16 code 5/subversion-zero
profile with complete catalog/index/allocation/forwarded-root ownership checks.
Edited Memo bodies must be inline; protected unedited external bodies may remain
unchanged. Qualified replacement can grow or shrink through the existing native
record-placement owner while preserving the file length, logical index roots
and every byte outside its proved scope. Unsupported topology, parameters,
compiled/cache/property payloads and unqualified ownership still refuse.

`ReviewQuerySql` returns a distinct `ReviewedQuerySql` capability for
`SaveWorker::start_query_sql`. Its ordered native request uses `TCQE0002` and the
existing fixed-image checked V1 journal. Request version and journal version are
separate. The low-level literal review/`ReviewedQueryExpression`/TCQE0001 route
keeps its original same-length, one-query, five-fragment language exactly; neither
decoder retries a failed request as the other version. The
[native QueryDef reference](../testing/NATIVE_QUERYDEF_FITTING_SAVE.md) preserves
that earlier profile and the independent SDK observations.

Canonical SQL visibility remains independent of writer eligibility. A readable
SELECT/UNION/crosstab can remain read-only. Even semantic no-op review performs
the complete native qualification; AST equality alone grants no write authority.
The host neither slices SQL nor encodes native fragments.

This does not create a QueryDef, implement Save As, add MSysQueries to ordinary
creator databases, qualify broader query shapes or establish Access 97–2019
write parity. Independent SDK observations, TableCraft byte preservation and
Microsoft Access behavior remain separate evidence. Native Microsoft Access
save/reopen, DateUpdate/events and Windows execution comparisons remain unrun.
The saved-SQL status/close surfaces have no retained native Access interaction
capture; their presentation is provisional.

## Exact review and save ownership

A UI request binds document revision, native-model object identity/name, exact
original/proposed strings and a monotonically increasing text revision. Missing
original SQL is not fabricated. Runtime review returns a separate native
capability and complete `QueryExpressionReopen` witness. Review holds no exclusive
file lease, creates no journal and executes no saved query.

The desktop retains that exact request, runtime review ID and witness through
one nonreplaceable save owner. UI prepares and budgets its complete status
before the actor starts, then binds the actual accepted save ID without another
allocation. Stale replies/actions cannot attach to another object, text revision
or accepted lifecycle. A stale close-Save cannot relabel an existing save as a
request to close its tab.

Changed native saves use the existing adapter, checked original intent and
journal. Every apply rechecks the complete original under its retained exclusive
target. Query completion facts are separate from row counts and table/settings
changes. Failed retained owners offer only their actual recover/release choices;
no edit is silently retried. Status remains query-owned even when an outcome
message cannot fit: an allocation-free fallback retains outcome kind and resource
controls.

Native acknowledgement alone never marks visible SQL saved. A known replacement
requires the SAME accepted full AFTER witness to reopen successfully first.
`SaveOutcome::query_expression_reopen_facts` rejects conflicting receipt scopes;
current original/rollback facts override old replacement history. Original,
no-write and failed outcomes release presentation review while preserving dirty
text. They never acquire unchanged-review authority.

Formatting-only Save is a separate qualified semantic no-op. An actually
unchanged native review drops its write capability and performs only its fresh
BEFORE-equal-AFTER witness read. Only that successful check can resolve the exact
submitted spelling. It invents neither a native actor ID nor a commit receipt.
Saved-query execution is always a later explicit Run action.

## Fresh revision and parked drafts

After the witness read, the shared object owner atomically validates the old
accepted request and fresh catalog, then publishes the new document revision.
It moves retained original/proposed SQL buffers for all active and parked saved
queries, preserving other dirty text, revisions and errors. Only the saved
target's baseline resolves. Object IDs/order/active selection, scratch SQL and
source-neutral parameter/design/totals preferences remain associated with their
owners. Old results, pending callbacks and source-bound native handles are
invalidated and reloaded separately; the source-neutral filter definitions use
the existing checked new-revision reapply route.

A layout-only preference restore cannot substitute for this owned transfer.
Canonical SQL inspection is not repeated over an already retained same-revision
SQL draft, so a successful Save keeps the submitted spelling. Publication/memory
refusal leaves all original sessions and text intact.

## Cancellation, failure and document transitions

Cancel requests stop only the matching unlocked review, actual save actor or
read-only reopen. Cancellation never implies rollback or clears a retained
resource owner. A failed witness read keeps every old session, baseline and
proposed string. Explicit Retry reopens through the same witness without replaying
the edit; Return to draft preserves dirty text and the historical known native
save warning plus any actual cleanup notice. An unchanged-review failure reports
only its read error. Returning to the old dirty view never claims the current
file is original or identical to the expected AFTER.

Open, Create, tab close and window close respect active and parked dirty saved
SQL through exact Save/Discard/Cancel decisions. Deferred host transitions remain
owned until their drafts are resolved, and stale decisions cannot clear newer
text. Close-Save resumes only after acknowledged/witnessed publication. A failed
or original result cancels the deferred transition while preserving the draft.
Unrelated row/schema/settings writes stay blocked while saved SQL is dirty;
the query-only save uses the explicit full-witness owned-transfer route.

## Connected expression expansion evidence

The [coordinated workflow record](../testing/results/full-sql-expression-workflows-2026-11-08.json)
retains 94 selected passing checks across the Access bridge/staging, runtime,
shared UI and headless desktop checkpoints. Eleven full-SQL runtime cases cover
multi-projection save/reopen/explicit Run, projection+WHERE review, inert syntax,
source invalidation and complete-witness tamper refusal. Five unchanged
literal-service tests retain the separate TCQE0001 boundary. The record preserves
initial compile/fixture/assertion failures separately from corrected checks.
Strict affected-library/desktop lint passes; no native Access or Windows runtime
execution is claimed. General QueryDef CRUD remains unfinished.

## Earlier bounded-route evidence

The [initial connected headless batch](../testing/results/query-save-rebind-2026-11-08.json)
passes 93 tests, including seven real desktop query-save cases and nine owned
transfer cases within the broader object filter. The UI interaction case drives
the editor, Command-S, the actual query-owned report modal, acknowledgement,
witness reopen and explicit Run returning 43. Formatting-only Save retains the
original file and creates no journal. Parked system-table/scratch sessions and
stale original/reopen cases are separate checks.

The subsequent stale-close guard and historical warning/cleanup correction add
two focused cases and strengthen the failed-AFTER presentation check. Their
focused rerun now passes all 37 selected checks: 21 object/transfer cases,
nine query-save controller cases and seven saved-query reading cases. Across
the retained service, integration and final gates, 109 workflow checks and four
localization checks pass;
repeated executions and the renamed opening case are not counted as new coverage.
A substituted
terminal cleanup envelope tests presentation only; it does not establish a real
retained-artifact filesystem outcome.
