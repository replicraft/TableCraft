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
receipts remain separately scoped historical evidence. No new recursive public
source/reference or original-file engineering candidate is claimed here.
Windows/native Microsoft Access comparisons remain deferred. Wider key/record
profiles, generated/defaulted values, required data macros and production native
saves remain unfinished work toward the full ACE 16 target.

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

New authentic recursive source/reference actions have not run. A producer build,
source/request-only frozen engineering images and original-file candidate/getter
qualification remain later phases, not completed by these checks.

The root-residue adversary now mutates its existing patch in place, preserving
canonical patch order so an unrelated order defect cannot explain rejection.
Healthy full-image and table/index/constraint controls precede these forgeries.
