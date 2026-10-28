# Missing Memo/OLE declarations in INSERT previews

The ACE 16 INSERT engineering preview can append a missing ordinary Memo/OLE
column-map declaration to a qualified single-page table definition. It preserves
existing declarations, creates fresh column map containers and places the new
external payloads on the shared append cursor. This extends
[explicit zero-pair initialization](ACE16_LONG_VALUE_INITIALIZATION.md) without
treating an absent declaration as an existing zero pair. Normal backend writes,
journalable ACE saves and native Access qualification remain separate gates.

## Exact source tail and eligibility

The source definition must have a complete parsed field/index/name layout and
column-map list ending in exactly two `FF` bytes at its declared logical end.
Its continuation pointer must be zero, and the original free word must equal
`max(4096 − declared_length − 8, 0)`. A stray `FFFF` earlier in a larger declared
extent, a truncated trailer or an inconsistent header does not authorize growth.
The two-byte trailer and free-word convention follow the pinned Jackcess writer
source; this source contract does not itself establish native Access behavior.

A missing declaration has its own source fact and no invented pointer offset or
zero references. Only missing columns selected for external new storage require
initialization eligibility: a complete immutable logical-row visit must show
that their original values are NULL or inline, including present empty values.
Existing single or chained external descriptors refuse. Missing declarations do
not erase legacy external references or grant permission to adopt shared pages.
Other catalog long-value references retain their existing tracing and protection.
Inline-only requests do not newly require unrelated missing columns to prove
empty ownership and do not extend the definition.

All existing declarations remain captured, including historical columns. Source
zero pairs retain their separate qualification and exact eight-byte patch
contract. A source containing both states cannot use one binding kind as authority
for the other.

## Definition growth and shared allocation

Each newly needed missing declaration adds ten bytes before the retained two-byte
trailer: the native column number and its two packed map references. New pairs
follow actual variable-ordinal order. Existing pairs, index aliases, names and
field metadata retain their original bytes and order.

The producer preflights the complete new logical length before allocating
containers. The new definition must still fit within its existing physical page:
length 4,086 can grow to 4,096; a 4,088-byte source requiring ten more bytes
refuses. Capacity fixtures reach these boundaries through real logical index
aliases and bounded names, rather than unparsed padding or relabeled counts.
The 27-index boundary fixture stays within the 32-index maximum, and its names
remain within 64 characters. See [Microsoft's Access specifications](https://support.microsoft.com/en-us/access/access-specifications).

Composition has three disjoint exact-before patches: the free word, the declared
length and the tail window containing the new pairs and trailer. The tail's
before-image includes every consumed spare byte. Those bytes may be nonzero;
their exact original identity, not an assumption of zero-filled slack, authorizes
the replacement. Every byte beyond the new logical end remains unchanged. This
avoids a full-definition-page patch overlapping ordinary row or index counters.

All fresh map containers precede LVAL bodies, whether their bindings replace a
zero pair or append a missing declaration. Containers follow global variable
ordinals; bodies retain request, variable-ordinal and chunk order. Each container
retains the closed definition/column purpose and two 69-byte map rows described
by the initialization contract. A mixed zero/missing operation extends the tail
only for missing entries and patches existing zero pointers separately. Inline-only
and empty operations allocate no container and retain the original TD extent.

## Independent proof and verification

The audit derives missing-column eligibility from immutable source metadata and
full requests. It reconstructs the two-byte trailer, header words, new pair order,
container references and exact consumed-spare window independently of producer
patches. It compares typed zero/missing bindings, both qualification lists, all
metadata purposes and complete candidate bytes. Correlated pointer swaps or
semantically similar map memberships cannot replace the required source binding.

Seventeen new functions cover six read-only source/expected-image checks, six
public API checks, one ordering function containing two-missing and mixed
zero/missing cases, and four independent audit-forgery controls. Coverage includes
both ACE 16 headers, OLE, raw/compressed Memo, single/chained bodies, valid sibling
declarations, exact capacity, nonzero spare preservation, malformed headers and
trailers, legacy external refusal, resource exhaustion and terminal cancellation.

The first read-only gate batch passed five checks and failed one literal model
constructor that overwrote original ordinary/global map bytes. The source-only
model correction was preserved, and all six gates then passed before producer
execution. The first combined runtime passed 227 functions and failed two: an
ordering assertion incorrectly imposed container order on source qualification
facts, and unsupported TD-tail geometry escaped as an inspection error rather
than the existing `Unsupported(RowLayout)` admission refusal. The assertion now
checks membership; only `InspectError::Unsupported` is normalized at that domain
boundary. Structural, I/O, cancellation and resource errors retain their classes.
All 229 INSERT functions pass after these corrections, including the added mixed
binding case; no producer output supplied expected image bytes.

The full ACE library passes 565 functions with one existing ignored fixture
generator. Thirty-six shared long-value UPDATE/map/compression checks, strict
Clippy across all ACE targets/features, minimal compilation, formatting and
documentation/dependency guards pass. These literal sources and byte proofs do not qualify original Access
files, Windows execution or native save/reopen behavior.

## Remaining scope

Definition continuation allocation, legacy external-page adoption or migration,
map-container slot expansion and global map growth remain separate work. The
[long-value INSERT](ACE16_LONG_VALUE_INSERTS.md) and
[engineering preview safety](ACE16_WRITE_PREVIEWS.md) boundaries continue to apply.
