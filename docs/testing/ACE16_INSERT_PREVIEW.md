# ACE 16 ordinary-row INSERT engineering preview

`NativeImage::preview_ace16_insert_rows`, behind `ace16-write-preview`, constructs
a separate `Ace16WritePreview` from complete explicit `RowInsert` values. This
is an engineering candidate for disposable offline copies. It is not a normal
save, SQL INSERT executor, journalable edit plan or Microsoft Access qualification.
The existing [preview safety boundary](ACE16_WRITE_PREVIEWS.md) still applies:
required data-macro semantics remain unqualified.

## Admitted physical profile

The selected ordinary ACE 16 table has two current fields: one fixed Int32 and
one variable Short Binary, in either declared order. Names do not select their
roles. Each request supplies both exact carriers in native column order; no
coercion, omitted values, defaults or generated identities are supplied. Binary
NULL, present empty Binary and nonempty Binary remain distinct. Declared widths
apply, with the native Short Binary ceiling of 255 bytes.

DefaultValue presence refuses, even when every value is explicit or the batch
is empty. AutoNumber, calculated and complex fields refuse. Existing active
payload, catalog-child, unsupported-property and constraint blockers remain.
This admission does not establish absence of unknown data macros.

Exactly one physical unique, required, ascending Int32 primary index is admitted.
Qualified logical aliases share that physical identity. Other physical indexes,
ignored-NULL profiles, additional key fields, non-leaf roots and required index
splits refuse. Original records and their logical ROW references remain intact.

## Placement, index membership and counters

Every new record has a complete ordinary row layout with explicit NULL bits,
variable offsets and stored values. New logical roots are visible and have no
deleted/hidden/forwarding flags. Deleted slots are retained rather than reused;
existing rows are not compacted or rewritten to make room.

The allocator tries the highest qualified free-space hint. A failed fit clears
that hint, preserves lower untried hints and proceeds to represented EOF space.
New DATA pages and any required hint bitmap metadata have distinct reservations.
The shared [allocation rules](ACE16_ALLOCATION_TRANSITIONS.md) remain bounded by
the original map authority and represented global free-page coverage.

The primary leaf receives the exact new key/ROW pairs, preserving its root
identity and unrelated header/residue bytes. Duplicate original or batch primary
values retain typed uniqueness failures. Capacity overflow refuses before a
candidate is returned; this entry point does not silently invoke an UPDATE split.

The declared table count increases by the number of inserted rows. The actual
[historical physical index counter](../access/indexes/NATIVE_INDEXES.md#explicit-historical-counter-inspection)
increases by the same delta once, including when logical aliases exist. The
historical counter is not reconstructed from the current live row count.

## Authority and publication

Builder plans are provisional. A separate final audit derives records, placement,
map transitions, primary membership and counter deltas from the original image
and immutable requests. Complete-image comparison protects bytes outside those
effects. Fresh catalog, table/index and constraint checks follow before returning
the candidate. Cancellation, allocation and work limits cover construction and
proof; failure never returns an incomplete candidate.

An empty qualified batch produces a byte-identical candidate with zero changes.
It still checks the table's admission profile and consumes inspection resources.
Preview construction consumes an in-memory capture and does not mutate the
source file.

The explicit `preview_ace16_insert` example uses clap and the
[bounded quoted TSV input protocol](ACE16_INSERT_INPUT.md). It validates complete
request input before opening the database and uses the existing fresh-output
publisher. Ordinary CLI writes remain unchanged.

## Remaining work

This profile does not finish general INSERT. Other scalar/complex fields,
defaults and AutoNumber, additional index families and splits, broader allocation
and reclamation, query INSERT planning, application events and normal native save
qualification remain separate work. Native Microsoft Access comparison evidence
is still deferred by the user.
