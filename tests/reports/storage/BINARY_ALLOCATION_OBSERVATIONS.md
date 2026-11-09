# Public ShortBinary allocation observations

A separate fresh-copy capture completed **34 public row actions** on both ACE 16
profiles: 26 INSERT checkpoints and eight subsequent UPDATE observations. All
107 bounded child commands passed on their first attempt, with original files,
helpers, reviewed JAR and compiled classes unchanged. No engine candidate was
called. The [compact receipt](results/ace16-binary-allocation-originals-2026-10-26.json)
pins every source/output, frozen selector, command result and exact executed
private source/archive. Full getters, maps and record bodies remain ignored.

The original [Slot producer](SlotUpdateFixture.java) supplies
`BinaryStorage(ID LONG primary, Bytes ShortBinary length 255)`. Bytes is variable,
flag 2/ext 0 and unindexed; PrimaryID is genuine required/unique. Initially its
one data page has ten rows and 3365 free bytes. Its NULL record is 13 bytes and
its existing 255-byte record is 268 bytes. That ample-space original remains a
non-witness for allocation; these new descendants establish the boundary.

## INSERT checkpoint construction

The new repository-owned [BinaryBoundaryAppend.java](BinaryBoundaryAppend.java)
uses only public builder/database row APIs on fresh copies, verifies the exact
schema and PrimaryID, disables expressions, rejects links and appends IDs
10001 through 10013. Each value is exactly 255 deterministic bytes. No original,
flag, width, hint, header or map is patched to create the shape.

| Profile | Root data / primary index | After 12 appends | After 13 appends |
| --- | --- | --- | --- |
| V2016 | 200 / 199 | 22 slots, 125 free, true hint | old hint false; alternate page 255:0, 3812 free |
| V2019 | 217 / 216 | 22 slots, 125 free, true hint | old hint false; alternate page 272:0, 3812 free |

Each of the first twelve records consumes 268 body bytes plus a two-byte slot.
The thirteenth cannot fit. Actual metadata, raw bodies, counts and map cells are
captured after every append. Existing rows/raw bodies stay unchanged, and the ID
primary order adds exactly the explicitly appended ID. These are INSERT facts;
they do not substitute for the following UPDATE calls.

## Actual UPDATE outcomes

From each after-12 and after-13 checkpoint, ID1 changes from NULL to a fixed
255-byte Binary value, then to a fixed seven-byte value on its actual growth
output. Both payloads include exact byte data; no Text conversion is involved.

| Profile / source | Stable root | Growth target | File growth |
| --- | --- | --- | ---: |
| V2016 / after 12 | 200:0 | new EOF hidden 255:0 | 4096 bytes |
| V2016 / after 13 | 200:0 | existing owned hidden 255:1 | 0 |
| V2019 / after 12 | 217:0 | new EOF hidden 272:0 | 4096 bytes |
| V2019 / after 13 | 217:0 | existing owned hidden 272:1 | 0 |

All four growth targets have 268-byte physical capacity. New EOF allocation
clears the represented future-page global-free bit, sets the owned/new-hint bits
and clears the insufficient root hint. Alternate-owned reuse changes no map
bytes. The original root page retains its 22 slots and 125 free bytes.

All four seven-byte shrinks retain the same hidden target and 268-byte physical
capacity, with no file/map changes. UPDATE counters remain 22 for the after-12
sources and 23 for the after-13 sources. All other typed rows/raw bodies, logical
index metadata/orders and every preexisting raw index-signature page byte remain
unchanged during UPDATE. The primary still references the original root row.
Padding is recorded exactly; general native-candidate/public-padding parity is
not inferred from these public writer results.

## Reproduction and scope

The promoted Java helper is byte-identical to the private executed source. The
parameterized [capture tool](capture_binary_allocation.py) is a new public wrapper,
with syntax/help verification only; it was not used for the historical capture.
Its fresh output directory and explicit inputs keep subsequent reproduction
separate from the accepted raw evidence:

```sh
python3 docs/testing/capture_binary_allocation.py \
  --facts .cache/ace16-slot-originals-next-20261026/original-facts-initial.json \
  --inventory .cache/ace16-binary-allocation-next-20261026/inventory.json \
  --jar .cache/tablecraft-fixtures/jackcess-5.0.4.jar \
  --workdir .cache/binary-allocation-fresh
```

The reviewed Jackcess 5.0.4 JAR and original producer provenance are retained in
the [Slot reference receipt](results/ace16-slot-original-references-2026-10-26.json).
No upstream code or fixture bytes are copied. The [Text cross-page evidence](CROSS_PAGE_UPDATE_OBSERVATIONS.md)
and [PAGE180 candidates](PAGE_UPDATE_OBSERVATIONS.md) keep their own immutable
scopes. These observations do not qualify an engine Binary allocation candidate,
Microsoft Access/native saves, macros, arbitrary free-page reuse or map growth.
