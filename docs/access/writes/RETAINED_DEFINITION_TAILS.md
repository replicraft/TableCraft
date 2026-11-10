# Row writes beside retained definition bytes

Fitting field-name shrink preserves the bytes exposed beyond the new declared
TDEF length. Those bytes can contain the former name/index tail. They remain
opaque data and must survive later row edits; erasing them to satisfy a source
validator would violate preservation.

## Admission boundary

The normal row-only absence-of-programs proof can now retain a nonzero free tail
on an unencoded `Ace2016` user definition. It still proves the complete active
header, fields, native index declarations and LVAL declarations, ending at the
exact declared terminator. Existing property, constraint, program-storage,
ownership and resource checks remain. Other formats keep their existing zero-tail
requirement. Global closed-content, system definitions and schema operations do
not acquire this permission.

The final eight bytes remain zero under this qualified profile. That is a
conservative retained-profile constraint, not a claim that every Access format
has a universal trailing footer. The original raw strict validator remains
strict. Original bytes select the strict or row-only attempt before validation;
a failed strict proof is never retried as a more permissive one.

## Preservation witness

After complete active framing validation, a private `RetainedDefinitionTail`
records the actual selected definition, declared end, full source length and an
immutable shared slice through the physical page end. It copies no page payload.
The row Source retains that witness after its borrowed program certificate ends.
No caller boolean, displayed capability or serialized token can construct it.

Normal fixed/indexed/fitted updates, insertion and deletion proofs check the
actual original allocation, selected definition and declared end, then require
every captured byte to remain exact in AFTER. The checks include the retained
profile margin. Independent complete-image reconstruction remains required.
Cancellation and work charges propagate through the existing native error types.
The witness grants no schema capacity, page allocation or file-growth authority;
any existing allocation route retains its separate proof and limits.

## Composition and recovery evidence

Independently authored complete images cover `Score` growing to `TaxAmount` or
shrinking to `X`, omitted generated ID and Number insertion (ID 1, Number 0),
a real Number update to 42 and logical deletion. The deletion preserves generated
history, the tombstoned record and inactive index key bytes as well as all name,
property, OFF-setting and definition-tail bytes. Raw Python predictions and Rust
fixtures have separate complete-image SHA-256 pins.

Fresh row review must derive the witness from the opened file. An earlier rename
review supplies no row-write authority. Original TCNA0002 reconstruction and cold
pending/committed journal qualification likewise rederive native source checks;
there is no serialized opaque-tail permission. Integrity-valid forged AFTER
bytes that alter the tail fail native reconstruction before any target write.

Headless desktop and fresh-open MCP journeys use the normal runtime services.
These local checks do not establish arbitrary imported-file write support,
other format profiles, broader schema editing or Windows/Access save parity.
See [field rename](FIELD_RENAME.md) and
[normal row insertion](NORMAL_ACE_INSERT.md) for their independent limits.
