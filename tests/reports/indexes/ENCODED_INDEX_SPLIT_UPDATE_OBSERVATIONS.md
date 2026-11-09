# General Short Text and Binary public split updates

Six fresh-copy public-writer replacements now demonstrate real repeated index
splits on the unchanged [qualified sources](ENCODED_INDEX_SPLIT_SOURCE_OBSERVATIONS.md).
All six updates and six subsequent inert getters passed on the first attempt.
These independent-writer observations are separate from the
[old-slot engine baseline](ENCODED_INDEX_SPLIT_BASELINES.md), which admits noops
and refuses the same growth requests. They do not establish a new engine split
candidate or Microsoft Access native save behavior.

## Exact actions and measured topology

For each V2016/V2019 original, ID 1 changes only `KeyValue`: TextLegacy and
TextModern use `aaa` to `zzz`; BinaryOne uses `00` to `ff`. Actual stored spans
remain six Text bytes or one Binary byte. Each action starts from its unchanged
original, not an earlier output. The accepted helper/classes and JAR were reused
without compilation or source generation.

Every action adds exactly six EOF index pages, grows both affected roots from
height zero to one while retaining their IDs, and leaves three children per
root. Ascending/Twin continue sharing one physical index. Each target-table logical index
retains 900 entries and every original physical ROW reference.

| Source | Target | Retained ASC/DESC roots | Added EOF pages | Result bytes |
| --- | --- | --- | --- | ---: |
| V2016 | TextLegacy | 92 / 93 | 136–141 | 581,632 |
| V2016 | TextModern | 106 / 107 | 136–141 | 581,632 |
| V2016 | BinaryOne | 120 / 121 | 136–141 | 581,632 |
| V2019 | TextLegacy | 109 / 110 | 154–159 | 655,360 |
| V2019 | TextModern | 123 / 124 | 154–159 | 655,360 |
| V2019 | BinaryOne | 137 / 138 | 154–159 | 655,360 |

With `F` denoting original EOF, the ascending leaf traversal is
`F+1, F+2, F`, containing 450/225/225 entries. Descending traverses
`F+5, F+4, F+3`, containing 225/225/450. Complete child maxima include key and
ROW bytes; raw separator, per-height sibling and full cursor streams agree.
The independent proof discovers this topology from the output instead of
imposing a predicted partition.

## Prefix policy is evidence, not a candidate golden

The public writer uses these measured leaf encodings in both profiles:

| Family/direction | 450-entry old-only prefix / active bytes | Each 225-entry quarter prefix / active bytes |
| --- | --- | --- |
| Legacy ASC | 6 / 1,806 | 0 / 2,250 |
| Legacy DESC | 7 / 1,807 | 0 / 2,475 |
| Modern ASC | 9 / 1,809 | 0 / 2,925 |
| Modern DESC | 10 / 1,810 | 0 / 3,150 |
| Binary ASC/DESC | 10 / 1,810 | 0 / 3,150 |

The 450-entry prefix lengths count encoded KEY bytes. Every fresh 225-entry
quarter uses prefix zero when its raw entries fit. Existing engineering packing
may use a longer legal common prefix including ROW bytes when needed. These
public images must not be substituted for that declared packing policy or used
to adjust independent candidate byte goldens. Normal Access/NLS equivalence and
future candidate/native byte parity remain separate questions.

## Preservation and execution boundaries

The after getter and raw model confirm the exact replacement and full index
membership. A separate protected-byte proof preserves every original byte
outside the selected slot, original affected index pages and exact derived map
bit changes. All other record bodies, masks, footers, fixed ID/Marker fields,
physical references, primary/sibling indexes, ordinary/LVAL maps, sentinel
payloads and table definitions/counters remain unchanged. Global clears equal
the six new index pages; no bitmap metadata, data page, relocation or map-window
growth occurs. Shared bitmap-byte effects are aggregated before comparison.

The fixed twelve-command phase ran once with 512 MiB JVM heaps, 30-second child
deadlines, live 16 MiB stream caps and 8 MiB database ceilings. All 49 frozen
input/freeze checks passed; both originals remained unchanged. Every fresh output,
raw getter and proof was retained. There were no native preview calls, retries,
payload/count/selector changes, new compilation or regeneration.

The earlier failed javac and corrected five-command source phase remain
historical source evidence. The twelve old-slot controls remain a distinct
native-engine baseline. This phase comprises exactly six public UPDATE calls
and six read-only getters.

The [action model](encoded_index_split_sources/action_model.py) derives from the
exact hardened source parser by adding a closed changed-table selector. Strict
boolean, duplicate/inventory, declared type/length, fixed/variable footer and raw
graph checks are retained. The
[separate byte proof](encoded_index_split_sources/reference_proof.py) does not
take planner-supplied patch authority. Public wrappers adjust root-path
composition/terminal whitespace only; function ASTs match the executed files.

The [later saved verifier](encoded_index_split_sources/verify_references.py)
rehashes 86 saved identities and recomputes all six proofs without starting any
child process. It compares 16,206 after-getter rows and 64,806 logical index
entries including aliases, and checks 3,563,520 original-prefix bytes across
the six copies. The [compact receipt](results/encoded-index-split-update-observations-2026-10-27.json)
pins the authoritative raw report, immutable sources, exact executed inputs and
these maintained tools.

```sh
PYTHONPATH=docs/testing python3 -B docs/testing/encoded_index_split_sources/verify_references.py
```

Only these non-NULL, ordinary nonunique single-column cases are observed here.
NULL/empty semantics, unique overflow, compound splits, other collations,
macros, application workflows and normal native save compatibility are not
qualified by this result.
