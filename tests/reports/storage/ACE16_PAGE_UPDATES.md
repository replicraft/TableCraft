# Existing-page ACE 16 update previews

With `ace16-write-preview`, `NativeImage::preview_ace16_page_updates`
accepts one table, one Short Text/Binary column and existing
`SlotScalarUpdate` requests. It returns a separate non-journalable
`Ace16WritePreview`. This engineering service does not enable normal ACE writes,
qualify data macros or establish Microsoft Access save/reopen compatibility.
The separate same-width slot service retains its existing contract.

## Storage behavior

Every request checks the expected decoded value before recognizing a semantic
no-op. No-ops preserve original bytes, including noncanonical text encodings.
Changed records rebuild the selected variable payload, offset directory and
presence bit. Other payloads, fixed gaps, unused mask bits and retained padding
remain exact. Text uses the qualified storage codec and decoder roundtrip proof;
leading-FEFF aliases refuse. Raw UTF-16 wins the two-unit compression tie.

A replacement fitting the existing final record retains its physical capacity,
slot and page free-space count. This includes already-forwarded roots and
shrinks. Growth appends a hidden last slot on the original logical root page and
repoints that root. The original root/index reference stays stable. Existing
forwarding-stub residue and older hidden targets remain exact; an ordinary root
becomes a checked four-byte pointer with its remaining former body cleared.
Multiple appends follow original row traversal order, independently of request
slice order.

Appending requires a represented true table candidate-hint bit, sufficient
actual header free bytes, a pointer-sized original root and an addressable new
slot. The qualified page maximum is 255 slots (last slot 254). A false or omitted
candidate bit does not imply lack of physical space: fitted changes and no-ops
may retain it, while new append refuses. Maps are preserved verbatim. Unknown
map layouts or references refuse; no threshold infers or changes hint membership.

There is no alternative-page selection, page allocation, compaction, relocation
of other records or split fallback. These remain subsequent storage milestones.

## Required proofs and boundaries

A complete budgeted logical-root census resolves checked forwarding paths and
requires exclusive ownership of each selected final target, including ordinary
records reached through another root. Data and map ownership/global-allocation
checks precede staging. Shared-target aliases, malformed paths, insufficient
capacity and unsupported layouts refuse atomically.

The service reuses the existing slot column/profile gates and genuine affected
single/mixed-tuple encoded key replacement. It retains tree topology, complete
before/after index membership, uniqueness, field rules and relationship checks.
French indexed text, ignored-NULL indexes and unsupported relationship carriers
remain blocked. No empty request bypasses qualification. Existing numeric and
same-width update services retain their admission rules.

Independent record and page audits limit candidate byte changes. The record
audit checks full physical prefix/suffix bytes, affine offset changes, retained
padding and every mask bit except the selected presence bit. Page audits check
exact slot/free-count changes and forwarding spans; protected map/table pages
remain unchanged. Work, allocation and cancellation checks cover the census,
serialization, key replacement and audits.

## Verification scope

The [scoped implementation record](results/ace16-page-updates-native-2026-10-26.json)
contains exact source/log hashes, commands and preserved failures. Author tests
reuse existing repository-authored encoded fixture mechanics; the separate
contract target independently authors its unindexed physical model. Four causal
private tests demonstrate that ordinary semantic checks accept forged opaque
bytes which the strengthened independent record audit rejects.

Public-builder observations informed same-page forwarding and candidate-hint
semantics. The subsequent [predeclared original-file comparison](PAGE_UPDATE_OBSERVATIONS.md)
passes all 180 controls using a separately committed frozen consumer: 156
candidates and 24 typed refusals. Its proof covers 122 fitted records, sixteen
appends and eighteen byte-identical no-ops. These later outcomes are separate
from the original scoped implementation record and native Access qualification.
