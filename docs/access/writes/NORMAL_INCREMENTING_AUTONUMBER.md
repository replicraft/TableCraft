# Normal Increment AutoNumber insertion

Normal omitted-field INSERT now generates checked Int32 Increment values for a
qualified native declaration. Typed/SQL sessions and CLI journal insertion use
the existing connected planner; no caller permission flag or new intent version
is involved.

The initial shape has two fixed Int32 fields, generated first, and one ascending
required unique primary root leaf over that field. Complete Source, property,
constraint and record checks precede planning. Native AutoIncrement flags and a
complete inspected absence of a DefaultValue select Increment in this qualified
shape. The stored `GenUniqueID()` declaration selects Random and refuses; unknown
or malformed declarations do not fall back to Increment. LAST alone, a column
name, omission or a primary index never establishes generation mode.

Original requests must omit the generated field and supply an Int32 companion.
Supplied IDs and explicit NULL remain supplied and refuse this route. LAST is
incremented with checked signed arithmetic; it is never repaired from the maximum
live key. Thus a generated collision still reports DuplicateKey. Exhaustion,
Random, unknown defaults, unsupported layouts and resource/cancellation failures
publish no candidate. Empty requests retain the same declaration checks.

The existing counter implementation is shared with the preview consumer. Normal
staging writes the fitted DATA row, primary key, counters and LAST; a separate
proof reconstructs those changes from the original source and original presence.
TCNA2 and checked V1 journal recovery retain that original intent, with no random
provider or inference during replay.

Fourteen independent source views passed complete native reads, CREATE_NEW export
and inert SDK observations before matching mutation tests. Four literal native
commits, empty requests, direct whole-image proof/forgery checks, resource and
recovery controls, eight retained typed/SQL saves and four CLI saves/readbacks
pass. The execution record retains initial integration compile failures and their
corrections. This is portable independent evidence; native Access save/reopen
remains unrun.

Fresh generated declarations, Random generation, arbitrary AutoNumber layouts,
reseeding and generated columns combined with broader field policies remain
separate work. See the [source record](../../testing/ACE16_AZL_AUTONUMBER_SOURCES.md)
and [execution record](../../testing/results/ace16-normal-increment-autonumber-writes-2026-11-01.json).
