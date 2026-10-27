# Combined variable DATA and encoded-index candidate observations

All six cases on the two unchanged maximum-width originals passed their first
combined-consumer replay: six complete image matches followed by six fresh
read-only inspections.
The committed consumer is a separate non-journalable engineering preview. These
results do not establish Microsoft Access save/reopen, normal writes, native SQL
UPDATE timing or full Access behavioral parity.

The [implementation contract](ACE16_VARIABLE_INDEX_SPLITS.md),
[qualified sources](VARIABLE_ENCODED_SPLIT_SOURCE_OBSERVATIONS.md),
[old-variable baseline](VARIABLE_ENCODED_SPLIT_BASELINES.md) and
[public-writer references](VARIABLE_ENCODED_SPLIT_UPDATE_OBSERVATIONS.md) retain
separate evidence. No original, public history, request, expected image or budget
was changed to obtain these candidate results.

## First actual replay

Session 29355 exited zero on its first attempt. Exactly twelve fixture commands
ran: six native candidates and six subsequent inert `VariableEncodedProbe`
inspections. There was no compilation, generation, public UPDATE, retry or
candidate-derived replacement input. All 76 before/after input pins matched;
112 created artifact identities and every output were retained. There were zero
unmet cases, input errors or artifact inventory errors.

The frozen executable was compiled from `abc9fd370f71a692a2b17774c7cba04df54e5c5d`.
Its [committed build receipt](results/ace16-variable-split-example-build-2026-10-27.json)
binds the exact executable, 285 selected compiled Git objects and a separate
4,194-file committed export manifest. Selected source pins are not an exhaustive
compiler/toolchain/dependency closure. The acceptance gate checked the committed
receipt Git object and all 285 selected source objects before any fixture call.
Later working-tree changes are not compiled input authority for this run.

For V2016 and V2019, ID 1 at original ordinal zero changes only `KeyValue`:
`aaa` becomes 255 `z` characters in TextLegacy/TextModern; one `00` byte becomes
255 `ff` bytes in BinaryOne. Each case starts from its original source.

| Profile | Target | New DATA page | Hidden record bytes | Candidate bytes |
| --- | --- | ---: | ---: | ---: |
| V2016 | TextLegacy | 149 | 547 | 638,976 |
| V2016 | TextModern | 149 | 547 | 638,976 |
| V2016 | BinaryOne | 149 | 297 | 638,976 |
| V2019 | TextLegacy | 167 | 547 | 712,704 |
| V2019 | TextModern | 167 | 547 | 712,704 |
| V2019 | BinaryOne | 167 | 297 | 712,704 |

Each candidate grows by 28,672 bytes: one DATA page and six index leaves. There
is no bitmap metadata allocation, map promotion or arbitrary in-file free-page
reuse in these six cases. Separate literal engineering controls exercise
promotion and late promotion; they do not turn this original-file run into
promotion evidence.

## Independent image and storage proof

The six image hashes were frozen before the combined consumer existed. The
[source-to-image model](variable_index_split_originals/candidate_model.py) uses
original raw records, metadata/getter facts, independent finite key weights and
native logical ROW references. It reads neither candidate output nor public
writer scratch to construct expected bytes.

The [record model](variable_index_split_originals/record_model.py) derives the
replacement from the original physical variable offsets, footer and presence
mask. Its capacity formula retains old padding/capacity before deciding growth.
The actual source records have 43 bytes, one variable slot and masks of one or
two bytes. Text's fixed prefix remains 30 bytes; Binary's remains 34. Fixed
ID/Marker/GUID/BYTE bytes, all other fields and footer dimensions remain exact.
The old logical root keeps its original range, slot and index ROW identity. Its
word gains relocation and its body becomes a four-byte target pointer plus 39
zero bytes. The new hidden slot is zero on a fresh selected-owner DATA page;
its word has `0x8000`. No original root space or abandoned storage is reclaimed.

The original highest hinted page has 32 free bytes and genuinely cannot fit the
new record plus slot. Its hint clears; ordinary-owned and candidate maps gain
only the new DATA page. Index-owned maps gain only their respective index pages.
Global free clears exactly all seven suffix pages. Existing inline/indirect map
representations, row ranges and bitmap pointers/storage remain unchanged.
Source-derived edits sharing a global byte are aggregated before comparison.
Every suffix page has exactly one independently derived DATA or INDEX purpose.

Each affected physical index retains its original root, grows to height one and
contains three leaves. Ascending/Twin remain one physical index. The model
reconstructs the complete sorted key/ROW stream, maxima including ROW ties,
child references, reciprocal links and root tail. Fresh pages are zero outside
encoded entries; original index-root suffix residue remains exact. All other
original bytes are preserved outside the bounded root/map changes.

Both directions are checked for General Legacy, General Modern and Binary. The
[finite key oracle](variable_index_split_originals/source_weights.py) matches the
original getter's whole-entry encoder, including Modern255 digest tails `c1ae`
and `ce3d`, separately corroborated by persisted public references. Engineering
compression uses complete key/ROW prefixes. For example, old-only 450-entry
Modern DESC leaves use prefix 12 and 912 active bytes, whereas the public writer
used key-only prefix 10 and 1,810 bytes. Public unused index scratch is not the
engineering golden. The mixed Modern DESC quarter independently needs prefix
two: 3,650 raw bytes reduce to 3,202, while Modern ASC fits prefix zero at 3,426.
The maximum payload was never reduced to avoid that condition.

The native candidate's entire bytes, length and predeclared SHA were checked
BEFORE its fresh JVM inspection. After inspection the hash was checked again.
The closed selected-table parser and separate protection proof then checked
16,206 after-row instances, 64,806 logical index entries including aliases,
3,883,008 original-prefix bytes, hidden DATA records, unchanged sentinel Memo/OLE
storage, counters, map roles and all unrelated payloads/indexes. Across six model
images, 60 physical trees and 16,206 original-record instances were checked.

## Reproducibility and limits

The [compact receipt](results/ace16-variable-index-split-originals-2026-10-27.json)
pins the authoritative raw report, first terminal/log, executed private model,
plan, controller, binding, source/getter/helper inputs, output hashes and public
tools. Public model/record/key functions have the same ASTs as their executed
private versions; only repository-root/import composition changes. The existing
source parser and protection functions likewise match their executed versions.
The historical implementation receipt remains its original snapshot.

The maintained [saved verifier](variable_index_split_originals/verify_candidates.py)
recomputes complete expected images before parsing cached getters. Its first
saved-only run passed: 348 live pin occurrences, all 285 compiled Git objects,
one receipt Git object and all six saved image/getter proofs. Git checks use
286 read-only metadata child processes; there are zero native/JVM fixture calls
and no database writes. Required ignored artifacts must already exist; the tool
does not regenerate missing evidence.

```sh
PYTHONPATH=docs/testing python3 -B docs/testing/variable_index_split_originals/verify_candidates.py
```

Native calls retain the example's shared 256 MiB/100,000-row/100,000,000-step
budget and one headerless five-field TSV assignment. Request caps remain 16 MiB,
4,096 rows and 1,024 encoded value bytes. Fixture commands have 30-second limits,
16 MiB live stream ceilings and an 8 MiB database bound; the inert JVM has a
512 MiB heap.

Before execution, the pure model preparer had an authored source-report lookup
error (`unchanged` versus actual `matches`), preserved before correction. Unbound
controller refinements separately tightened future path qualification and final
output rechecking. They changed no source/request/model image bytes and preceded
all fixture calls. The actual twelve-command run had no failure or retry; the
first maintained saved verifier also passed without a correction.

These six cases cover non-NULL, nonunique, single-column maximum replacements
with aliases in both directions. NULL/empty transitions, uniqueness, multiple
requests, already-forwarded histories, tuples, other collations, macros, general
allocation/reclamation and normal saves retain their separate contracts and
qualification gaps. Literal NULL/empty engineering tests are not nullable
original-file observations. No native Access comparison was performed here.
