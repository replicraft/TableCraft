# General height-one primary INSERT

The optional [ordinary-row INSERT preview](ACE16_INSERT_PREVIEW.md) admits a
qualified height-one primary with any nonempty number of leaf children, including
one child. The table still has the same explicit Int32/Short Binary row profile
and sole required, unique, ascending Int32 physical primary with genuine logical
aliases. No scalar, metadata, constraint, map grammar or resource ceiling is
widened. This is the existing non-journalable engineering API and example, with
no new normal host write path or public preview method.

## Source forms and INSERT routing

The original root is a checked height-one NODE with nonempty LEAF children and a
complete reciprocal sibling chain. Every stored bound equals its child's full
maximum key/ROW pair. IndexOwned contains exactly the original root and children.
Duplicate pages, malformed bounds, wrong owners/heights/links and unknown metadata
retain their refusals. Existing zero-bound parsing still requires prefix zero;
no special reader bypass admits otherwise malformed single-tail roots.

Both a right-tail root and an all-bounded root with tail zero are admitted.
Only the first K-1 original child maxima are routing thresholds. Final immutable
key/ROW pairs at or below a threshold go to that child; all remaining pairs go to
the last child. The final child receives a new global maximum even when the root
has no tail. Every original pair remains present, and duplicate keys or ROWs
refuse. This routing is independent of original leaf counts; UPDATE's fixed-count
partitioning does not supply INSERT ranges.

The root keeps its original tail/bounded form. A tail root retains the original
last child's page as its tail. An all-bounded root stores every final child
maximum, including a changed last maximum. Preserving that source form is an
explicit engineering policy: public Jackcess may promote the final leaf to a
tail while modifying an all-bounded root. New reference coverage for these forms
remains separate from their literal checks.

## Multiple and repeated leaf normalization

Original children are completed in logical key order, independent of their page
numbers. A fitting range stays on its original page. An overflow first splits
at floor(count / 2): a new symbolic LEFT receives the lower half and the original
page retains RIGHT. Work then begins as `[RIGHT, LEFT]`, with separate logical
order `[LEFT, RIGHT]`.

Each paid pass visits each existing work item once. An overflowing item retains
its symbol/page as RIGHT, appends a new LEFT work item and inserts LEFT before
RIGHT in logical order. Newly appended LEFT may be visited in the same pass;
the just-processed RIGHT waits for the next pass. Passes repeat until the routed
range fits, then processing moves to the next original child. Fresh-symbol
creation order determines later allocation; it is separate from logical order.
This deterministic multi-offender schedule is an engineering policy, not a new
native/public bulk-order qualification.

Original pages always use their original prefix seed and source residue. A fresh
symbol always uses seed zero and zero final unused bytes, even when its RIGHT
portion splits again. Intermediate compression grants no retained-byte authority.
Origin is relative to the current immutable image; a page from a previous accepted
candidate is an original page in a later operation.

All final leaf capacities and final root capacity are checked before INDEX or
bitmap reservations. Only final nonempty fresh symbols receive physical pages;
there are no intermediate orphan reservations. The original root keeps its page,
height, owner, stamp and root-level links. All final bounds and leaf links are
reconstructed. Parent capacity is checked even if every leaf fits: changing an
all-bounded root's last maximum can reduce its prefix and overflow its bytes
without allocating a child. Parent overflow refuses atomically in this stage.

## One append state and independent raw proof

The actual DATA phase moves its one append state into the generic continuation.
Final fresh symbols are reserved in creation order, with separately typed index
bitmap metadata allowed between INDEX pages. Qualified represented membership,
inline rebases and bitmap promotion retain their canonical owner/range rules.
Only INDEX pages become IndexOwned; DATA and ordinary/index metadata bitmaps
remain distinct. Complete seeded DATA map effects, including unchanged bytes in
full 69-byte rewrites, cannot be retired by an index transition.

The final audit independently decodes the raw 13-byte root bounds and 9-byte
leaf pairs, validates source topology/ownership, rebinds immutable requests and
certified DATA ROWs, and derives its own routed spans and symbolic passes. Its
capacity arithmetic, maxima, sibling chain, EOF assignment and byte ledger do
not borrow the builder layout or event list. Parent preflight uses actual typed
maxima and represented widths; resolved child IDs are rechecked in complete raw
wires rather than supplied as dummy values during measurement.

Changed existing pages have exact full-page patches in root-then-original-leaf
logical order. Fresh INDEX/bitmap inventory is exact, including reservation
purpose and map ownership. Whole-image comparison protects all unrelated bytes.
Empty qualified batches remain byte-identical. Table and actual historical
physical-primary counts increase by inserted rows once per physical index,
including aliases. All prior accepted fitting, root-leaf and one-child complete
images remain unchanged.

## Verification boundary

The focused INSERT batch passes 78 functions after test-only counter-carrier and
fixture arithmetic corrections. The first runtime batch retained 77 passes and
one authored intermediate-i32 overflow; using i64 arithmetic with checked
conversion preserves the intended literal keys. Production and independent
expected images were unchanged. Four existing fitting tests moved to the actual
generic owner; the obsolete fitting helper files were deleted without wrappers
or compatibility exports.

Literal coverage includes arbitrary child counts, both source forms, last-child
clamping, fitting peers, multiple/repeated splits, immutable fresh-page seeds,
root-capacity refusal and shared DATA/index/map effects. Independent controls
retain the former both-offender and oversized-half sources as positive cases,
then use a source-valid all-bounded parent to prove fitting new-MAX admission
and genuine parent overflow. A forged alternate fresh-page destination order
first passes complete semantic table/index/constraint checks, then fails the
independent schedule proof.

The complete ACE unit batch passes 373 functions with one existing ignored test.
The 78-function INSERT subset is included in that count. Four public preview
contracts and three compile-fail checks pass, along with minimal compilation,
strict all-target Clippy, strict rustdoc and formatting. Additional fitting
controls on the same large parent-capacity sources pass before their overflow
requests refuse. Initial compilation, fixture-arithmetic and formatting failures
remain preserved separately; production and independent image policy did not
change to satisfy them. The previous full integration-target inventory was not
rerun for this slice.

All 25 bounded Jackcess source/reference calls passed on their first attempts.
Every source qualified before any final batch: 1,128 rows, three original leaves
with populations 451/226/451, and prefix-one outer leaves with seven free bytes.
Each final batch inserts [-227, 228], yielding 1,130 rows and five 226-entry leaves
under the original height-one root. Every new record fits existing DATA space;
each result adds two INDEX pages and no DATA or bitmap. These cases observe
multiple outer offenders in a tail-present three-child source. All-bounded forms,
other child counts and repeated splitting retain separate literal coverage.

Four source/request-only complete engineering images were frozen before the
committed producer build and candidate calls. All four first-attempt candidates
match those complete images. Eight distinct fresh getter/map calls also pass,
preserving all 4,512 original ROWs and 4,520 final primary key/ROW pairs across
the four files. Each file adds two INDEX pages, for 32,768 aggregate bytes, with
no DATA or bitmap allocation. Public compression and scratch remain observations,
not expected engineering bytes.

The getter controller's first run stopped after four successful calls because a
copied process ceiling was four rather than eight. A separately frozen continuation
verified the stopped prefix, every complete image and all original pins, then ran
only the four unattempted commands. No completed inspection was repeated. The
original run also used a stricter 8 MiB stream cap than its declared 16 MiB;
neither cap was hit. The stopped report and continuation remain separate in the
[evidence record](results/ace16-insert-general-height-one-native-2026-10-28.json)
and [saved-only verifier](ace16_insert_general_height_one/verify.py).
Earlier [fitting](ACE16_INSERT_HEIGHT_ONE.md) and
[one-child split](ACE16_INSERT_CHILD_SPLITS.md) original-file evidence stays
separate. Windows/native Microsoft Access comparisons remain deferred by the user.

## Remaining work

The final root must remain height one and fit its page. Original root-leaf growth
retains its separate two-fitting-halves limit. Recursive parent splitting
and root growth, deeper source trees, wider key/record profiles, generated/defaulted
values, required data macros and production native saves remain separate unfinished
work. General height-one admission removes per-child-count restrictions; it does
not redefine the existing full ACE 16 compatibility goal.
