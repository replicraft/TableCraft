# Global free-map growth in INSERT previews

The separate ACE 16 INSERT engineering preview can promote the canonical inline
GLOBAL free map when allocation first reaches page 512. Ordinary DATA, INDEX,
external LVAL bodies, column-map containers and column-owned bitmaps use one
shared allocator and independently replayed page purposes. Normal backend writes,
journalable ACE saves and native Access qualification remain separate gates.
The [preview boundary](ACE16_WRITE_PREVIEWS.md) and
[long-value INSERT contract](ACE16_LONG_VALUE_INSERTS.md) continue to apply.

## Qualified source and first crossing

Growth requires the original global map at page 1, slot 0, in its owner 1 container:
an inline 69-byte record with base zero, in an original image of at most 512 pages.
A fresh catalog census must bind the complete original record to the canonical
GlobalFree role and prove rewrite exclusivity. The immutable source binding is
retained for the operation; a staged map or candidate pointer cannot grant that
permission.

Existing represented-cell behavior remains unchanged until an actual allocation
needs page 512. Earlier global cell edits retain their original source offsets and
before-images. Promotion absorbs those edits into one complete original-before
record replacement. An already occupied represented EOF bit still refuses; the
allocator cannot skip it by promoting. Inline origins already larger than 512
pages receive no new growth authority. Existing wider or indirect represented
maps keep their previous allocation paths, and UPDATE does not opt into this
INSERT policy.

## Actual allocation identities and bitmap policy

The first unrepresented purpose receives page 512 and is immediately followed by
a separately typed GLOBAL bitmap at 513. The allocator returns actual purpose
page numbers: a LVAL body at 512 can therefore be followed by its column-owned
bitmap at 514. If the body is 511 and its owned bitmap is the first unrepresented
purpose at 512, GLOBAL 513 follows that bitmap instead. Consumers do not infer
metadata locations by adding one to a body page.

The original global record becomes an indirect record pointing to GLOBAL 513.
The new bitmap is globally used and has its own closed reference/block identity.
It is never an ordinary DATA or LVAL body, an INDEX member, a column-owned bitmap
or a column-map container. All later allocations consult the shared current
global state; they do not manufacture original `MapCell` offsets for fresh storage
or consult a stale inline future cell.

The qualified initialization policy marks every page through the actual
allocation frontier used, including previously free in-file holes. Subsequent
purpose pages clear their own bits. The bitmap's own bit remains clear. Before
promotion, an untouched in-file free hole remains unchanged.

The pinned Jackcess writer calls `fillRange(dataStart, PAGE_SIZE − dataStart)`
with an exclusive endpoint. For the qualified 4,096-byte page this fills bytes
`[4..4092)` with `FF`, leaving the final four bytes zero. The producer and
independent audit preserve that observed initialization behavior, then clear the
used frontier and allocated cells. The physical payload still represents 32,736
page numbers; this fill quirk does not redefine its geometry or establish
Microsoft-reserved bytes. The retirement and fill policies are source-pinned
engineering observations, not a claim of universal Microsoft Access behavior.

Only block zero is initialized by this continuation. The existing 64 MiB image
ceiling remains unchanged and fits within that block; no resource ceiling was
raised to admit growth.

## Ordinary ownership and independent replay

INSERT can also rebase a canonical ordinary-owned inline 69-byte row when a new
DATA page falls outside its original window. It retains every original and
previously appended DATA member, aligns the minimum down to a byte boundary and
requires `maximum − base < 512`. A represented edge bit still uses the original
cell path. After a rebase, complete row rewrites retain the original before-image
and absorb prior owned-cell edits. They never invent cells from a staged base.
Fresh Metadata authority binds the original OrdinaryOwned role and record range.
Wider, indirect and UPDATE paths retain their existing behavior; ordinary-owned
bitmap promotion beyond a fitting window remains separate work.

The independent INSERT audit derives every allocation from original bytes and
full requests. Its shared append replay inserts GLOBAL metadata at the actual
crossing, reconstructs record/bitmap bytes and checks the exact closed purpose
inventory. It independently derives ordinary-owned rebasing and column/index
map effects, including metadata interleaving, before comparing the entire image.
Staged global bytes, an equal final bitmap or a plausible page count cannot
substitute for the immutable source binding.

## Verification and retained failures

Forty independent literal source/expected-image cases cover both ACE 16 headers,
EOF 511/512, an in-file free-hole control and all five allocation phases. Complete
read-only qualification passes before global preview execution. All 40 producer
outputs match their source/request-derived whole images. Five State controls
check actual IDs, closed roles, duplicate/unordered groups, reservation quota and
cancellation atomicity, disabled UPDATE growth and reuse of an existing indirect
next source. Five audit controls reject correlated record, bitmap, purpose and
source-identity forgeries. Public controls also exhaust each resource pool and
cancel at the actual terminal healthy callback.

The first test compilation exposed an explicit test-owner import and a literal
row-argument type mismatch; both were corrected without changing fixture bytes.
Before any global preview ran, independent pinned-source and saved-page review
corrected the initializer's last four bytes from `FF` to zero. Its earlier code
and read-gate results remain preserved.

The first combined runtime passed 232 functions and failed seven. The DATA 512
source exposed a missing ordinary-owned rebase in producer and replay; five
healthy forgery setups stopped at that same producer boundary. A separate
unsupported-origin control exposed an unintended error-class change. The actual
owned rebase was added and the original inspection refusal restored, without
changing source or expected images. The corrected mutation batch passes 337
functions, including all 40 image comparisons and the State/audit controls. The
full ACE library passes 580 functions with one existing ignored fixture generator;
seven shared integration targets pass 49 tests. These totals overlap and are not
summed. Strict ACE Clippy across all targets/features and minimal no-default
compilation pass. The first strict pass reported four nested-if style issues
and one test range expression; their style-only corrections changed no
runtime semantics. Formatting also passes.

## Original-file comparison

The [maintained original-file proof](ace16_insert_global_bootstrap/README.md) now
records twelve exact engineering candidates on four unchanged originals, followed
by twelve fresh combined typed-row/map inspections. V2016/V2019 and raw/compressed
Memo use fixed public Padding counts 400/382, yielding original EOF 512 without
patching or tuning. Four saved gates separately qualify canonical GLOBAL storage,
complete column maps, records, aliases, history and catalog roles before any image.

SOURCE9 capture, twelve prebuild SOURCE/request images, 24 public-reference calls,
a fresh isolated committed `7c282f43` build, native12 and fresh12 retain distinct
receipts. All runtime phases pass first attempt. Single and chained LVAL requests
put the first body at 512 and GLOBAL at 513. The 32-row inline 64+64 batch places 21
rows in original DATA and 11 at new DATA 512, with no external LVAL or new INDEX.
Public and engineering outputs match all twelve prefrozen images in this corpus.
Old free-hole membership is retired under the declared policy while every old
hole body and the opaque adjacent global-container row remain unchanged.

These literal and original-file independent-reader checks do not establish
Windows Access execution, normal editing, native save/reopen compatibility,
global block-one initialization, map-record expansion or arbitrary page reuse.
