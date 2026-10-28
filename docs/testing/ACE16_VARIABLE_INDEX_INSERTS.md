# Variable-key INSERT engineering preview

The separate `NativeImage::preview_ace16_insert_rows` service now maintains
qualified Short Text and Short Binary secondary indexes and their mixed fixed
tuples. General legacy/2010 Text profiles retain their exact stored descriptors;
French and other unqualified mutation orders remain refused by the write gate.
The required unique ascending Int32 primary and the ordinary record, validation,
default/generated-field and required data-macro gates remain unchanged. This
is an engineering candidate API, not a journalable or normal ACE save path.

## Complete rows and retained bytes

INSERT has its own narrow preparation contract over the existing retained-key
service. New keys retain row-derived semantics and encoded bytes together.
Original leaf keys are bound to their exact logical ROW and all selected stored
fields before their NULL facts become authoritative. Declaration-order record
decoding is remapped separately from the index's written component order.
Original key bytes must match the complete decoded row; separators retain only
comparison facts and cannot invent complete tuple NULL facts.

The recursive tree and shared DATA-first allocator transport non-Copy retained
keys without introducing UPDATE replacement semantics. Completed plans expose
owned key/ROW facts. The independent final audit derives its own component
framing, whole-entry digest and ROW membership from source bytes and requests;
it shares only the qualified primitive Text weight service. It separately checks
original primary membership, complete selected-field NULL facts, historical key
families, maps and allocation. Logical aliases share one physical owner.

Short Binary supports its existing declared bound through 510 payload bytes:
577 encoded key bytes, 581-byte leaf entries and 585-byte node entries. Text and
encoded tuples retain the 510-byte whole-key bound and 518-byte node bound.
The audit retains short wires inline and explicitly pays for wider storage,
copying, comparison, source hydration and NULL-fact lookup. Cancellation and
resource failures return no candidate.

## Constraints and boundaries

NULL, present empty Text/Binary and all-NULL tuples remain distinct. Required
rejects any selected NULL; unique exempts only all-NULL keys. IgnoreNULL omits
only all-selected-NULL keys, including when unrelated fields are NULL. Partial
NULL tuples remain indexed and participate in uniqueness. Original and new
uncertain digest keys at the whole-key ceiling remain refused for unique
indexes; a full row does not establish collision-free physical uniqueness.
Nonunique digested keys retain complete row-derived NULL facts.

New empty Text still requires an explicit typed `AllowZeroLength` declaration.
An existing empty stored value does not authorize new empty Text. External Memo/OLE and
other unqualified record/index profiles, Memo/OLE indexes, foreign indexes, general reclamation
and production saves retain their separate gates.

## Verification

Five public complete-image functions construct and read the literal expected
images before invoking the producer. They cover both layouts/directions, both
General families, single and reversed mixed tuples, NULL versus empty, nullable
uniqueness, ignored membership, aliases, historical counters, Modern digest
retention and uncertain unique-key refusal. A genuine six-to-seven-entry Binary
case grows a secondary root with a 585-byte node bound while DATA still fits.
The missing empty-Text property in the initial test fixture correctly triggered
`ConstraintCoverage`; adding an explicit source declaration fixed that fixture
without relaxing the runtime gate. A separate causal control retains the refusal.

Four internal producer functions cover original digest/wide-key retention,
hidden tuple NULL facts, factless separators, source/wire drift and paid
retention/cancellation. Two audit functions independently test segment framing,
DESC continuation tags, padding, width limits, digest tails and full-row NULL
facts. The full ACE library batch passes 491 functions with one existing ignored
fixture generator. Thirty-seven focused shared DELETE/UPDATE consumer checks
and eight French-profile checks also pass, including explicit INSERT refusal
for the unqualified French mutation family. Strict ACE Clippy across all targets
and features, minimal compilation, formatting and documentation/dependency guards
pass. The first Clippy attempt found a redundant test-only String conversion;
removing it did not change image construction or runtime behavior.
Independent original-file variable-key INSERT comparisons
and native Microsoft Access save/reopen qualification remain separate work.

## Key comparison precedes the ROW suffix

The independent INSERT audit now compares complete key bytes first and the
ROW reference only when keys tie. Sorting the concatenated key/ROW wire was
incorrect when one variable key prefixes another: descending Binary empty
values and FF-prefixed values could be valid in the native reader yet refused
by the final audit. The typed native key profile already used the correct order.

Audit wires now distinguish leaf entries from branch entries with a child
pointer. Branch comparisons retain the same key/ROW boundary; child bytes do
not become key or ROW data. Copying retains that wire kind, and constructors
refuse undersized or nested branch forms. Source bounds still strip the child
pointer before strict qualification; duplicate logical bounds gain no authority.

A two-layout whole-image control checks existing empty/FF/zero Binary values
and inserted FF255, FF9, empty and NULL values. Its independently built source
and expected image pass native reading/checks before the API call. The initial
producer call failed with `Preservation`; it now succeeds without changing those
bytes. Four independent wire functions additionally check leaf/branch ordering,
ROW versus child ties, decoder kinds, paid copies and short-wire refusal.
The fixture model also uses separate key/ROW ordering. No native storage codec
or UPDATE/DELETE ordering was changed.
