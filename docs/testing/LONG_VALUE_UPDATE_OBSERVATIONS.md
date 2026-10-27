# Memo and opaque OLE UPDATE observations

All **60 public-writer updates succeeded**, with exact requested getters and
unchanged other fields, rows, index orders and counters. The bounded run used
**183 processes**: one helper compilation, two prerequisite metadata calls and
60 update/getter/metadata triples. No operation was retried or removed.

These are independent Jackcess 5.0.4 storage observations. They do not implement
TableCraft Memo/OLE editing, establish Microsoft Access native save/reopen
compatibility or qualify data macros. The
[compact receipt](results/long-value-update-observations-2026-10-26.json)
retains exact sources, original/getter identities, commands and saved-byte proofs.

## Original inputs and actual metadata

The unchanged [registered Slot originals](SLOT_UPDATE_OBSERVATIONS.md#fresh-independent-originals)
contain the selected `Sentinel` table, ID 91. Both fields have native flag 2,
extended flags 0 and zero declared length: Memo type 12, variable slot 0; OLE type
11, variable slot 1. Memo is uncompressed. Only the ordinary ID primary is
indexed; neither long field has an index or stored field/table properties.

| Profile | Definition / data page | Memo owned pages / hinted page | OLE owned pages / hinted page |
| --- | --- | --- | --- |
| V2016 | 241 / 253 | 244–248 / 248 | 249–252 / 252 |
| V2019 | 258 / 270 | 261–265 / 265 | 266–269 / 269 |

The trailing column-map list begins at definition offset 274 and terminates at
294. Each ten-byte entry contains a logical column number and packed owned/hint
references. For V2016, Memo references are `0xf203`/`0xf204`, OLE
`0xf205`/`0xf206`; V2019 uses `0x10303`/`0x10304` and
`0x10305`/`0x10306`. Both actual format getters report 4,096-byte pages,
64-byte inline ceiling, 4,076-byte external-row ceiling and 12-byte descriptors.

Initial Memo/OLE encoded lengths are 19,600/12,288 bytes in five/four chunks.
Static reconstruction matches their independent getter hashes. Actual read-only
metadata calls matched frozen descriptors, map extents/memberships and values
before any write. Selected LVAL owned/hint maps and global free maps are inline;
the ordinary table-owned map is already indirect. This series does not qualify
indirect LVAL map allocation or map-layout transitions.

## Fixed actions and outcomes

Each profile has nine direct Memo and nine direct OLE updates, plus six fresh
history descendants per field. Every action uses a fresh copy and a one-entry
public update map; no accepted original or source parent is modified.
Expressions are disabled and linked opens throw.

Memo inputs are NULL, empty and raw UTF-16 lengths 2, 62, 64, 66, 4,076, 4,078
and 8,146 bytes using U+0100. OLE inputs are NULL, empty and lengths 1, 63, 64,
65, 4,076, 4,077 and 8,145 bytes. Histories cross inline → single → chained →
small inline → empty → NULL → inline. Boundary labels were hypotheses before
execution; actual descriptors/getters determine the recorded storage forms.

The actual boundaries agree: up to 64 bytes can be inline when ordinary row
space permits, 65-byte OLE / 66-byte Memo values use a single external row,
4,076 bytes remains single, and 4,077-byte OLE / 4,078-byte Memo values chain.
This does not infer Unicode compression, leading-FEFF authoring or broader
format behavior from the chosen raw text values.

| File growth | Cases |
| --- | ---: |
| None | 44 |
| One page | 4 |
| Two pages | 8 |
| Three pages | 4 |

There are **32 EOF column-owned page additions**, exactly matching global free
bit clears. No owned-page removal or global free addition was observed.
**Eight cases reuse an existing column-owned LVAL page** by appending rows.
Old chunks can therefore share their page with newly appended payloads.

All 444 immediate old-chunk comparisons and **592 cumulative ancestor/orphan
chunk comparisons** preserve the exact bodies. Old slot words are unchanged.
Unreachable chains and owned memberships persist through the tested histories;
shrinking or assigning NULL does not demonstrate retirement. This is evidence
for these cases, not a universal garbage-collection policy.

## Preservation and retained diagnostic

Saved-byte verification proves 96 exact map-bit edits, preserves every other
represented bit (including global future bits), and protects 94 original table
definition/index pages per case. Full getters protect all other fields, rows and
index orders; live counts and historical counters remain unchanged. Raw
captures retain forwarding frames, descriptors/reserved bytes, chunk links,
map representations, complete changed-page hashes and headers.

The first post-run byte-audit script incorrectly assumed every map was inline.
It stopped on the already-indirect table-owned map. Its exact source and failing
log remain pinned. Supporting that existing indirect bitmap grammar in the
read-only proof passed without any database/action rerun or selector/value
change. The original 60 successful action records remain untouched.

## Tools and replay boundary

[LongValueUpdateProbe.java](LongValueUpdateProbe.java) is byte-identical to the
repository-owned source actually compiled. It has explicit passive metadata and
single-field update operations, bounded literal payloads and inert settings.
No upstream implementation or binary fixture was copied.

[capture_long_value_updates.py](capture_long_value_updates.py) is a parameterized
action-phase derivative. It requires a frozen 60-case plan, completed metadata
gate, compiled getter/probe classes and a fresh work directory; it does not
silently prepare or qualify a source. Its action phase was not rerun during
promotion. The exact executed private runner remains separately pinned.

[verify_long_value_update_bytes.py](verify_long_value_update_bytes.py) rechecks
saved outputs without database calls. The public parameterized proof was run
against all 60 saved outputs and matches the accepted private proof byte for
byte. For the retained local evidence:

```sh
python3 docs/testing/verify_long_value_update_bytes.py \
  --report .cache/ace16-long-update-observations-initial-20261026/actions-report.json \
  --original-facts .cache/ace16-slot-originals-next-20261026/original-facts-initial.json \
  --output .cache/long-value-update-proof-new.json
```

A future writer needs explicit column-map provenance, LVAL ownership/global
allocation checks, bounded descriptor/chunk serialization, full constraints and
independent preservation proofs. Readable descriptors alone grant no write
authority. Indexed Memo/digest, unknown/historical maps and retirement remain
separate qualifications.
