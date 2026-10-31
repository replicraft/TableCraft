# Access offline write sessions

The optional `tablecraft-access` feature `offline-write-session` supplies
`editing::offline::OfflineWriteSession`. It composes normal native planning,
journal recovery and filesystem finalization over one retained exclusive target.
The normal mutation profile remains the qualified Jet 4 fixed Int32 profile in
[NATIVE_WRITES.md](NATIVE_WRITES.md). ACE engineering candidates remain separate;
this service grants no normal ACE write or data-macro absence permission.

The host must provide an explicitly offline target, exclude noncooperating
writers and resolve any other journals before editing. This is not native
Access multiuser coordination. Filesystem pairing currently operates on Linux
and macOS; other Unix targets refuse before target acquisition. The module is
Unix-only. See [filesystem journals](../../foundation/FILESYSTEM_JOURNALS.md) for the underlying
namespace and lock assumptions.

## Plan before effects

`OfflineWriteSession::open(path, scope, budget, cancelled)` reserves its retained
resource owner before opening the target. Opening acquires the exclusive target
descriptor; it does not create a journal or establish native write eligibility.

`session.plan_update(query, budget, collation, cancelled)` consumes the session
and returns `StagedUpdate`. It captures the native image from that descriptor,
uses the existing shared SQL update planner and retains the exact resolved
recoverable plan. `matched_rows()` and `changed_rows()` let a host allocate and
format its completion report before journal setup or target mutation.
`staged.into_session()` abandons the unprepared plan while retaining the lease.

`staged.apply(bundle, budget, cancelled)` creates the explicitly selected durable
bundle, prepares the bound plan and commits through one lexical storage borrow.
The existing native prepare step checks the expected image again. Success retires
and cleans the bundle, acknowledges journal unlock and moves the same still-held
target back into the session. It never reopens the target path or unlocks the
target between successive edits. The next plan captures a fresh image from that
descriptor; it does not reuse a previous snapshot.

`session.release()` explicitly releases the remaining resources. Hosts publish
their preformatted success report only after the required completion and release
steps, and must retain the receipt when reporting output errors. Session methods
do not publish output themselves.

## Failures retain resources and facts

Failures after opening return `Failure`, which owns the same session rather than
implicitly unlocking, deleting a journal or retrying an operation. `phase()` and
`cause()` identify the failed step. `state()`, `pair_state()` and
`pair_namespace_state()` distinguish session readiness, journal lifecycle and
uncertain release. `setup_failure()` exposes retained incomplete setup details.
`into_session()` recovers the owner for an explicitly permitted next action.

`current_outcome()` describes the present retained operation. Native completion
is recorded before fallible retirement, cleanup or transfer, so those failures
cannot turn a known commit or recovery into an unknown result. `last_completed()`
retains the previous fully finalized receipt independently. It may describe an
earlier operation when a later attempt fails. Neither fact is permission to
reapply SQL.

Retained journals and incomplete setup prevent normal planning. An uncertain
unlock prevents retrying release or normal reuse. Dropping an owner only runs
its existing handle cleanup; it does not certify successful release or remove
retained namespace entries. A failed complete native capture requires an explicit
`refresh`; refresh reads the retained target and cannot repair or replay a
journal.

## Explicit recovery and current-image confirmation

`recover_update(bundle, budget, cancelled)` opens one explicitly selected active
bundle. `recover_pending(budget, cancelled)` works on the same retained active
pair, refuses an already known completion and invokes the existing qualified
Jet 4 recovery protocol. There is no automatic recovery, SQL replay or fallback
to a different journal or target.

`confirm_retired(bundle, budget, cancelled)` opens an intact reserved retired
bundle and uses write-denied storage views to confirm its complete current image.
It then acknowledges the parent barrier and cleans the retired bundle without
renaming it again. Its `Confirmed` receipt exposes `verified_image()` as
`Original` or `Replacement`; `outcome()` is `None`. This describes a currently
verified complete image and barriers, not proof that a historical rollback or
commit occurred. `Applied` and `Recovered` receipts instead expose their native
`RecoveryOutcome`. Partial retired garbage remains outside this intact-bundle
confirmation path. See [image journals](../../foundation/IMAGE_JOURNAL.md).

## Evidence

The independently authored original and successive complete expected images were
read-qualified before session execution and committed as `7a604fa7`. Eleven direct
filesystem session tests pass, including same-lease successive saves, actual
pending/committed recovery, current-image confirmation, stale-source rejection,
late commit cancellation, foreign-entry cleanup failure and explicit refresh
without losing the previous receipt. A separate real-protocol regression verifies
that confirmation cleanup failures retain Original/Replacement without claiming
historical commit or rollback. The IO transfer has its own complete regression.

The [focused record](../../testing/results/offline-write-sessions-2026-10-31.json)
separates source admission, compiler/metadata diagnostics, direct tests and CLI
validation. These are local engineering observations. Native Access save/reopen,
power-loss/network filesystems, Windows and mixed-client behavior remain unqualified.
