# Retained-tree BeforeChange sources

These independently authored originals and complete expected images precede the
normal retained-tree writer. Reader success does not grant write admission or
establish Microsoft Access event execution or save compatibility.

`tablecraft_testkit::access::canonical_tree_before_change` consumes only BEFORE
from the qualified indexed source owner. Its composite macro case also retains
the exact original catalog/MR2 carrier from the earlier macro-only-target source.
No production serializer, previous candidate or observed writer output supplies
these bytes. All fields remain ordinary manual Int32 with the original three
rows `[1,10]`, `[2,20]`, `[3,30]` in `Items(Id, Score)`.

Nine profiles under each header selector 5 and 6 provide eighteen originals and
eighteen complete expectations:

| Profile | Intended result and structural coverage |
| --- | --- |
| Leaf prefix | Score 28 in an existing compressed leaf |
| Ascending node | Two retained leaves, an explicit left bound and a tail child |
| Descending node | Compressed descending keys across the retained partitions |
| Composite macro | User changes Score; macro changes the indexed Id component |
| Unique swap | Two final keys swap across the leaf boundary atomically |
| Leaf prefix loss | Final Int32 maximum shortens prefix 4→1 and fits the existing leaf |
| Node prefix loss | Final −20 moves across leaves; shortened prefix and parent separator change |
| Zero-tail node | Both children have explicit bounds, with no header tail child |
| Retained upper bound | A real request restores Old; loose bound 25/ROW1 and the entire image remain unchanged |

Leaf files contain ten pages. Node files contain twelve: table definition 7,
DATA 8, root 9 and leaves 10/11. The original seventeen map records retain their
roles; INDEX-owned map 3:16 contains exactly the appropriate original pages.
There is no modeled allocation transition. Expectations retain each leaf's entry
count while allowing keys to move between existing leaves. They preserve owner,
links, stamp, topology, counters, original metadata and unused bytes.

The native source gate checks all thirty-six views through ordinary scalar and
ordered-index reads, complete index and constraint checks, exact map/role/raw
framing and native program compilation with empty intents. It evaluates no event
row and obtains no normal mutation plan. The create-exclusive export retains all
thirty-six images and eighteen original request contexts; ordinary tests anchor
the complete image hashes, total 1,703,936 bytes and original request binding.

All 108 independent read-only SDK calls pass on their first attempt: thirty-six
complete inventories, thirty-six index metadata observations and thirty-six
named cursors. The same three observer classes are reused unchanged. Independent
raw decoding checks reconstructed prefixes, child bounds, row suffixes, complete
owned reachability and sibling order; expected stdout was frozen before calls.
The saved-only verifier passes with all source, input and class hashes unchanged.
These observers neither execute event rows nor obtain a normal write plan.

The [source execution record](../../results/indexes/ace16-tree-before-change-sources-2026-10-31.json)
separates native validation, hash anchors and independent reader observations.
Generated databases, observer classes and raw observations remain ignored local
evidence. These are source hypotheses and read evidence, not native Access
write timing, allocation, split, cascade or complete backend qualification.
