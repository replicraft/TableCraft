# ACE 16 cross-page indexed INSERT source qualification

Two independently SDK-created originals now have complete literal INSERT AFTERs
that exercise request-to-ROW association across existing DATA pages. Source-only
construction and all 24 exact independent SDK observations passed on the first
attempt; saved verification passed without launching children. All four native
source views also pass the independent complete-image and reader gate. Matching
producer and retained-host checks now pass separately; see the
[write record](../../results/indexes/ace16-indexed-write-followons-2026-11-01.json).

The [original qualification](ACE16_INDEXED_INSERT_ASSOCIATION_ORIGINALS.md) records
source creation, the explicit SDK candidate-hint restoration and native original
admission. This tranche preserves those originals. The [literal requests](../../access/indexes/indexed_insert_association/requests.json)
and observed input closure were frozen before the [pure byte model](../../access/indexes/indexed_insert_association/model.py).
The model imports no native mutation code, serializer, decoder, reservation,
staged key array, earlier expected output or SDK writer.

Each original has nine complete rows: eight on earlier owned DATA page A
(free218) and one on later B (free3599), with both pages explicit candidates.
The ordered requests are Id10/Score17/Guard7/Label200L and then
Id11/Score18/Guard7/Label"new". The first complete421B row needs423B including
its slot and cannot fit A; it selects B:1 at3194..3615. The later27B row needs29B
and selects A:8 at221..248. Final B/A free counts are3176/189. No page is
allocated and neither map membership nor candidate hint changes.

Request order is therefore B:1 then A:8, while physical scan order is A:8 then
B:1. The complete sequential AFTER order is Id1..8,11,9,10. PrimaryID and
PrimaryAlias order Id1..11; GroupOrder and GroupAlias order
Id1,4,7,2,5,8,3,6,9,11,10. The two Guard7 rows use actual ROW suffix order,
placing Id11 before Id10. Recovering request order by sorting new physical slots
would bind the wrong primary keys to these rows.

The independent model rebuilds each complete raw record/footer/mask and both
retained prefix0 leaves from literal original/request cells and actual ROWs.
It advances TD count9→11 and physical histories9→11 /3→4 exactly once per
physical index, retaining all four logical aliases. Each final leaf has99 active
bytes and3517 free. It preserves every original record and all bytes outside
explicit DATA/directory/count/free, TD count/history and INDEX active/bitmap/free
ranges, including unused tails, properties, LVAL, system data, maps and EOF.

The unchanged bootstrap observer covers four complete inventories, IndexOracle
covers four complete alias metadata reports, and all four IndexRowsOracle
cursors run on each view (16 cursor calls). Original predictions must match the
retained original reports before calls. Every affected physical page/hash/slot,
TD byte and selected row changes in the prediction; every unrelated observation
remains exact. All source, request, reader-class, jar and code pins are checked
before and after calls. No new compiler or native producer was invoked.

The [source result](../../results/indexes/ace16-indexed-insert-association-sources-2026-11-01.json)
records exact original/AFTER hashes, pre-model and pre-call closures, first
construction/read receipts and saved-verifier hashes. Binary artifacts, compiled
classes and jars remain private evidence rather than maintained source files.

This qualification establishes independent reads of the authored complete
images. It does not establish Microsoft Access save/reopen behavior, splitting,
allocation, arbitrary index profiles or new NULL-key transitions. Producer tests
must independently exercise one original ordered two-row TCNA operation, reject
self-consistent journals with swapped new ROW associations, and retain complete
recovery/preservation checks. Existing host INSERT contracts expose one row per
command; any host comparison uses two honest successive saves under one lease.
