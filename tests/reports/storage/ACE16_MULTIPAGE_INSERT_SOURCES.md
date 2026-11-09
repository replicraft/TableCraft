# Source-first ordinary INSERT across two represented EOF DATA pages

Four source images passed the native read/admission gate and complete read-only
SDK qualification before any matching multi-page INSERT producer. These are
independent expected-byte and reader results. They do not enable a normal save,
qualify recovery or establish native Access save conformance.

Both ACE 16 header families use the unchanged genuine Sentinel originals from
the [one-page sources](../rows/ACE16_EOF_INSERT_SOURCES.md). Each contains manual ID 23,
a Text marker and readonly inline OLE. The request supplies 600 complete rows,
ID 24..623, with Marker and Bytes explicitly NULL. Original pins, ordered values,
placement and map laws were frozen before model code. No writer, engineering
candidate or previous producer AFTER supplies expected bytes.

Each new record is 15 bytes. The first 236 records fit original DATA slots
1..236, producing count 237/free 2. The next 240 fill original EOF page 112/129,
producing count 240/free 2. The last 124 occupy page 113/130, producing count
124/free 1974. The table has 601 rows and the file grows by exactly 8192 bytes.
This is byte-capacity exhaustion before the 255-slot ceiling.

Each new page has three already represented claims: GLOBAL free 1→0, ordinary
owned 0→1 and candidate 0→1. Six bit changes share three physical bytes: mask 03
for header 5 and mask 06 for header 6. Original candidate hints remain set.
An original-only raw catalog map census checks both future pages against ordinary,
index and LVAL ownership/candidate roles. No map storage growth, index allocation,
relationship, generator, default or new LVAL payload is modeled. The old live
66-byte record and OLE, passive properties, unrelated objects and unnamed byte
complement remain exact.

Pure construction and the two new full AFTER SDK observations passed on their
first attempts. Two unchanged original observations complete the four-view
closure. The bounded observer checks every physical page hash, DATA body/slot,
map record, bitmap storage, catalog/TD declaration, property and selected cell.
Expressions remain disabled and linked opens denied. Saved verification starts
zero children and confirms exact input/tool/class pins.

The native source gate passed one function over four views in 0.19 seconds. It
checks full rows, Source/fitted admission, NULL counts, readonly OLE, placement,
map polarity and complete byte complement. The [result record](../../results/storage/ace16-multipage-insert-sources-2026-11-01.json)
keeps these source results separate from pending multi-page producer, transaction,
recovery and host qualification. The existing one-page TCNA3 evidence is unchanged.
A source batch does not imply a new typed or SQL batch service.
