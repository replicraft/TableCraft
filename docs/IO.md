# Bounded synchronous I/O

`tablecraft-io`, under `crates/platform/`, supplies format-independent read helpers
used by the native engine. It depends on core budgets and thiserror, imports no
engine/query/adapter/application packages, and has no optional features or runtime.

## Implemented operations

- `read_exact`: fill a supplied buffer, continuing short reads and retrying
  Interrupted within the same work allowance.
- `read_exact_at`: seek to an absolute byte offset, then perform the same checked
  exact read. It changes the source's cursor and requires exclusive mutable access;
  it does not provide concurrent OS positional reads or file coordination.
- `IoError`: sanitized ErrorKind, invalid reader byte count, cancellation or shared
  resource exhaustion. Host error messages, paths and payloads are not retained.
- SharedFile / ExclusiveFile: owned, nonblocking shared/exclusive OS file locks,
  sanitized LockBusy and explicit close reporting. They open existing regular
  files without creation/truncation and expose no raw/cloned handle. See
  [offline transaction scope](NATIVE_TRANSACTIONS.md#os-file-locks).

These reuse std Read/Seek. Callers charge requested bytes once before I/O; the
helpers charge one work step/check cancellation before each read attempt and once
on completion. Read-at-offset additionally checks/charges before seeking. Empty
exact reads still check completion. EOF returns Io(UnexpectedEof); other read/seek
errors retain only ErrorKind. A reader returning more bytes than requested fails
with InvalidReadCount. Interrupted read retries cannot escape the work budget.
Seek errors return immediately; there is no automatic seek retry.

A failure can leave partially filled caller buffers and a changed source position.
Consumers must discard partial data when their API promises complete results.
Budgets are cumulative and failed work is not refunded. Individual blocking host
callbacks still own timeouts; cancellation cannot interrupt a callback that has
not returned. The crate does not allocate payload buffers or choose byte ceilings.

## Native engine integration

ACE header/page reads use read-at-offset and translate IoError into the existing
InspectError contract: EOF to Truncated, other I/O kinds to Io, invalid counts to
Malformed, and cancellation/limits to their existing variants. Native page sizes,
packed references, allocation, records, codecs and structural ceilings remain in
ACE. Complete native snapshots still publish no partial results on read failure.
Absolute-seek work is now charged explicitly, so very small step allowances may
reject earlier than before extraction. CLI output and supported native formats
are unchanged.

Buffering/caches, journal durability protocols and async I/O remain unimplemented
here. ExclusiveFile supplies ordinary Write/flush and explicit sync_all; these
alone are not transactional writes. Jet/ACE shared-file locking rules belong in ACE. Shared fixture
builders and broader failpoint tooling remain candidates for tablecraft-testkit;
the native integration fault tests currently stay with their actual consumers.

## Verification

Focused tests cover interrupted retry exhaustion/cancellation, cancellation during
the final read, short chunks at a nonzero absolute offset, byte-charge ownership,
EOF/partial buffers, sanitized host read/seek errors, invalid byte counts and
pre-seek cancellation/work exhaustion. ACE tests verify error translation; existing
native snapshot/adapter fault-injection tests exercise the extracted implementation.
The standalone package check is included in CI; configured jobs do not establish
that other-platform or remote CI runs passed. See [IMPLEMENTATION.md](IMPLEMENTATION.md)
for the observed local verification.
