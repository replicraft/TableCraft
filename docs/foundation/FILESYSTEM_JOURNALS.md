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

Restart finalization remains incomplete: exiting after a retirement error loses
the same-owner barrier-confirm capability, and active reopening refuses reserved
retired names. A later explicit read-only completed-target verification and retired
bundle resolution path is needed. Never blindly replay pending recovery or SQL.
Successful apply/recovery and same-process confirmation do not establish a full
crash lifecycle. Windows support, broader filesystem/platform execution, native
shared-file rules and ACE write admission remain separate work.
