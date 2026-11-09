# ACE16 nullable descending INSERT sources

This source-only tranche starts from the two unchanged SDK originals qualified
by [the descending/NULL DELETE source record](ACE16_NORMAL_INDEXED_DESC_NULL_DELETE_SOURCES.md).
It does not reuse any DELETE AFTER or native writer output. Items contains three
manual Int32 fields with `(1,-1,NULL)`, `(2,0,1)`, `(3,1,2)`; ascending primary
aliases and descending ordinary Guard aliases share two physical roots.

The independently modeled requests append `(4,17,NULL)`, then an ordered batch
appends that row plus `(5,18,1)`. Both added Guard families already exist. The
pinned SDK source confirms duplicate detection compares key bytes excluding ROW,
and commit increments the historical distinct count only for a new family.
Guard history 3 was frozen before modeling and never adjusted from observations;
primary history becomes 4/5. New ROW suffixes preserve NULL and present-key ties.
A NULL member remains stored, distinct from absent membership or inactive zeros.

Six images (original/single/batch per header) have exact fifteen-byte DATA rows,
new slot starts 4036/4021, DATA free 4014/3997, prefix-zero primary active 36/45 bytes
and Guard active 28/37 bytes. Only row/count/free/new-slot/new-payload, historical
primary count and leaf bitmap/active spans change. Old masks/inactive data,
map roles, aliases/descriptors, unused tails and EOF remain original.

Observed source results: the pure constructor and all 36 independent SDK reads
passed first attempt: six complete inventories, six metadata reports and 24
named alias cursors. Saved-only verification checks every exact report, argv,
exit, empty stderr and immutable source/code/classes/JAR. The native gate passed
one test covering all six sequential/constraint/index/Source views. Root's only
pre-runtime gate adaptation replaced an equivalent inactive-payload match with
unwrap_or(0) for lint; frozen private source and all expected bytes remain exact.
No new dependency or failed source/golden retry was involved.

These source-phase runs called no native producer. Corresponding normal INSERT
implementation/proof/host results pass in the separate
[write record](results/ace16-normal-indexed-desc-null-insert-writes-2026-11-01.json). SDK read success and primary-source history interpretation
are not Microsoft Access save/reopen conformance. Allocation, interior-tree
mutation, AutoNumber/default generation and larger/external LVAL remain separate.

The maintained pure create_expected tool accepts both exact pinned originals,
both complete original reports and a fresh output directory; it writes the
six images, four literal proofs, 36 report predictions and exact source manifest.
It invokes no SDK, native engine, network or subprocess. Fixed historical
input/code authority hashes in that manifest preserve the original source-phase
provenance and byte-exact replay; root records maintained tool pins separately.
The model/report ASTs are unchanged from the qualified private source owners.
The maintained replay now reproduces all 47 historical source artifacts,
including the original manifest, byte-for-byte without SDK or native calls.
