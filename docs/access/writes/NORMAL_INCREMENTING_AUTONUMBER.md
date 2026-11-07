# Normal Increment AutoNumber insertion

Normal omitted-field INSERT generates checked Int32 Increment values from a
qualified native declaration. Typed/SQL sessions and CLI journal insertion use
the connected native planner; no caller permission flag or new intent version
is involved.

The generated field is found at its actual declared position. Ordinary companions
can use the implemented Boolean, Int32, Int64, Currency, legacy Date, Short Text
and Short Binary carriers, with NULL only where representable. Complete dense
field layout and fixed/variable spans must qualify. An ascending required unique
primary root leaf remains required. A separate qualified multiple-owner profile
also retains present manual Int32 ascending secondary leaves. Logical aliases
must agree on each physical owner, with exactly one actual primary declaration.

Complete Source, property, constraint and record checks precede planning. Native
AutoIncrement flags and complete inspected generated-field property absence
select Increment in this qualified profile. Checked constant DefaultValue-only
groups on ordinary companions no longer obscure that mode; unknown properties
and policies still refuse. The stored `GenUniqueID()` declaration selects Random
and refuses; unknown or malformed declarations do not fall back to Increment.
LAST alone, a column name, omission or a primary index never establishes mode.

Original requests must omit the actual generated field. Ordinary companions may
be supplied with exact runtime carriers or omitted when the captured field has
a checked constant literal default. Explicit companion NULL remains NULL and
does not activate a default. Missing defaults, executable expressions and clock
functions remain refused for omitted companions. Supplied generated values and
explicit generated NULL remain supplied and refuse. Boolean NULL refuses, and existing empty-Text declaration checks
remain mandatory. LAST is incremented with checked signed arithmetic; it is
never repaired from the maximum live key. Generated collisions report
DuplicateKey. Exhaustion, unsupported declarations and resource/cancellation
failures publish no candidate. Empty requests repeat declaration checks.

The counter implementation is shared with the preview consumer. Normal staging
writes the fitted DATA rows, actual primary owner, counters and LAST. Separate
proof rebinds original presence, derives signed values with wider arithmetic,
reconstructs DATA and INDEX changes independently, compares the whole image and
recaptures final Source. It consumes no staged rows, prepared keys or producer
counter vector. TCNA2 and checked V1 recovery retain the original intent, without
random providers or generation inference during replay. Companion defaults are
reread from the original image by the producer and independently derived during
proof; TCNA2 retains the user’s original omission rather than storing a fabricated
supplied value. Recovery reconstructs the same operation against its full bound
source.

The initial two-Int32 route retains its
[execution record](../../testing/results/ace16-normal-increment-autonumber-writes-2026-11-01.json).
The [companion AFTER sources](../../testing/ACE16_INCREMENT_COMPANION_AFTER_SOURCES.md)
were qualified before matching generation. New acceptance covers twenty exact
commits and twenty empty plans, forty direct proofs with 208 refusals, 120
recovery contexts, 168 journal forgeries and forty measured resource contexts.
It includes Text, Binary, Boolean/Int64 and genuine middle-position layouts with
aliases and a deleted high physical slot. All earlier Increment regressions pass.

Twenty typed and twelve SQL saves, stale-handle/prior-receipt controls and twelve
CLI saves/readbacks pass. Non-NULL Binary and Int64 assignment remains typed
coverage; these tests invent no SQL literal or coercion. See the
[companion execution record](../../testing/results/ace16-increment-companion-writes-2026-11-02.json).

[Empty Increment creation](INCREMENTING_CREATION.md) remains its separately
qualified creator profile. The
[multiple-index source family](../../testing/ACE16_MULTI_INDEX_INCREMENT_SOURCES.md)
now has two exact ordered batch commits, independent proof, original TCNA2/V1
recovery, public resource controls and actual retained batch saves. `LAST = 137`
generates 138 then 139; a long first row can occupy a later DATA page while the
second fits an earlier page. Each physical index history changes once from its
complete distinct population, regardless of alias count. The generated primary
history becomes 11 and duplicate `Guard = 7` contributes one new secondary value.
The shared `OmittingInsertableBatch` host retains one lease, journal and receipt;
eight filesystem cold restart contexts pass. See the
[batch execution record](../../testing/results/ace16-multi-index-increment-writes-2026-11-02.json).

Nullable/descending secondary keys, retained NODEs, allocation/splits,
broader default/policy composition, Random generation, reseeding and broader field
properties remain active work. Independent reader agreement does not establish
native Microsoft Access save/reopen compatibility.
