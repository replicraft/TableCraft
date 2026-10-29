# Explicit zero-pair Memo/OLE map initialization

The separate ACE 16 INSERT engineering preview can initialize an existing
Memo/OLE column declaration whose owned and candidate map references are both
zero. It creates fresh map storage before placing external payloads and patches
only the original declaration's eight pointer bytes. This extends the
[long-value INSERT service](ACE16_LONG_VALUE_INSERTS.md); it does not enable
normal backend writes, journalable ACE saves or native Access qualification.

## Source eligibility

A zero pair is an explicit ten-byte native declaration: a visible ordinary
Memo/OLE column number followed by two zero references. The complete list must
parse through its genuine `FFFF` terminator. Later valid declarations, including
historical columns, retain their own map claims. Duplicate visible declarations,
truncated pairs, one-zero pairs and unknown trailing grammar do not establish
initialization authority. A missing declaration at `FFFF` is a separate state;
its terminator is never treated as a zero pair or a patch location.

The current transformation requires the selected table definition to fit one
physical page. Its immutable source proof retains the exact column identity and
absolute eight-byte pointer span. The ordinary long-value inspection API still
reports the original zero-pair list as `UnknownDeclaration`; that status alone
does not grant write permission or prove an empty column.

Initialization qualification separately visits every live logical source row,
following forwarding roots and checking actual record masks, variable offsets
and descriptors. Every source value in each qualified zero-pair column must be
NULL or inline, including present empty data. Existing single or chained external
values refuse even though an ordinary value reader may still follow them. The
service does not adopt legacy shared LVAL pages or infer ownership from absent
map references.

The catalog census keeps global, ordinary, physical-index, other column and
historical map footprints. Only the explicitly qualified zero pairs lack source
map claims; valid siblings remain captured. Aliases of mutable map records still
refuse. Other live long-value references and protected storage retain their
existing checks. Inline-only requests without a zero pair keep their previous
fast path and do not trigger a new catalog payload census.

## Allocation and preservation

Only zero-pair columns whose new values require external storage receive fresh
containers. All needed containers are allocated first, in actual variable-ordinal
order across the selected columns. Payload bodies then retain request, variable
ordinal and chunk order. Thus a first request using a later ordinal cannot move
that column's container ahead of an earlier ordinal used by another request.

Each new container is a globally allocated ordinary map-definition page with
owner zero, two visible 69-byte inline map rows, row count two and free word
3,940. Slot zero is the column-owned map and slot one is its candidate map. Their
packed references replace exactly the original two zero pointers. The container
has its own closed allocation purpose bound to the original definition and
column; it is not an ordinary DATA page, LVAL body, owned bitmap or INDEX member.

External LVAL bodies, any subsequent map growth, ordinary DATA and INDEX work
share the resulting EOF cursor. Existing containers and their unreferenced
historical rows remain byte exact. The source definition's length, names, fields,
indexes, remaining declarations and terminator remain unchanged. Inline-only
requests on a qualified zero-pair source preserve both zero references and
allocate no container. Empty requests retain the original image.

The independent audit reparses the source declaration and full requests, repeats
source qualification, derives container order and complete bytes, and reconstructs
the exact TD pointer patch. It derives fresh map contents directly in its own
expected image and never reads a map at reference zero. It compares the qualified
zero-column list, metadata inventory, typed reservations and full candidate
bytes. Staged descriptors, source-status labels or a plausible container do not
supply authority for different values or metadata.

## Initialization through later INDEX growth

After ordinary DATA placement, INSERT retains the actual immutable source metadata
census for later physical indexes. That authority is bound to the same source
buffer, byte length and table definition. Both root-leaf splitting and recursive
NODE routing use it for map exclusivity, original map-row rewrites and final
allocation checks. This avoids treating an independently qualified zero pair as
an unknown declaration during a later strict recensus. It grants no authority
from staged column lists or new map contents. Operations without retained
metadata keep the existing strict catalog capture.

A literal two-owner case starts with 400 parents, a zero Memo declaration and an
existing Blob declaration. Its 52 new rows require Memo container 20, Memo body
21, Blob body 22, DATA 23 and primary INDEX pages 24/25. The old Memo page and
unclaimed historical map rows remain byte exact. A separate NODE-root source
places the existing primary rows in two real linked leaves; its independent
source/request image places container 22, bodies 23/24 and DATA 25 while retaining
the existing index pages.

The original root-leaf regression passed its source/expected-image read gates
before the API refused with `Unsupported(RowLayout)`. The retained-census change
addresses that later recensus, without changing those images. The recursive
source and complete expected image also passed their separate read-only gate
before the API check. The first combined INSERT batch passes all 249 functions,
including the six source-authority/zero-Memo functions. Source-identity and owner
mismatches, foreign column/index map sharing, and the existing full-API
external-under-zero refusals remain explicit guard controls. The full ACE library
passes 590 functions with one existing ignored fixture generator. All 49 selected
shared integration tests, strict ACE Clippy across all targets/features, minimal
compilation, formatting and documentation/dependency guards pass. These totals
overlap; they do not establish original-file or Microsoft Access qualification
for the initialization/index combination.

## Verification

Ten literal source/request cases cover both ACE 16 headers and OLE, raw Memo and
compressed Memo, with single and chained external payloads. Their source and
complete independently constructed expected images passed read-only qualification
before the first combined producer runtime. No expected image was derived from
producer output.

Fifteen new functions cover six public/helper controls, five source-qualification
controls and four independent audit-forgery controls. They exercise exact complete
images, inline-only and empty requests, two-owner container/body ordering,
existing external-value refusal, complete declaration grammar, row-count/footer
drift, metadata aliases, forged TD pointers/container bytes/purposes and cumulative
resource or terminal-cancellation boundaries. Healthy controls precede the
forgeries; sources and expected images remain unchanged.

The first compilation stopped on a producer return-type mismatch and test code
that reused a consumed `NativeImage`. The corrections converted the existing
error type and recaptured images for separate calls; they changed no fixture or
expected bytes. The first complete runtime passed all 212 INSERT functions. The
full ACE library passed 548 functions with one existing ignored fixture generator;
36 existing long-value UPDATE, map and compression checks also passed.

Strict Clippy initially found an unused internal capture method after integration.
The initialization metadata service now dispatches an empty qualification list
through the existing strict capture method, preserving behavior. Strict checks
and formatting then passed. Minimal compilation passes, and the INSERT rerun
after that dispatch adjustment again passes all 212 functions.
The initial minimal build succeeded with three unused preview-helper warnings;
the helpers and their internal export now share the preview feature gate.
Minimal and all-feature compilation remain separate checks.

## Remaining boundaries

The later [missing-declaration continuation](ACE16_MISSING_LONG_VALUE_DECLARATIONS.md)
adds qualified single-page TD tails. Table-definition continuation growth, legacy
external-page adoption or migration and map-container slot expansion remain
separate work. The later [GLOBAL INSERT continuation](ACE16_GLOBAL_MAP_INSERT_GROWTH.md)
qualifies the canonical first page 512 crossing; wider global growth remains open. These tests qualify synthetic engineering sources
and independent byte proofs; they do not establish original Access-file behavior,
Windows Access execution or native save/reopen compatibility. The
[preview safety boundary](ACE16_WRITE_PREVIEWS.md) remains unchanged.
