# Fresh qualification of reviewed native DELETEs

This is a working engine/adapter boundary for reviewing SQL before taking an
exclusive offline lease. The desktop SQL editor does not yet dispatch DELETE;
its runtime command, shared confirmation and consuming save route remain work
for the next application slice. Existing single-row GUI deletion is separate.

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
new format/profile. No DELETE warning capture is retained; the eventual warning
must be documented as an adaptation until native visual evidence exists.
