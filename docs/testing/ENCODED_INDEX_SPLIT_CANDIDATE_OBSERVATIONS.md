# Exact-slot Text/Binary split candidate acceptance

The first committed-consumer replay passes all six cases on the two unchanged
ACE 16 originals: General Legacy Text, General Modern Text and Short Binary in
each format. Every complete candidate image matches its independently frozen
expectation before a fresh read-only getter runs. All six getters then confirm
typed values, full key/ROW streams, index graphs and maps. This is separate
engineering candidate qualification, not normal saves or Microsoft Access
save/reopen evidence.

The [receipt](results/ace16-encoded-index-split-originals-2026-10-27.json) preserves
the actual first run and its source identities. Earlier
[source observations](ENCODED_INDEX_SPLIT_SOURCE_OBSERVATIONS.md),
[old-service admissions/refusals](ENCODED_INDEX_SPLIT_BASELINES.md)
and [public UPDATE observations](ENCODED_INDEX_SPLIT_UPDATE_OBSERVATIONS.md)
remain distinct phases.

## Frozen inputs and execution

The executable was built from committed revision
`85576d139a3c02a89d2ef5eeef2720e7d6aaa8fb` in an isolated export. Its
[build receipt](results/ace16-encoded-split-example-build-2026-10-27.json) pins
263 selected Git source objects, all 2,602 exported files and the frozen
executable. The source-object checks use that revision rather than a concurrent
working tree. This unoptimized build retains debug assertions; it is not a
performance result.

The predeclared six requests each select ID 1, original row ordinal zero, in
`KeyValue`: raw UTF16LE `aaa` → `zzz` or Binary `00` → `ff`. Actual variable
footers place the selected span after fixed ID and Marker fields: six bytes for
Text, one for Binary. No record resizes or moves. Every target contains 900
rows, separate ascending/descending physical indexes and a genuine logical alias
of the ascending tree. The ID primary index, the two unselected target tables
and the Memo/OLE sentinel remain protected.

The first actual replay executes **12 fixture commands**: six native candidates
and six fresh read-only JVM inspections. All exit zero; all six complete-image
and getter proofs pass. All 71 before/after input pins remain unchanged. No
generation, public UPDATE, retry, selector change or golden adjustment occurs.
Failed or partial outputs would have been retained; there were no unmet cases.

The consumer uses one 256 MiB/100,000-row/100-million-step budget. TSV requests
retain the 16 MiB/4,096-assignment/1,024-byte encoded-value limits. Each getter
uses the previously frozen class/JAR, a 512 MiB heap, a 30-second deadline and
16 MiB per captured stream.

## Complete expected images

The published [candidate model](encoded_index_split_originals/candidate_model.py)
is byte-identical to the executed private proof. It derives slot spans, original
physical ROW references, complete keys, canonical physical aliases, map cells and
EOF allocation from the unchanged saved sources and independent getter facts.
It uses no production serializer or candidate output as a golden.

Each candidate adds six index pages, **24,576 bytes**, with no retirement or
ordinary/LVAL allocation. Both original selected roots retain their IDs and
become height-one nodes with two complete key/ROW bounds and a right tail.
Allocation follows ascending physical owner, then descending owner; the alias
adds no duplicate allocation. With `F` the original EOF:

| Physical tree | Final leaf pages, in key order | Entry counts |
| --- | --- | --- |
| Ascending | `F+1`, `F+2`, `F` | 450, 225, 225 |
| Descending | `F+5`, `F+4`, `F+3` | 225, 225, 450 |

Fresh pages use prefix zero when raw entries fit. The uniform 450-entry leaves
require compression and use the valid complete key/ROW common prefix: Legacy
8/9, Modern 11/12 and Binary 12/12 bytes, ascending/descending respectively.
The 225-entry fresh leaves fit with prefix zero. Original root residual bytes
stay unchanged; fresh unused bytes are zero.

These are engineering framing rules. The public writer's key-only compression
and unused scratch are separate observations, even where its active topology
agrees. Its after-images are not whole-image goldens for this preview.

The model checks 16,206 original record instances across the six copies, covering
5,402 user records in the two unique originals. It decodes 60 complete physical
tree instances, including the 12 changed trees. Exact whole-image comparison
protects masks, footers, fixed siblings, counters, old LVAL bodies, unrelated
indexes and every original byte outside the selected payload, selected roots and
independently aggregated owned/global map cells.

## Saved verification and setup history

With the recorded ignored artifacts retained, run from the repository root:

```sh
python3 docs/testing/encoded_index_split_originals/verify_candidates.py \
  --report .cache/ace16-encoded-split-acceptance-first-20261027/report.json \
  --plan .cache/ace16-encoded-originals-next-20261027/plan.json
```

This later verifier first rebuilds each complete expected image, then reparses
the cached fresh getter using the explicitly selected changed table. Its first
public run passes: 326 live pin occurrences, 263 compiled Git objects and the
committed build-receipt object are checked. It issues **zero native/JVM fixture
calls**; the Git checks do spawn read-only metadata child processes.

Setup failures remain recorded before any candidate call: an initial pure
verifier compared tuple metadata with JSON lists, and an unbound runner selected
the five-process source helper instead of the twelve-process controller. Both
failed closed and were corrected with their source/log archives retained. An
unbound metadata revision replaced provisional budget labels with the actual
consumer limits. None changed original sources, requests, proof or expected
image bytes. The actual first six-case replay remains a first-attempt success.

The [implementation contract](ACE16_ENCODED_INDEX_SPLITS.md) has separate literal
coverage for NULL/empty values, maximum keys, nonroot/internal growth, aliases,
map promotion, resources and request authority. These particular originals
qualify non-NULL exact-slot root/repeated growth. They do not establish changed
non-NULL long-digest keys, affected tuple allocation, French/unknown collation,
ignore-NULL mutation, record resizing, normal backend writes, journaling or
required macro/native Access save semantics.
