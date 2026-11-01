# Explicit filesystem journal pairing

The optional `tablecraft-io/filesystem-journal` feature composes the existing
[fixed V1 image journal](IMAGE_JOURNAL.md) with retained filesystem objects,
durable bundle setup and explicit retirement. It is generic I/O, not native
Access permission, a normal ACE save, macro discovery or shared-file coordination.
Initial filesystem implementation support is macOS/Linux; only macOS execution
has been observed. Executable hosts retain help and refuse unsupported platforms
before opening sources. Standalone IO defaults remain empty.

## Retained objects and original authority

`OfflineTarget::open` captures the actual parent and raw final component, opens a
regular file without following a final symlink, and acquires a nonblocking
exclusive lock. FIFO/special-file opens are nonblocking before the regular-kind
check. Read/seek and later storage use that same descriptor; there is no shared
lock upgrade or pathname reopening.

`OfflineJournalPair::create` moves the target and encoded opaque intent into a new
private bundle. It creates distinct binding/intent/journal objects, synchronizes
records and the empty journal, then synchronizes the bundle and captured parent
before native preparation can start. `open_existing` checks the selected actual
objects and bounded intent without interpreting or executing native content.

The fixed 184-byte `TCJPAIR1` binding records five object roles, an operation nonce,
intent length/hash and a record checksum. Device/inode values are binding checks
under stable object lifetime, not global immutable identities or authentication.
Mode0700/current-owner bundle and mode0600/current-owner children are checked;
they do not prove ACL privacy or defeat hostile namespace replacement. The host
excludes noncooperating writers and protects its private namespace, including
against writers to shared non-sticky parents. Renames/aliases do not change a
retained object; a byte-identical replacement does not establish its identity.

Binding and intent are data, never native authority. The native consumer must
independently qualify its original requests and images. In particular, normal
Jet4 recovery verifies the typed intent's BEFORE fingerprint, rebuilds the normal
plan and compares both full images before any recovery write. ACE engineering
previews remain non-journalable. No ordinary reader automatically discovers an
arbitrary selected bundle.

## Explicit preparation and completion

`storage()` lends two `PairedStorage` views inside one lexical scope and marks the
pair recovery-required before handing them out. Their private inner guards cannot
be replaced/cloned/unlocked by the borrower. Ordinary Write still relies on the
trusted host/protocol; absence of a resize API does not prevent extending a file.
Dropping a borrow does not acknowledge commit or recovery.

A failed/incomplete prepare can retain a truncated journal with target bytes
unchanged by preparation; malformed recovery must retain it for diagnosis. Every
commit error still requires explicit recovery. No automatic rollback, overwrite,
SQL/default/clock replay or namespace cleanup follows failure.

After acknowledged commit/recovery, `retire(outcome)` records that known result
BEFORE any fallible namespace action. It renames the INTACT bundle without
clobbering and synchronizes the held parent. Known rename plus failed barrier
permits explicit barrier-only `confirm_retirement`; unknown rename permits
diagnosis, never an automatic rename/recovery retry. Per-filesystem no-replace
support can fail at the actual callback; this preserves known target outcome and
retained resources rather than claiming the edit did not happen.

`cleanup_retired` removes only owned retired entries after acknowledged retirement.
Failures preserve known outcome and report possible retired garbage. `release`
attempts explicit lock release without deleting namespace entries. A failed unlock
may have taken effect: retained identity/FD does not prove exclusivity, and further
storage access is denied. `known_outcome` and `namespace_state` preserve facts
through release failure. Drop closes handles only.

Path buffers, report preparation and terminal namespace work are paid before
effects. Fixed retirement-name formatting/copy/validation and variable active-name
scans are included. A bounded one-slot vector for incomplete setup ownership is
fallibly reserved before target synchronization or namespace effects; late errors
retain capabilities without allocating. Budgets remain cumulative and are not
silently replenished after an error.

## Observed checks and remaining recovery scope

Six new read-only SOURCE functions plus the existing V1/V2 SOURCE functions passed.
The first readonly filesystem slice passed 15 focused functions. Initial factory4
and retirement7 functions passed, followed by the full IO81 unit, nine integration
and two documentation tests. Strict initially rejected large inline retained error
owners; the paid one-slot correction then passed strict all-target/all-feature,
full IO81+9+2 and minimal builds. Wire/source literals were unchanged. See the
[observed result](../testing/results/filesystem-journal-lifecycle-2026-10-30.json).
These are injected/local filesystem checks, not hardware power-loss or native
Access qualification. CLI validation is recorded separately by its actual host.

## Explicit intact-bundle restart finalization

`open_retired` repeats the five-object and intent checks and requires the exact
nonce-derived reserved retirement leaf. Active `open_existing` still refuses
that name. `confirmation_storage` lends retained views that deny `Write` and
`flush` before reaching either descriptor.

`CheckedV1::confirm_completed` requires exact AFTER for a committed marker or
exact BEFORE for a pending/partial marker, then synchronizes target and journal.
Opposite or mixed images refuse without repair. The native qualified method
retains the original-intent normal Jet4 replan and full BEFORE/AFTER comparisons.
Neither path replays SQL, copies back pending bytes or grants ACE writes.

The outcome describes currently verified complete bytes and acknowledged barriers,
not historical commit/rollback events or external effects. `acknowledge_retired`
records that fact before checking the selected entry and synchronizing only its
held parent; it never renames. Entry mismatch blocks cleanup and barrier-only
retry. Known entry plus failed parent barrier permits explicit barrier-only
confirmation, followed by existing cleanup and release.

Both SOURCE functions passed before implementation. The IO target passes seven
functions (SOURCE plus six API controls), and the native target passes four
(SOURCE plus three API controls). Strict IO/ACE lib/tests with all features, full
IO81 unit +16 integration +two documentation tests, and minimal build pass. See
[the restart confirmation result](../testing/results/intact-retired-confirmation-2026-10-30.json).
CLI adoption is recorded separately after its own checks.

Partial garbage or missing-child resolution, automatic discovery, Windows support,
broader filesystem/platform execution, hardware power-loss qualification and
normal ACE save admission remain separate work. Current filesystem execution is
macOS; Linux implementation support is not Linux runtime qualification.

## Retain the target after clean completion

`OfflineJournalPair::into_target_after_clean` consumes a pair only after
`RetiredClean`, a separately known outcome and a still-held target lease. It
acknowledges journal unlock and moves out the same `OfflineTarget`; the target
is never unlocked, reopened or cloned. The captured directory/name and actual
descriptor remain with that owner. Existing `release` keeps its original behavior.

`TargetTransferFailure` retains the cause and complete pair inline. An invalid
state is unchanged. Journal unlock failure leaves `ReleaseUncertain`, retains
`RetiredClean` as the namespace fact and preserves the known outcome. No target
or automatic retry is offered on failure. The two narrow large-error lint
allowances deliberately avoid allocating a boxed owner after completion.

Four local filesystem functions pass, covering committed and rolled-back
completion, uninterrupted target exclusion, path retargeting without reopening,
unsettled/unclean refusals and journal-unlock errors before and after their
possible effect. The earlier invocation failed at Cargo dependency selection
before compilation or tests; it supplies no protocol evidence. All-target,
all-feature strict checks pass, along with the complete IO regression: 85 unit
functions, 16 integration functions and two doctests (103 total). This transfer adds no native write eligibility,
Access locking, journal discovery or normal ACE save permission.

See the [target-transfer record](../testing/results/offline-journal-target-transfer-2026-10-31.json).

## Explicit retained-target resize capability

An active pair can issue `resizable_storage()`: its target view is a private
`ResizablePairedTarget` borrowing the same locked descriptor, while its journal
retains the ordinary bounded storage view. Issuance uses the existing state guard
and recovery-required transition. It does not open, clone, unlock or replace a
resource. Existing `storage()` behavior is unchanged; ordinary Write continues
to rely on the trusted protocol.

Retired confirmation still receives write-denied ordinary views and cannot
obtain the resize factory. V2 completion uses the same inspected journal and
requires complete settled bytes; namespace retirement and cleanup remain explicit.
Known outcomes survive finalization failures, and retained ownership prevents
normal access until the required cleanup is finished.

Five filesystem functions cover grow/shrink commit and rollback, path retargeting,
retired write-free confirmation, resize failures before/partway/after their real
effect, target/journal sync failures and retired garbage cleanup failure. They
retain the same lease throughout. Local runtime evidence is macOS; generic
protocol checks do not establish native Access or other-platform qualification.
See the [V2 verification record](../testing/results/checked-v2-retained-targets-2026-11-01.json).
