# ACE 16 index-owned usage-map promotion

The optional `ace16-write-preview` feature extends the existing
`NativeImage::preview_ace16_split_fixed_updates` and Int32 convenience method
with bounded inline-to-indirect index-owned map promotion. The existing
[fixed split](ACE16_FIXED_INDEX_SPLITS.md) scalar, index, request, property,
constraint and macro gates remain in force. This engineering preview returns
the existing non-journalable candidate; ordinary editing and native save
admission are unchanged.

## Qualified metadata transition

An existing 69-byte inline index-owned map can become indirect when its original
members plus the next required index child span more than its 512-page inline
capacity. If a shifted inline window could still fit those members, this profile
refuses `Unsupported(RowLayout)` instead of promoting. An unsupported future-cell
lookup alone does not authorize promotion, and reserving the bitmap cannot itself
create the need for promotion.

Other inline record widths retain the existing `Inspect(Unsupported)` refusal.
The recognized 69-byte shape with a still-shiftable window returns
`Unsupported(RowLayout)`. Shape qualification precedes the broader ownership
census so unqualified old profiles retain their original error precedence.

The original map record retains its physical range. Its indirect record points
to one freshly initialized usage bitmap; remaining pointers are zero. The bitmap
contains all original members and the actual new index children. Its own page is
metadata and is excluded from index membership. Global free bits account for
both kinds of allocation. There is no map-record resizing, continuation,
additional-pointer allocation, global-map bootstrap, in-file reuse or reclamation.
The existing image ceiling remains unchanged.

Both the root-leaf and recursive/repeated split planners use one operation-wide
allocation state. A first-reservation promotion places the right child at EOF,
the bitmap next, then the left child. Later promotion preserves prior reservations
and incorporates their membership into the new bitmap; their old inline byte
edits are retired. Physical indexes compose global effects even when they share
a bitmap byte. Represented-only cases retain their existing placement and bytes.

## Authority and independent certification

When any map promotes, the shared
[map authority service](ACE16_MAP_AUTHORITY.md) proves exclusivity for global free
storage and every index-owned map mutated by the operation, including
represented-only peers. It captures current and historical declarations,
canonical physical-index aliases, exact map-record ranges and bitmap storage.
Distinct nonoverlapping map rows may share a container. Unknown or incomplete
ownership profiles remain refused; writable cells still require their original
strict role grammar.

Final certification rebinds the original operation's table/index metadata and
repeats this proof. An independent allocation replay derives split demand, child
IDs, the need for promotion, metadata reservations, every rewritten map byte and
every bitmap/global effect. Planner-supplied touched bits or append roles are not
authority. Earlier represented reservations and mixed promoted/represented
owners are included in the replay.

Every appended page is independently certified as a complete index node/leaf or
usage bitmap, with no unclassified suffix or gap. Neither page kind has the
ordinary DATA signature required by ordinary forwarding and external LVAL reads.
The extension therefore cannot activate such a foreign reference by appending a
new DATA/LVAL target. This proves nonactivation for these append purposes; it
does not establish global health of unrelated foreign payloads.

All original bytes outside the exact selected field, affected index pages,
qualified map rows and independently derived map-bit edits remain protected.
The actual grown catalog, selected table/index membership and constraints are
checked before publication. Typed malformed, unsupported, cancellation and
resource failures remain visible and publish no candidate.

## Evidence and remaining qualification

[Public source/reference observations](USAGE_MAP_GROWTH_OBSERVATIONS.md) establish
two independent-writer inline-to-indirect transitions. Their 18-command history
includes the failed mutating V2016 helper and its separate read-only partial
classification; it is not an all-success reference run. The two old frozen
consumer MAX refusals are separate baseline evidence.

The scoped [implementation record](results/ace16-index-map-growth-native-2026-10-27.json)
records literal whole-image, late-promotion, mixed-owner, role-alias, resource,
reader-purpose and private finalizer checks, with first failures retained.
These checks do not establish Microsoft Access native save behavior. The separate
[original-file replay](INDEX_MAP_GROWTH_UPDATE_OBSERVATIONS.md) now passes both
unchanged originals through a newly committed frozen consumer. Each complete
image matches its pre-execution model and fresh typed row/index/map inspection;
all four commands pass without retries or changed inputs. Ordinary table/LVAL
map growth, shifted inline windows, larger map records and missing-pointer
creation remain unfinished work.
