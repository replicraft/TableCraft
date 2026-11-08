# Fresh qualification of reviewed native DELETEs

This engine/adapter boundary now supplies the runtime and desktop SQL DELETE
workflow. SQL and typed parameters are reviewed before an exclusive offline lease
is acquired. The shared action warning requires explicit confirmation, and save
consumes the sealed candidate without reevaluating SQL. Existing single-row GUI
deletion remains a separate service.

## Original-row query selection

`plan_delete_with_options` accepts explicit typed bindings and stored-field
comparison policies through shared `MutationOptions`. It validates declarations
and carriers before visiting snapshot rows, evaluates every original predicate,
and only then calls the backend once with snapshot-local handles. Late eager
errors, cancellation or exhausted query work cannot finish a partial selection.
Zero matches still call the backend so source/action restrictions remain checked.

The query allowance pays binding, parameter work, predicate evaluation and
selected handles. The backend allowance pays its consuming finish. The legacy
`plan_delete` retains one pool and refuses declared parameters. Neither API opens
files or grants native publication permission.

The Access `plan_recoverable_delete_with_options` consumer captures actual native
column declarations, aligns field policy identities to the scalar snapshot and
supplies the explicit native resolver. Schema, snapshot and native admission use
the native pool; query selection uses the query pool. Literal parameter values
never become SQL. Joins, external targets and hidden retrieval remain refused.
The same working options/source owner also serves UPDATE; consumers import
`MutationOptions` and `NativeMutationOptions` directly without old-name aliases.

## Sealed native candidate

`RecoverableNativeDeletePlan::requalify_with_budgets` consumes the reviewed
candidate and a fresh native image. It checks complete BEFORE equality and the
original intent's source digest, then reconstructs the original raw deletion
requests through normal native admission. Both rebuilt images and the deleted
root count must reproduce the sealed candidate. The original `TCND0001` intent
is retained unchanged. No SQL, expression provider, UI ordinal or mutable selection
is reevaluated, and no journal is created.

Native reconstruction uses its caller's cumulative native allowance. Source
binding, full-image comparisons and terminal cancellation checks use the separate
proof allowance. Failure publishes no replacement capability. This does not
expand the [normal DELETE profile](NORMAL_ACE_DELETE.md), bypass required programs,
permit arbitrary catalogs or reinterpret UPDATE intents as DELETE intents.

## Retained offline staging

`OfflineWriteSession::stage_reviewed_delete_with_budgets` captures fresh bytes
from the same retained exclusive target, checks the whole reviewed original and
uses native requalification. Success returns the existing `StagedDelete`; explicit
`apply` owns journal setup, commit, recovery and retirement through the normal
retained session lifecycle. Staging itself creates no journal.

Failure retains the same target and receipts. Capture failures leave the session
refresh-required; successful capture followed by source/proof refusal retains its
ready state. The target remains locked until explicit release. Hosts must review
before acquiring this lease and retain all accepted-save outcomes through view
changes, just as the [UPDATE service](REVIEWED_UPDATES.md) already does.

## Runtime and shared confirmation

`ReviewQueryDelete` accepts an immutable inspected query from the same open
runtime document and literal typed inputs. Source identity, document revision,
input limits and declaration constraints are checked before native capture and
record processing. The response separates `QueryDeleteReview` display facts from
the opaque `ReviewedQueryDelete` capability. No exclusive lease or journal is
held while the user considers the warning.

UPDATE and DELETE share one presentation session and its parameter/SQL witness,
confirmation, bounded diagnostics, cancellation and terminal-outcome lifecycle.
The expected operation is retained with the original witness; a wrong-kind effect
cannot enter or replace the decision. Desktop retains distinct native capability
variants and routes them to their corresponding consuming save methods. UI counts
and operation labels cannot construct a native candidate or choose another action.

`SaveWorker::start_query_delete` checks the capability's current document/source
and uses the fresh staging boundary above. Before acceptance, No/Escape releases
review without changing the database or prior query results. After acceptance,
cancellation requests preserve the real save/recovery owner until terminal
acknowledgment. Successful acknowledgment reopens the actual source, restores the
SQL tab/draft and clears stale snapshots without executing DELETE again.

Microsoft's [DELETE statement](https://support.microsoft.com/en-us/access/delete-statement)
documents whole-record deletion, retained table structure and no Undo. Its
[screen-reader workflow](https://support.microsoft.com/en-us/accessibility/access/use-a-screen-reader-to-create-a-query-in-access-desktop-databases)
documents confirmation of the selected row count before deletion. The
[delete-query guide](https://support.microsoft.com/en-us/access/create-and-run-a-delete-query)
also describes inspecting the records selected by the same criteria. The current
GUI warning shows counts and no-Undo text; it does not implement a separate action
record-preview mode. See the [runtime contract](../../application/RUNTIME.md#reviewed-sql-delete-actions)
and [SQL workspace](../../ui/SQL_WORKSPACE.md#reviewed-local-update-and-delete).

## Evidence and limits

The engine tests use the existing independent native-17 canonical Boundary source.
Their complete AFTER oracle changes only literal DATA tombstone flags and the
live-row count, independently of production encoders. They also cover original
intent preservation, same-source intent substitution, unrelated source drift,
independent pool exhaustion and entry/middle/terminal cancellation.

Adapter tests separately prove that the sealed candidate is applied and reopened,
including empty operations, with retained lock/state checks for refusal. Their
persistence oracle is the sealed AFTER, not an independent native encoder oracle.
The query tests cover typed handles/NULL, complete parameter validation, stored
field policy versus the default, late eager errors and split budgets. A browsing
fixture was initially rejected as active content; the correction selected an
already qualified source and did not weaken native gates.

These observations do not establish Windows/Access execution parity or qualify a
new format/profile. No DELETE warning capture is retained; the current warning
is an adaptation until native visual evidence exists. Joined/external DELETE,
broader predicate contexts, native saved-action persistence and full older-profile
CRUD remain separate roadmap work.
