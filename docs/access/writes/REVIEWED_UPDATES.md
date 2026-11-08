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

`NativeRecoveryQualificationError` preserves typed capture, intent, normal-planner
and image/count mismatch failures. Requalification creates no journal and writes
no target. Its consumed candidate is discarded on failure; any separately owned
filesystem resource remains the caller's responsibility.

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
[owned-growth](../../testing/ACE16_OWNED_GROWTH_SOURCES.md) and
[EOF-allocation](../../testing/ACE16_EOF_ALLOCATION_SOURCES.md) images. They remain
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
comparison, non-macOS execution and physical durability qualification. Runtime
consuming action reviews and GUI UPDATE confirmation/execution remain subsequent
work in the [parity roadmap](../../features/ACCESS_PARITY_ROADMAP.md).
