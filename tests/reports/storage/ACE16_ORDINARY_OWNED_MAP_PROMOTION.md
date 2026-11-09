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
ceiling. Existing represented indirect and wider owned maps retain their prior paths;
UPDATE does not opt into this continuation. No map-record resizing,
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

## First INSERT with a missing indirect block-zero pointer

INSERT can also initialize block zero in an existing canonical 69-byte indirect
ordinary-owned record. Its first pointer must be zero; complete original Metadata
authority must bind the record to the selected table's OrdinaryOwned role. The
new DATA reservation precedes the separately typed ordinary-owned bitmap on the
same append cursor. The bitmap starts with the actual DATA member, while its own
page and other allocated metadata remain excluded. Neither this operation nor
inline promotion grants UPDATE a new allocation policy.

The source record has seventeen four-byte pointer slots after its one-byte tag.
Initialization copies the complete original record and changes only pointer zero.
Every higher pointer and its existing bitmap storage remains intact. Independent
audit captures that original row separately, derives the new bitmap and source
record rewrite from actual placements, and checks the closed purpose inventory.
It does not accept planner bytes as authority for erasing other pointers.

Four literal source/request cases cover both ACE 16 headers with empty live
membership, zero historical count and an empty primary leaf with a logical alias.
The owned record is reference-backed with a missing first pointer; the candidate
map is empty. The first row occupies DATA 16, owned bitmap 17 records only page
16, and both table and primary counters become one. An occupied but unclaimed
old DATA page remains byte exact and is never adopted. Two cases additionally
retain a valid allocated bitmap at the last pointer slot, proving preservation
of all seventeen slots rather than only the first sixteen.

The baseline source and expected-image gate passed before the first API
regression refused with `Inspect(Unsupported)`. No expected bytes changed to
resolve that failure. The higher-pointer sources and complete expected images
also passed their separate read-only gate before preview execution. All five
owning functions then passed: baseline whole-image insertion, both source gates,
correlated staged/candidate higher-pointer erasure refusal, all resource pools
and actual final-callback cancellation. The first combined INSERT batch passes
all 263 functions. The full ACE library passes 613 functions with two ignored,
and the full Access adapter suite passes 506 tests. Strict Clippy across all
ACE/Access targets and features, minimal builds, formatting and documentation/
dependency guards pass. The two ignored cases are explicit engineering fixture
and dictionary generators; they are not skipped acceptance checks.

Existing represented cells, other record widths, table-definition growth,
block-one initialization and general in-file reuse retain their earlier limits.
These four cases are repository-authored native-format literals. A subsequent
[original-file proof](ace16_insert_empty_owned/README.md) qualifies this
missing-pointer continuation on one immutable public-created V2016 empty table.
SOURCE3 capture and a separate raw gate precede two complete SOURCE/request-only
expected images. Four public reference calls pass two transition proofs; a fresh
isolated build of committed `463853e1` then produces two byte-exact candidates,
followed by two separately bound fresh reader checks. Both public files also
match the earlier complete images with zero differing bytes. The batch reuses
its owned bitmap across two DATA pages without adopting unrelated storage.
The [result](results/ace16-insert-empty-owned-2026-10-28.json) and
[build receipt](results/ace16-insert-empty-owned-example-build-2026-10-28.json)
retain phase identities and preparation stops. Saved-only replay passes with
subprocess creation blocked; missing evidence returns status 2.

## Qualification boundaries

These are independent synthetic native-format sources and expected images.
Jackcess's public table creator produces reference-backed ordinary-owned maps,
so it does not provide this inline-origin corpus directly. Authentic inline
ordinary ownership has been observed in an inert private sample, but that
sample's schema is outside the INSERT write profile and is not an admitted write
original. Its contents are not used as test fixtures.

The original-file missing-pointer proof does not qualify the separate
inline-origin promotion profile. Both forms of independent-reader evidence
remain separate from actual Windows Access mutation and native save/reopen
qualification.
The [engineering preview boundary](ACE16_WRITE_PREVIEWS.md) remains unchanged.
