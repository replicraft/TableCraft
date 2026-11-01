# Normal ACE growth within existing owned DATA pages

Normal typed, retained-session and `query journal-update` requests now select
fitting or owned-page growth from the original bound requests. Every target and
expected value is checked before selection. Qualified encoding determines the
minimum payload size against actual record capacity; a failed planner is never
retried in another mode. Existing explicit fitting APIs keep their previous limits.
The automatic route can additionally choose [one represented EOF allocation](NORMAL_ACE_EOF_ALLOCATION.md)
when all owned capacities are exhausted; explicit owned-only planning retains
this document's fixed-length contract.

## Admission and physical scope

Growth requires the native seventeen-field catalog family and its complete
NoRequired program/property/declaration checks. It supports the existing unindexed
Short Text/Binary groups with optional qualified fixed siblings. Indexed,
dependent/generated and Memo/OLE targets retain their refusals, including empty
groups. Qualified readonly siblings, properties and unrelated objects remain
protected. The initial global closed-content family remains fitting-only.

A growing record appends a hidden terminal on an eligible already-owned DATA
page. Placement first tries the original logical root's page, then other owned
pages in ascending page order. Eligibility requires the original candidate hint,
qualified ownership and enough directory and byte capacity. All changed records
are serialized and their combined capacity reserved before any page is staged;
multiple rows cannot independently claim the same final slot or free extent.

The original root becomes a forwarding capsule; its logical ROW identity stays
stable. Existing slots do not move or get reused. Destination slot/free counts
and the new terminal/directory entry change. A separate root page retains its
original count and free space. Map contents, indexes, catalog, EOF and all unrelated
pages remain exact. Empty deleted directory entries remain inert. A 255-slot
page cannot receive another terminal; a 254-slot page can use the final slot
when enough byte capacity remains.

Fresh source-bound protection and the complete declared/live LVAL inventory are
retained through placement. Every prospective DATA page must qualify against
metadata, map and LVAL overlap. A separate proof reopens BEFORE, binds original
requests, independently rebuilds records and complete page geometry, and compares
the entire result and changed-row count. It consumes no producer page events or
selected ranges. Final constraints still apply. Fixed overlays use the resized
record's mask position; NULL retains inactive numeric payload bytes.

This profile allocates no page, changes no map and cannot extend EOF. Allocation
remains a subsequent milestone toward general native writes. Exhausting all
eligible owned-page capacities returns an explicit refusal before staging.

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

## Alternate-page continuation

The [alternate-page source record](../../testing/results/ace16-alternate-owned-sources-2026-11-01.json)
adds independently authored BEFORE files whose original root directories are full
and whose separate destination DATA pages are already owned. Native reads and
six independent SDK observations qualify both first outcomes and separate
same-length forwarded continuations before corresponding producer calls.
Source construction adds a page; the actual edit preserves its existing EOF.

Exact native first/second commits, retained typed/SQL sessions and CLI saves and
readbacks pass. Recovery exercises each page order of genuinely partial writes,
settled states and write-free confirmation; committed mixed targets refuse.
Protected-content forgeries, exact/one-less resource pools, cancellation,
missing hints, full destinations, ownership conflicts and root-first preference
have focused checks. The separately qualified [two-row sources](../../testing/ACE16_MULTIROW_OWNED_GROWTH_SOURCES.md)
now match both request orders, three-page partial recovery and a destination
combining same-page and cross-page events. Healthy second-reservation exhaustion
refuses before publication. Ten native commits, eight typed/SQL saves and four
CLI saves/readbacks pass against sixteen independently read images; the full
regression batch and strict feature builds pass. See the
[alternate-page write record](../../testing/results/ace16-normal-alternate-owned-writes-2026-11-01.json)
for exact coverage and retained first failures. These results do not establish
native Access save/reopen or mixed-client safety.
