# Request-bound INSERT root-leaf growth

The optional [ordinary-row INSERT preview](ACE16_INSERT_PREVIEW.md) can now grow
its sole required ascending Int32 primary from an original leaf into a retained
height-one root and two fresh leaves. The table still has one Int32 and one Short
Binary field, with complete explicit values. This is an engineering candidate,
not a normal save or native Microsoft Access qualification.

## Cardinality and allocation

The continuation validates the complete final sorted key/ROW list, retaining
every original pair and rejecting duplicate keys or references. It qualifies
the original root as a leaf and checks that both final halves fit before using
the shared physical splitting mechanics. UPDATE key preparation and its
unchanged-cardinality recursive arena do not authorize INSERT.

DATA placement transfers its append state into index continuation and receives
that same state back. The retained root becomes a node with one left-maximum
separator and a right tail. The allocator reserves the right leaf before the
left leaf; index bitmap metadata may be reserved while allocating either child.
DATA, index and bitmap pages remain separate purposes under one EOF cursor and
global map ledger. Bitmap pages are excluded from index membership.

Fitted leaves retain their existing source-seeded representation. Fresh leaves
start with zero padding and repair their prefix only if capacity requires it.
The retained root keeps unowned residue. Table and historical physical-index
counters increase by inserted rows once per physical index, including aliases.

The independent audit rederives DATA placement first, then raw index framing,
separators, sibling links, map transitions and counters. It protects DATA map
edits during full index-map rewrites and compares the final aggregate ledger,
typed reservation inventory and complete image. It does not treat the builder's
plan or UPDATE certificates as authority.

## Verification scope

Focused runtime checks cover exact signed-key capacity, aliases, simultaneous
DATA/index EOF allocation, strict inline rebase boundaries, early and late bitmap
promotion, retained-root and fresh-page residue, source/request drift and resource
refusal. Public entry-point tests derive complete expected images and check them
before invoking the API. The full engine batch passes 341 unit functions (one
existing ignored test), four public preview contracts and seven example functions.
Strict all-target/feature lint passes.

An independent reference experiment generated four fresh V2016/V2019 sources
before appending anything. All four sources qualified as 451-entry leaves with
prefix one and seven free bytes. The 452nd entry requires 3,617 bytes against
3,616 available. Fitted sources use NULL/empty Binary values; full sources use
84-byte Binary values, making 41 records per DATA page and requiring DATA growth.
All 25 bounded reference calls passed on their first attempts.

Four engineering expectations were independently derived and frozen from source
bytes and requests before the new producer was built. Public-reference differences
are confined to unused fresh-index padding, which the engineering policy zeros.
All four engineering candidates subsequently matched those complete images on
their first attempts. Eight fresh read-only getter/map calls also passed, after
the whole-image checks. The [scoped record](results/ace16-insert-root-splits-native-2026-10-27.json)
pins the [committed producer](results/ace16-insert-root-split-example-build-2026-10-27.json),
raw reports and checks. Across the four results, 1,808 primary entries and 1,804
old ROWs agree, eight new index leaves and two new DATA pages are correctly owned,
and all 15 map-byte changes match. The 12,656 unused fresh-index bytes are zero;
14,412 bytes of retained-root residue remain unchanged.

The [saved verifier](ace16_insert_root_split/README.md) rederives all four images
and checks archived reference, candidate and getter evidence. Optional Git checks
read the producer source closure in one batch. Private artifacts are required;
their absence reports unavailable and is not a passing result or CI prerequisite.
No database or JVM is started by saved verification.

## Remaining work

A later fitted INSERT into this exact height-one, two-leaf form is supported by
the [dedicated retained-tree continuation](ACE16_INSERT_HEIGHT_ONE.md). That
branch keeps the entire root and original bound unchanged and allocates no index
pages. [One overflowing child](ACE16_INSERT_CHILD_SPLITS.md) now has a separate
continuation that adds one left leaf and preserves the original right page and
root identities. Multiple offenders, oversized halves, arbitrary-tree
cardinality growth, broader key/row profiles, defaults, generated fields, required data macros and normal
native saves remain unfinished. Independent Java results
do not replace native Microsoft Access comparisons, which the user has deferred.
