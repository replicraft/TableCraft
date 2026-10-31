# Image transaction protocols

The optional `tablecraft-io/journal` feature implements prepare, commit, explicit
rollback and restartable recovery over host-owned DurableStorage resources.
ExclusiveFile implements that interface using its OS lock and sync_all. The
protocol is format-independent, experimental and not a Jet/ACE journal format or
native database writer. An additive V2 API supports independently sized images
through explicit resizable storage; the original fixed-length V1 APIs and bytes
remain unchanged. Standalone IO defaults remain empty; merely enabling a journal
feature does not discover, prepare or recover a transaction while opening data.

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

## Fixed-length V1 ordering

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
transaction. V1 supplies no resizing, multi-file commit, savepoints, page allocation or native
object semantics.

## Framing and resource bounds

The original V1 TableCraft framing consists of TCIMG001, a little-endian u64
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
[dependency records](../development/DEPENDENCIES.md#optional-image-journal-integrity) record the
feature/target/license closure. No homegrown hash is introduced.

```sh
cargo clippy -p tablecraft-io --all-targets --all-features --locked -- -D warnings
cargo test -p tablecraft-io --all-features --locked
```

These tests establish local protocol invariants under the modeled assumptions.
They do not qualify physical sector failures, arbitrary storage corruption,
network filesystems, Windows/Linux execution or native Access behavior. Native
writes remain unavailable until the [remaining engine gates](../access/writes/NATIVE_TRANSACTIONS.md#evidence-and-remaining-write-gates)
are implemented and independently qualified.

## Validated native consumer

`prepare_expected` additionally binds a caller-validated complete before image.
A changed target fails before journal/target writes, with bounded chunk comparison
and work charging. The optional [native edit integration](../access/writes/NATIVE_WRITES.md#optional-journaled-persistence)
uses this entry point; ordinary `prepare` retains its existing contract.

## Read-only V1 inspection sessions

`inspect_v1(journal, budget, cancelled)` checks the existing V1 framing, equal
image lengths, digest and complete or partial marker without supplying or
changing a target. It reads/seeks the journal but neither writes nor synchronizes
it. The returned `CheckedV1` owns the already paid immutable images and retains
a mutable borrow of that same journal. `before_bytes()` and `after_bytes()`
borrow those images without another copy; `state()` reports `V1State::Pending`
or `Committed`. Dropping the session leaves the journal intact.

Consuming `CheckedV1::recover(target, budget, cancelled)` uses that exact loaded
record and the same journal, with no journal replacement or reload. It applies
the existing target checks and recovery write/barrier sequence. The one-shot
`recover` API, V1 bytes, callback ordering and cumulative resource behavior remain
unchanged. Inspection failures do not write either resource; consuming recovery
can fail after changes and retains the existing recovery-required contract.

This is an integrity-inspection boundary, not authentication, target identity,
native policy approval or a normal-save capability. Hosts still own exclusive
access and correct pairing. A native consumer must independently qualify its
original intent and both images before asking this generic session to recover;
inspection does not perform those checks.

The independent literal qualifier passed first: 41 valid marker forms, 64
incomplete bodies and 104 single-byte corruptions, with writes and syncs forbidden.
All five inspection/session tests then passed, including same-journal recovery,
old/new callback and budget equivalence, cancellation and foreign-target refusal.
The complete I/O run passes 54 unit tests, nine integration tests and two
compile-fail documentation tests; strict all-target/all-feature Clippy passes.
These are local protocol checks, not physical power-loss qualification.

## Additive resizable V2 transactions

`prepare_resizable_expected` binds the complete caller-validated before image and
returns `PreparedResizableImage`. Its target must implement
`ResizableDurableStorage`; its separate journal still needs only
`DurableStorage`. `ExclusiveFile` supplies the real resizable implementation
without exposing or cloning its locked handle. Fixed-capacity implementations do
not implicitly acquire the new capability. The complete I/O round passes 36 unit,
three journal-file, four lock, two seek and one compile-fail documentation test;
strict all-target/all-feature and minimal checks also pass. The
[result record](../testing/results/variable-length-image-journal-2026-10-30.json)
pins current source, independent records and the actual logs.

The new capability requires honest contiguous writes that can extend EOF. Resize
success establishes exactly the requested length; an error can follow a partial
length change within the old/requested interval. It cannot change retained bytes
or undershoot that interval. The protocol always writes its complete image first,
then establishes the final length, so its resize calls only retain or truncate
length. It never pre-extends storage or invents zero-filled image tails.

Each image is independently bounded by 64 MiB and may be empty. Preparation
requires an empty journal and exact actual target length/content agreement with
the caller's before image, before any journal write. It owns the two exact images,
writes and synchronizes the journal, and leaves the target untouched. Incomplete
journals are retained for diagnosis and are never silently overwritten.

Commit rechecks exact before length and bytes. It writes the complete after
image, establishes its exact length and synchronizes the target before writing
and synchronizing the commit marker. Truncation is last. A failed write, resize,
sync or completion check requires recovery; an error does not mean that bytes or
length stayed unchanged. Dropping the prepared value also retains the journal.

### V2 framing and recovery

The V2 record is `TCIMG002`, a little-endian u64 before length, a separate
little-endian u64 after length, a 32-byte SHA-256, and the two exact images in that
order. Its header is 56 bytes. The digest covers the version magic, both encoded
lengths and both images. Different length splits of the same concatenated bytes
therefore have different digest domains. The final marker is `COMMIT02` plus the
same digest. Empty or exact partial marker prefixes remain pending; a complete
valid marker is committed. Unknown versions, malformed/truncated bodies, size or
arithmetic violations, mismatched digests, bad prefixes and trailing bytes refuse
before target mutation. No expected image is padded to the other image's length.

`recover_resizable` reads V1 journals through the unchanged fixed recovery path
and V2 journals through the new path. The old `recover` continues to reject V2,
because its target has not promised resize capability. No V1 journal is upgraded,
rewritten or reused by this dispatch.

For pending V2 recovery, the actual target length must lie between the smaller
and larger image lengths. Every actual byte within their shared extent must
match its real before or after counterpart. An existing byte beyond one image's
EOF must match the other real image; absence is never replaced by a zero byte.
Out-of-range lengths and unrelated bytes refuse before any write or resize.
Recovery restores the complete before image, establishes its final length and
synchronizes target and journal. Contiguous writes and truncate-last ordering
keep interrupted rollback recognizable and restartable in both size directions.
Partial markers remain unchanged, including across repeated rollback.

A committed journal requires exact after bytes and length. Target sync already
preceded marker writes, so successful committed recovery rewrites neither bytes
nor length: it verifies the after image and re-establishes the two barriers. A
foreign or postcommit mixture is not repaired merely because its bytes resemble
the two images. Both outcomes retain the journal for host-owned cleanup.

The host still owns exclusive resources, correct pairing across restart,
durable namespace setup/cleanup, authorization and blocking-call timeouts. A
checksum does not authenticate target identity. No native Access shared-file
protocol, physical power-loss, arbitrary torn-sector or independent filesystem
metadata/data persistence guarantee is established by the local storage model.

### V2 independent controls and native boundary

Nine independently authored complete pending/committed record goldens and fault
expectations were frozen before the V2 producer was authored. They cover grow,
shrink, equal length, both empty directions, empty-to-empty, real zero payload
and equal concatenated bytes with different length splits. Their source-only
qualification runs before transaction APIs. The focused tests compare produced
journal bytes exactly and check every truncation/corruption, all partial marker
lengths, every common-byte mask for the bounded literals, exact target-length
bounds, real tail origins and committed refusal with zero writes/resizes.

The existing deterministic fault-device owner also exercises V2 with three-byte
short writes, errors before/after every callback, full or partial truncation before
an error, unsynced-device survival/loss and interrupted recovery. Real disposable
locked-file consumers cover grow/shrink/empty images, retained-journal reopen and
injected resize failures before, partway through or after their real file effect.
All these controls pass. Exact/one-below byte and step allowances and final-poll
cancellation retain explicit recovery state.
All owned copies, reads, hashing and comparisons use paid, bounded work.

The independent source-golden test passed before transaction APIs. The first
full-suite invocation stopped at an integration-module path compile error; root
added only the explicit child-file path and the corrected run passed all 46
checks. No record golden or protocol behavior changed after API execution.

Three additional Proptest controls now generate 256 cases each from a fixed
seed. Independently encoded V2 records cover varied image lengths, arbitrary
real before/after byte mixtures and partial markers, repeated pending rollback,
committed verification without rewrites, foreign targets and single-byte record
corruption. Invalid records must change neither resource. All 768 generated
cases pass; the complete I/O suite passes 49 checks, and strict all-target/all-feature
Clippy passes. This extends the local model checks without changing the protocol
or establishing physical power-loss behavior.

The actual native consumer above continues to call V1 `prepare_expected` with a
validated `NativeEditPlan`. Seven existing native transaction/write-transaction
semantic tests also pass; this is separate from the strict I/O package check. V2 supplies no new native plan, no conversion from
`Ace16WritePreview`, and no permission to bypass ACE/data-macro/normal-save gates.
The current fresh-copy engineering publisher remains a separate host operation.


## Explicit filesystem association

The optional [retained filesystem owner](FILESYSTEM_JOURNALS.md) validates existing
private bundle objects and lends the same locked descriptors to explicit
recovery. It does not discover journals, qualify native writes or implement
bundle creation/retirement yet. Generic V1/V2 wire bytes remain unchanged.


## Explicit filesystem pairing and retirement

The additive [filesystem journal owner](FILESYSTEM_JOURNALS.md) supplies retained
target/journal descriptors, bounded binding/intent checks, durable private-bundle
setup and whole-bundle retirement for explicit macOS/Linux hosts. It reuses V1
bytes and never reopens a journal from a publication receipt. A borrowed storage
view does not expose a replaceable owned lock guard. Native format/constraint
qualification remains with its consumer; this feature grants no normal ACE save.

Setup reserves incomplete-resource ownership fallibly before effects and retains
it on error. Known commit/recovery outcome is recorded before fallible retirement;
namespace/barrier/cleanup/unlock errors preserve that outcome. Same-process
barrier-only confirmation is explicit. Restart resolution of a retired bundle
still requires a future completed-target verification/cleanup path, so this is
not yet a complete crash lifecycle or native power-loss qualification.
