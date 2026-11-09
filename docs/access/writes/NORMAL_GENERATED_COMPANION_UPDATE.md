# Ordinary updates beside generated IDs

The existing normal fixed UPDATE service can change a qualified ordinary Int32
field beside an untouched generated Id. This save qualification uses native17
Source admission, TCNI0004 original-request intents and checked V1 journals. It
adds no planner route, generation mode or default-evaluation permission.

## Admission and preservation

The qualified corpus has two header variants for each of two schemas. Numbered
has an ordinary Earlier field, a middle-position generated Id and ordinary Score.
Its deleted high slot remains present: LAST and primary history stay 3 although
the largest live Id is 2. Items retains its exact GenUniqueID DefaultValue
metadata, LAST 137 and existing generated Id. These Items sources establish
preservation of a Random-style declaration, not native Random generation.

Actual Score targets still pass the ordinary fixed-field, carrier, duplicate,
ordinal and stale-value checks. Complete native Source, required-program absence,
constraints, ownership and index checks remain mandatory. Selecting generated Id
refuses even for a same-value assignment or a named empty group. Retaining a
generated declaration does not authorize updating it.

Numbered changes Score 10 to 17 and 20 to 23 in either request order. Items
changes Score 10 to 17. NULL transitions clear only the selected presence bit and
preserve its inactive payload. No-op and named-empty requests retain the original
image. All other bytes remain exact, including Id values and flags, LAST, live
counts, histories, deleted records, indexes and aliases, maps, MR2 properties,
defaults, residual storage and EOF. Neither a generator nor a default, clock or
stored expression runs.

## Consumers and recovery

The engine ordinary fixed planner, retained typed `plan_columns`, SQL
`plan_batch_update` and CLI `query journal-update` use the same existing service.
For example, the qualified Numbered schema accepts:

```sql
UPDATE Numbered SET Score=IIf(Id=1,17,23) WHERE Id=1 OR Id=2
UPDATE Numbered SET Score=NULL WHERE Id=2
```

Native and typed Int32 assignments require exact Int32 or NULL carriers. SQL
retains its checked integral-to-Int32 assignment rule; out-of-range assignments
refuse before publication. Retained saves preserve the exclusive lease, refresh
snapshots, reject stale preoperation handles and retain the completed receipt
through subsequent refusals. Journal namespaces are cleaned through the existing
session lifecycle.

Recovery consumes the same checked V1 token and original TCNI0004 intent,
recaptures Source and reconstructs the complete expected image before effects.
Pending/partial targets roll back. Committed incomplete or mixed targets refuse
without writes or sync. Successful completion confirmation is write-free; pending
rollback may rewrite an intact BEFORE image.

## Observed evidence

The [source qualification](../../../tests/reports/rows/ACE16_GENERATED_COMPANION_UPDATE_SOURCES.md)
preceded matching writes: twelve views and thirty inert SDK reports passed with
unchanged independent image expectations. Sixteen principal request cases have
eighteen ordered batch contexts; reverse order is tested only for the two actual
Numbered batches. No-op and empty requests reuse original images.

One direct-proof function and five native functions pass: eighteen exact-image
commits and direct proof positives, 272 protected-byte forgeries at each proof and
journal boundary, sixteen wrong counts, twelve distinct stale proof inputs,
sixteen measured public resource contexts and 78 recovery contexts with repeated
recovery. Ten committed mismatch contexts refuse without effects. Eighteen typed
and sixteen SQL retained saves, plus sixteen CLI saves/readbacks, match the same
frozen files. Generated-target, exact-carrier, stale-handle and overflow controls
pass. Scoped strict checks, minimal testkit build, dependency boundaries and
formatting also pass on their first matching attempts.

Shared testkit machinery owns image pins, literal requests and TCNI4 framing;
independent literal image oracles remain separate from production encoding. The
framing helper has both the earlier Marker fixture and this fixture as actual
consumers. See the [write result](../../../tests/results/rows/ace16-generated-companion-updates-2026-11-02.json)
for receipt pins, verification scope and counts.

These are TableCraft engine, adapter and CLI save observations with independent
reader evidence. Microsoft Access save/reopen, Windows mixed-client locking and
native Random generation remain unrun. This scoped qualification does not claim
full ACE 16 compatibility or a new full-workspace test run.
