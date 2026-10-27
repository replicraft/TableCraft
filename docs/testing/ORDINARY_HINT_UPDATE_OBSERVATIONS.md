# Ordinary hint relocation and map-promotion public updates

Twelve fresh-copy maximum-width updates and twelve complete read-only getters
passed on their first attempt with the reviewed Jackcess 5.0.4 writer. One new
helper compilation is counted separately: **25 fixture processes**, not 24.
The updates use the unchanged four [enlarged sources](ORDINARY_HINT_GROWTH_SOURCE_OBSERVATIONS.md).
These observations establish public-writer results, not Microsoft Access parity,
a native engineering candidate, general INSERT support or a normal save path.

## Sources and actual actions

Each V2016/V2019 source contains exactly 384 or 700 passive `PaddingHint` rows,
plus the original 2,701 user rows. Every padding row retains all seven raw
255-unit wide Text fields. The twelve cases independently select each of
TextLegacy, TextModern and BinaryOne from a fresh copy of each source. Only ID1
`KeyValue` changes: `aaa` to 255 `z` units, or Binary `00` to 255 `ff` bytes.
No source was regenerated, retuned or reused as another case's output.

The natural Legacy hint contains only the highest original DATA page. Modern
and Binary retain their explicitly prepared lower-plus-highest hints; the source
phase's public `UsageMap.addPageNumber` preparation is distinct from these
ordinary row UPDATE operations. The getter captures all five tables, complete
field values/properties, raw records, logical/final locations, full index
key+ROW streams, actual encoders and all 28 inventoried user/global map roles.
No padding projection or digest replaces user data.

| Profile / padding | Source EOF | Legacy after EOF / new pages | Modern after EOF / new pages | Binary after EOF / new pages |
| --- | ---: | --- | --- | --- |
| V2016 / 384 | 537 | 544 / 7 | 544 / 7 | 544 / 7 |
| V2016 / 700 | 853 | 862 / 9 | 863 / 10 | 863 / 10 |
| V2019 / 384 | 555 | 562 / 7 | 562 / 7 | 562 / 7 |
| V2019 / 700 | 871 | 880 / 9 | 881 / 10 | 881 / 10 |

Every case allocates its new hidden DATA page at the original EOF, followed by
six index leaves and any required map bitmap pages. Ordinary ownership gains
only that DATA page. Both affected physical roots keep their page IDs, grow
from height zero to one and have three leaves. Ascending/Twin remain one
physical owner; the independent Descending owner has its own map and tree.
The original logical ROWs, aliases and complete 900-entry streams are retained.

The ordinary root retains its 43-byte allocation. Its directory word gains
`0x4000`, and its body becomes a four-byte target pointer plus 39 zero bytes.
The fresh hidden slot has `0x8000` and a separately reconstructed 547-byte Text
or 297-byte Binary record. ID/Marker/GUID/BYTE companions, mask and footer stay
exact. All original DATA pages keep their source slot geometry and free words;
no parent capacity or lower-page space is reclaimed.

## Observed map transitions

The highest hinted source DATA page has only 32 free bytes and fails the growth
request. Exactly that hint is removed. A lower controlled hint, when present,
remains set even though that page also has only 32 free bytes. The writer does
not try another hinted page before allocating at EOF.

| Source class | Hint result | Affected physical index-owned maps | New bitmap pages per case |
| --- | --- | --- | ---: |
| 384, Legacy | Empty hint rebases around the new DATA page | Existing windows represent all six leaves | 0 |
| 384, Modern/Binary | Rebase preserves the lower member and adds DATA | Existing windows represent all six leaves | 0 |
| 700, Legacy | Empty hint rebases around DATA | Both owners promote to indirect | 2 |
| 700, Modern/Binary | Indirect promotion preserves the lower member | Both owners promote to indirect | 3 |

The actual 384-source hint bases are 536/112/128 for V2016
Legacy/Modern/Binary, and 552/128/144 for V2019. The 700-source Legacy bases are
848 and 864. The controlled 700-source hints instead point to a new bitmap at
854 or 872, immediately after their new DATA page. These sources do **not**
exercise index-owned inline rebasing: their index changes are represented
additions or indirect promotions.

The independent replay preserves an existing window for an already represented
addition. Only an outside-window addition aligns the minimum down to eight and
rebases when the inclusive span is strictly below 512; otherwise it promotes.
The proof checks every byte of each final 69-byte row, all original bitmap effects
and complete fresh bitmap pages. Current-window membership matters: a later
span-512 edge can remain represented after an earlier rebase.

Let `F` be the source EOF. The measured suffix allocation orders are:

- 384 cases: DATA `F`, ASC leaves `F+1..F+3`, DESC leaves `F+4..F+6`.
- 700 Legacy: DATA `F`; ASC leaf `F+1`, its bitmap `F+2`, leaves `F+3,F+4`;
  DESC leaf `F+5`, its bitmap `F+6`, leaves `F+7,F+8`.
- 700 Modern/Binary: DATA `F`, hint bitmap `F+1`; ASC leaf `F+2`, its bitmap
  `F+3`, leaves `F+4,F+5`; DESC leaf `F+6`, its bitmap `F+7`, leaves `F+8,F+9`.

All 16 new map bitmap pages are allocated globally and belong to no ordinary
owned map or index-owned map. Global free membership loses exactly the complete
100-page suffix union across the twelve copies: 12 DATA, 72 index leaves and
16 bitmap pages. Effects sharing an original byte are compared as a union.
No second map bitmap window, global-map growth or arbitrary free-page reuse is
observed.

## Persisted framing and preservation

Ascending logical leaves have 450/225/225 entries; Descending has 225/225/450.
Full encoded keys and ROW separators match the independently checked maximum
encodings, including Modern's 510-byte truncated keys. The mixed Modern DESC
quarter has prefix two and 3,202 active bytes; the corresponding ASC quarter
uses prefix zero and 3,426 bytes. The other measured key-only prefix choices
match the [earlier maximum-width references](VARIABLE_ENCODED_SPLIT_UPDATE_OBSERVATIONS.md).
Public index packing and scratch bytes remain observations, not engineering
full-key/ROW or original/fresh-residue goldens.

The separate byte proof preserves every original byte outside the exact root
word/body, original affected index pages and independently reconstructed map
rows/bitmap effects. It retains every other raw row and field, all padding DATA
pages and values, primary/unaffected index pages, table definitions, map-container
gaps and Memo/OLE payload/storage bytes. All 120 physical 12-byte counter blocks,
including their known historical count fields, and all table row counts remain
unchanged. No arbitrary observed counter bytes receive permission to change.

The 28 captured user/global roles are not a complete foreign/historical catalog
ownership census. Complete original-byte and definition preservation does not
supply unparsed semantic ownership or global database-health certification.
Native admission and whole-candidate qualification remain separate.

## Execution and saved verification

Session 2537 completed javac plus twelve fresh UPDATE commands; session 18893
then completed the twelve read-only getters/proofs. Both first attempts returned
zero. Phase B bound the successful Phase A report, generated classes and all
12 output hashes before its first call. Every child retained the unchanged
512 MiB heap, 30-second deadline, 8 MiB database ceiling and separate live
16 MiB stdout/stderr limits. Expressions and free-page reuse were disabled,
foreign keys enabled, UTC selected and linked opens refused.

No executed helper, parser, source, request, expectation or output was corrected.
The source phase's earlier integer-versus-JSON-key failure and its distinct
six-command continuation remain [source history](ORDINARY_HINT_GROWTH_SOURCE_OBSERVATIONS.md),
not failed or repeated UPDATE calls. Separate terminal postchecks verified 131
Phase A identities and 168 Phase B identities without rewriting either raw report.

The maintained [helper](ordinary_hint_growth_references/OrdinaryHintReferenceProbe.java)
and [map proof](ordinary_hint_growth_references/map_proof.py) are byte-identical
to their executed private sources. The [action model](ordinary_hint_growth_references/action_model.py)
and [reference proof](ordinary_hint_growth_references/reference_proof.py) change
only repository-root/import composition. The new
[saved verifier](ordinary_hint_growth_references/verify.py) is a maintained replay,
not the historical execution. Its first saved-only run passed: 1,418 pin
occurrences/449 identities, 38,916 after rows including 6,504 unchanged padding
rows, 336 map-role instances, 129,612 logical index entries including aliases and
34,603,008 original-prefix bytes. It starts no database/getter/native process.

```sh
python3 -B docs/testing/ordinary_hint_growth_references/verify.py
```

The [compact receipt](results/ordinary-hint-update-observations-2026-10-27.json)
pins the executed freeze, both raw reports, postchecks, source/output identities
and maintained tools. No database, getter dump or compiled binary is committed.
