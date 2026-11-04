# Access offline write sessions

The optional `tablecraft-access` feature `offline-write-session` supplies
`editing::offline::OfflineWriteSession`. It composes normal native planning,
journal recovery and filesystem finalization over one retained exclusive target.
The normal mutation routes are the existing Jet4 fixed Int32 profile and the
separate [positive native ACE BeforeChange profile](NORMAL_ACE_BEFORE_CHANGE.md).
The latter requires unencoded ACE 2016/2019 subversion zero, complete source-owned
pure program binding and an admitted ordinary Int32 target. Checked
indexes may retain qualified compressed prefixes and interior nodes, including
consistent aliases and Int32 composites. Fitting is selected before preparation;
page identities, child topology and per-leaf entry counts remain fixed.
A separate metadata-selected mixed-schema route supplies actual scalar snapshots
and preserves admitted unavailable siblings. Its changed fields remain ordinary
non-NULL Int32. Complete scalar tables may maintain checked I32 indexes beside
untouched NULL components;
rich/unavailable siblings retain unindexed edits only. Readonly Int32 AutoNumber
may supply operands/keys, never writable targets. Original NULL-key facts and
potential targets choose the prior unindexed policy or checked indexed route
before tree parsing. Actual NULL targets remain unsupported.
Snapshot handles stay local to the captured image. Relationships, unsupported
required content, splits and map/storage growth refuse. It grants no general ACE save or
data-macro absence permission;
ACE engineering candidates remain separate.

The separate [fixed-scalar batch service](NORMAL_ACE_FIXED_SCALARS.md) now
advertises `StoredScalar` for qualified complete ordinary tables and consumes
`EditableBatch` through the same retained session. It supports admitted unindexed
Boolean/Int32/Int64/Currency/DATE groups with original typed intents and fresh
recovery replay. Present single-column Int32 requests keep the existing route.
Complex, Memo/OLE, calculated and partial declarations do not select this new
capability; the prior Int32 service keeps its own qualified preservation scope.

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

For a host editing a previously viewed snapshot,
`session.edit_table_if_original(table, expected, budget, cancelled)` compares the
complete captured physical image with `expected` before creating a fresh table
editor or resolving snapshot-local row handles. Capture uses the same retained
target descriptor. Length or byte mismatch returns `Cause::SourceChanged` with
the planning failure owner; it creates no journal and changes no target bytes.
The chunked comparison pays for both complete inputs and checks cancellation.
The ordinary `edit_table` API retains its existing behavior and costs.

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
retained namespace entries. Incomplete setup now admits one explicit `release`
through the retained setup owner. It unlocks target/journal handles without
changing colliding or partially created entries. Failure retains the same typed
setup diagnostics in `ReleaseUncertain`, with no retry, recovery or storage reuse.
A successful release does not establish that those entries are repaired or safe
to ignore. A failed complete native capture requires an explicit
`refresh`; refresh reads the retained target and cannot repair or replay a
journal.

## Explicit recovery and current-image confirmation

`recover_update(bundle, budget, cancelled)` opens one explicitly selected active
bundle. `recover_pending(budget, cancelled)` works on the same retained active
pair, refuses an already known completion and invokes the closed record-selected
native recovery validator. Both routes requalify the recorded BEFORE and compare
both complete images. ACE rederives/evaluates its admitted pure required programs
from the original typed intents; SQL filtering and user expressions are not
replayed. There is no automatic recovery or fallback to another validator,
journal or target.

`confirm_retired(bundle, budget, cancelled)` opens an intact reserved retired
bundle and uses write-denied storage views to confirm its complete current image.
It then acknowledges the parent barrier and cleans the retired bundle without
renaming it again. Its `Confirmed` receipt exposes `verified_image()` as
`Original` or `Replacement`; `outcome()` is `None`. This describes a currently
verified complete image and barriers, not proof that a historical rollback or
commit occurred. `Applied` and `Recovered` receipts instead expose their native
`RecoveryOutcome`. Partial retired garbage remains outside this intact-bundle
confirmation path. See [image journals](../../foundation/IMAGE_JOURNAL.md).

## Typed table edits under the retained lease

`session.edit_table(table, budget, cancelled)` consumes the offline session and
returns `OfflineTableEditor`. It captures from the same exclusively held target
descriptor and delegates to the existing `NativeJournalEditor`. The editor
implements `tablecraft_db::EditableTable`: hosts can inspect its real `snapshot()`
and `update_profile()` and build ordinary `CellUpdate` requests from that
snapshot's row handles. This is a backend capability; no GUI is introduced.

`editor.plan_updates(column, updates, budget, cancelled)` consumes the editor and
returns the existing `StagedUpdate`. Handle identity, duplicate ordinals, original
expected values and native eligibility remain in the normal editor/planner.
`matched_rows()` counts validated supplied requests; `changed_rows()` comes from
the final normal native plan. Jet4 retains its qualified no-op behavior. ACE
rejects any supplied equal assignment with `EventTiming`, after required-program
discovery/preflight, while a real change restored by the program may report zero
effective changed rows. Hosts can prepare their report before the existing
journaled `apply`.
There is no public constructor accepting raw images or invented change counts.

Both `editor.into_session()` and `staged.into_session()` explicitly abandon an
unprepared snapshot/plan while retaining the target lease. After a committed edit,
a new editor captures fresh values and issues fresh handles. Handles from the old
or another snapshot refuse before bundle creation, even when database bytes match.
Failure retains the same session and any previous completed receipt. It does not
unlock, retry, reset blocked state or discard a pending journal.

Typed and SQL edits share the same staged persistence capability. Existing SQL
planning, resource/cancellation checks and error precedence are unchanged. The
normal write profile remains fixed non-NULL Int32. The journal editor advertises
ACE eligibility only after positive native BeforeChange discovery and complete
program preflight; the normal planner still checks the actual requested update.
Tables without that evidence remain unavailable. No engineering candidate
conversion, new DB contract, macro-absence certificate or general ACE save
permission is added. Hosts must not await an external approval while holding
this offline lease.

### Typed acceptance

The initial typed phase passed three focused functions. It exercised snapshot
reads under the actual held lease, typed 10→12→17 whole-image saves,
stale/foreign/duplicate handles, previous-receipt retention, snapshot/plan
abandonment and exact no-op bytes using the unchanged source-qualified images
from `7a604fa7`. That phase's fourth typed control retained readable-ACE refusal
before the separate positive-program route was added; its historical record is
unchanged. At that phase, the complete session target passed 15 functions;
all eight CLI journal regressions, strict checks, default-disabled compilation
and formatting passed.
See the [typed result](../../testing/results/offline-typed-write-sessions-2026-10-31.json).

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

## Normal deletion

The separate [normal DELETE service](NORMAL_ACE_DELETE.md) implements
`DeletableTable` on typed editors. `plan_deletes` validates issued handles and
captures original raw witnesses; `plan_delete` uses the shared SQL predicate
planner. `StagedDelete` uses the same retained target, receipt and cleanup
lifecycle. TCND recovery reconstructs original DELETE from the same checked V1
journal, preserving existing TCNI1–7 UPDATE dispatch. Refreshed snapshots issue
new handles; live ordinals are not persistent identities. The initial route
refuses selected indexes and relationships involving the target.

## Normal insertion

The [normal INSERT service](NORMAL_ACE_INSERT.md) supplies metadata-only
`NativeJournalInserter` and `OfflineWriteSession::insert_table` typed preparation.
`plan_insert` binds a complete SQL VALUES row through the shared query service.
The consuming finish performs full original-source admission and returns
`StagedInsert`; counts are available before journal setup. INSERT receipts retain
`matched_rows=None` and the actual inserted count. The common apply lifecycle
preserves UPDATE/DELETE counts and the same target/pair through failures,
retirement and release. TCNA recovery consumes the same checked V1 journal.
