# INSERT split of one existing primary child

The optional [ordinary-row INSERT preview](ACE16_INSERT_PREVIEW.md) can split
one overflowing child of its admitted height-one primary. The original source
must still have exactly one inclusive Int32 key/ROW bound, two nonempty leaves
and a right tail. The same required ascending unique primary, true physical
aliases, complete explicit Int32/Short Binary rows and metadata gates apply.
This is an engineering candidate under the existing preview API, not a normal
save, journalable edit plan or native Microsoft Access qualification.

## Routing, page identity and framing

The complete sorted final key/ROW stream retains every original pair. Requests
route against the original inclusive bound: pairs at or below it enter the left
partition, and the rest enter the right tail. Exactly one final partition must
overflow. If both fit, the [fitting continuation](ACE16_INSERT_HEIGHT_ONE.md)
applies. Two overflowing partitions or a split half that still does not fit
refuse; no broader recursive INSERT normalization is activated.

The offender splits at floor(count / 2). Its lower half goes to a fresh LEFT
leaf; the original page retains the RIGHT half. Both halves, the fitting peer
and the final root must fit before index allocation starts. The peer may also
receive fitting INSERTs in the same batch. An unchanged peer keeps its content
and only changes a sibling field when the new page is inserted beside it.
Odd cardinalities follow this declared engineering floor policy; they have not
been qualified against new public/native reference actions.

For a left-child split, leaf order becomes `[freshLEFT, oldLeft, oldRight]`.
For a right-child split, it becomes `[oldLeft, freshLEFT, oldRight]`. All reciprocal
previous/next links follow that order. The original root remains at height one
and keeps its page identity, owner, root-level links and original right tail.
It now stores two bounded complete key/ROW maxima with big-endian child pointers.
A left split adds the fresh-left maximum before the original bound; a right
split adds it after the original bound.

Retained leaves and the root use their original prefix seeds, reduced to the
full-wire common prefix and repaired to that common prefix only when capacity
requires it. Fresh LEFT uses seed zero and zero unused bytes. Existing page
headers and residue remain unchanged outside the exact new entry framing and
required links. With an original zero-prefix root, two 13-byte bounds occupy
26 bytes; its protected suffix begins at byte 506, after the expanded entries.
The old 493 boundary belonged to the earlier one-bound representation.

## Shared allocation and independent proof

DATA construction moves the same append state into index continuation. The new
LEFT is the next DATA-phase EOF page. If index-owned map promotion is needed,
its separately typed bitmap immediately follows. Represented index membership,
qualified inline rebasing and bitmap promotion reuse the existing canonical map
authority. Only actual index pages become IndexOwned members; DATA and both
ordinary/index bitmap purposes remain distinct under the shared cursor and
global-byte ledger. No original index page is retired.

The final audit captures the original root and leaves directly, binds the final
key/ROW stream to immutable requests and independently certified DATA placements,
and replays the original-bound partitions. Its own wire renderer and capacity
checks derive the offender, halves, root maxima, page identities, sibling chain
and exact retained/fresh images. UPDATE's original-count partitioning, builder
arena, maxima and capacity decisions do not supply that authority.

Independent map replay starts with the certified DATA EOF and original-before/
current-after ledger. Complete catalog ownership and canonical record permission
qualify index map rewrites. Every certified DATA map offset, including unchanged
bytes in a rewritten 69-byte record, remains protected. The final proof checks
exact full-page patches for changed root/old leaves, one fresh INDEX reservation,
any separate bitmap, rebase/promotion inventory, all aggregate map effects,
complete suffix purposes and the complete candidate image. Table and historical
physical-primary counters increase by request count once, including aliases.

## Verification boundary

The first integrated INSERT batch passes all 68 functions, including 13 new
builder, public-preview and independent audit controls. Literal sources measure
a 451-entry child at prefix one with seven free bytes; adding the missing key
requires 3,617 bytes against 3,616 available. Both headers and split sides are
covered, along with fitting peer batches, floor-sized odd halves, DATA-first EOF,
shared map bytes, represented allocation, strict inline rebase and promotion.
Healthy checked candidates precede residue, request-drift and extra-patch
forgeries. Both-offender and still-oversized-half sources retain explicit refusal.

The subsequent complete ACE unit batch passes 363 functions with one existing
ignored test. Four public preview contracts and three compile-fail doctests pass.
The 68-function INSERT subset and its 13 new controls are included in the unit
count. Strict all-target/feature Clippy passes after moving an unused test-only
`KeyRef` import into its local test module; the initial warning and strict failure
remain preserved. Minimal-feature compilation, strict rustdoc, formatting,
the 15-package dependency guard and 6,019 local documentation targets pass.
This slice did not rerun the preceding fitting checkpoint's full 135-summary,
1,228-test target inventory.

A separate public Jackcess source/reference phase passed all 25 bounded calls
on their first attempts. All four originals qualified with 677 rows, one root
bound and two leaves; the right child had 451 entries, prefix one and seven free
bytes. Each reference INSERT produced 678 rows and three leaves. All four new
records fit the prepared highest DATA hint: the references add one INDEX page,
no DATA or bitmap, and only represented global/index-owned membership effects.
Left-child, fitting-peer bulk, odd-half and broader DATA/map allocation cases
remain literal controls.

Public retained RIGHT resets its prefix from one to zero, with 2,034 active bytes
and 1,582 free bytes. The engineering policy retains the original seed one,
producing 1,809 active bytes and 1,807 free bytes for that same half. Engineering
expectations must be independently derived from original bytes and requests;
public page bytes, compression and scratch are observations rather than candidate
goldens. The public writer also changes unused bytes in the retained root and
right child and copies nonzero padding into fresh LEFT. Engineering policy keeps
original retained residue and zero fresh padding.

Four source/request-only engineering images were frozen before the committed
producer build and candidate calls. All four first-attempt candidates match
those complete images, followed by eight successful fresh independent getter/map
calls. Across the four candidates, all 2,708 original ROWs remain intact and
2,712 complete primary key/ROW pairs agree. Each result adds one INDEX page;
the aggregate growth is 16,384 bytes, with no DATA or bitmap allocation. Exact
counter deltas and the engineering prefix/residue policy remain independently
checked, rather than borrowed from public output. The
[evidence record](results/ace16-insert-child-splits-native-2026-10-27.json) and
[saved-only verifier](ace16_insert_child_split/verify.py) retain the source,
reference, complete-image and getter phases separately. The earlier
[fitting height-one qualification](ACE16_INSERT_HEIGHT_ONE.md) and
[root-leaf growth qualification](ACE16_INSERT_ROOT_SPLITS.md) remain separate.
Windows/native Microsoft Access comparisons remain deferred by the user.

## Remaining work

A source with three children, deeper trees, multiple overflowing children,
repeated splits of a half, broader index/record profiles, generated/defaulted
values, required data macros and normal native saves remain unfinished. A later
INSERT into this new three-leaf result is outside the current source profile.
