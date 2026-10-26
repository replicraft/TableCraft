# Bounded ACCDC read-only spooling

`AccessInput` supports complete CAB database extraction into host-owned storage
without retaining a whole cabinet or database image in a `Vec`. The source remains
read-only: package content does not choose a filesystem path, execute application
content or authorize native writes. Package signatures remain **unverified**.

## Explicit source selection

| Constructor | Qualified CAB selection | Storage factory |
| --- | --- | --- |
| `AccessInput::open` | Existing memory capture; physical package and database each at most 64 MiB | None |
| `AccessInput::open_spooled` | Explicit complete extraction into supplied storage | Called after bounded geometry qualification |
| `AccessInput::open_with_spool` | Existing memory capture only when both physical package and database fit 64 MiB; otherwise spooled extraction | Called only for the larger qualified CAB branch |

Raw native input retains its original `Read + Seek` source and never requests a
spool. Selection uses content and validated lengths. A failed capture/extraction
is not retried with another strategy. The hybrid constructor preserves the
existing small-package memory strategy and its ordinary default-budget behavior;
the additional streamed preflight still consumes cumulative work and bytes.

The existing public type keeps a default storage parameter:
`AccessInput<R, S = Cursor<Vec<u8>>>`. Both new constructors accept a lazy
`FnOnce() -> Result<S, InspectError>` factory, where `S: Read + Write + Seek`.
The factory must return fresh empty storage. A nonempty target is rejected before
writes; it is never truncated or reused. The returned source exposes only
`Read`, `Seek` and `NativeReadSource`, with `is_deployment()` true for either CAB
strategy.

Hosts own storage creation, permissions, source stability, I/O timeouts and
cleanup. The CLI uses its existing `tempfile` dependency to supply an unnamed
file. The adapter never creates paths, persists/renames a temporary file or
extracts using the stored CAB filename. Ownership drops on every failed open and
when the successful source is dropped. Generic host storage does not itself
promise deletion; an unnamed temporary file supplies its own cleanup semantics.
No durable save, sync/recovery or writable-session capability follows from this
read-only temporary extraction.

## Geometry, publication and trust

The selected profile retains exactly one folder and one file, folder zero and
zero file offset, no spanning/previous/next cabinets, a bounded path-free entry
name, and the existing uncompressed/MSZIP/LZX restrictions. Fixed metadata and
all data-block headers are read with bounded synchronous I/O before the decoder
or factory is requested. Offsets, block extents, expanded lengths, reserve sizes
and decoder geometry are qualified before library allocations.

CAB's single-folder member ceiling is `65535 × 32768 = 0x7FFF8000` bytes,
slightly below the native engine's separate 2 GiB addressable source envelope.
The CAB logical image is at most `0x7FFFFFFF` bytes. An outer overlay is separately
bounded to 64 MiB and is not presented to the decoder or returned as database
bytes. These are structural/addressability limits; the host's cumulative budget
can reject much smaller inputs. Default memory capture still pays for the full
physical image, including an overlay, before its validated CAB view is truncated.

The existing `cab` decoder remains responsible for decompression and nonzero
block checksums. Zero checksums are legal; CAB checksums are not signature or
publisher verification. The large path supplies a logical-end-bounded source,
uses a single 32 KiB output scratch buffer and completes every declared block
before publication. Exact decoded EOF, successful flush, exact spool extent,
ACE header qualification and rewind must all succeed before a source is returned.
A late error drops a partial spool instead of publishing a complete database.

Temporary writes preserve original selected bytes, including any database
password/encryption envelope. Native page decryption remains the separate reader
operation. The 64 MiB whole-image encryption add/remove/change-password and
mutation ceilings are unchanged.

## Shared budgets and failures

Preflight, callback I/O, decoder metadata/reserves, decoder workspace and scratch
use the same caller-supplied native budget and cancellation function. Block work
includes retained per-block reserve data; filename work accounts conservatively
for decoding expansion and cloned library metadata. No whole-image allocation is
charged or made by the spooled branch.

Byte accounting is cumulative work as well as allocation: source callbacks
prepay **requested** bytes, decoded chunks and spool writes are charged separately,
and short reads can conservatively charge requested bytes again. It is not a
measurement of peak retained memory or exactly the successful physical bytes.
No default allowance is raised to hide extraction work. The always-spooled path
can require more byte allowance than a small memory capture of the same file.

Cancellation and step checks surround factory/I/O/flush operations and bound
Interrupted retries. Budget borrows end before host callbacks. The first typed
host, cancellation or quota failure is retained through the decoder's I/O error
boundary; it is not replaced with a generic malformed-package error. Dependency
format/decompression failures without an earlier typed failure remain malformed.
Host error messages, paths and package names are not included in diagnostics.

## Scoped verification

The [scoped record](../../testing/results/read-query-accdc-spooled-2026-10-25.json)
records 56 passing checks: nineteen Access integration/layout controls and
thirty-seven CLI browsing/deployment/budget/password controls, with strict Access
and CLI linting and formatting. Fifteen controls are new.
They cover all four retained ACE header codes with stored/MSZIP extraction,
exact bytes and seeks, lazy factory selection, original default-budget small
capture, short/Interrupted writes, fresh-target refusal, factory/host/seek/flush
failures, typed decoder read failures, repeated Interrupted quota exhaustion,
late-checksum prefix discard, overlays, cancellation and cumulative limits.

The internal virtual sources establish ceiling arithmetic, a 65535-block header
walk, excessive payload/logical-image/overlay refusals, per-block reserve metadata
charges and strategy selection without large allocations. Their compressed bytes
are deliberately invalid: these are structural tests, not native database or
successful multi-gigabyte decompression evidence. The integration fixtures are
original small CAB layouts and the repository's previously registered example;
this record does not claim new native Access-created deployment qualification.

Primary/upstream references:

- [Microsoft CAB format specification](https://download.microsoft.com/download/4/D/A/4DA14F27-B4EF-4170-A6E6-5B1EF85B1BAA/%5BMS-CAB%5D.pdf)
- [Pinned CAB reader API](https://docs.rs/cab/0.6.0/cab/struct.Cabinet.html)
- [CAB reader implementation](https://docs.rs/cab/0.6.0/src/cab/cabinet.rs.html)
- [Unnamed temporary-file ownership](https://docs.rs/tempfile/3.27.0/tempfile/fn.tempfile.html)
