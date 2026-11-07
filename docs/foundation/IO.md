# Bounded synchronous I/O

`tablecraft-io`, under `crates/platform/`, supplies format-independent bounded I/O
and OS lock guards used by the native engine. It consumes core budgets, thiserror
and tracing, without engine/query/adapter/application imports or a runtime.
Default features are empty; optional journal enables the working
[image transaction protocol](IMAGE_JOURNAL.md) and its SHA-256 dependency.

## Implemented operations

- `open_regular_readonly` (optional `fresh-publication` feature): opens an
  existing input read-only and checks the actual descriptor is a regular file.
  Unix uses nonblocking open so a FIFO without a peer refuses immediately.
  Regular symlinks retain ordinary open behavior. The helper provides no lock
  or stable-snapshot guarantee; other platforms retain standard open behavior.
  CLI write-file inputs consume it. Three focused tests and the existing TSV
  write-file regression pass; see the
  [input-opening record](../testing/results/regular-input-opening-2026-10-31.json).
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

## Prefix-protected image journal storage

The optional `journal` feature exposes `BoundedJournalRegion`. It exclusively
borrows an existing `DurableStorage`, verifies its physical length against a
checked prefix/cap and establishes logical offset zero after that prefix. V1 and
V2 journals can use it without changing their record formats. The host supplies
and qualifies the prefix; the region proves no identity, binding, namespace or
native write permission.

Logical seeks and transfers cannot reach the prefix or exceed the cap. Writes
past logical EOF refuse instead of creating holes. Reads, writes and seeks add no
hidden metadata or positioning callbacks; normal I/O helpers retain cumulative
work accounting. Constructor callbacks use explicit resource and cancellation
checks. The view does not implement target-resizing capability.

Length queries and flush/sync callbacks may move an arbitrary backend's cursor,
so subsequent transfers require an explicit absolute seek. A non-Interrupted
transfer error also invalidates the cached length; callers must refresh actual
length and reseek before recovery. Interrupted retries retain the existing I/O
contracts. Underlying sync provides the actual barrier; a region adds no stronger
durability guarantee.

Actual V1 preparation, commit and pending recovery, and V2 preparation, commit
and recovery preserve the complete prefix in local tests. Fault cases cover
partial transfers, invalid counts/positions, moving callbacks, cap/overflow/hole
refusal, callback counts and exact/one-below setup work and cancellation. The full
121-case IO library suite passes on macOS. Those historical region-adapter
results alone do not qualify the later Windows filesystem pairing or application
Save integration described below.

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
terminal publication/durability work is reserved before visibility. Known successful
publication always attempts its terminal durability callback even after cleanup failure, with
no later cancellation or shared-budget rejection. An unknown publication-call
outcome instead stops, retains possible staging entries and reports
`VisibilityUncertain` without another namespace callback. No failure authorizes
automatic retry or deletion of the final entry. `NotPublished`,
`PublishedUnconfirmed` and `PublishedCleanupFailure` distinguish the other
visibility/barrier outcomes; failed cleanup means a temporary *may* remain.
Blocking filesystem callbacks retain their own timeout responsibilities.

Windows has a separate local-NTFS publisher. Capture holds the actual parent
handle without delete sharing, rejects a reparse-point parent, and queries that
handle's filesystem and device information. The admitted destination is an
absolute drive path with ordinary validated UTF-16 components. An ordinary
relative path resolves once during capture; drive-relative and root-relative
forms refuse. UNC/device paths,
alternate streams, reserved DOS names, trailing dots/spaces and unsupported
filesystems refuse before staging. Ancestor pathname resolution is not an
attestation that every ancestor is free of reparse points; publication uses the
parent actually captured, not a later interpretation of its path.

A random staging file is created exclusively relative to that parent through
`NtCreateFile`, with synchronous write-through I/O and no sharing. The retained
file is written and flushed, then renamed through the same handle using
`SetFileInformationByHandle(FileRenameInfo)`, a parent-relative final name and
replacement disabled. A known successful rename consumes the temporary name;
cleanup never deletes that final file. The terminal operation flushes the retained
renamed file. A collision refuses without replacing the existing output; other
unresolved rename failures retain the visibility-uncertain outcome.
[FILE_RENAME_INFO](https://learn.microsoft.com/en-us/windows/win32/api/winbase/ns-winbase-file_rename_info)
documents parent-relative names and no-replacement behavior.

The receipt exposes `PublicationGuarantee`: Unix returns
`CapturedParentBarrier`; the Windows route returns `NtfsWriteThroughRename`.
`namespace_synced()` is true only for the separately acknowledged captured-parent
barrier. The NTFS route does not fabricate a Unix-style directory flush. Its
contract relies on Microsoft's documented write-through metadata behavior,
including renames, plus the retained file's terminal flush.
[CreateFileW caching behavior](https://learn.microsoft.com/en-us/windows/win32/api/fileapi/nf-fileapi-createfilew#caching-behavior)
and [FlushFileBuffers](https://learn.microsoft.com/en-us/windows/win32/api/fileapi/nf-fileapi-flushfilebuffers)
describe those operations. On Unix, the separate file/directory barrier distinction
follows [fsync](https://man7.org/linux/man-pages/man2/fsync.2.html).

This Windows implementation and its API contract are separate from verification:
Windows filesystem execution, native Access open/save and hardware power-loss
qualification have not been performed locally. The historical macOS results below
do not establish those Windows outcomes. Unsupported routes do not downgrade to
another publication method.

Checked Windows filesystem calls now have one private `windows_fs` owner,
shared by the required publisher. Namespace policy and cumulative accounting
remain with its caller. This extraction supplies actual handle identity, local
NTFS qualification, exclusive write-through creation, no-replace rename, file
flush and owned-stage deletion-on-close primitives. Fresh publication and the
separate flat filesystem journal now consume that same platform owner; the
extraction alone did not establish native Save or Windows execution evidence.

That Windows-gated module is IO's sole local unsafe-code exception. IO declares
`unsafe_code = "deny"` instead of inheriting the workspace's non-overridable
`forbid`; workspace `forbid` and Clippy `all = "warn"` remain unchanged elsewhere.
The exception covers the documented Windows ABI calls, bounded aligned buffers
and adoption of newly owned handles. Each unsafe block records its lifetime,
layout or ownership conditions. This remains a scoped implementation boundary,
not independent unsafe-code or Windows runtime qualification.


The focused publication tests exercise sanitized pre-publication faults,
visibility-uncertain stop behavior, post-publication cleanup/barrier failures,
pre-reserved cleanup/terminal pools, final cancellation, actual no-clobber writes,
parent-path retargeting, invalid raw leaf spellings, and exact/one-below work and
payload pools. These are injected and local filesystem observations, not a
power-loss qualification of the filesystem/device or another operating system.
The first strict check found only a test-only function-pointer type-complexity
lint, corrected by a named alias without behavior or trace changes.
See the [observed IO result](../testing/results/io-required-fresh-publication-2026-10-30.json).


## Windows same-file journal integration

With `filesystem-journal`, Windows selects a separate retained flat-sidecar
implementation of `journal::filesystem`. The qualified storage target is local
NTFS; unsupported filesystems/network locations refuse instead of falling back
to another publication or lock protocol. The Access offline session and runtime
Save worker now compose this owner. Implementation and cross-compilation are
separate from actual Windows execution, which remains unrun locally.

The sidecar contains an immutable binding/intent prefix and a bounded,
region-relative V1/V2 image journal. Its closed identity scheme retains the
volume serial and complete 128-bit file ID for the target, sidecar and both
captured parents. It does not reuse Unix device/inode slots. The checked binding
validates role aliases, exact prefix/journal offsets, intent length/digest and
checksum before exposing the journal region. Region construction performs no
hidden I/O; explicit storage calls retain accounting and checked length/seek
behavior. Stored intent/names grant no authority to discover or open a target.
[Microsoft's FILE_ID_INFO contract](https://learn.microsoft.com/en-us/windows/win32/api/winbase/ns-winbase-file_id_info)
describes the underlying identity fields.

Setup creates the sidecar with an explicit protected current-user DACL before
writing original/replacement data or intent. It writes and flushes the complete
prefix, verifies retained objects, publishes the active name without replacement
and flushes the same retained file before exposing native writable storage.
Reopening verifies actual security and binding rather than trusting a pathname.
The target and parent handles deny sharing that could replace their association;
acknowledged release closes the relevant handles. A close error retains its
owner, reports uncertain release and forbids automatic retry. Unix file-unlock
semantics are not substituted for Windows handle release. See Microsoft's
[security descriptor controls](https://learn.microsoft.com/en-us/windows/win32/secauthz/security-descriptor-control)
and [handle-sharing contract](https://learn.microsoft.com/en-us/windows/win32/api/fileapi/nf-fileapi-createfilew#parameters).

After separately acknowledged native completion, the portable terminal core
checks phase/outcome and spends only a previously reserved terminal permit.
The Windows adapter queries all four actual retained file identities before
renaming the write-through sidecar to its reserved nonce-derived name without
replacement, then flushes that file. A collision blocks retirement; another
rename error leaves an uncertain outcome and cannot replay rename. A successful
rename with failed flush permits one explicit held-file barrier retry. Retired
confirmation verifies identities and flushes without renaming or replaying a
native edit. Known native completion survives every namespace failure. This is
`RetirementGuarantee::NtfsWriteThroughRename`, not a fabricated Unix directory
barrier. See [FILE_RENAME_INFO](https://learn.microsoft.com/en-us/windows/win32/api/winbase/ns-winbase-file_rename_info)
and [write-through metadata behavior](https://learn.microsoft.com/en-us/windows/win32/api/fileapi/nf-fileapi-createfilew#caching-behavior).

Windows uses `RetiredCleanupPolicy::RetainIntact`. Successful strict
`into_target_after_retirement` closes sidecar resources and returns the same
still-held target only from `Retired` with a known outcome; the complete retired
artifact remains for explicit later disposal. The receipt reports
`CleanupDisposition::RetainedArtifact`. `into_target_after_clean` refuses and
Windows never claims `RetiredClean`, durable deletion or Unix unlink-plus-parent
barriers. A failed cleanup is never retried as retention. Ordinary Windows
[deletion](https://learn.microsoft.com/en-us/windows/win32/api/fileapi/nf-fileapi-deletefilew)
and [flush](https://learn.microsoft.com/en-us/windows/win32/api/fileapi/nf-fileapi-flushfilebuffers)
contracts do not supply that removal guarantee.

Portable binding/terminal fault tests exercise the actual core used by Windows,
including exact callback traces and bounded retry/refusal states. They do not
execute Windows filesystem APIs. Native Windows create/save/reopen, security and
sharing behavior, crash/power-loss durability and Access comparison remain
required qualification; historical macOS passes do not establish them.

## Inspecting a versioned image journal

`journal::inspect_image` reads the exact eight-byte discriminator and selects
one V1 or V2 inspector. Unknown, short and corrupt framing refuses; a failed
inspection never retries another decoder. `CheckedImage` retains the selected
fully checked journal and the same exclusive mutable storage borrow. It grants
no native format admission or target identity. Native consumers must separately
rederive their original requests and compare complete images before recovery.

Five focused tests cover literal framing, truncation, corruption, partial commit
markers, selected-inspector traces, exact byte/step limits and every measured
cancellation boundary. Fixtures forbid writes, flushes and syncs during inspection;
loaded byte views require no later reads. A compile-fail example proves that the
borrowed journal cannot be replaced while its checked session remains live.
