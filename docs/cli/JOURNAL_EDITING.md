# Offline native journal editing

`query journal-update` applies one qualified normal UPDATE to the selected file
itself, including admitted atomic fixed-scalar SET groups on ACE 16.
`query recover-update` explicitly resolves its paired
journal without replaying SQL. `query finalize-update` confirms an intact retired
bundle before completing namespace cleanup. These are separate from the unchanged
[fresh-copy UPDATE](../access/writes/QUERY_EDITING.md) command.

```sh
tablecraft query journal-update 'UPDATE Items SET Score=12 WHERE Id=1' \
  --database offline.mdb --journal fresh-edit-journal

tablecraft query recover-update \
  --database offline.mdb --journal fresh-edit-journal

tablecraft query finalize-update \
  --database offline.mdb --journal ACTUAL_RETIRED_DIRECTORY
```

Use an offline file and retain a separate backup. Close other applications and
ordinary readers until the operation or explicit recovery finishes successfully.
The OS lease coordinates cooperating handles; it does not implement Access
ldb/laccdb locking. Ordinary readers do not automatically discover these journals.
An interrupted target must not be used merely because its process lock expired.

## Admission and resources

Apply accepts one local table under the existing
[normal native write gates](../access/writes/NATIVE_WRITES.md). Closed-shape SQL
validation happens before opening files. All selected expressions finish against
one original snapshot before journal setup. Zero matches still pass native gates.
The shared [Access offline session](../access/writes/OFFLINE_WRITE_SESSIONS.md)
owns planning, journal apply/recovery and retired confirmation through one retained
target. The CLI supplies explicit paths, comparison selection, resources and
preformatted reporting. The same backend service supports successive edits
without releasing the target between operations.

The [ACE fixed-scalar route](../access/writes/NORMAL_ACE_FIXED_SCALARS.md) also
accepts ordinary unindexed Boolean, Int32, Int64, Currency and legacy DateTime
targets, including atomic multiple SET assignments. Numeric NULL transitions
retain inactive payload bytes; Boolean remains nonnullable. All right-hand sides
read the same original row, independent of SET order. The required native macro
language remains Int32/NULL only. These requests still require positive complete
supported BeforeChange coverage; they do not certify macro absence or enable
general saves. For an already qualified offline database, the syntax is:

```sh
tablecraft query journal-update \
  'UPDATE Items SET Flag=True,Int64Value=CLngLng(42),[Money]=CCur(-1.0001) WHERE Int32Value=0' \
  --database offline.accdb --journal fresh-edit-journal
```

Bracket reserved field names such as `[Money]`. Indexed Int32 targets retain the
separate restrictions below; the fixed-scalar route does not add indexed NULL
transitions, record growth or relationship actions. Jet4 retains its single-SET
Int32 profile.

The [ordinary-content ACE route](../access/writes/NORMAL_ACE_ORDINARY_FIXED.md)
also admits the initial complete data-only family: known unencoded header 5/6,
1252/General declarations, variable inline-map windows under the captured-image
and cumulative work/read limits, closed ordinary metadata and no
relationships or program/property carriers. A qualified Int32 AutoNumber remains
read-only. Manual fields in checked local single-Int32 indexes support fitting
updates, including numeric NULL transitions and atomic final UNIQUE swaps.
Primary/required NULL keys and final duplicate keys refuse. Compatible aliases
share one physical rewrite; unaffected index bytes and historical counters remain
unchanged. Rewrites retain page identities, topology and per-leaf entry counts;
splits, allocation and map/EOF growth refuse. Metadata is only a routing hint;
complete content validation must pass before capability is advertised and
again for every native plan/recovery. There is no caller absence flag. Ordinary
equal assignments and genuine empty selections retain bytes and complete the
normal journal lifecycle. This is not admission for arbitrary imported databases.
The earlier 64-page ceiling is removed; indirect maps remain outside this route.

A distinct [native target-table profile](../access/writes/NATIVE_TARGET_PROGRAM_ABSENCE.md)
also supports the exact seventeen-field catalog when the selected local table
has four NULL OLE carriers (including LvProp), five NULL remote fields and no
children or ambiguous identities. Shared TD/index declaration checks, all-local
LVAL map declarations and full normal Source protection remain mandatory. Only
bounded zero-length deleted catalog tombstones are exempt from flagged-slot
refusal; unknown storage kinds and linked origins refuse. Known unrelated
application objects are preserved without execution. This is a bounded inference
backed by native locator evidence, not general macro-absence certification or
Microsoft Access save/reopen conformance. Ten genuine CLI saves and exact TSV
readbacks match frozen whole images; the CLI duplicate refusal asserts exit code
2 and the exact constraint diagnostic. Native recovery and retained-session
checks also pass; see the [write record](../testing/results/ace16-genuine-ordinary-target-writes-2026-10-31.json).

Qualified ordinary ACE 16 tables also support [fitting Text/Binary batches](../access/writes/NORMAL_ACE_FITTED.md),
with optional unindexed fixed siblings and shared original-record capacity.
For an eligible source, the existing command is:

```sh
tablecraft query journal-update "UPDATE Items SET TextValue='beta' WHERE Id=1" \
  --database example.accdb --journal example-update
```

Fitted requests retain TCNI0005; fixed-only requests retain their prior profiles.
The adapter validates every target even for zero matches. This route allocates
no DATA/index pages and rejects indexed variable targets, unsupported properties
and required programs. Typed/native Binary writes have broader coverage than
SQL literal assignment; the CLI does not invent a Binary literal dialect.

The indexed-target CLI checks pass 24 literal saves and exact row readbacks,
including NULL transitions, swaps, tree fitting and noops, with eight refusal
controls. See the
[indexed-target write record](../testing/results/ace16-ordinary-indexed-target-writes-2026-10-31.json).

Jet4 retains its existing fixed Int32 admission. The separate
[normal ACE BeforeChange route](../access/writes/NORMAL_ACE_BEFORE_CHANGE.md)
requires an unencoded ACE 2016/2019 layout with subversion zero and a positive,
complete source-owned pure BeforeChange program. The selected ordinary Int32
table may have checked non-NULL Int32 indexes with qualified compressed
prefixes and existing interior nodes. Page identities, child topology and each
leaf's entry count remain fixed; fitting serialization is selected before
preparation, without retrying a failed encoder. Ascending/descending components,
composites, consistent logical aliases and final unique-key swaps are supported. Relationships,
ignore-NULL index rewriting, splits and map/storage growth refuse. A separately
selected mixed-schema route preserves supported scalar/rich siblings. Complete
ordinary scalar tables can maintain checked fixed-I32 indexes beside untouched
NULL components, including
readonly counter components and descending composites. Rich/unavailable siblings
retain unindexed-only edits. Original nullable key populations and potential
targets select the previous unindexed qualifier or checked indexed route before
tree parsing; NULL target transitions remain refused.
Int32 AutoNumber may be read but never assigned. Unsupported field operands,
generated targets and calculation/collection dependencies refuse; changed target
values remain non-NULL. Unknown/opaque/unrepresented event content,
body/module payloads and unsupported required effects refuse; an empty macro inventory is not an absence certificate.

On the positive-program route, native programs are discovered and fully
preflighted even for zero matched rows.
A supplied equal expected/replacement assignment refuses with `EventTiming`,
including one inside a mixed list; native equal-assignment event timing is
unqualified. A real user change that a program restores to Old may succeed with
zero effective changed rows. The original user intent is retained in either case.

`--password` refuses before opening files and never prompts. There is no
decrypt/re-encrypt route or engineering-candidate conversion. This bounded route
does not enable general ACE saves or bypass required programs. Explicit
`--collation ordinal` can serve qualified Text predicates; no ambient comparison
policy is selected. Ordinary fresh-copy `query update` remains Jet4-only.

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
record-selected native plan and compares both complete images before a recovery
write. A different target object with identical bytes does not satisfy
association. Keep the selected namespace stable while an operation runs.

SQL and its original filtering/SET expressions are never replayed. For the
positive-program ACE routes, qualification rediscovers and evaluates the admitted
bounded pure stored programs against the recorded BEFORE image and original typed
requests. Ordinary TCNI0004 recovery instead freshly repeats the selected content
or target-local absence profile and reconstructs the original fixed requests;
no certificate is serialized. No external
callback or effect is replayed. Closed intent version/profile dispatch refuses
unknown records without trying another validator.

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

Incomplete setup, partial garbage and uncertain namespace state can require
diagnosis. No command discovers arbitrary sidecars or promises a complete crash
lifecycle. The
[filesystem owner](../foundation/IMAGE_JOURNAL.md) and
[native recovery contract](../access/writes/NATIVE_WRITES.md#optional-journaled-persistence)
retain their separate guarantees and limits.

## Confirm an intact retired bundle

For `finalize-update`, select the actual surviving nonce-named retired directory;
`ACTUAL_RETIRED_DIRECTORY` in the example is a placeholder. The command does not
accept an active bundle, SQL or copy-output options. It associates the retained
objects, requalifies the original native intent and both complete recorded images,
then lends write-denied target/journal views to completed-image confirmation.

A committed V1 record requires exact current replacement bytes. Pending state,
including an admitted partial marker, requires exact current original bytes.
Both storage barriers must complete. Opposite or mixed current images refuse
without copying either image back, rolling forward, rolling back or falling back
to `recover-update`.

Success reports the verified current side:

```text
Verified image	Replacement
Journal namespace	Retired and synchronized
```

`Original` is the other possible image label. These labels describe current
verified bytes/barriers, not historical commit, rollback, callback execution or
external effects. No-op before/after images can be identical. The command writes
no target or journal payload bytes. After verification it acknowledges only the
held-parent barrier, cleans the owned retired namespace and releases locks; it
does not perform another rename.

An acknowledgement, cleanup, unlock or output failure retains the verified image
fact and returns failure, including BrokenPipe. Missing records or partly deleted
garbage refuse the intact-bundle profile and remain for diagnosis. This addition
does not widen either normal native profile or add automatic recovery.

## Initial Jet4 checks

The initial Jet4 phase read and checked an independent original/complete expected
pair before command tests. The first host run passes four journal units, one protected-output
unit and five black-box functions, including that source check. The four black-box
API functions cover complete-image apply, real pending/committed recovery and
retirement, occupied/missing resources and unsupported SQL/source refusals.
Output units retain both known outcomes through write/flush failures and keep
failure status when diagnostic stderr has BrokenPipe. No host product-test
failure occurred in this first round. CLI all-target/all-feature Clippy passes,
along with all 65 binary unit tests, six shared adapter integration tests and
22 existing copy/inspection/guard integration functions. The focused counts
overlap the full binary run; they are not additional distinct unit coverage.

The finalization addition reuses the exact existing source/expected pair and
reader gate. Its first run passes four new units and three black-box functions:
two complete-image confirmations and six active/partial/opposite-image refusals.
Presentation checks exclude historical outcome labels even from secondary errors.
The first broader strict check found an oversized retained setup-error variant,
not a source/expected mismatch. Finalization now pays and reserves one error-owner
slot before opening the target; failed setup moves its complete cause and retained
capabilities into that capacity without late allocation. Apply/recovery behavior
is unchanged. A fifth unit checks exact/one-below reservation budgets and actual
retained-error/lock ownership. Corrected all-target/all-feature CLI Clippy, all
70 binary units and all eight journal integration functions pass. Focused checks
overlap those complete target runs. Final CLI/Access formatting, documentation
links and dependency-boundary checks/tests also pass.

See the [local result](../testing/results/jet4-journal-cli-2026-10-30.json).
The [finalization result](../testing/results/jet4-retired-journal-finalization-cli-2026-10-30.json)
records the separate restart-confirmation consumer.
