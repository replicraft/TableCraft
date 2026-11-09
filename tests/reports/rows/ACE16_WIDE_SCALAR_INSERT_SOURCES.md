# Wider scalar INSERT source qualification

The [independent source owner](../../../crates/engineering/tablecraft-testkit/src/fixtures/access/canonical_ordinary_wide_insert.rs)
defines eight complete-row INSERT recipes from immutable canonical originals and
the independently qualified seventeen-field catalog projection. Both header codes
5 and 6 have BEFORE and literal AFTER views: 32 views in total. Repeated BEFORE
bytes remain separate named views; this count does not assert 32 distinct images.
No insertion producer or creator supplied the expected records.

Fixed cases cover Byte, Int16, Int32, Int64, Currency, Single, Double, DATE, GUID
and Boolean with exact carriers. The new 63-byte record uses literal fixed
payloads, native GUID ordering and Boolean/presence masks. Cases retain signed
zero, subnormal and nonfinite IEEE payload bits; calendar DATE observations are
kept separate from raw stored bits. Decimal cases retain native precision 28 and
scale 4, using literal 43-, 34- and 17-byte records for exact, maximum and NULL
requests. New inactive numeric bytes are zero; original rows and inactive bytes
are unchanged.

One separate BEFORE starts with 62 fixed rows and explicitly represents future
page 9 as GLOBAL FREE. Its existing DATA page has 52 free bytes; the 63-byte row
needs another directory word and therefore does not fit. The literal AFTER adds
one owner-7 DATA page, with slot start 4033 and free bytes 4017, and changes only
the three original allocation claims and TD row count outside that new page.
Other original maps, schema, catalog and system bytes stay exact.
This is a source witness, not stored allocation permission or creator authority.

The [read-only native gate](../../../crates/engines/tablecraft-ace/src/table/mutation/normal_ordinary_sources/wide_scalar.rs)
passed its one exact test on the first run, checking all 32 complete reads,
constraints and fresh normal Source captures. The separate create-new export
passed its one exact test and pinned all 32 files. Neither operation invoked an
insertion planner, original-intent encoder or creator.

Independent SDK qualification matched all 32 complete reports on the first read
run in 5.927 seconds. Expressions were disabled, links refused and every source,
prediction and reader input remained unchanged. The saved-report verifier passed
with zero child calls. The read controller performed no compilation, native
writes or retries. Root performed one bounded Java compile beforehand, which
succeeded in 0.762 seconds.

Two observer/preflight corrections are retained before the first SDK read. Static
review found that the inherited file guard still allowed only nine pages; its
successor admits exactly the qualified nine- and ten-page shapes and checks their
row/page geometry. The successful compile also emitted the legitimate enum-switch
companion class. The first class-count preflight stopped before reads, and its
successor pins exactly that companion and the main class. No source, requested
value or predicted report was retuned after an observation.

The [result record](../../results/rows/ace16-wide-scalar-insert-sources-2026-11-01.json) pins the
32 views and saved receipts. These results qualify sources and independent reads;
matching TCNA4 insertion, publication and recovery acceptance is a separate phase.
Wider indexed-key writes, multi-page plus wider-carrier composition, SQL nonfinite
execution and native Access/Windows save/reopen are not established by this phase.
