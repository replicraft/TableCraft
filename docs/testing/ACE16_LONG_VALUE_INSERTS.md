# External Memo/OLE INSERT engineering preview

The separate `NativeImage::preview_ace16_insert_rows` service now inserts ordinary
Memo/OLE payloads using existing owned LVAL pages or represented EOF space. It
constructs complete records and maintains qualified indexes through one append
state. Normal saves, journalable ACE editing and native Access save/reopen
qualification remain separate gates. The [ordinary record contract](ACE16_ORDINARY_INSERTS.md)
and [preview safety boundary](ACE16_WRITE_PREVIEWS.md) still apply.

## Complete requests before allocation

The producer validates every requested carrier, fixed field and variable payload
before reserving a page. It reuses the working long-value codec. NULL occupies
no variable payload; present empty data retains its 12-byte INLINE descriptor.
Preferred encoded payloads through 64 bytes stay inline, through 4,076 bytes use
a single external row, and larger payloads use chained rows with 4,072 payload
bytes plus a next-ROW reference per chunk. Memo retains the qualified raw/compact
Unicode policy and round-trip checks; OLE remains opaque binary.

If preferred inline values make a complete record too wide, eligible nonempty
inline values become single external rows in actual variable-ordinal order until
the record fits. NULL and present empty values never externalize. An irreducible
record still refuses. Decisions use the complete record, including fixed extent,
variable footer and mask; declaration order does not select the first payload.

## One cursor and explicit ownership

External work proceeds in request, variable-ordinal and chunk order. Existing
column scopes require complete qualified owned/candidate map pairs. Fresh source
ownership inspection covers local catalog tables, including hidden/system tables,
without opening linked targets. Shared live references, overlapping ownership,
map aliases and protected metadata cannot become allocation authority. Old live
and orphan chunks retain their bytes.

The LVAL allocator tries highest hinted pages repeatedly, clearing failed hints
until one fits or hints are exhausted. It then reserves represented EOF space.
This differs from the ordinary DATA allocator's existing single-highest-attempt
policy. External LVAL pages and their exact column/map identities have typed
reservations. Complete linked descriptors exist before ordinary DATA records are
placed; INDEX normalization then consumes that same state. Requests needing no
external LVAL retain their existing DATA-before-INDEX path.

Existing 69-byte column-owned inline maps can retain their window, rebase to an
aligned window covering all old and new members, or become indirect maps with
fresh owned bitmap pages. Existing indirect rows can fill an unused pointer for
a newly needed block. Each bitmap immediately follows the LVAL body that needs
it on the shared cursor. It is globally allocated but never becomes an owned
LVAL body, candidate LVAL page, ordinary DATA page or INDEX member. Subsequent
chunks can reuse the new bitmap without allocating it again. Candidate inline
rows can rebase after their old hints are exhausted, including an originally
empty candidate row. These transitions preserve the original map references and
record lengths; they do not initialize missing column maps.

The independent audit derives payload choices, fallback, chunk bodies/links,
descriptors, page placement and map transitions from original bytes and full
requests. It shares only immutable source-ownership inspection. It compares the
exact touched-page inventory and typed reservations, then carries its independently
derived EOF and map ledger through DATA and INDEX proof. Final whole-image
comparison preserves unrelated records, deleted slots, old LVAL bodies and slack.
Staged descriptors or planner page lists cannot authorize different data.
Complete rewritten map rows retain their original before-images while absorbing
earlier changes to the same record. Fresh bitmap bytes, column identity, owned
reference and block number must match independent replay and the closed bitmap
reservation. Neither an extra metadata page nor an unused reservation can hide
outside the visible candidate suffix. The same emptiness checks apply when no
column declares Memo/OLE or no request needs external storage.

## Current verification

A twelve-case literal corpus covers both layouts and OLE/Memo single/chained
boundaries. Its source and expected images pass complete table/column-map reads
before producer execution. A further four-case composition corpus checks two
LVAL owners, old 4,076-byte orphan rows, 400 retained live parents and a deleted
filler, followed by 52 inserted rows. Its exact suffix is LVAL20/21, DATA22 and
INDEX23/24. Table rows grow to 452; historical primary count grows 417→469 and
the existing all-NULL secondary family retains count 829.

The separate small composition control externalizes only the first physical
variable field when two 64-byte values would make a record too wide. Its final
4,073-byte record follows a fresh LVAL page and needs a fresh DATA page. Three
independent choice controls check literal widths, ordinal order, NULL/empty
handling and irreducible size. Three correlated-forgery controls reject changed
requests, links, payloads, descriptor words, ownership and page inventories.
Cumulative byte/row/work exhaustion and cancellation at the actual final healthy
callback return no candidate.

The initial external-payload milestone passed 180 table INSERT/audit functions
and 516 full ACE library functions, with one existing ignored fixture generator.
Its 36 focused existing long-value UPDATE/map/compression consumer checks also
passed. The subsequent column-map growth batch passes 194 INSERT functions and
530 full ACE library functions, with the same existing ignored generator. Strict
ACE Clippy across all targets/features and minimal compilation also pass. Three
later producer controls pass separately and retain strict Clippy and formatting.
Those literal checks alone do not establish original-file INSERT or native
Access qualification. The separate original-file evidence below exercises
existing declared maps, rather than the later map-growth/initialization scope.

## Original-file external payload evidence

Four fresh public-builder originals cross both ACE 16 layouts and raw/compressed
Memo declarations, with two long columns, two physical I32 indexes and true
logical aliases. SOURCE9 capture passed separately from the saved-only source
gate. The gate decodes all original parent records, descriptors and chunk chains,
checks both complete column-map pairs, and scans catalog-wide ownership/live
references. Public deletion of a seventh seed row leaves six live parents and
one observed orphan chunk per column. These bodies stay protected; no source
patch, selector retargeting or regeneration forces that observation.

The [maintained proof](ace16_insert_external_long_values/README.md) reconstructs
sixteen complete images from only those originals and immutable full-row requests
before producer execution. The cases cover OLE 65/4076/4077 bytes, Memo 66/4076/4078
encoded bytes, mixed owners, NULL/empty/64-byte inline data and raw/compact `ABC`.
They allocate 48 new LVAL pages and 52 chunk rows across all candidates; existing
DATA pages and both original index leaves fit. The earlier literal corpus owns
the separate LVAL→fresh DATA→INDEX composition and row-width fallback evidence.

All sixteen native candidates match those whole images on their first attempts.
Sixteen fresh combined typed-row/map inspections then pass, after every whole
image was checked before the first reader. A separate public reference phase
has 32 unique successful child calls and 16 transition proofs. Its initial two
calls preceded a Python footer-offset assertion failure; a saved-only one-offset
correction passed before the thirty unattempted calls continued. The failed
report remains unchanged and no completed call was repeated. Public after-images
were not constructor inputs; all sixteen nevertheless match the pre-existing
engineering images exactly in this corpus.

The first consumer build reused an older shared-target artifact and was declared
unqualified before any native call. Its original report, artifact and relocated
receipt remain preserved. A corrective build from the same committed `8374bad2`
source used an initially absent isolated target and recorded actual engine
compilation. The [compact build receipt](results/ace16-insert-external-long-value-isolated-example-build-2026-10-28.json)
pins the full 4,526-file Git-bound archive/manifest and unchanged source inventory,
with explicit failure/correction linkage. This is a build-freshness issue, not
a native producer failure or a changed golden. The
[result](results/ace16-insert-external-long-values-2026-10-28.json) retains all
phase records. Native Access/Windows save/reopen remains unrun.

## Original-file column-map growth

Eight additional public-builder originals establish early real LVAL ownership
before an unrelated fixed 430/700-row Padding table advances EOF. Both layouts
and raw/compressed Memo modes qualify through complete saved raw gates after
SOURCE17 capture. Actual EOF is 543/813 for V2016 and 561/831 for V2019. The lower
cohort rebases existing 69-byte owned rows to their observed aligned minima; the
upper cohort promotes each owner to a new bitmap. Source files are never patched
or regenerated to force a transition.

The [maintained growth proof](ace16_insert_long_value_map_growth/README.md) derives
sixteen whole images independently before public references or a fresh-target
committed `18bbfee7` consumer build. Single 65/66 and chained 8,145/8,146-byte requests
exercise both column owners, owned bitmap allocation/reuse and candidate rebasing.
The V2016 upper chained case links Blob 813→815→816 around bitmap 814, followed by
Memo 817→819→820 around bitmap 818. Bitmaps remain global metadata, excluded from
LVAL body/hint membership. All source references, old live/orphan bodies,
Padding/Sentinel rows and unrelated bytes retain exact preservation checks.

All 32 public reference calls and sixteen transition proofs pass on their first
attempts. All sixteen native candidates then match the prebuild whole images;
sixteen fresh combined typed-row/map calls pass after an all-image barrier. The
corpus adds 64 LVAL body pages and 16 owned bitmap pages, with no new DATA/INDEX
pages because both existing destinations fit. The public files also match the
independently frozen images exactly; this is an observed result, not constructor
authority. The [compact result](results/ace16-insert-long-value-map-growth-2026-10-28.json)
pins the distinct source/model/reference/build/native/reader phases. A tuple/list
transport comparison in the first maintained replay was corrected without
changing physical proofs, source bytes or runtime outcomes; its failure remains
retained. No native Access/Windows save/reopen has run.

These small originals do not reach the next 32,736-page bitmap block near 128 MiB.
Authentic missing-pointer growth needs separately qualified large-file budgets;
the existing literal missing-slot control remains engineering evidence. These
files also contain complete declarations and do not qualify later missing-map
initialization behavior.

## Consumer controls and remaining scope

The engineering example keeps its 1,024-byte encoded TSV value default. Explicit
`--value-bytes BYTES` accepts 1 through 16 MiB; Binary counts encoded hex digits,
so a 4,077-byte payload needs at least 8,154 bytes. Validation precedes file I/O.
The existing 16 MiB request-file ceiling, 4,096-row limit and shared operation
budget remain unchanged. Thirteen example/input/output tests pass. Strict ACE
Clippy across all targets/features, minimal compilation, formatting and
documentation/dependency guards pass. The initial audit integration compile
exposed a private helper boundary and an unused argument; both were corrected
before the passing runtime batches, without changing the source or expected
image bytes.

## Column-map growth controls

Literal source and complete expected images cover both ACE 16 headers at the
inline boundary (EOF527 versus528), owned-map promotion, a chained payload whose
bitmap interrupts consecutive LVAL page numbers, and empty existing owned and
candidate maps. A separate indirect-map control fills a zero pointer in an
already declared row: body600 is owned, while bitmap601 remains metadata only.
The source and expected images qualify before preview execution. These four
focused growth functions and three producer composition controls passed their
initial runtime batches.

Seven independent audit controls additionally exercise foreign/self bitmap
membership, wrong column/map/block/page identity, original-before and indirect
pointer tampering, hidden metadata, extra reservations, DATA-purpose substitution
and a chain pointing at its bitmap. Healthy controls first match complete literal
images. The no-Memo/OLE and no-external-request cases keep candidate bytes
unchanged while adding forbidden metadata. The first batch passed six controls;
the remaining control stopped on a test-only assumption that the first staged
page was appended page528, although visited original page16 precedes it. The test
now selects page528 by identity; all seven controls passed in the combined
194-function retry. No fixture or expected image changed, and this correction
was not a production defect. The later producer controls exhaust each resource
pool one unit before healthy completion, cancel at the actual final callback,
refuse an otherwise checked source whose global map cannot represent the next
page, and reject identical bytes under the wrong canonical map role/reference.

The later [zero-pair initialization continuation](ACE16_LONG_VALUE_INITIALIZATION.md)
can add fresh column maps after source emptiness qualification. Other external
placement still requires existing declared column-map pairs and represented
global free cells. The [missing-declaration continuation](ACE16_MISSING_LONG_VALUE_DECLARATIONS.md)
now qualifies bounded single-page tails. Wider declaration layouts, legacy external migration, global map
growth and map-record slot expansion remain unfinished. Other metadata, text-codec/BOM
cases, Memo/OLE index keys, required unavailable validation/data macros and
production saves keep their explicit refusals. Literal map-growth checks are not
original-file or Windows Access qualification; independent original-file
comparisons remain separate evidence.
