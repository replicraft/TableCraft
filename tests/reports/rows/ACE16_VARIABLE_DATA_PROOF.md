# Request-derived variable DATA proof

The ACE 16 ordinary-variable engineering preview now certifies record bytes and
placement lengths from the immutable original table, selected column and request
slice. The existing public API and consumer remain unchanged. This checkpoint
adds no combined DATA-plus-index writer, journalable edit plan or normal save path.

The finalizer rechecks the original table and recaptures logical roots through the
ordinary reader's physical traversal, forwarding and exclusive-target checks.
It validates the actual selected field and complete original record geometry,
then checks each original expected value. Semantic noops are excluded before
serialization, preserving noncanonical marker-empty encodings and their entire
original record. Changed records retain their original logical ROW identity.

A separate proof encoder derives the preferred requested Text/Binary bytes.
Compressed Text requires the stored compression flag, more than two UTF16 units,
and every unit in 1..255. A two-unit tie uses raw UTF16; NUL, wide units and
surrogate pairs require raw bytes. NULL and present-empty both have zero payload,
but distinct presence bits. Strict reader decoding must reproduce the requested
Text, including the existing leading-marker refusals. The exact-slot chooser and
record builder do not supply certification bytes.

Let `L` be original physical record capacity, `E` its final stored variable
payload offset, `S` the selected span, `P` requested payload length and `F` the
original footer size. The proof derives `max(L, E - S + P + F)`: original padding
absorbs growth before allocation, while shrink, NULL and empty retain capacity.
It preserves the stored column count, stored variable-slot count, historical
opaque slots, complete mask and physical column identities. It reconstructs all
offsets and the selected mask bit; unconsumed padding stays at its original
positions. The existing smaller-count writer refusal remains in place, even
where the ordinary reader can interpret a historical record.

The resulting audit-owned records contain independently reconstructed bytes and
original root/final-record geometry. The finalizer compares exact event count,
physical request order, root identity, word, range, fitted offset and complete
record bytes. The map audit now derives fitted versus growing placement from
those records rather than staged target lengths. It independently replays the
highest hinted candidate, actual failed fit, represented EOF allocation and exact
owned/hint/global byte union. Existing page framing, protected-byte checks and
fresh catalog/table/constraint validation remain additional checks.

A private causal control first passed a healthy finalized candidate, then changed
one staged Binary byte. The previous finalizer accepted that unrequested value;
the new request-bound proof returns `Preservation`. This is a private certification
gap, not evidence of corruption emitted by a public request. Other controls keep
a semantically equal but nonpreferred Text wire readable before requiring final
refusal, and reject correlated destination lengths and root/branch forgeries.
Literal record tests cover padding absorption, actual growth, shrink, NULL/empty,
historical offsets and a selected bit in the second mask byte.

The [scoped result](results/ace16-variable-data-proof-native-2026-10-27.json)
records 33 distinct runtime functions, including eight added private functions,
and three compile-fail doctests. The runtime set comprises eleven private DATA
functions, eleven existing variable integration functions and eleven existing
consumer functions. Original inputs and expected candidate images are unchanged.
Feature/default strict, minimal and scoped formatting checks pass. Additional
source traversal, reconstruction, decoding and comparisons are paid and may
tighten exact resource/cancellation boundaries; no performance claim is made.
No original-file candidate, JVM process or public-writer action ran for this
implementation checkpoint. Native Access save compatibility is not established.
