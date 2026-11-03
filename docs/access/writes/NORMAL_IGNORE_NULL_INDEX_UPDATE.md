# Single-Int32 IgnoreNULL index updates

Focused matching validation passes for the scope below. Remaining regression
and strict checks are still in progress.

The ordinary fixed UPDATE service has a fitting continuation for a manual native
Int32 secondary index that omits NULL keys. Every physical index must have a
qualified single-Int32, prefix-zero root leaf. Complete native seventeen-field
Source, program-absence checks, original declarations, properties, constraints,
all logical aliases, map ownership and live LVAL exclusion remain mandatory.
Metadata selects this route before staging; a failed planner never selects a
fallback. Read certification does not grant mutation authority.

The selected read-declaration qualification admits only the manual Int32 secondary
shape. It keeps global, Actions and unrelated retained-index declaration policies
separate. Every alias is checked before physical-owner deduplication, so an
ordinary alias cannot conceal a primary owner. IgnoreNULL primary, tuple and NODE
mutations, broader ignored key families and allocation remain separate work.
Generated fields remain outside writable fixed targets.

## Membership, history and preservation

A logical NULL row remains part of the table while emitting no secondary key.
Original and final membership are derived from complete logical ROW associations,
not only the original emitted entries. Required is checked before omission, and
UNIQUE is checked against the complete final non-NULL population.

An originally omitted row introducing a distinct new present family increments
that physical index's historical counter once. Joining an original family does
not increment it. Represented replacements and removals neither increment nor
reduce history; historical counts are never replaced with live-row counts.
Ambiguous removed/reintroduced or jointly introduced families refuse atomically,
in either request order. No sequential timing is invented for an atomic batch.

Present-to-NULL assignments retain inactive fixed payload bytes. Equal represented
assignments, omitted NULL-to-NULL assignments and named empty groups preserve the
whole image while still performing full admission. They are distinct requests;
an empty group does not bypass checks.

Only selected DATA payload/presence bits, affected index entries/framing and an
independently justified historical increment may change. Original ROW ties,
unselected index bytes, aliases, catalog/properties, maps, LAST, DATA geometry,
protected inactive tails, other tables and EOF remain unchanged. The independent
Single outcomes select Left; their primary index remains byte-exact.
The independent proof rebuilds original/final emitted populations and history from
fresh BEFORE and original requests, without producer keys, patches or deltas.
Final Source, constraints, changed-row counts and complete bytes must agree.

## Consumers and recovery

Native `plan_recoverable_ordinary_fixed_updates`, retained typed `plan_columns`,
SQL `plan_batch_update` and CLI `query journal-update` use the existing service.
Native/typed fixed carriers remain exact; SQL retains checked integral Int32
assignment. An example against a qualifying table is:

```sql
UPDATE [Single] SET [Left]=30 WHERE ID=3
```

The original `TCNI0004` request and checked V1 journal remain unchanged. Recovery
freshly reconstructs program absence, Source, records, emitted keys and history
before effects. Pending/partial states roll back; committed incomplete or mixed
targets refuse without writes or sync. Successful completion confirmation is
write-free. Retained sessions keep their exclusive lease, receipts, snapshot
refresh, stale-handle refusal and durable namespace lifecycle.

## Focused observations

The [acceptance record](../../testing/results/ace16-normal-ignore-null-update-2026-11-03.json)
retains commands, log hashes, source pins and first failures.

Eight unchanged source images supplied 12 native read/admission contexts. Actual
program-absence and canonical Source capture admitted all 12 before matching.
Two direct-proof, eight native, two retained-session and two CLI functions pass.
They cover 12 distinct principal requests, 324 protected-byte forgeries at each
direct and journal boundary, 114 recovery contexts with 26 committed mismatches,
and 12 measured full public resource/cancellation operations. Twelve typed,
12 SQL and 12 CLI saves match the independent whole images; 24 old handles refuse
after retained saves. The first test-only resource fixture compile correction
rebuilt its request under the measured controls; production and expected bytes
did not change. Remaining regression and strict results are not yet claimed.

## Focused observations


Eight unchanged source images supplied 12 native read/admission contexts. Actual
program-absence and canonical Source capture admitted all 12 before matching.
Two direct-proof, eight native, two retained-session and two CLI functions pass.
They cover 12 distinct principal requests, 324 protected-byte forgeries at each
direct and journal boundary, 114 recovery contexts with 26 committed mismatches,
and 12 measured full public resource/cancellation operations. Twelve typed,
12 SQL and 12 CLI saves match the independent whole images; 24 old handles refuse
after retained saves. The first test-only resource fixture compile correction
rebuilt its request under the measured controls; production and expected bytes
did not change. Fifty-seven retained reader/preview/normal-recovery functions pass,
with strict all-target checks across ACE/Access/CLI/testkit, standalone reader,
journal and preview builds, dependency guards and formatting. A later test-only
chunk-iteration lint correction preserves the same page comparisons. Two added
native functions pass six outcomes: counter-MAX zero delta versus overflow, and
unindexed Marker edits preserving every ignored-owner byte. These explicit
adversarial counter variants are not new source qualification.

The [independent source and admission record](../../testing/ace16_fixed_ignore_null_updates/NORMAL_ADMISSION.md)
keeps those observations separate from earlier engineering comparisons and
preserved admission failures. No native Microsoft Access save/reopen, Windows
mixed-client or full ACE 16 compatibility claim follows from these portable
sources and checks.
