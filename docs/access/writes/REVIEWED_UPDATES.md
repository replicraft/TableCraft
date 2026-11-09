# Fresh qualification of reviewed native UPDATEs

## Native plan boundary

`RecoverableNativeEditPlan::requalify` consumes a normal qualified UPDATE plan
and a fresh immutable `NativeImage`. It requires exact equality with the complete
reviewed BEFORE, verifies the original intent binding, reconstructs through the
same closed native profile, and compares complete rebuilt BEFORE/AFTER images
and effective changed-row count. Success retains the original intent unchanged.
There is no public constructor from replacement images or serialized requests.

The method performs no filesystem/journal I/O or SQL statement replay. Hosts own
fresh capture, file identity, exclusivity, durable namespace setup and cleanup.
Review should occur before acquiring an exclusive lease. Native required-program,
constraint, ownership and physical qualification remain in the unchanged normal
planner; a reviewed image or count alone cannot bypass those gates.

All ten current `NativeUpdateProfile` variants use one reconstruction owner:
Jet 4 Int32; ACE BeforeChange Int32/fixed; ordinary fixed, fitted, owned growth,
EOF allocation, IEEE, scalar and GUID. This profile selects mutation reconstruction;
file-format eligibility remains unchanged. Current Jet 4/ACE writer restrictions,
encryption limitations and unsupported programs still apply. No older-format
writer is enabled by this API.

The same private reconstruction service is used by generic native UPDATE recovery
and EOF recovery. Existing V1/V2 selection, EOF length rules and original intent
bytes remain unchanged. The specialized Jet 4 checked-journal API retains its
existing path. A failed profile decoder is never retried as another action.

## Work and failure handling

Fresh image capture has its normal allocation/read budget. Complete comparisons,
BEFORE digest verification, native reconstruction and borrowed request traversal
also consume the caller's work allowance. Cancellation is checked on no-op and
empty operations too. Hosts must fund those complete proof traversals explicitly;
requalification does not enlarge budgets or retry failed work.

`requalify_with_budgets` keeps native reconstruction and original request
traversal in the caller's native pool, while complete image comparisons and
original BEFORE hashing consume a separate proof pool. The existing single-pool
method remains available with its original cumulative charging order. Neither
method grants, resets or transfers allowance between pools. A proof pool cannot
finance additional native rows, allocations or constraint traversal.

The adapter's corresponding `stage_reviewed_update_with_budgets` also puts its
complete source comparison in the proof pool. That comparison accounts for both
byte streams, so the adapter proof pool needs byte allowance as well as work
steps; engine-only image comparison and hashing consume steps. Capture continues
to consume native bytes/rows/work. Successful capture followed by proof refusal
retains a Ready session; capture failure still requires explicit refresh.

These entrypoints separate fresh qualification only. For review-time planning,
`tablecraft_query::plan_batch_update_with_budgets` evaluates every original-row
expression and allocates the complete column requests under a query allowance,
then calls the consuming backend once under its independent allowance. Existing
single-pool APIs use the same preparation and finish without changing their
cumulative order or request semantics.

The Access `plan_recoverable_batch_update_with_budgets` consumer uses the native
allowance for snapshot construction and backend finish, including initial intent
hashing, and the query allowance for binding/evaluation/request materialization.
A query failure cannot trigger native finish or retry evaluation. The local-target,
parameter, collation, carrier and constraint restrictions remain unchanged.

Journal preparation/commit retain their existing explicit persistence budget.
The [runtime action service](../../application/RUNTIME.md#reviewed-sql-update-actions)
now supplies bounded query, native review, fresh proof and persistence allowances
for the qualified UPDATE route. Initial native intent hashing still belongs to
the combined native review allowance; fresh requalification's hashing belongs to
its separate proof allowance. Recovery uses an explicit finite combined pool.
These policies do not automatically enable GUI action confirmation or expand
native writer eligibility.

`NativeRecoveryQualificationError` preserves typed capture, intent, normal-planner
and image/count mismatch failures. Requalification creates no journal and writes
no target. Its consumed candidate is discarded on failure; any separately owned
filesystem resource remains the caller's responsibility.

## Stored field comparison policies

The optional query `MutationOptions` and
`plan_batch_update_with_options` route preserve field text identities through
the binder and complete original-row cells, supplying the existing resolver to
WHERE and SET evaluation. Existing shared-pool and split-pool APIs retain their
policy-free defaults and charge order. Scalar DB contracts stay query-neutral.

The Access `plan_recoverable_batch_update_with_options` consumer derives these
identities from actual native column declarations and aligns by field name and
carrier to the real scalar projection. Native schema capture and alignment spend
native allowance; validation and query evaluation spend query allowance. Supplied
policy numbers alone are not a native metadata source. Missing, ambiguous or
inconsistent alignment refuses; unknown/conflicting policies retain the existing
query errors. The runtime action service uses this route with its actual database
resolver instead of reducing every comparison to the database default.

Eager conditional branches retain the existing shared query policy rules: IIf
merges both result-branch policies. This change does not redefine mixed-policy
expressions, qualify additional native sort orders or provide missing wildcard,
clock, random or application contexts.

## Typed query-layer UPDATE parameters

`MutationOptions.parameters` explicitly admits a complete set of typed literal
bindings to the neutral batch planner. `None` preserves the existing refusal of
parameter declarations; `Some` requires every declaration exactly once, with no
extra inputs. Names follow the query schema comparison policy. Declared carriers,
text/decimal constraints, finite numbers and scalar/Variant provenance use the
same validation as retrieval. Parameter text is never parsed as SQL.

Binding and parameter validation precede row evaluation. Every WHERE and SET
expression still uses the original row, and the backend finish occurs only after
all rows succeed. Parameter comparison and text validation consume query work
and observe cancellation. The Access consumer accepts `NativeMutationOptions`,
derives stored-field policies itself and supplies the explicit typed bindings to
the neutral planner. Legacy planners without a parameter input contract still
refuse declarations.

The runtime validates literal envelopes and declared constraints from immutable
inspected SQL before native reads. Desktop routes confirmed scratch prompts into
that review; the original SQL and confirmed input allocations move into its
source-bound UI witness. Save consumes the sealed native plan without SQL or
parameter reevaluation. Prompt cancellation retains previous results and creates
no journal.

## Retained offline session boundary

`OfflineWriteSession::stage_reviewed_update` consumes the complete sealed query
`UpdatePlan<RecoverableNativeEditPlan>`, preserving its matched count. It captures
fresh bytes from the session's retained exclusive descriptor, requires the whole
reviewed BEFORE, then performs native requalification. It does not replay SQL or
create a journal. Explicit apply uses the existing staged-update journal, receipt
and recovery lifecycle.

Every failure returns the same session and its retained receipts. A failed capture
leaves the session requiring explicit refresh; a completed capture restores Ready
before subsequent qualification. Stale bytes are a typed source-change failure.
Hosts must obtain confirmation before acquiring the session's exclusive lease.

Five portable adapter cases pass: multirow SQL staging/save/reopen with an
independent complete AFTER; no-op/empty admission; protected-tail source changes;
cancellation/resource failure and explicit refresh; and prior-receipt retention.
The existing post-write fault case exercises both ordinary and reviewed staging
with fresh sessions, retaining its cancellation, lock, recovery and retry checks.
The broader adapter gate passed 105 unit tests and 15 offline-session integration
tests. Runtime's existing 160 tests also passed without integration changes.
Windows x64 strict lint compiled the offline-session library/tests; it did not
execute them. These checks do not yet establish a connected runtime/GUI SQL action.

## Verification — 8 November 2026

Nine focused native tests pass, covering all ten current mutation profiles and
both ACE 16 format headers where their existing raw fixtures provide them.
They check independent complete AFTER images, byte-identical encoded intents,
original profiles/counts, protected/unselected BEFORE changes on no-ops, a private
same-BEFORE/same-count mismatched AFTER, cancellation and resource limits.

Two of these tests are opt-in and were executed with the existing hash-pinned
[owned-growth](../../../tests/reports/rows/ACE16_OWNED_GROWTH_SOURCES.md) and
[EOF-allocation](../../../tests/reports/storage/ACE16_EOF_ALLOCATION_SOURCES.md) images. They remain
ignored by default so ordinary builds need no private cache. Those input bytes
were only read and remain unchanged. No producer or native Access process ran.

The broader default engine unit gate passed 1,286 tests with 260 ignored; the
qualified-recovery target passed 125 with 279 ignored. The selected two opt-in
cases are additional evidence, not a claim that every ignored case ran. Logs
remain under ignored `.cache/reviewed-update-*20261108.log`.

Seven selected existing opt-in recovery cases also passed against pinned
owned-growth, EOF and compound-EOF inputs. The first compound run lacked its
required source-directory setting; that configuration failure remains in its
log, and the corrected run passed. Strict affected-package lint, minimal-feature
checks and production-library cross-compilation passed for Windows x64/ARM64,
Intel macOS and Linux x64/ARM64. Cross-compilation is not platform execution.

This is local Rust qualification on macOS ARM64, separate from native Access
comparison, non-macOS execution and physical durability qualification. The later
[runtime and GUI workflow](../../ui/SQL_WORKSPACE.md#reviewed-local-action-queries) consumes
these capabilities; broader action support remains in the
[parity roadmap](../../features/ACCESS_PARITY_ROADMAP.md).

### Independent fresh-qualification pools — 8 November 2026

The existing all-profile native checks now exercise both the shared-pool and
split-pool entrypoints, retaining complete images, counts and identical original
intent bytes. Three additional native cases check independent exhaustion, equal
combined work charges and cancellation before final capability publication.
The focused native batch passed ten default cases and two separately executed
hash-pinned growth cases. Seven adapter cases passed, including exact proof-byte
admission and separate native/proof failures with the original lease retained.
Logs remain under ignored `.cache/update-budget-*20261108.log`.

### Independent query/backend batch planning — 8 November 2026

Ten neutral batch UPDATE tests pass, including three new split-pool cases for
query failure before backend finish, backend exhaustion after complete query
evaluation and original-snapshot multi-SET evaluation. Two initial assertions
incorrectly counted only row visitation; the recorded failed run was retained,
and corrected expectations also account for existing expression-row materialization.
The corrected batch passes without changing production accounting.

Two Access adapter cases additionally exercise real native planning against an
independent complete AFTER and distinguish native snapshot failure from query
refusal. These are API/fixture checks, not connected runtime or Access evidence.

### Runtime action ownership and field policies — 8 November 2026

The complete runtime unit batch passes 169 cases, including seven new SQL action
scenarios and two phase-budget cases. Actions cover Jet 4 and both supported ACE
16 headers, independent complete AFTER images, native reopen, lease-free review,
nonreplaceable saves, stale inspected identities/document invalidation, protected
tail drift, retained release, unchanged/empty counts and eager-error/read-only
refusals. No runtime action silently activates the desktop confirmation flow.

The field-policy journey retains the existing positive BeforeChange program and
varies only its Text descriptor between supported General Legacy and General
2010, keeping the database default unchanged. Actual row selection differs; the
independent complete AFTER includes the program-adjusted value, then the native
file is saved and reopened. An initial plain unindexed 15-column-catalog source
was rejected by its existing ordinary-content writer grammar before query
execution. That failed admission is retained in the log; it was not bypassed.
The positive journey uses an already-admitted program source instead.

Thirteen neutral batch tests and all 112 adapter unit cases pass. Initial compile
and test failures (borrowed-source lifetime, missing public field documentation,
an incorrect IIf policy expectation and an unsuitable encryption test header)
remain in ignored `.cache/runtime-query-update-*20261108.log`. Corrections retain
the shared eager conditional policy contract and existing native/encryption gates.
All execution evidence is local Rust/macOS qualification, separate from native
Access comparison and non-macOS execution.

Final validation also passed 127 desktop tests, strict affected-package lint,
minimal/offline-only feature checks, formatting, dependency boundaries and runtime
production-library cross-compilation for Windows x64/ARM64, Intel macOS and Linux
x64/ARM64. Logs use the `qualified-*` suffix in the same ignored directory. No
native app window or native Access process was launched.
