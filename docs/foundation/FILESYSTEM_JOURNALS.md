# Retained filesystem journal association

The optional `tablecraft-io/filesystem-journal` feature adds
`journal::filesystem`. It composes the existing journal and held-directory
primitives without importing native formats, adapters or query execution.
Current functionality opens and validates an existing bundle; durable bundle
creation, retirement and CLI consumers remain implementation work.

## Retained resources

`OfflineTarget::open` captures the actual parent and final component, opens an
existing regular file without following a final symlink, and acquires one
nonblocking exclusive OS lock. Capture and later paired access retain that same
descriptor. Explicit hard-link aliases can name the same object; copied files
with equal bytes are different objects. Symlinks and nonregular files refuse.
FIFO opens are nonblocking before the kind check, so refusal needs no peer.

`OfflineJournalPair::open_existing` moves this target owner and opens the selected
bundle relative to its held directory. Fixed children are `binding`, `intent`
and `journal`. The bundle and children must satisfy the current-user private
0700/0600 profile; ordinary database permissions are unchanged. The owner checks
actual object identities, distinct roles and exact bounded intent bytes before
returning. Its caller supplies the intent ceiling; the native consumer uses
`NativeInt32EditIntent::MAX_ENCODED_BYTES`.

The 184-byte `TCJPAIR1` record binds the operation nonce, five Unix device/inode
pairs, intent length and SHA-256 digest, with a checksum over its fixed prefix.
The identifiers describe retained objects under stable-lifetime/private-namespace
preconditions. They are neither permanent global IDs nor authentication or an
anti-replay registry. Checksums do not authorize native mutation. Callers must
exclude noncooperating writers and namespace replacement, including same-user
actors and permissions not represented by ordinary mode bits.

## Access and completion

`storage()` lends two `PairedStorage` views in one lexical scope. Their private
inner guards cannot be replaced through a mutable slot, cloned or unlocked
through the view. No explicit truncation/resize capability is exposed; ordinary
Write/Seek still require correct host use. The generic/native journal protocol owns image
checks and actual recovery. Association alone parses no native program, grants
no native write profile and does not make normal reads safe after interruption.

An opened pair starts `RecoveryRequired`. Explicit `note_outcome` records an
already acknowledged protocol result; it does not execute that protocol.
`known_outcome` remains separate from lifecycle state. A failed release makes
lock ownership uncertain and prevents further storage access, while retaining
any known result. Setup errors retain the original target owner for diagnosis.
Dropping owners closes handles without deleting target or bundle entries.

The implementation is enabled for macOS and Linux. Unsupported Unix targets
refuse before source capture; executable hosts must provide their own unsupported
handler on other targets. Local verification here is macOS, not another platform,
native Access qualification or a physical power-loss test.

## Verification

The independent source qualifiers ran first, covering twelve literal binding
records, observed private ownership, original target bytes, existing bundle roles
and a real FIFO fixture. All fifteen filesystem functions pass, including
identity replacement, aliases, changed textual parents, exact/short budgets,
final cancellation and uncertain unlock controls. The complete I/O run passes
70 unit tests, nine integration tests and two compile-fail documentation tests.
All-target/all-feature strict checks and the minimal I/O build pass. The existing
ten publication tests retain their behavior and resource checks.
