# Offline Jet4 journal editing

`query journal-update` applies one qualified normal Jet4 Int32 UPDATE to the
selected file itself. `query recover-update` explicitly resolves its paired
journal without replaying SQL. These are separate from the unchanged
[fresh-copy UPDATE](../access/writes/QUERY_EDITING.md) command.

```sh
tablecraft query journal-update 'UPDATE Items SET Score=12 WHERE Id=1' \
  --database offline.mdb --journal fresh-edit-journal

tablecraft query recover-update \
  --database offline.mdb --journal fresh-edit-journal
```

Use an offline file and retain a separate backup. Close other applications and
ordinary readers until the operation or explicit recovery finishes successfully.
The OS lease coordinates cooperating handles; it does not implement Access
ldb/laccdb locking. Ordinary readers do not automatically discover these journals.
An interrupted target must not be used merely because its process lock expired.

## Admission and resources

Apply accepts one local table and one Int32 assignment under the existing
[normal native write gates](../access/writes/NATIVE_WRITES.md). Closed-shape SQL
validation happens before opening files. All selected expressions finish against
one original snapshot before journal setup. Zero matches still pass native gates.
The shared Access planner owns editing semantics; the CLI owns explicit paths,
comparison selection, resources and reporting.

Only clear Jet4 is admitted. `--password` refuses before opening files and never
prompts. There is no decrypt/re-encrypt route, engineering-candidate conversion,
ACE save permission or required-macro bypass. Explicit `--collation ordinal` can
serve qualified Text predicates; no ambient comparison policy is selected.

The database must be an existing regular file; final symlinks are refused. Apply
requires a fresh journal-directory name beneath an existing parent. It never
reuses or overwrites an existing bundle. The same exclusive target descriptor is
retained through capture, planning, preparation, commit and release. Setup holds
the actual parent/directory/file resources and completes the required barriers
before native preparation.

Work allowances accumulate throughout each invocation:

| Option | Default |
| --- | ---: |
| `--work-bytes` | 268435456 |
| `--work-rows` | 100000 |
| `--work-steps` | 100000000 |

Native image/request/journal ceilings remain separate. Larger images may need
explicitly larger byte and step allowances. No budget resets between phases.
The filesystem implementation targets macOS and Linux; the local execution
record covers macOS. Other platforms retain help and refuse before opening files.
Filesystem/device compliance and physical power-loss behavior are not qualified
by completed API barriers.

## Recovery and completion

Recovery accepts no SQL, output-copy option or password. It checks actual object
association, bounded persisted intent and the checked V1 record, then rebuilds the
normal Jet4 plan and compares both complete images before a recovery write. A
different target object with identical bytes does not satisfy association. Keep
the selected namespace stable while an operation runs. No stored expression,
clock, default or callback is reevaluated.

Successful apply reports:

```text
Transaction	Committed
Matched rows	1
Changed rows	1
Journal namespace	Retired and synchronized
```

Recovery reports `Recovery` with `Committed` or `RolledBack`, followed by the
same namespace line. Reports are paid and formatted before target writes. Success
is printed only after acknowledged retirement, owned garbage cleanup and fallible
lock release. The active bundle is retired intact before cleanup; no foreign
entry or target is removed.

Every commit error requires explicit recovery. Do not retry the UPDATE blindly
or delete its journal. Setup/preparation failure can leave an incomplete bundle
requiring diagnosis; preparation itself does not change target bytes. Refused
recovery does not establish that an interrupted target is ready for ordinary use.

Cleanup, unlock and output failures retain the separately known target outcome.
They return failure, including stdout or diagnostic-stderr BrokenPipe; they do
not mean rollback or authorize SQL replay. Earlier success-text bytes may have
reached stdout before a later output failure. No automatic retry, error-path
bundle deletion or fallback occurs.

Incomplete setup and uncertain namespace retirement can require diagnosis.
This CLI does not reconstruct terminal retirement state after restart, discover
arbitrary sidecars or promise a complete crash lifecycle. The
[filesystem owner](../foundation/IMAGE_JOURNAL.md) and
[native recovery contract](../access/writes/NATIVE_WRITES.md#optional-journaled-persistence)
retain their separate guarantees and limits.

## Local checks

An independent original/complete expected Jet4 pair was read and checked before
command tests. The first host run passes four journal units, one protected-output
unit and five black-box functions, including that source check. The four black-box
API functions cover complete-image apply, real pending/committed recovery and
retirement, occupied/missing resources and unsupported SQL/source refusals.
Output units retain both known outcomes through write/flush failures and keep
failure status when diagnostic stderr has BrokenPipe. No host product-test
failure occurred in this first round. CLI all-target/all-feature Clippy passes,
along with all 65 binary unit tests, six shared adapter integration tests and
22 existing copy/inspection/guard integration functions. The focused counts
overlap the full binary run; they are not additional distinct unit coverage.

See the [local result](../testing/results/jet4-journal-cli-2026-10-30.json).
