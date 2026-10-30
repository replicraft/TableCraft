# Bounded synchronous I/O

`tablecraft-io`, under `crates/platform/`, supplies format-independent bounded I/O
and OS lock guards used by the native engine. It consumes core budgets, thiserror
and tracing, without engine/query/adapter/application imports or a runtime.
Default features are empty; optional journal enables the working
[image transaction protocol](IMAGE_JOURNAL.md) and its SHA-256 dependency.

## Implemented operations

- `seek_absolute` / `seek_to_end`: bounded absolute and End(0) targets, including
  Interrupted retries, position checks where known and completion cancellation.
  Relative seeks are deliberately absent from this retry API.
- `read_exact`: fill a supplied buffer, continuing short reads and retrying
  Interrupted within the same work allowance.
- `read_exact_at`: seek to an absolute byte offset, then perform the same checked
  exact read. It changes the source's cursor and requires exclusive mutable access;
  it does not provide concurrent OS positional reads or file coordination.
- `write_exact_at`: bounded seek/short-write/Interrupted handling, without implicit
  flush, sync or atomicity. Impossible counts and zero progress return typed
  errors; partial changes require an appropriate recovery protocol.
- `IoError`: sanitized ErrorKind, invalid reader byte count, cancellation or shared
  resource exhaustion. Host error messages, paths and payloads are not retained.
- SharedFile / ExclusiveFile: owned, nonblocking shared/exclusive OS file locks,
  sanitized LockBusy and explicit close reporting. They open existing regular
  files without creation/truncation and expose no raw/cloned handle. See
  [offline transaction scope](../access/writes/NATIVE_TRANSACTIONS.md#os-file-locks).

These reuse std Read/Seek. Callers charge requested bytes once before I/O; the
helpers charge one work step/check cancellation before each read attempt and once
on completion. Absolute seek helpers likewise charge each attempt and successful
completion; read/write-at-offset use them. Empty exact reads still check
completion. EOF returns Io(UnexpectedEof); other read/seek
errors retain only ErrorKind. A reader returning more bytes than requested fails
with InvalidReadCount. Interrupted read retries cannot escape the work budget.
Only Interrupted seek errors retry the same absolute/End(0) target under the
existing budget; other seek errors return immediately. Both absolute read/write
helpers compare the returned position with the requested offset, including for
empty buffers. A mismatch returns payload-free InvalidSeekPosition before any
read/write callback, with the seek work retained and no automatic retry of an
invalid result. This cannot detect a source that falsely reports the requested
position.

A failure can leave partially filled caller buffers and a changed source position.
Consumers must discard partial data when their API promises complete results.
Budgets are cumulative and failed work is not refunded. Individual blocking host
callbacks still own timeouts; cancellation cannot interrupt a callback that has
not returned. These read/write helpers do not allocate payload buffers or choose
byte ceilings; optional image journaling has explicit allocation/size policies.

## Native engine integration

ACE header/page reads use read-at-offset and translate IoError into the existing
InspectError contract: EOF to Truncated, other I/O kinds to Io, invalid counts to
Malformed, and cancellation/limits to their existing variants. A reported
seek-position mismatch also maps to Malformed before native record publication. Native page sizes,
packed references, allocation, records, codecs and structural ceilings remain in
ACE. Complete native snapshots still publish no partial results on read failure.
Absolute-seek work is now charged explicitly, so very small step allowances may
reject earlier than before extraction. CLI output and supported native formats
are unchanged.

Buffering/caches and async I/O remain unimplemented here. ExclusiveFile supplies
ordinary Write/flush and explicit sync_all. Optional fixed-length image journaling
adds tested barrier/recovery ordering under explicit host durability preconditions;
it is not a native database writer or shared-file protocol. Jet/ACE shared-file
locking rules belong in ACE. Shared fixture
builders and broader failpoint tooling remain candidates for tablecraft-testkit;
the native integration fault tests currently stay with their actual consumers.

## Verification

Focused tests cover interrupted retry exhaustion/cancellation, cancellation during
the final read, short chunks at a nonzero absolute offset, byte-charge ownership,
EOF/partial buffers, sanitized host read/seek errors, invalid byte counts and
pre-seek cancellation/work exhaustion. ACE tests verify error translation; existing
native snapshot/adapter fault-injection tests exercise the extracted implementation.
The standalone package check is included in CI; configured jobs do not establish
that other-platform or remote CI runs passed. See [IMPLEMENTATION.md](../IMPLEMENTATION.md)
for the observed local verification.


The native consumer also observes cancellation immediately after its page-decoder
hook, before parsing or retaining decoded bytes. This hook stays in ACE, separate
from generic raw I/O. [Continuation fault coverage](../access/storage/NATIVE_READ_FAILURES.md)
checks read/seek/decode errors, terminal cursors and interrupted short reads.

## Interrupted absolute seeks

Interrupted attempts may have changed the host cursor. Repeating Start(offset)
still addresses the original byte target; relative-to-current retries would not
have that property. End(0) is repeated only as an explicit length observation of a
caller-owned stable source. Its result is not a promise that a live file retained
its size. [Rust Seek](https://doc.rust-lang.org/std/io/trait.Seek.html#tymethod.seek)
and [Interrupted](https://doc.rust-lang.org/std/io/enum.ErrorKind.html#variant.Interrupted)
provide the underlying cursor/error contracts.

A successful seek now has its own completion work/cancellation check before
subsequent reads or writes, including empty buffers. This adds one successful-seek
work step to existing at-offset calls; byte charging remains with the caller.
Incorrect reported absolute positions fail before data I/O, and non-Interrupted
errors—including WouldBlock—are never retried. Partial cursor/data changes remain
possible on failure; no retry allocates a fresh allowance or promises atomicity.

Native header length inspection, complete-image length revalidation and Agile
header revalidation now use the same bounded End(0) helper. Native page reads and
the existing writer already use the shared absolute I/O entry points. Generic
checkpoint ownership remains in I/O; Jet/ACE page geometry, encryption, locking,
constraints and transaction qualification remain in their owning layers.


## Explicit required fresh-copy publication

The optional `fresh-publication` feature adds
`publication::RequiredOutput::capture` and its consuming `publish` operation.
It is a generic fresh-output protocol, not an image journal or native-save
permission. The caller opts in; the existing host publisher remains unchanged.
The first macOS all-feature library check and initial eight focused publication
functions passed. Static review then corrected name-buffer allocation accounting
and reserved-final staging-name handling; all ten focused functions passed with
those controls. The final full IO suite passed 49 functions, the IO minimal build
passed, and all-target/all-feature strict checks for IO, Access and CLI passed.
The 20 pre-implementation fault-trace literals are design expectations, distinct
from the ten executed focused test functions.

The Unix implementation captures the actual parent directory, preflights its
barrier, creates a private random staging directory, writes and synchronizes a
regular payload, then creates a no-clobber final hard link relative to the held
parent. Cleanup addresses only the operation's own staging entries. A successful
receipt confirms the barrier on that captured directory, subject to the
filesystem/device honoring it. Retargeting the original textual path does not
redirect publication to the replacement directory. Callers needing a stable
textual destination must separately maintain that namespace. Callers must also
prevent concurrent replacement of private staging entries by any actor able to
change those entries, including writers to a shared non-sticky parent. Mode0700
protects the staging contents but does not prevent such parent-entry changes;
identity checks followed by cleanup do not provide atomic protection against that
excluded race. Concurrent creation of the final name remains safely refused.

Payload bytes are charged once by IO. Cleanup work is reserved before staging;
terminal publication/barrier work is reserved before visibility. Known successful
publication always attempts the parent barrier even after cleanup failure, with
no later cancellation or shared-budget rejection. An unknown publication-call
outcome instead stops, retains possible staging entries and reports
`VisibilityUncertain` without another namespace callback. No failure authorizes
automatic retry or deletion of the final entry. `NotPublished`,
`PublishedUnconfirmed` and `PublishedCleanupFailure` distinguish the other
visibility/barrier outcomes; failed cleanup means a temporary *may* remain.
Blocking filesystem callbacks retain their own timeout responsibilities.

Required Windows durability currently returns `UnsupportedDurability` before
staging. Directory handles and file-content synchronization alone are not claimed
to establish a Windows namespace barrier. The documented
[MoveFileExW](https://learn.microsoft.com/en-us/windows/win32/api/winbase/nf-winbase-movefileexw)
WRITE_THROUGH behavior is not substituted for this descriptor-relative Unix
protocol. Existing cross-platform content-sync publication remains available.
On Unix, the separate file/directory barrier distinction follows
[fsync](https://man7.org/linux/man-pages/man2/fsync.2.html).


The focused publication tests exercise sanitized pre-publication faults,
visibility-uncertain stop behavior, post-publication cleanup/barrier failures,
pre-reserved cleanup/terminal pools, final cancellation, actual no-clobber writes,
parent-path retargeting, invalid raw leaf spellings, and exact/one-below work and
payload pools. These are injected and local filesystem observations, not a
power-loss qualification of the filesystem/device or another operating system.
The first strict check found only a test-only function-pointer type-complexity
lint, corrected by a named alias without behavior or trace changes.
See the [observed IO result](../testing/results/io-required-fresh-publication-2026-10-30.json).
