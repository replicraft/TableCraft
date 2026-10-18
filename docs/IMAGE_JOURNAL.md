# Fixed-length image transaction protocol

The optional `tablecraft-io/journal` feature implements prepare, commit, explicit
rollback and restartable recovery over host-owned DurableStorage resources.
ExclusiveFile implements that interface using its OS lock and sync_all. The
protocol is format-independent, experimental and not a Jet/ACE journal format or
native database writer. Default CLI/native-reader builds do not enable it.

## Ownership and durability boundary

The host must supply distinct, correctly paired target/journal resources with
exclusive access. It must make the journal's creation/directory entry durable
before preparing a transaction, retain that pairing across restart, and later
perform durable namespace cleanup. File sync alone does not establish directory
durability. The protocol deliberately does not create/discover/delete journals,
replace paths, upgrade locks, reuse existing journals or authenticate file identity.
An empty journal is mandatory for prepare; an existing one is never overwritten.

Successful storage sync must persist preceding contents and length before later
writes. These are explicit storage assumptions, not local power-loss evidence.
Filesystem behavior, correct target identity, namespace setup, native shared-file
coordination and ordinary-access exclusion remain host responsibilities. Do not
read a target normally after an interrupted/failed commit until recovery succeeds.

## Implemented ordering

Only equal, nonempty images up to 64 MiB are supported. Prepare reads the original
image, owns both images, writes their checked journal and synchronizes it. The
target is unchanged when PreparedImage is returned. Dropping that value retains
the prepared journal for explicit recovery; it does not silently delete it.

Commit first verifies that the target still equals the original image. It then
writes and syncs the new target, writes a digest-bound commit marker, and syncs
the journal. Any commit error is explicitly recovery-required: it may have
persisted some changes or even the marker. Error does not imply rollback.

Recovery verifies complete framing, lengths, SHA-256 and marker state before
writing anything. It verifies the entire paired target as well:

| Retained journal | Required target | Recovery outcome |
| --- | --- | --- |
| Complete images, no marker or valid marker prefix | Each byte equals its original or replacement counterpart | Restore/sync the complete original image; RolledBack |
| Complete valid commit marker | Exactly the replacement image | Preserve/sync the new image; Committed |
| Bad framing/digest/marker or mismatched target | No recovery writes | Typed failure; retain resources for diagnosis |

Rollback never overwrites a partial commit marker. A second interrupted overwrite
could create a mixed marker that cannot be recognized. Retaining the original
journal instead makes rollback restartable and repeatable. Completed commit and
rollback both retain the journal; a host must finish durable cleanup before a new
transaction. No resizing, multi-file commit, savepoints, page allocation or native
object semantics are supplied.

## Framing and resource bounds

The current original TableCraft framing consists of TCIMG001, a little-endian u64
image length, a 32-byte SHA-256, then original/replacement images. The digest covers
magic, length and both images. The optional final marker is COMMIT01 plus that
digest. Unknown framing, oversized images, trailing bytes and non-prefix partial
markers fail. This is not a compatibility promise for a released persisted format.

SHA-256 detects journal-content corruption; it is not authentication or encryption.
Journals contain complete original and replacement data. The host owns their
access policy and lifecycle along with target identity.

Fallible image allocations and requested read/write bytes consume cumulative
ResourceBudget bytes. Hashing and target comparison charge per-byte work; callbacks
and completion checks charge additional steps. Reads, writes, hashes and comparison
use at most 64 KiB blocks between cancellation checks. There is one fixed 64 KiB
comparison scratch buffer. Individual blocking callbacks still need host timeouts.
Failures do not refund allowances or trigger automatic retries/recovery. The
underlying bounded writer retries Interrupted only within its work allowance and
rejects zero progress or impossible byte counts. It does not flush implicitly.

## Verification and references

The deterministic storage model fails every callback in a successful commit and
recovery path, both before and after side effects. It forces three-byte short
writes, including partial markers, and tests both survival and loss of unsynced
bytes for each device. Completed barriers constrain those simulated crash states.
Recovery must produce a complete old/new image; failures during recovery must
remain recoverable. It also verifies journal-before-target and target-before-marker
barriers. Flush deliberately panics in this model so it cannot replace sync.

Other tests cover every journal truncation and individual-byte corruption,
independent framing/digest goldens, cancellation/limits, wrong targets, journal
reuse rejection and idempotent recovery. Two 256-case fixed-seed properties cover
arbitrary bytewise old/new intermediate images and bounded untrusted journal
inputs. Real disposable locked-file tests reopen retained journals and exercise
both outcomes; they do not simulate a physical power failure.

The ordering research uses [SQLite atomic commit](https://www.sqlite.org/atomiccommit.html)
and [locking/recovery](https://www.sqlite.org/lockingv3.html) as design references,
without copying SQLite code, journal layout or native locking assumptions.
[RustCrypto SHA-2](https://docs.rs/sha2/0.11.1/sha2/) supplies the digest implementation;
[dependency records](DEPENDENCIES.md#optional-image-journal-integrity) record the
feature/target/license closure. No homegrown hash is introduced.

```sh
cargo clippy -p tablecraft-io --all-targets --all-features --locked -- -D warnings
cargo test -p tablecraft-io --all-features --locked
```

These tests establish local protocol invariants under the modeled assumptions.
They do not qualify physical sector failures, arbitrary storage corruption,
network filesystems, Windows/Linux execution or native Access behavior. Native
writes remain unavailable until the [remaining engine gates](NATIVE_TRANSACTIONS.md#evidence-and-remaining-write-gates)
are implemented and independently qualified.

## Validated native consumer

`prepare_expected` additionally binds a caller-validated complete before image.
A changed target fails before journal/target writes, with bounded chunk comparison
and work charging. The optional [native edit integration](NATIVE_WRITES.md#optional-journaled-persistence)
uses this entry point; ordinary `prepare` retains its existing contract.
