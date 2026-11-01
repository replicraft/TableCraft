# Normal ACE growth within an owned DATA page

Normal typed, retained-session and `query journal-update` requests now select
fitting or same-page growth from the original bound requests. Every target and
expected value is checked before selection. Qualified encoding determines the
minimum payload size against actual record capacity; a failed planner is never
retried in another mode. Existing explicit fitting APIs keep their previous limits.

## Admission and physical scope

Growth requires the native seventeen-field catalog family and its complete
NoRequired program/property/declaration checks. It supports the existing unindexed
Short Text/Binary groups with optional qualified fixed siblings. Indexed,
dependent/generated and Memo/OLE targets retain their refusals, including empty
groups. Qualified readonly siblings, properties and unrelated objects remain
protected. The initial global closed-content family remains fitting-only.

A growing record appends a hidden terminal on the original logical root's owned
DATA page. The original root becomes a forwarding capsule; its logical ROW
identity stays stable. Existing slots do not move or get reused. The slot/free
counts and new terminal/directory entry change, while map contents, indexes,
catalog, EOF and all unrelated pages remain exact. Empty deleted directory
entries remain inert. A 255-slot page refuses atomically; a 254-slot page can
use the final slot when enough byte capacity remains.

Fresh source-bound protection and the complete declared/live LVAL inventory are
retained through placement. Every prospective DATA page must qualify against
metadata, map and LVAL overlap. A separate proof reopens BEFORE, binds original
requests, independently rebuilds records and complete page geometry, and compares
the entire result and changed-row count. It consumes no producer page events or
selected ranges. Final constraints still apply. Fixed overlays use the resized
record's mask position; NULL retains inactive numeric payload bytes.

This profile allocates no page, changes no map and cannot choose another DATA
page or extend EOF. Those are subsequent milestones toward general native writes.
Insufficient original-page byte or slot capacity returns an explicit refusal.

## Original intent and recovery

`plan_recoverable_ordinary_variable_updates` chooses one native planner before
staging. Fitting retains TCNI0005; growth uses closed TCNI0006/profile6 with
original ordered fixed/variable groups, exact carriers and a complete BEFORE
binding. The new `owned_growth()` accessor keeps that body distinct from
`fitted()`. Public fitted encoding/decoding and TCNI1–5 bytes remain unchanged.
No physical destination, final page, permission or absence certificate is stored.

Recovery uses the stored operation profile, freshly repeats source admission and
reconstructs both complete images. It does not repeat Auto selection or replay
SQL. Equal file lengths retain the existing V1 journal and retained-target
lifecycle. Pending, partial and committed states retain typed outcomes and
write-free completion confirmation.

## Independent evidence

The [first and forwarded-continuation sources](../../testing/ACE16_OWNED_GROWTH_SOURCES.md)
provide two original, two growth and two second-edit images. Four additional
[joint sources](../../testing/results/ace16-joint-owned-growth-sources-2026-11-01.json)
combine growth with ID NULL/present changes, preserving inactive bytes and using
the new mask offset. Native source reads and eight independent SDK observations
precede their respective producer calls; none of the expected images come from
a writer's output.

Native commits, both original group orders, real typed/SQL sessions and CLI
saves/readbacks match the complete images. Recovery includes genuinely mixed
page prefixes and protected-content forgeries. Exact/one-less resource budgets,
cancellation and slot-boundary controls pass. Twelve native commits, twenty-four
typed/SQL session saves and twelve CLI saves/readbacks pass, alongside 1,105 ACE
units, 83 recovery regressions,
79 adapter units, 94 CLI units and strict feature builds. See the
[write record](../../testing/results/ace16-normal-owned-growth-writes-2026-11-01.json)
for exact counts and retained first failures. Microsoft Access save/reopen,
mixed-client and physical power-loss qualification remain unrun.
