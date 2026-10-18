# Bounded offline native edits

`NativeImage::plan_int32_updates` produces complete before/after images without
writing to its source. The current engineering profile is unencoded Jet 4 only:
existing non-NULL fixed Int32 fields in unflagged local tables, with the native
updatable flag and no AutoNumber/extended flags. Assignments use sequential
snapshot-local row ordinals plus expected original values. They are not persistent
row identities. Duplicate, absent or stale assignments fail the entire plan.

Fields participating in any index are rejected; indexes on unchanged fields must
have complete supported checks. Unknown constraints, properties and property
scopes block planning. Required/AllowZeroLength and supported field/table rules
and associated qualified relationships must pass before and after edits. Defaults
remain inert because this profile changes existing records. No cascades execute.
Only Required, AllowZeroLength, ValidationRule/Text, DefaultValue, Caption and
Description property keys are admitted. Missing declarations remain absent.

The physical writer rejects selected relocated/historical rows and overlapping
fixed payloads. It changes four payload bytes per changed row, without altering
null masks, variable tails, row lengths, page allocation, indexes or file length.
A bounded independent comparison proves every byte outside these patches remains
identical. Whole-image copying and comparisons charge shared allocation/work
budgets and check cancellation between 64 KiB units. Failures publish no plan.

All ACE writes remain blocked: header recognition does not establish absence of
table data macros. Microsoft describes [data macros](https://support.microsoft.com/en-us/access/add-data-macros-in-a-desktop-database)
and [cross-version read-only behavior](https://www.microsoft.com/en-us/copilot/blog/2010/01/13/cross-version-compatibility-between-access-2010-and-access-2007/).
No unavailable macro semantics may be skipped. This Jet 4 profile does not qualify
ACE application save/reopen or VBA preservation through actual Access edits.

## Verification and laboratory harness

Independent synthetic golden files exercise all Int32 replacements with a
256-case fixed-seed row model, exact unchanged-byte comparisons, no-op/stale/NULL
updates, AutoNumber/ACE rejection, post-edit rule failures, unknown properties and
cancellation. These tests require neither Java nor extra binary fixtures.

The original [UpdateFixture.java](testing/UpdateFixture.java) recipe creates a
Jet 4 table with an Int32 primary index, a non-indexed range-validated Score,
Unicode memo and 12 KiB binary payloads. The engineering example writes only a
fresh disposable output:

```sh
cargo build -p tablecraft-ace --example update_int32 --locked
java -Dstdout.encoding=UTF-8 -cp path/to/jackcess-5.0.4.jar docs/testing/UpdateFixture.java new.mdb V2000
python3 docs/testing/compare_updates.py --example target/debug/examples/update_int32 --cli target/debug/tablecraft --jar path/to/jackcess-5.0.4.jar --jet new.mdb
```

The comparison confirms Score 20 becomes 27, exactly one byte differs, memo/binary
lengths survive, and Jackcess plus the native index checker reopen the output.
Rule violations, stale values and indexed-field assignments create no output.
Original files remain unchanged. A failed harness output write can leave an
incomplete copy; this example is not an in-place transaction. Optional journal integration is described below. Allocation/index writes,
INSERT/DELETE, native shared-file
coordination and Microsoft Access comparison remain separate work.

## Optional journaled persistence

With `tablecraft-ace/journal`, `NativeEditPlan::prepare` binds the validated before
image to `tablecraft-io`'s `prepare_expected`. The complete target must match
before any journal write. Preparation syncs the journal without changing the
target; `PreparedNativeEdit::commit` rechecks, writes/syncs the candidate and
syncs its marker. Every interrupted/failed commit requires explicit recovery
before ordinary access. Success returns changed rows and leaves the journal.
`rollback` restores the original; `recover_native_edit` performs checked generic
recovery and captures the resulting native image. Recovery is repeatable.

Hosts must supply distinct correctly paired exclusive offline resources, establish
the journal namespace durably before preparation, retain interrupted journals and
perform durable cleanup explicitly. The engine never creates/deletes/discovers
journals or implements Access shared-file locking. See [journal preconditions](IMAGE_JOURNAL.md).
OS locks coordinate cooperating handles, not live Microsoft Access clients.

Three native integration tests cover stale targets, retained/reused journals,
interruption after a partial payload side effect, rollback/repeated recovery and
real locked-file commit/close/reopen. Generic I/O fault/corruption tests remain
separate. These are software fault and local filesystem observations, not power
loss, network filesystem or native Microsoft Access durability qualification.
