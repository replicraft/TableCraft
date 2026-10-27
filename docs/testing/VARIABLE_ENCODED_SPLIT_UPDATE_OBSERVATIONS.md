# Combined variable DATA and encoded-index public updates

Six fresh-copy public-writer replacements passed on their first attempt, with
six subsequent inert inspections and complete record/index/map proofs. They use
the unchanged [qualified maximum-width sources](VARIABLE_ENCODED_SPLIT_SOURCE_OBSERVATIONS.md).
These observations are separate from the
[old-variable baseline](VARIABLE_ENCODED_SPLIT_BASELINES.md), which admits no-ops
and refuses the growth requests. They do not qualify a combined engineering
candidate, normal Access save or Microsoft Access behavioral parity.

## Actual actions and placement

For each V2016/V2019 original, ID 1 changes only `KeyValue`: `aaa` to `z` repeated
255 times in TextLegacy/TextModern, or `00` to 255 `ff` bytes in BinaryOne. Every
action starts from the same unchanged original, never an earlier output. The
accepted helper/classes and reviewed Jackcess 5.0.4 JAR are reused without javac,
creation, source regeneration or native preview calls.

Every action adds seven EOF pages: one ordinary DATA page first, followed by
six index leaves. This order was discovered from actual ownership/signatures;
the proof did not require DATA-first placement. Both affected index roots retain
their page IDs, grow from height zero to one, and acquire three children each.
Ascending/Twin remain one physical index. Every target-table logical index keeps
900 entries with the original logical ROW references; the sentinel remains one.

| Profile | Target | Retained ASC/DESC roots | New DATA | New index pages | Output bytes |
| --- | --- | --- | ---: | --- | ---: |
| V2016 | TextLegacy | 92 / 93 | 149 | 150–155 | 638,976 |
| V2016 | TextModern | 110 / 111 | 149 | 150–155 | 638,976 |
| V2016 | BinaryOne | 128 / 129 | 149 | 150–155 | 638,976 |
| V2019 | TextLegacy | 109 / 110 | 167 | 168–173 | 712,704 |
| V2019 | TextModern | 127 / 128 | 167 | 168–173 | 712,704 |
| V2019 | BinaryOne | 145 / 146 | 167 | 168–173 | 712,704 |

The old logical root retains its 43-byte allocation and slot. Its word gains
`0x4000`; the body becomes a checked four-byte hidden-target pointer followed by
39 zero bytes. No root capacity is reclaimed. Actual LOCATION records distinguish
that root from new hidden slot zero, whose word has `0x8000`. Text records grow
to 547 bytes and Binary to 297, retaining their exact fixed GUID/BYTE/ID/Marker
prefixes, footer dimensions and presence masks. The fresh DATA page has count
one and free bytes 3,533 for Text or 3,783 for Binary.

All ten original DATA pages per target keep their 90-slot geometry and 32 free
bytes. The highest hinted old candidate cannot fit the new record plus its slot;
its hint is cleared. Ordinary ownership and candidate membership gain only the
new DATA page. The two affected physical index-owned maps gain only their new
leaves, and global free membership clears exactly all seven new pages. Effects
sharing an original bitmap byte are aggregated before comparison. Existing map
row ranges, representation kinds and bitmap pointers/storage remain unchanged.
No map growth or arbitrary old free-page reuse occurs.

## Persisted keys and measured packing

All new persisted key entries match the unchanged source getter's independent
whole-entry `createEntryBytes` and the finite model. This includes Modern's
510-byte truncated keys with ASC digest tail `c1ae` and DESC `ce3d`. The prior
source-only encoder prediction is now separate from actual changed-key storage.
Full native cursor streams, raw key/ROW separators and per-height sibling chains
agree with all 900 logical identities and the retained aliases.

Let `F` be the original EOF. Ascending leaves traverse `F+2,F+3,F+1`, with
450/225/225 entries. Descending traverses `F+6,F+5,F+4`, with 225/225/450.
Each family has one old-only 450-entry leaf, one old-only 225-entry leaf and one
225-entry leaf containing the new key. Both profiles have these actual encodings:

| Family/direction | Old-only 450 prefix / active bytes | Old-only 225 prefix / active bytes | Mixed 225 prefix / active bytes |
| --- | --- | --- | --- |
| Legacy ASC | 6 / 1,806 | 0 / 2,250 | 0 / 2,502 |
| Legacy DESC | 7 / 1,807 | 0 / 2,475 | 0 / 2,727 |
| Modern ASC | 9 / 1,809 | 0 / 2,925 | 0 / 3,426 |
| Modern DESC | 10 / 1,810 | 0 / 3,150 | 2 / 3,202 |
| Binary ASC/DESC | 10 / 1,810 | 0 / 3,150 | 0 / 3,429 |

Modern DESC therefore does not inherit the preceding same-width result's blanket
fresh-prefix-zero pattern. Its raw mixed quarter would require 3,650 bytes,
above 3,616 capacity; common prefix two reduces it to 3,202. The source and action
proofs never reduced the maximum value to avoid this boundary.

These are public-writer packing observations, using KEY prefix bytes with ROW
bytes retained per entry. Engineering full-key/ROW prefix choices and preserved
index scratch remain a separate image contract. The actual DATA-first order,
leaf partition and unused index scratch are not normative candidate goldens.
The new DATA page, in contrast, is checked against a complete independent
zero-initialized header/slot/record construction supported by the reviewed
buffer-allocation source. No Microsoft Access writer behavior follows from it.

## Preservation and execution history

The separate byte proof preserves every original byte outside the exact root
stub/word, original affected index pages and derived map cells. It checks all
unselected logical/raw rows, every fixed companion, ordinary page headers and
slots, primary/sibling indexes, table definitions/counters, all other map roles,
Memo/OLE sentinel payloads and LVAL storage. Every appended page is independently
classified as the selected DATA target or an affected index node/leaf; there are
no unclassified suffix pages or metadata allocations.

Session 49157 completed all twelve fixture commands once. Each used a 512 MiB
heap, 30-second deadline, 8 MiB writable-file ceiling and live 16 MiB limit per
stream. All 47 frozen input/freeze checks and 37 created artifact pins remained
unchanged. The source5 pre-execution prose correction and five successful source
commands remain historical; the twelve old-variable native controls remain a
distinct baseline. Neither is counted among these twelve public reference calls.

During preparation, a static assertion used the wrong class-name argv index.
Its unexecuted source/freeze were archived, and only that static index check was
corrected before authorization. The actual reference helper, model, proof,
selectors, values and argv did not change in response to an executed action.
There was no fixture retry or repaired database output.

The maintained [action model](variable_encoded_split_sources/action_model.py)
and [separate proof](variable_encoded_split_sources/reference_proof.py) match all
executed function ASTs; only repository-root composition changes for publication.
The model retains strict duplicate, boolean, inventory, field/footer, LOCATION,
raw tree and map checks. It does not grant full-catalog mutation authority from
the source's 26 user/global map roles.

The [saved verifier](variable_encoded_split_sources/verify_references.py)
rehashes 84 saved identities and repeats all six proofs, comparing 16,206 after
rows, 64,806 logical index entries including aliases, and 3,883,008 original-prefix
bytes across the copies. Its first saved-only run had a copied source-path suffix
and failed before model/proof replay; the exact source and diagnostic were
archived. Correcting only that suffix did not rerun any fixture command.
The [compact receipt](results/variable-encoded-split-update-observations-2026-10-27.json)
pins the authoritative raw report, before/after identities and maintained tools.

```sh
PYTHONPATH=docs/testing python3 -B docs/testing/variable_encoded_split_sources/verify_references.py
```

Only these non-NULL, nonunique, single-column General/Binary maximum-width
replacements are observed. NULL/empty, unique key overflow, tuples, other
collations, data macros, journaling, general saves and application/native Access
qualification remain separate. No engineering candidate was executed here.
