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

The independent audit derives payload choices, fallback, chunk bodies/links,
descriptors, page placement and map transitions from original bytes and full
requests. It shares only immutable source-ownership inspection. It compares the
exact touched-page inventory and typed reservations, then carries its independently
derived EOF and map ledger through DATA and INDEX proof. Final whole-image
comparison preserves unrelated records, deleted slots, old LVAL bodies and slack.
Staged descriptors or planner page lists cannot authorize different data.

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

All 180 table INSERT/audit functions pass. The full ACE library batch passes 516
functions with one existing ignored fixture generator; 36 focused existing
long-value UPDATE/map/compression consumer checks also pass. These are engineering
checks, not authentic external-file INSERT or native Access qualification.

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

Current external placement requires represented existing column-owned, candidate
and global cells. LVAL map growth/rebasing/promotion and missing-map initialization
remain unfinished. Other metadata, text-codec/BOM cases, Memo/OLE index keys,
required unavailable validation/data macros and production saves keep their
explicit refusals. Independent original-file comparisons are being prepared;
no Windows Access execution is claimed.
