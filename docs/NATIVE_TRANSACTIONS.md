# Offline transaction and coordination foundations

## Immutable read transactions

`tablecraft_ace::OfflineReadTransaction::open` captures an explicitly supplied
offline database into a bounded owned image while holding a shared OS file lock.
It validates the header, charges both image allocation and read bytes, checks
length/header agreement after capture, and publishes no partial transaction.
The current native 64 MiB file ceiling applies. Callers provide budgets and
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

## Evidence and remaining write gates

An optional [generic image journal](IMAGE_JOURNAL.md) now supplies tested prepare,
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
