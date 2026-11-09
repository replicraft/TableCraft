# Original-file recursive Int32 split acceptance

All **four original-file candidates matched** their independently frozen complete
expected images and fresh read-only getters. Session 27053 terminated with status
0 after exactly eight native/JVM fixture commands: four candidate calls and four read-only
JVM descriptions. No input, request, expected image or budget changed, and no
public UPDATE or generation was repeated. The
[compact acceptance receipt](results/ace16-recursive-index-split-originals-2026-10-27.json)
retains the exact first terminal/report/source archives and every output hash.

## Two separate measured families

| Original family | Profiles | Target rows | Root before/after | New index pages |
| --- | --- | ---: | --- | ---: |
| Root parent overflow | V2016/V2019 | 782 | Height 1 to 2, same root ID | 3 each |
| Below-root internal parent overflow | V2016/V2019 | 963 | Height 2 retained, same root ID | 2 each |

The [root-growth references](PARENT_INDEX_SPLIT_OBSERVATIONS.md) and
[below-root references](INTERNAL_PARENT_INDEX_SPLIT_OBSERVATIONS.md) remain
separate public-action observations, including their prior-service atomic
refusals. This candidate replay uses the unchanged root plan `35700dcb` and
internal plan `693ee5ec`, with distinct proof modules and original source histories.

Root-growth candidates retain roots 91/108, allocate a new LEFT leaf followed by
RIGHT then LEFT internal nodes, and store two complete bounded maxima with tail
zero at the retained root. The leaf halves contain 227 entries each; internal
halves contain 165 children each. All 782 full key/ROW references are retained.
The resulting trees have 333 index pages and 330 leaves.

Below-root candidates retain roots 91/108 at height two with three bounded
children and tail zero. They allocate a new LEFT leaf then a new LEFT internal
node, retaining original RIGHT leaf and RIGHT internal identities/forms. Leaf
halves contain 235 entries each; internal halves contain 165 children each.
All 963 full key/ROW references remain, with 499 index pages and 495 leaves.
Sibling chains at every height, complete child maxima and exact represented
owned/global map bytes agree with the original-derived models.

Whole-image equality proves only the intended four-byte LONG record change,
qualified index framing/map changes and EOF allocations occur. Original root,
RIGHT-page and predecessor residual bytes remain unchanged outside qualified
spans; fresh unused bytes are zero. All other row references, markers, historical
counters, sentinel records and unrelated storage are retained. Public scratch
rewrites are not these engineering candidates' byte goldens.

## Frozen consumer and read-only verification

The committed artifact has SHA256
`4576adabdfa9dd074c1338de69c414aeee54a14f904f7f4ac77ed85f321cd01a`,
compiled from `452f2bde94a2580bd0a64e166cc11384f1ff8a4a`.
Its [build receipt](results/ace16-parent-split-example-build-2026-10-27.json)
pins 197 historical Git objects. The executed private runner `838275e3`, static
freeze `43325bdd` and selectors `174a5596` remain untouched. Separate root and
internal whole-image models were independently reviewed before execution.

The [new read-only verifier](parent_index_split_originals/verify_candidates.py)
requires the pinned ignored report, sources, cached/fresh getter outputs and
exact executed runner/proofs. It reconstructs all four expected images, checks
complete typed rows/index/maps and rehashes 74 artifact identities plus 197
compiled Git objects without native or JVM calls:

```sh
python3 docs/testing/parent_index_split_originals/verify_candidates.py \
  --output FRESH_OUTPUT.json
```

Public command examples are portable renderings. Raw expanded arrays, exact
commands and source archives stay ignored and pinned. This is the explicit
single-Int32 engineering preview; it does not authorize normal saves, data-macro
execution, other typed split carriers or Microsoft Access compatibility claims.
