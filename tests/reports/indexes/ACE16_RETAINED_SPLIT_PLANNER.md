# Retained-key split planning

The feature-gated split planner now borrows complete keys from an immutable,
profile-owned key arena. Its working public consumers remain the existing
[Int32](ACE16_INDEX_SPLITS.md) and [fixed numeric](ACE16_FIXED_INDEX_SPLITS.md)
engineering previews. This refactor adds no Text/Binary update API, mutation
eligibility, normal save path or native Access qualification.

The physical builder and independent replay accept the existing retained-profile
contract. Leaf ranges and internal maxima reference checked positions in the
original key arena, preserving full key plus ROW boundaries without cloning
owned keys into each topology node. Encoding still uses the actual profile;
there are no surrogate numeric keys or fabricated widths. Borrowed entry vectors,
encoding, copying and traversal retain allocation, work and cancellation charges.
`WriteProfile`, numeric request staging and original-request authority are unchanged.

Original postorder, stable allocation-order normalization, appended-left visits
in the same pass, retained-right revisits in later passes, logical sibling order,
root allocation order and original/fresh residue rules are unchanged. The audit
reconstructs topology and maxima independently and measures capacity through its
own framing arithmetic. It does not use builder arena decisions or capacity
predicates. Allocation-map algorithms are unchanged.

Three new private tests exercise real preserved General legacy/modern and Binary
profiles, including both directions, NULL/empty/tied keys, long keys, complete ROW
maxima, original root/node forms, literal fresh-page bytes and resource refusal.
Their imported authored fixtures are independently read/index checked. These are
internal physical transport tests, not new slot-update admission or original-file
candidate comparisons. The existing numeric image tests remain unchanged.

Two additional tests reproduce and close pre-existing framing certification gaps.
A fresh numeric split leaf with an unnecessary prefix of one, and a retained
fitting numeric root with an alternative prefix of seven instead of the required
eight, both passed ordinary row/index/constraint checks and were accepted by the
old finalizer. The preserved before run fails both expected-Preservation assertions
because the finalizer returned success. The corrected audit uses its own layout
measurement with fresh seed zero or the immutable original prefix respectively;
both forgeries now return Preservation. Emitted numeric images are unchanged.
The stricter checks add paid encoding and candidate reads, so identical resource
boundaries are not claimed.

The framing correction covers fresh single-key leaves and retained affected
single numeric root framing. The existing-fit non-single/composite fallback is
outside this correction and still warrants a separate prefix-authority probe.
Its private proposed discriminator has not been compiled or executed here. This
record does not claim that all opaque composite framing is independently fixed.

The [scoped result](results/ace16-retained-split-planner-native-2026-10-27.json)
records 115 distinct runtime functions, including five new functions, plus three
compile-fail doctests. Feature/default strict checks, minimal build and scoped
formatting pass. Initial lifetime and test-visibility compilation failures, the
two causal failures and exact source snapshots are retained separately. No JVM,
database generation, public reference action or native candidate process ran in
this implementation chunk. Future General Short Text/Binary allocation still
requires qualified sources, an actual request-bound consumer and its own evidence.
