# Ordinary DATA ownership promotion in INSERT previews

The separate ACE 16 INSERT engineering preview can promote a canonical ordinary
DATA-owned inline map to bitmap-backed storage when its complete member set no
longer fits an aligned 512-page window. This continues the INSERT-only rebasing
in the [GLOBAL allocation contract](ACE16_GLOBAL_MAP_INSERT_GROWTH.md). Normal
backend writes, journalable ACE saves and native Access mutation remain separate
gates.

## Source authority and allocation

The original owned map must have the qualified 69-byte inline record in its
owner-zero metadata container. Complete Metadata authority binds that original
record to the selected table's OrdinaryOwned role. Existing represented cells
keep their original path. A rebase retains every original and previously added
DATA page while `maximum − aligned_minimum < 512`; it preserves the original
window whenever that window still represents the new member.

When no such inline window fits, the producer reserves a separately typed
ordinary-owned bitmap after the DATA page. It uses the append state's actual
next page, which accounts for interleaved GLOBAL metadata, then rewrites the
original owned record as an indirect pointer. The complete original before-image
absorbs earlier cell or whole-row edits. No source cell is invented from a staged
base or fresh bitmap.

The bitmap includes all current ordinary DATA members and the new DATA page.
Its own page, GLOBAL metadata, INDEX pages, column-owned LVAL storage and opaque
allocated pages do not become ordinary DATA members. The bitmap is globally
used and has a closed definition/reference/block identity. It cannot substitute
for a candidate bitmap or a DATA reservation. Later DATA allocations update the
same new bitmap. Candidate-map maintenance follows owned metadata allocation,
and INDEX work continues on the shared resulting cursor.

The current promotion initializes block zero within the existing 64 MiB image
ceiling. Existing indirect and wider represented owned maps retain their prior
paths; UPDATE does not opt into this continuation. No map-record resizing,
in-file page reuse or additional resource allowance is introduced.

## Independent proof and literal corpus

The audit derives the complete owned membership from original DATA pages and
new placements. It independently decides between represented cells, an aligned
inline row and bitmap promotion, reconstructs metadata bytes and the original
record rewrite, and checks the exact ordinary-owned bitmap purpose inventory.
It then compares the complete candidate image. Staged map bytes or plausible
page counts cannot authorize a different member or metadata role.

Four source/request-only cases cover both ACE 16 headers with one or sixteen
new rows. They consume a prior independent literal image as a new source, retain
452 real visible rows and append allocated opaque pages to reach EOF 520. A
literal deleted filler makes the highest existing DATA page unable to fit the
new row while preserving its old logical ROW.

The single-row expected image adds DATA 520 and owned bitmap 521. The batch fills
DATA 520 and adds DATA 522, reusing bitmap 521. Existing GLOBAL bitmap 513 only
changes the corresponding allocation bits; fitting primary leaf 514 retains its
source prefix and unused suffix. Expected table and primary historical counters grow to
453 or 468. The expected images retain all old ROWs and every unrelated byte,
including the deleted filler and opaque pages; owned membership excludes the
metadata pages explicitly.

All four complete source/expected images passed native-format table, constraint,
row and ownership reads before any preview call. The new public controls compare
all four actual images, exhaust each resource pool one unit before healthy
completion and cancel at the actual terminal callback. A correlated private
control replaces the bitmap's closed purpose with DATA while retaining the same
candidate bytes, suffix cardinality and fresh source Metadata authority; its
healthy reconstruction precedes the refusal. All four owning test functions
passed in the first combined INSERT runtime: 249 functions passed. The full ACE
library passes 590 functions with one existing ignored fixture generator; all 49
selected shared integration tests pass. Strict ACE Clippy across all targets and
features, minimal compilation, formatting and documentation/dependency guards
pass. The first strict lint run found one nested conditional in the composer;
its let-chain correction changes no runtime semantics or expected image bytes.

## Qualification boundaries

These are independent synthetic native-format sources and expected images.
Jackcess's public table creator produces reference-backed ordinary-owned maps,
so it does not provide this inline-origin corpus directly. Authentic inline
ordinary ownership has been observed in an inert private sample, but that
sample's schema is outside the INSERT write profile and is not an admitted write
original. Its contents are not used as test fixtures.

An original-file producer comparison followed by fresh independent reader checks
has not yet qualified this continuation. Such comparisons would still be
separate from actual Windows Access mutation and native save/reopen evidence.
The [engineering preview boundary](ACE16_WRITE_PREVIEWS.md) remains unchanged.
