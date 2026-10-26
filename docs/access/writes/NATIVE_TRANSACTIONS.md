# Offline transaction and coordination foundations

## Immutable read transactions

`tablecraft_ace::OfflineReadTransaction::open` captures an explicitly supplied
offline database into a bounded owned image while holding a shared OS file lock.
It validates the header, charges both image allocation and read bytes, checks
length/header agreement after capture, and publishes no partial transaction.
A separate 64 MiB owned-image ceiling applies before image allocation; larger
read sources do not become complete transactions. Rejection releases the acquired
OS lock. Callers provide budgets and
cancellation; individual filesystem calls still own blocking/timeout behavior.

The transaction implements Read + Seek, so existing catalog/table/index/check
operations and NativeSource can read it directly. Each transaction owns a cursor;
subsequent reads use captured bytes. It exposes no Write implementation, raw file
handle, clone or native commit operation. Dropping it releases its owned image and
lock; explicit close reports unlock failure. Existing generic readers and CLI defaults still accept stable/offline
sources without implicitly copying an entire file.

This is an offline read transaction, not a native Access shared-file snapshot.
The caller must exclude noncooperating writers during capture. Header/length
checks cannot detect every concurrent edit. This read API supplies no write transaction, savepoint, mutation or live
Access interoperability. The separate bounded native journal consumer is below.

## OS file locks

`tablecraft-io::SharedFile` and ExclusiveFile open existing regular files with
nonblocking shared/exclusive std file locks. They never create/truncate a file or
invent a lock-file protocol. Contention returns sanitized LockBusy immediately;
other failures preserve ErrorKind without host messages/paths. No retry or
lock upgrade is automatic. Private handles prevent cloning/unlocking through the
wrapper. Explicit close reports unlock failure; Drop attempts release and closes
the handle as fallback. Explicit unlock avoids extending the guard's lifetime
through descriptors transiently inherited by another thread's child process.

The [standard library contract](https://doc.rust-lang.org/std/fs/struct.File.html#method.try_lock)
permits platform/filesystem-dependent interactions with non-locking I/O and
renames. These locks coordinate users of the same OS lock mechanism; they do not
implement Microsoft's record/page locks or Access's shared-file protocol. No
network filesystem, Windows or Linux execution has been qualified locally.

## CLI read coordination

Native CLI input now uses SharedFile for raw MDB/ACCDB reads, encrypted-page
sources, CAB capture, template detection/capture and encryption-copy source reads.
A cooperating exclusive writer causes immediate sanitized LockBusy before file
interpretation, credentials or record/output publication. Shared readers coexist;
no waiting, lock upgrade or retry is added. A raw native handle retains its guard
through cursor/query use and closes on success or error. CAB/template capture
uses owned bytes after successful capture and can release the source guard then.
The existing offline query-update source already used SharedFile.

This uses the OS mechanism described above, not Access's ldb/laccdb coordination.
An offline/stable source remains required. Noncooperating writes, renamed paths,
network filesystem semantics and coherent capture during live Access edits remain
outside this guarantee. No whole-file copy is imposed on ordinary native reads;
OfflineReadTransaction remains the separate bounded immutable-image API.

## Evidence and remaining write gates

An optional [generic image journal](../../foundation/IMAGE_JOURNAL.md) now supplies tested prepare,
commit, rollback and recovery ordering over explicitly paired storage. Its format
is independent of Access. Optional [native edit plans](NATIVE_WRITES.md#optional-journaled-persistence)
now consume it with complete validated preimage binding. Host-controlled
durable namespace setup/cleanup and identity are explicit preconditions. Default
CLI/native-reader builds do not enable the journal feature or its hash dependency.

Original tests verify independent shared handles, writer exclusion, contention
and release across a child process, no file creation on failed open and sanitized
errors. Native tests read the complete registered fixture image and run existing
table checks through captured transactions. Cancellation, memory allowance and
busy-open failures release acquired resources; a compile-fail test prevents use
as a writer. The registered fixture is copied before filesystem experiments.

The implemented writer admits only its bounded offline Jet 4 fixed Int32 profile.
ACE/data macros, allocation/index mutation, broad application preservation and
native shared-file/power-loss qualification remain separate gates. Read-only
snapshots and generic OS locks do not establish those guarantees.
