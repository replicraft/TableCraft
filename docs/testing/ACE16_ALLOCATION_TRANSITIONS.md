# Ordinary hints and index allocation transitions

The existing feature-gated
`NativeImage::preview_ace16_variable_split_updates` now supports qualified
ordinary candidate-map and index-owned-map inline window changes. It still
returns a separate non-journalable engineering candidate. No normal CLI/backend
save path is enabled. The [combined service contract](ACE16_VARIABLE_INDEX_SPLITS.md)
and its existing field, index, constraint and active-content restrictions apply.

## Allocation policy

Ordinary placement tries the highest hinted owned DATA page once. A real capacity
failure clears that hint alone; lower untried hints remain. A new DATA page is
reserved before any metadata page needed to represent its candidate membership.

For an exactly 69-byte inline record, an addition inside the current 512-page
window keeps its base, including the final represented bit. An out-of-window
addition selects the minimum member rounded down to eight pages. The new window
is permitted only when its aligned inclusive span is strictly less than 512.
Otherwise the operation appends an indirect bitmap and rewrites the original
record into its pointer form. The new bitmap contains candidate DATA members;
the bitmap itself is only global allocation metadata.

Qualified affected index maps use the same window rule with their own original
members and prior INDEX reservations. They never acquire DATA or candidate-map
metadata members. A later promotion absorbs prior index-cell/rebase effects
against the original record bytes. Existing slot, page, represented-variable and
numeric split APIs keep their previous admission rules.

Ordinary owned and global maps must already represent each required allocation.
Existing indirect candidate maps retain their represented-only boundary. Empty
indirect-pointer creation, global-map bootstrap, arbitrary page reuse and general
allocation remain unfinished. These transitions do not implement INSERT.

## Authority and final proof

A checked raw inline record supplies framing, not permission. A complete original
metadata census binds the selected owner, canonical map reference and entire
record range before a rewrite. Source table-definition chains, map containers,
bitmap storage and other owners remain protected. Unknown row framing does not
gain rewrite authority.

The shared byte ledger replaces a complete authorized record, retaining original
before bytes and retiring only that record's provisional cells. It preserves
global bits sharing a byte with other allocations. Reservation purposes distinguish
DATA, ordinary candidate bitmap, INDEX and index-owned bitmap pages.

Independent DATA replay reads original raw map bytes and requests. It reconstructs
failed hints, window choice, all 69 rewritten bytes, bitmap contents, allocation
order and the complete DATA-phase suffix. Its opaque certificate binds the exact
original image and candidate slice. Index replay derives its continuation policy
from that certificate and cannot retire any certified DATA-phase record bytes.
The composer requires exactly one typed object for each appended page.

## Verification

The [scoped result](results/ace16-allocation-transitions-native-2026-10-27.json)
records checks and retained initial compiler/lint diagnostics. Fifteen new test
functions cover framing, canonical authority, complete-record composition,
reservation identity, bitmap bounds, strict boundaries and complete candidate
images.

Independent Binary900 images exercise both ACE 16 header codes and index
directions. Actual deleted-record DATA pages and a relocated physical index root
establish rebase geometry. Promotion images require DATA, candidate bitmap, INDEX
and index bitmap pages in distinct exact positions. Both source and expected
images pass reads, index and constraint checks before invoking the writer.
Forgeries remove a preserved lower hint, restore the failed highest hint or add
metadata to ordinary ownership; final proof rejects each.

Private index controls distinguish aligned spans 511 and 512, preserve the last
bit of an already represented window, and promote after prior rebase/represented
reservations. Whole-record controls retain shared global bytes and reject stale
before bytes, invalid ranges, exhausted resources and cancellation without a
partially rewritten ledger.

The [four enlarged source copies](ORDINARY_HINT_GROWTH_SOURCE_OBSERVATIONS.md)
are separate preparation evidence. They do not yet qualify this implementation
on original files. Their public UPDATE references and committed-consumer replay
are separate: the [twelve public-writer cases](ORDINARY_HINT_UPDATE_OBSERVATIONS.md)
now pass, while native candidate replay remains pending. Native Microsoft Access
comparison, required data-macro qualification and production saves remain
unfinished.

The [isolated consumer build](results/ace16-hint-transition-example-build-2026-10-27.json)
compiles committed `69443c6a` into a separate frozen example. Its complete export
contains 4,217 Git-verified files; all remain unchanged after compilation, with
295 selected source pins recorded separately. The first offline dev build passes
with explicit debug information disabled. This is build evidence only, with no
original-file candidate calls or performance qualification.
