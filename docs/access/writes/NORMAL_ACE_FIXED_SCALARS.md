# Journaled ACE fixed-scalar batches

The normal journal API accepts owned original request groups for qualified
Boolean, Int32, Int64, Currency and legacy DateTime fields. The native entry is
`NativeImage::plan_recoverable_fixed_updates`; `NativeFixedIntentGroup` owns each
column name and its `FixedScalarUpdate` requests. The Access adapter consumes
the same API through `NativeJournalEditor` and `OfflineTableEditor`'s
`EditableBatch` contract. No engineering candidate is converted into a save plan.

## Admission and evaluation

The source must satisfy the existing unencoded ACE 2016/2019 subversion-zero
profile and complete, native-selected positive pure BeforeChange coverage.
All required properties and dormant programs are compiled. Unknown required
content, body/module carriers and unsupported effects refuse. This provides no
macro-absence certificate or permission to save arbitrary ACE databases.

The new route uses complete actual ordinary scalar snapshots. Rich, calculated
and partial projections remain outside it. Generated fields, indexed targets
and calculation dependencies refuse, including empty target declarations.
Qualified unchanged index siblings remain protected. Relationships, record
resizing, allocation and index splits are not added by this route.

User requests may contain the five admitted fixed carriers; numeric fields may
also contain NULL. Boolean remains a nonnullable mask value. Currency retains
its signed scaled integer, DATE retains exact bits, and numeric NULL changes
retain inactive payload bytes. Other user carriers refuse. Native macro operands
and SetField targets remain Int32/NULL: a wide user request does not make that
field available to the macro expression language.

Every expected value names the original snapshot. Groups, row ordinals, target
carriers and stale values are checked before event execution. Supplied equal
assignments refuse with `EventTiming`; native equal-set timing is unqualified.
Genuinely empty groups still compile the required programs. All user assignments
seed one source-ordinal event union; sequential macro assignments then see
Current and Old under the existing program contract. Final groups compose once,
and changed rows count the final physical root union.

## Shared physical proof and recovery

`mutation/fixed_records` owns working staging and the independent manual proof.
Existing preview consumers import that owner directly. Their original admission
rules remain separate from normal save permission. Staging uses the original
rooted-record visitor and coalesces payload and shared mask changes. The proof
freshly captures BEFORE, binds raw Int32 operands, rediscovers native programs
and replays original requests. Its independent codec reconstructs the complete
expected image, validates final constraints and compares every AFTER byte and
the changed-row count. Staged patches and final frames are not proof authority.

The closed original-request record is `TCNI0003`/profile 3. It retains source
length/digest, table, ordered column groups and exact expected/replacement
carriers. It contains no SQL, program selectors, final macro assignments or
permission flags. Decode validates bounded framing, carrier tags, padding and
core DATE constructors. Metadata decoding consumes bytes/steps; actual row work
is charged during native planning and recovery qualification.

`NativeUpdateIntent` exposes typed Int32/fixed bodies. `TCNI0001` and `TCNI0002`
retain their original codecs and payloads. A single present-to-present Int32
update, including an empty single-column request, keeps the old Int32 route even
when the table advertises `StoredScalar`. Recovery verifies the original digest,
reconstructs from original requests and compares complete BEFORE/AFTER before
the existing checked journal can recover or confirm completion.

## Host capabilities and current evidence

The adapter selects `StoredScalar` from original metadata, actual complete
snapshots and static native targets. Index shapes and indexed macro targets are
classified before entering the narrower preserved-source qualifier. Unsupported
capability remains readable but unavailable for editing; failed mutation is
never caught and retried through another writer. The profile does not promise
that every declared column is writable. Existing indexed/rich Int32 services
retain their own admission and record formats.

Typed session batches retain the existing exclusive offline lease, stage before
journal creation, report distinct original matched rows and publish only through
the existing apply/recovery lifecycle. Stale handles and cancellation retain
their typed failure and session state.

Thirty-two native complete-image commits and the original-request recovery
controls pass. Fifty-six real typed session saves cover the first 28 literal
outcomes in both group orders. Separate controls verify the old Int32 record
dispatch. SQL/CLI qualification and wider feature checks remain separate in the
[execution record](../../testing/results/ace16-normal-fixed-before-change-2026-10-31.json).
The [source contract](../../testing/ACE16_FIXED_BEFORE_CHANGE_SOURCES.md) records
independent source/read evidence. Native Access event/save/reopen and
mixed-client qualification remain unrun.
