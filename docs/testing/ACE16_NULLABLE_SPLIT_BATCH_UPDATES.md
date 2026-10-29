# ACE 16 nullable batch UPDATE with index allocation

Status: implemented engineering preview, including shared allocation across
multiple tables. The initial one-table source-only gates pass all
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

The allocation profile accepts Int32 assignments and affected Int32 single/tuple
indexes. Multiple-table operations additionally certify all physical owners of
every participating table under that profile before reserving pages. Tuples keep
their native component order/directions and existing format component bound;
the complete-image checkpoints exercise one and two components. Wider indexed
carriers and broader combinations remain unfinished rather than permanent
exclusions. The original single-table path retains its admission for unaffected
owners.

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
not permit normal native saves. Four focused example tests
and strict example Clippy pass. The committed producer and original-file
qualification are recorded below.

## One original operation and append ledger

Logical aliases are grouped by physical data number. Every original physical
owner is inventoried, and affected typed trees and complete final populations are
prepared before the first INDEX reservation. Cross-owner page overlap, ambiguous
table-definition aliases, unqualified index coverage and unreachable affected
owned storage cannot supply mutable spans. Physical processing follows ascending
table-definition page and then data number, independently of table names,
column-group order or row-request order. One shared ledger preserves earlier
tables' GLOBAL bitmap changes. Reservations, root certificates and map transitions
retain both definition and physical-owner identity. Ordinary DATA, column-owned
LVAL and foreign INDEX body claims cannot overlap mutable selected INDEX pages;
distinct map records alone do not establish body exclusivity. Historical-counter
overflow is checked during preparation, before any reservation.

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

The [source-only original-file checkpoint](results/ace16-nullable-split-batch-sources-2026-10-29.json)
now passes its first nine public-builder/inspection commands and eight raw target
gates plus four Sentinel gates. Four unchanged V2016/V2019 ASC/DESC originals
contain 4,004 user records. Each target has the predicted four logical declarations
over two physical owners, leaf packing and 201/200 histories. All 2,400 known
creation warning records are preserved and classified exactly; no stderr is
silently suppressed. No expected image, public UPDATE, native candidate or native
Access comparison ran in that source phase.

The [complete original-file proof](results/ace16-nullable-split-batch-originals-2026-10-29.json)
now derives and independently reconstructs 20 whole expected images from those
sources and immutable requests before either writer runs. All 20 first native
candidate calls match all 9,928,704 expected bytes. Only after all full-image
comparisons pass do 20 separately bound fresh SDK readers verify actual values,
complete index/ROW streams, histories, maps and protected bytes. Every candidate
is rehashed before and after every reader. No candidate or model adjustment was
needed. The producer is an isolated fresh release build of commit `50d028cc`,
bound by the [committed receipt](results/ace16-nullable-split-batch-example-build-2026-10-29.json).

The separate public reference passes 20 ordered actions and 20 fresh reads over
8,008 row updates, preserving all 1,600 exact Clear warnings. Four fitted
duplicate-family images are byte-identical to the canonical expectations.
Public growth packing differs: Full appends six INDEX pages and one FREE page;
Clear appends five INDEX pages and one FREE page. The engine appends four INDEX
pages in either case. Public NULL payloads are not zeroed: cleared fields contain
opaque recycled scratch bytes, including 11,184 differing bytes. These and other
inactive INDEX/FREE bytes stay explicitly opaque. No public AFTER image supplies
canonical placement or expected payload authority. There are no unexplained
typed-value, mask, counter, map or protected-prefix differences.

Further root-height growth, map-boundary variants, wider tuples/carriers,
broader cross-table combinations and contraction need their own acceptance. Required
[data-macro semantics](DATA_MACROS.md), native Access save/reopen and recovery
qualification remain open; this candidate has no journal or save operation.

The [maintained saved replay](ace16_nullable_split_batch_updates/README.md) now
reconstructs the complete expectations and rechecks all saved phases without
starting children or rebuilding the producer. Its default root replay verifies
9,766 pin occurrences, 1,001 distinct paths and all 4,791 archived source files.
Missing consumer/output and tampered maintained-source controls refuse. The
retained initial verifier-authoring failure concerns argv reconstruction only;
executed programs, source bytes and canonical images remain unchanged.

## Multiple-table continuation

Forty-eight independently constructed source/expected pairs cover both ACE 16
selectors, both directions, UNIQUE/nonunique owners, NULL clearing, fitted/growing
table combinations and forwarded logical roots. Names sort in the opposite order
from definition pages; both tables repeat physical IDs zero and one. Allocations
share a GLOBAL bitmap byte and preserve opaque allocated orphans. Both source
qualification functions pass before the first candidate comparisons. All 96
normal/reversed-request comparisons then match complete expected images on the
first run, without changing those expectations.

The independent audit reconstructs tables and cells from original groups, sorts
actual source owners and advances one EOF/map replay. Correlated GLOBAL-map,
whole-population and same-number certificate substitutions refuse. Original
ordinary/foreign-index ownership claims also cannot waive a no-op's source checks.
Resource-pool and terminal-cancellation controls return no candidate.

Four relationship integration functions check complete parent/child assignment,
untouched incoming children and late stale values. Their initial read-qualified
donor needed genuine declaration terminators, distinct candidate-map records and
measured free-space metadata before it qualified for this allocating service.
Those fixture corrections preserve all ROWs, slots, indexes and relationships;
they do not relax the writer or change expected relationship errors. The initial
failures and diagnostic probe are retained separately.

The combined engine library passes 713 functions with two existing ignored tests;
this includes the separate ordinal-deletion helper. The
[continuation record](results/ace16-multitable-index-growth-2026-10-29.json) keeps
the literal freezes, checks and failed preparation observations. These are local
engineering checks, not native Access comparisons or production save permission.
