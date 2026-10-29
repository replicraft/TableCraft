# ACE 16 nullable batch UPDATE with index allocation

Status: implemented engineering preview. The first source-only gates pass all
80 independently constructed source/expected pairs. All 13 focused functions
pass, including 80 complete candidate comparisons, 16 exact comparisons with the
existing fitted batch service, and adversarial request/ownership controls. Three
correlated independent-authority controls also pass. The complete ACE library
passes 698 tests with two existing ignored tests; 22 shared integration targets
pass 148 tests. Strict ACE/CLI lint across all targets and features and the minimal
ACE build pass. CLI checks pass eight final tests and 30 browsing tests, with no
default behavior changes after the browsing run. These are literal engineering checks, not native
Access comparisons or permission to save ACE files.

## Working entry point

With `tablecraft-ace/ace16-write-preview`,
`NativeImage::preview_ace16_split_batch_updates(groups, budget, cancelled)` accepts
existing `FixedColumnUpdates` groups and returns `Ace16WritePreview`. Requests use
original snapshot-local live-row ordinals and exact expected values. Int32 and
NULL are distinct carriers; values are not coerced. Duplicate/stale assignments,
missing rows, failed constraints, resource exhaustion and cancellation return no
candidate.

The initial allocation profile accepts one table, Int32 assignments, and affected
Int32 single/tuple indexes. Tuples retain their native component order/directions
and existing format component bound; this complete-image checkpoint exercises
one and two components. Different valid table targets refuse before append-state
construction. Multiple-table allocation and wider indexed carriers remain
unfinished rather than permanent exclusions.

The existing fitted fixed/batch API keeps its output policy. Normal Jet 4 edits,
exact-slot updates, UPDATE cascades and DELETE/SET NULL consumers retain their
separate admission. The [strict cascade consumer](ACE16_SELECTED_FORWARDED_UPDATES.md)
does not inherit this allocation path. No formula, default, provider, macro or
application callback executes. Ordinary ACE saves remain blocked.

## Explicit engineering consumer

The existing batch example selects this service only with `--index-growth`:

```sh
cargo run -p tablecraft-ace --features ace16-write-preview \
  --example preview_ace16_batch -- original.accdb candidate.accdb \
  --index-growth --set Items Group 0 int32 null 42 \
  --set Items Score 0 int32 null 777
```

The source is captured without modification and the candidate path must be new.
The flag conflicts with `--cascade-updates` before any file is opened. Omitting
both flags retains the fitted API; cascade dispatch remains separate. This does
not expose normal CLI editing or permit native saves. Four focused example tests
and strict example Clippy pass; original-file candidate calls remain pending.

## One original operation and append ledger

Logical aliases are grouped by physical data number. Every original physical
owner is inventoried, and affected typed trees and complete final populations are
prepared before the first INDEX reservation. Cross-owner page overlap, ambiguous
table-definition aliases, unqualified index coverage and unreachable affected
owned storage cannot supply mutable spans. Physical processing follows ascending
data number, independently of column-group or row-request order.

Original key membership is bound to every logical table ROW, including omitted
keys. Every row's final tuple incorporates all requested components before its
constraints and final stream are checked. Required precedes omission; only an
entirely NULL secondary key is omitted. Partial-NULL UNIQUE keys remain
represented. An IgnoreNULL owner cannot become admissible through a nonprimary
alias when another alias makes that owner primary.

The [original-association history law](ACE16_IGNORED_NULL_UPDATES.md#historical-counters-and-batch-order)
is unchanged. Represented replacements/removals do not increment or decrement
history. An originally omitted row can introduce a new family, deduplicated across
incoming rows and physical aliases. Ordering-sensitive removed/reintroduced or
co-introduced families remain refused. Final distinct-family counts alone are
not historical-counter authority.

Existing trees retain each earlier leaf's original survivor capacity, reserving
entries for later retained leaves; the final leaf takes the remainder. The
existing recursive split owner normalizes overflowing partitions and propagates
complete child maxima. Root-leaf growth retains the root and allocates right then
left leaves. Nonroot growth inserts a fresh left sibling beside the retained
right page. All physical owners share one append cursor and map ledger. This
service does not search arbitrary alternative redistributions. Depletion needing
empty retained leaves, retirement or contraction remains refused.

INDEX bodies and separately qualified map metadata have distinct allocation
purposes. Existing supported index-owned inline-map transitions reuse the shared
split owner. The current complete-image corpus stays within represented map
windows; it does not qualify missing indirect pointers or GLOBAL-map bootstrap
for this service. DATA records never resize or allocate here.

## Payload and independent authority

A selected forwarded row retains its original logical ROW for index entries and
changed-row accounting. Its payload/mask patch targets the resolved final record
only after the complete original owned/exclusive-target census. Forward pointers,
slot words, row directories and hidden target headers remain unchanged. A
non-NULL replacement writes its exact four-byte payload and presence bit;
clearing presence retains the ignored old payload. NULL-to-NULL is unchanged.

The independent authority receives the original image, checked schema and
original request groups, not the producer's prepared key vector. It independently
rebinds cell expectations, logical ROWs, complete eligible key populations,
history and physical owner order. It replays capacities, splits, inclusive bounds,
siblings, allocation purposes and map effects, and closes an unchanged-byte proof
before returning the candidate. Source reads, materialization, sorting, allocation
and final checks share the caller's resource budget and cancellation callback.

## Evidence and remaining qualification

The [verification record](results/ace16-nullable-split-batch-updates-2026-10-29.json)
pins source/model checkpoints, actual commands and predecessor observations.

The [source checkpoint](ACE16_NULLABLE_SPLIT_BATCH_SOURCES.md) was committed before
producer implementation. Its 80 cases cover both ACE selectors, both directions,
UNIQUE/nonunique declarations, genuine aliases and selected forwarded payloads.
Five shapes cover two-owner root growth, an initially empty ignored leaf,
nonroot growth, simultaneous removal/new eligibility, and fitted branch retention.
Expected histories, DATA masks, opaque NULL payloads, complete INDEX pages and
shared allocation order were fixed before the first API comparison. All 80
candidates match those complete expectations without model repairs.

Additional controls cover repeated new families/history deduplication, reversed
requests, exact typed refusals, all resource pools and terminal cancellation.
Reader-valid source pairs distinguish unclaimed preserved INDEX bytes from an
unreachable page claimed by an affected owner. Independent correlated forgeries
change original requests, historical counters or source ownership together with
otherwise coherent candidate facts; all three refuse.

The first combined compile stopped on documentation lints in a separately added
XML inspection slice; documentation/import corrections followed. The first API
batch passed. The first strict lint run then identified the bounded inline
prepared-profile enum's size difference. Its local justification retains the
fallibly reserved, budget-metered vector instead of adding unmetered per-owner
boxing. These preparation/lint results are separate from candidate behavior.

Public-builder original-file sources and independent-reader comparisons for this
allocation path are still planned. No Java/native producer calls have run for
that study. Root-height growth, map-boundary variants, wider tuples/carriers,
cross-table allocation and contraction need their own acceptance. Required
[data-macro semantics](DATA_MACROS.md), native Access save/reopen and recovery
qualification remain open; this candidate has no journal or save operation.
