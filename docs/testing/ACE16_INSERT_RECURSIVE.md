# Recursive primary INSERT engineering preview

This checkpoint extends the existing non-journalable ordinary-row INSERT preview
through the same [API and example](ACE16_INSERT_PREVIEW.md). The admitted row profile remains explicit
Int32 plus nullable Short Binary, with one required, unique, ascending Int32
physical primary and genuine logical aliases. It adds recursive index routing,
internal-node splitting and one retained-root growth event. It adds no normal
host write path, wider scalar profile or native-save qualification.

The focused INSERT batch passes 88 functions within 383 passing ACE unit
functions, with one existing ignored test. Four public preview contracts, three
compile-fail checks, minimal compilation, strict Clippy, rustdoc, formatting, the
15-package guard and 6,048 documentation targets pass. Its literal checks are
separate from new original-file or Microsoft Access qualification evidence.

## Immutable source and routing

A NODE source may have any admitted nonempty child count and any supported
height below the existing depth ceiling of 16. All branches at each level have
consistent heights; leaf pages are nonempty. Source capture independently reads
raw nine-byte key/ROW pairs and thirteen-byte child bounds. Every original bound
equals its child's complete maximum pair. Root links, per-height reciprocal
sibling chains across parent boundaries, owners, stamps, child identities and
exact IndexOwned membership are checked. Repeated pages, cycles, unbalanced
branches and malformed empty-bound prefixes refuse. The existing reader is not
bypassed to admit a malformed source.

Each original subtree receives an assigned final sorted-key range. Only its
first K-1 original inclusive child maxima are routing thresholds; its final
child receives the rest of that assigned range. This rule applies independently
at every depth. A final child of an all-bounded branch can therefore receive a
new maximum, and its stored parent maximum changes accordingly. Routing never
uses the number of original entries as an INSERT partition.

Existing branches retain their original tail or all-bounded form. Retaining a
bounded final child is an explicit engineering policy, distinct from a public
writer that may convert it to a tail. All original key/ROW pairs remain present;
new keys and ROW references come from immutable requests and the independent
DATA-record/placement proof.

## Postorder normalization and source origin

Original subtrees finish in logical postorder. A fitting candidate keeps its
original page. An overflow divides at floor(count / 2), by leaf entries or branch
children: fresh LEFT receives the lower half and the existing symbol retains
RIGHT. A new internal LEFT uses a tail; an existing internal RIGHT retains its
source tail form. Every resulting child maximum and parent capacity is checked,
even when no leaf allocated a page.

Each local normalization starts with work order `[RIGHT, LEFT]` and separate
logical order `[LEFT, RIGHT]`. Paid passes retain stable work/allocation order.
An overflowing item keeps its RIGHT symbol, appends fresh LEFT, and places LEFT
before RIGHT logically. New LEFT may be visited in the same pass; the processed
RIGHT waits until the next pass. Processing completes this original subtree
before moving to its next original sibling. This scheduling is an engineering
policy, not a claim about arbitrary public-writer bulk order.

Existing pages always derive framing from their immutable source prefix and
retain source bytes beyond the new active payload. Fresh pages always use seed
zero and zero unused bytes, including a fresh RIGHT split again later in the
same operation. A page produced by a previous accepted operation becomes an
Existing page when that new immutable image is the next source.

All final leaf and branch capacities are preflighted before physical INDEX or
bitmap reservations. Only final reachable Fresh symbols receive pages; temporary
orphan reservations are not created.

## Retained-root growth

When the original root cannot fit, its completed contents first become a Fresh
RIGHT symbol. Fresh LEFT is created next, and these same local passes normalize
both sides at the old height. The original root ID becomes a GrownRoot one level
higher, with seed zero and original residue. Newly nested internal pages use
tails and have no original-residue authority.

An original NODE produces an all-bounded grown top with tail zero. An original
LEAF produces a tail-present grown top. The existing two-fitting-halves LEAF
constructor remains the first path and preserves its accepted complete images;
only larger/repeated LEAF overflow uses this recursive continuation. The choice
uses independent audit-owned capacity measurement, propagating typed errors
rather than treating arbitrary errors as an overflow signal.

One root growth event is permitted in an operation. The resulting top must fit
and its new height must remain below the existing depth ceiling. Failure refuses
the entire candidate. Additional nesting of a still-oversized new top is separate
future work; later operations may use any currently admitted grown source.

## One DATA/index state and independent final proof

The DATA phase transfers its sole append state into index continuation. INDEX
symbols reserve in creation order, with independently typed index bitmap pages
allowed between them. Metadata pages are excluded from IndexOwned. Qualified
represented membership, inline rebase and bitmap promotion retain the existing
canonical-owner and exact-row authority. Earlier INDEX reservations are absorbed
when later promotion occurs. Full seeded DATA map effects cannot be erased by
an index rewrite or promotion.

The final auditor independently captures original raw topology and membership,
routes immutable final pairs, normalizes its own symbols, measures capacity and
reconstructs every final page. It does not consume the builder's layout, maxima,
original-count partitions, event schedule or capacity decisions. Physical IDs
come from independent EOF/map replay. Measurement uses actual typed maxima and
wire widths; child IDs are resolved before exact final wire checks.

Every original page has its exact full-page patch in source root-first preorder
when changed. Fresh INDEX and bitmap inventory, reservation purpose, promotion
and rebase metadata are checked exactly. The final expected image protects all
unowned bytes. Counters still advance once per inserted row for the table and
actual physical primary, including aliases. No-op batches retain their exact
original bytes.

## Verification boundary

Independent literal coverage includes:

- all-bounded prefix-loss root growth and a later fitted operation on its H2
  result;
- nonroot internal splitting, per-height cross-parent links and multiple leaf
  offenders under distinct parents;
- an existing height-three fitted source and final-child clamping;
- DATA-first allocation, inline rebase followed by bitmap promotion, and a large
  original LEAF requiring four final fresh leaves;
- source/request-bound full images, semantic controls before padding/residue
  forgeries, separate resource pools, late cancellation and retry.

Prior height-one fitting, general-routing and child-split controls move to the
actual recursive auditor without compatibility wrappers. The old large bounded
parent sources retain their exact bytes; fitting requests remain controls and
previous capacity refusals become independently modeled recursive positives.
Depth-ceiling and new-top-capacity refusal guards remain explicit; the current
positive literals do not establish every source height or arbitrary top nesting.
The prior fitting, general height-one and one-child split images remain unchanged
under the migrated controls.

Existing original-file root-leaf, fitted-height-one, child-split and general-H1
receipts remain separately scoped historical evidence. New two-field source
preparation, public references and original-file engineering candidates are
recorded below as separate phases.
Windows/native Microsoft Access comparisons remain deferred. Wider key/record
profiles, generated/defaulted values, required data macros and production native
saves remain unfinished work toward the full ACE 16 target.

## New original-source preparation

Four fresh Jackcess sources now qualify the exact Int32/Short Binary profile:
V2016/V2019 crossed with root and nonroot internal-parent pressure. Existing
two-LONG databases are geometry precedents only; they were not renamed,
retargeted or patched into this profile. Public builders create the new tables,
then public cursor deletion on fresh copies retains the selected dense tail and
each other leaf's maximum. Every retained old key/ROW, bound, index page graph,
height and per-level sibling chain remains exact.

| Mode, in both profiles | Live rows | Historical physical counter | Retained INDEX pages | Required leaf/parent bytes after one MAX insertion |
| --- | --- | --- | --- | --- |
| Root pressure, height one | 782 | 99,424 | 330 | 3,641 / 3,621 |
| Nonroot internal pressure, height two | 963 | 149,184 | 497 | 3,769 / 3,621 |

These are source-capacity certificates against the 3,616-byte page payload;
they are not observed post-INSERT allocation or topology. Actual historical
counters are read separately from live rows after deletion, never reset to live
count. The unchanged schema has manual required Int32 ID and nullable Binary255,
a sole ascending required/unique primary, and an inert sentinel. Selected schema,
full live records, deleted-slot geometry, complete index membership and existing
future map cells pass the source gates. Reported selected/global map roles do
not claim a complete native metadata census or writable-cell authority.

The first source run completed five fixture commands successfully, then its
Python source checker rejected ordinary public-delete tombstones. That initial
report remains incomplete. The native words are `DELETED | RELOCATED` (`0xc000`),
which the existing reader skips as deleted and the existing delete writer also
emits. A separate proof derivative explicitly admits zero, DELETED, or
DELETED|RELOCATED flags, retaining unknown-bit and live-relocation refusals.
Saved-only recertification then passed the first source without repeating any
completed command. A separately frozen continuation ran only the twelve
unattempted source commands; all twelve and all remaining source gates pass.
No helper, population, scalar, selector, cap or original source byte was changed.
Across all four sources, ordinary DATA pages remain byte-exact to their larger
originals except for the public deleted-slot tombstone words.

The [source record](results/ace16-insert-recursive-sources-2026-10-28.json) keeps
those first-five/checker-failure/saved-proof/remaining-twelve phases distinct.
Total fixture work is one compile and sixteen generation/prune/read-only probe
commands, with zero INSERT reference or native candidate calls. The later
public writer may reuse deleted DATA slots; engineering append-only ROWs must
be modeled from immutable sources rather than coerced to public placement.
No new Microsoft Access comparison or recursive candidate acceptance follows
from these source certificates.

## Actual reference and engineering candidates

Four source/request-only complete engineering images were frozen before the
committed producer build or any candidate calls. All four first-attempt producer
outputs match those complete images. Eight separate fresh read-only Jackcess
getter/map calls pass on their first attempts, after all four full-image matches.
They confirm model-derived records, ROWs, complete index trees/maps and unchanged
sentinel data; public AFTER output does not supply expected bytes or placement.

| Case | New ROW | Root height | New INDEX pages |
| --- | --- | --- | --- |
| V2016 root | 816:229 | 1 to 2 | 818, 819, 820 |
| V2016 internal | 1180:9 | 2 retained | 1181, 1182 |
| V2019 root | 834:229 | 1 to 2 | 836, 837, 838 |
| V2019 internal | 1198:9 | 2 retained | 1199, 1200 |

Every new 13-byte MAX/empty-Binary record fits an existing DATA page. Root cases
split a leaf and grow the retained root through two fresh internal pages;
internal cases split a leaf and its nonroot parent while retaining the height-two
root. Across the four candidates, all 3,490 old ROWs survive and 3,494 final
primary pairs agree. Ten INDEX pages add 40,960 bytes; no DATA or bitmap page is
allocated. Table and actual historical physical counts each increase once.
The [candidate record](results/ace16-insert-recursive-native-2026-10-28.json)
keeps original sources, pre-build goldens, producer provenance and fresh getters
separate from the source and public-reference phases. The
[saved-only verifier](ace16_insert_recursive/README.md) requires the retained
private artifacts, starts no fixture/JVM and reports missing evidence as
unavailable rather than a pass.

All twelve public reference child commands also succeed, but their first report
is incomplete: three successful INSERT/getter/map commands were followed by a
protected-prefix assertion on a relinked peer's unused bytes. Saved-only analysis
proves that peer's original header, bitmap and active key/ROW payload unchanged
except its required next pointer. The separate proof derivative allows only that
source-framed unused suffix and records its changes. A frozen continuation then
runs only the nine unattempted commands; no completed call repeats. In each root
case the relinked peer changes 1,776 unused bytes. Internal cases change 1,859 and
1,009 unused bytes in their two relinked peers. All other original prefix bytes
remain protected by the closed transition proof.

Public and engineering new ROWs happen to agree in all four actual cases: both
append to the same fitting DATA page. Public ROWs were neither forced nor used
to derive the engineering model. Jackcess moves the ceiling count-half to LEFT,
producing root-case leaf populations 228/227 and internal populations 236/235.
Engineering floor-halving produces 227/228 and 235/236 respectively. INDEX
framing and unused bytes also differ: public scratch is an observation, while
engineering policy retains source residue and zero fresh padding. Direct complete
comparison finds differences only on INDEX pages: 12,328/12,329 bytes in the two
root files and 15,674/15,673 in the internal files. DATA, counters, map effects and
all other bytes agree in these actual outputs.

The [public-reference record](results/ace16-insert-recursive-public-reference-2026-10-28.json)
preserves the stopped first-three proof, separate saved recertification and
remaining-nine continuation. These Jackcess observations and ACE engineering
images do not establish Microsoft Access save/reopen compatibility. Required data
macros and normal native saves remain unqualified.

## Preserved first-result history

The first INSERT test batch retained 87 passes and one authored boundary-assertion
failure. The failing function had already constructed and structurally checked
its full expected image, run the actual preview, and matched the complete bytes.
Its later assertion incorrectly expected bitmap promotion after INDEX 518 and
519. Adding 518 rebases the inline window to base 8; 519 is then represented at
the final bit and does not promote. A third out-of-window page 520 is needed to
exercise later promotion. The correction keeps the original 518/519 source and
complete image as a positive final-inline-cell control, and adds a separately
constructed three-offender source that allocates INDEX 520 and bitmap 521. The
model and production code did not change; the focused retry passes all 88
functions. The first 87-pass/one-assertion-failure result and sources remain
preserved.

The first strict lint run retained three mechanical style findings: one late
height initialization in the independent source reader and two collapsible
conditions in the builder source reader. Expression initialization and let-chain
corrections preserve image policy and behavior; the strict retry passes. The
first formatting failure concerned module registration order and its scoped
retry passes. These failures and before sources remain separately preserved.
The preceding full integration-target inventory was not rerun for this slice.

The source/request-only images, committed producer and candidate/getter phases
above are separate evidence; these unit checks did not supply their outcomes.

The root-residue adversary now mutates its existing patch in place, preserving
canonical patch order so an unrelated order defect cannot explain rejection.
Healthy full-image and table/index/constraint controls precede these forgeries.
