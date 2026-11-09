# Source-first ordinary INSERT with represented EOF DATA

Eight source images have passed native read/admission gates and eight complete
read-only SDK observations before any matching INSERT allocation producer. These
are independent expected-byte and reader qualifications. They do not establish
native Access save conformance or enable a normal growing INSERT consumer.
At this source stage, implementation, native recovery and host acceptance were
pending. Subsequent producer results belong to the
[normal INSERT contract](../../../docs/access/writes/NORMAL_ACE_INSERT.md#represented-eof-allocation).

Both ACE 16 header families start from the immutable
[preserved fixed-scalar originals](ACE16_PRESERVED_FIXED_SCALARS.md). The selected
unindexed Sentinel table contains one manual ID 23 row, a Text marker and readonly
inline OLE bytes. Passive properties and all unrelated objects remain unchanged.
Original requests and raw placement/map laws were frozen before model code.
Models consume original bytes and literal values, never a TableCraft writer,
engineering candidate or prior producer AFTER image.

The batch witness supplies 255 complete rows, ID 24..278, with Marker and Bytes
explicitly NULL. Each record is 15 bytes: a three-column count, I32, no variable
payloads, two variable offsets, count and mask 01. The first 236 rows fit original
DATA slots 1..236; old count/free become 237/2. The final 19 rows use one new visible
EOF DATA page, count 19/free 3759. Table row count becomes 256. This exhausts **byte
capacity before the 255-slot ceiling**; it is not a saturated-directory claim.

The separate single-row witness authors a healthy near-capacity BEFORE from the
same original plus 236 literal rows, with 237 rows/free2 and no allocation. It then
supplies only `(260,NULL,NULL)`: table 237→238, one new EOF direct record at 4081,
count 1/free 4065. It supplies a source for the actual one-row typed/SQL/CLI service;
the batch source does not imply a host batch API.

Every AFTER is exactly original length+4096. The new page is original EOF 112/129,
owned by actual Sentinel TD 107/125. Exactly three already represented cells
change: GLOBAL free 1→0, ordinary owned 0→1, candidate 0→1. Existing root candidate
hints remain set, even at free 2. No map record/storage growth, index change,
relationship, identity generation or default substitution is modeled. The old
live record and inline OLE, all other map bits, passive properties, metadata and
unrelated bytes outside explicit spans remain exact.

Both pure constructors and all SDK calls passed on their first attempts. The
unchanged bounded complete bootstrap observer reads all physical PAGE hashes,
DATA bodies/slots, catalog/TD declarations, maps/bitmap storage, table metadata,
properties and cells; expression evaluation is off and linked opens denied.
Four batch-view observations and four single-view observations matched literal
predictions. Saved comparisons started zero children and confirmed unchanged
input/class/tool pins. No new SDK compiler, data program or native writer ran.

The native source gates each passed one function over four views: batch 0.18s,
single 0.17s. They validate complete selected rows, fresh Source/fitted admission,
NULL counts, readonly OLE, properties, exact map polarity and byte complement.
The maintained gates are
[batch sources](../../../crates/engines/tablecraft-ace/src/table/mutation/normal_insert/eof_sources.rs)
and [single-row sources](../../../crates/engines/tablecraft-ace/src/table/mutation/normal_insert/single_eof_sources.rs).
Their literal pins and the [result record](../../results/rows/ace16-eof-insert-sources-2026-11-01.json)
keep source evidence separate from future producer and recovery results.
