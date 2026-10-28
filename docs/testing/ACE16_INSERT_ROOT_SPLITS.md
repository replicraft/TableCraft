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
Actual engineering candidate and fresh getter qualification is recorded separately;
the reference experiment alone does not establish producer success.

## Remaining work

An INSERT into an existing non-leaf source still refuses, including a second
operation against this height-one result. Arbitrary-tree cardinality growth,
larger cascades, broader key/row profiles, defaults, generated fields, required
data macros and normal native saves remain unfinished. Independent Java results
do not replace native Microsoft Access comparisons, which the user has deferred.
