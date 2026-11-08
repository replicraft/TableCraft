# Empty saved-table column addition

The native engine now has an authored source-qualified candidate and a separate
original-intent persistence/recovery API for appending one nullable ordinary
Int32 field. The focused native and offline adapter checks pass; these APIs do not yet
enable saved-column editing in the GUI or CLI.

## Actual source and preservation

`NativeImage::prepare_add_int32_column_candidate` requires a complete standard
capture of the canonical single-table ACE 16 (`Ace2016`) creator layout: one
empty keyless table with one through fifteen ordinary Int32 fields. Actual
decoded table/field spellings select the comparison; no literal source hash,
remembered creator flag or particular name grants admission. Complete source
comparison refuses other objects, properties, nonzero timestamps, indexes,
populated tables, inactive history and unknown bytes.

The operation validates the native proposed name, duplicates and the resulting
sixteen-field ceiling through the existing constructive declaration owner. It
changes only the selected definition page's counts, length/free-space and
appended descriptor/name-vector scope. Other catalog, allocation and user data
bytes remain exact. Recapture checks the resulting declaration geometry. The
independent testkit fixture models the SDK-observed whole AFTER without calling
production codecs; broader names and field-count boundaries remain explicit.

This first operation is not general ADD COLUMN: generated keys, required/default
programs, other scalar types, populated storage changes and broader Jet/ACE
profiles remain work to do. Existing draft authoring remains a separate workflow.

## Original request and journal

With both `journal` and `ace16-create`,
`plan_recoverable_add_int32_column` captures a sealed candidate and
`NativeAddColumnIntent`. The closed `TCAC0001`/profile-1 language has a 60-byte
header, full original length/digest and exact UTF-8 table/new-field spellings.
Profile 1 always means this specific nullable Int32 operation. Unknown versions,
profiles, flags, framing and malformed borrowed names refuse before either name
is allocated. Decoding request data does not grant save authority.

Fresh requalification compares the complete retained original, verifies its
binding, repeats native source/declaration qualification and compares the entire
rebuilt AFTER. Native and proof budgets remain separate cumulative pools. The
consuming plan prepares the existing fixed-length checked V1 image journal;
request version and journal version are independent. Hosts still own exclusive
resources, durable namespace setup, cleanup and recovery discovery.

`qualify_native_add_column_recovery` reconstructs both complete images before
returning the same checked journal session. Explicit recovery restores BEFORE or
retains the committed AFTER according to that session's checked state. Settled
confirmation performs no repair writes. No stored patch, ordinal or arbitrary
AFTER is accepted as mutation authority. Existing row, creation and table-rename
intent languages remain unchanged.

The focused authored tests cover independent request bytes, complete-image
reconstruction, malformed framing, stale originals, request substitution,
pending/committed/torn journals, settled confirmation and independent proof
limits. Five candidate, six native intent/recovery and five offline adapter tests pass.
The adapter stages a reviewed plan against fresh retained-target bytes and
dispatches only the closed TCAC family during cold and settled recovery. A
committed `SchemaChange::ColumnAdded` has no matched/changed row counts.

See the [focused batch record](../../testing/results/schema-save-workflows-2026-11-08.json).
Native Windows and Microsoft Access save/reopen behavior, timestamp parity and
GUI column editing are not qualified by this checkpoint. Five additional runtime
checks now pass: consuming review/save, full-schema reopen, stale revisions and
capabilities, source drift and whole-content reopen witness validation. The native
and unsupported host services share the typed contract; this gate executes on
macOS only.

## Generated-primary source and versioned request

The separate `prepare_created_add_int32_column_candidate` source proof now passes
seven focused cases, including complete SDK AFTER images for an inline existing
default zero and mixed scalar declarations. It admits canonical empty header-5
creator tables with a first LONG Increment primary and constructively known
inline field properties. Original keys, defaults, descriptions, field metadata
and all bytes outside the definition-page envelope remain unchanged. It still
appends only a nullable Int32 with an absent new default; authoring a new default
zero needs a separate property-storage/allocation operation.

`plan_recoverable_created_add_int32_column` now seals that separate purpose as
`TCAC0002`/profile 2. The old API and `TCAC0001`/profile 1 keep their exact keyless
admission and original wire body. Both use a 60-byte request header and the
existing fixed-length checked V1 image journal. Version/profile pairs are closed;
crossed or future pairs refuse before name allocation. Fresh reconstruction
selects only the captured native constructor, never another profile after a
failure.

The shared runtime selects one declared constructor from actual native index
metadata. Selection does not grant admission: the selected source proof remains
complete, and unsupported layouts refuse. This metadata selection is not used
to reinterpret retained requests during recovery. Three new `TCAC0002`
wire/recovery cases and the expanded runtime save/reopen case pass in the
coordinated local batch. This is not general populated-table alteration or GUI
default authoring.

## Independent generated-primary SDK observations

The [source/provenance record](../../testing/results/add-column-created-sdk-source-2026-11-08.json)
separates whole-image SDK observations from native candidate and normal-save
qualification. The existing inline-default-zero source and the fourteen-field
Mixed source both yield the complete independently authored AFTER when a nullable
LONG with an absent new default is appended. Only page 7 changes; file length,
primary root, allocation, catalog properties and zero native dates remain exact.
The raw fixture owns original bytes independently of the production serializer.

Explicitly declaring default zero on the new field is a different SDK operation:
it changes catalog and allocation pages and grows the file by one property LVAL
page. It is outside both current source-qualified candidate target declarations.
A future target-property operation needs an explicit original declaration and
complete property-storage/allocation preservation and recovery proof. Existing
default-zero preservation does not authorize inventing a new field default or
enabling the ordinary GUI Number-field workflow. All observed SDK dates are
`1899-12-30T00:00`; this does not qualify native Access timestamp changes.

## Existing-table UI boundary

Saved native datasheets keep Click to Add disabled. The runtime review, consuming
save worker and whole-content reopen are implemented, but the desktop has no
existing-column naming/save controller. The current primitive's absent default
must remain distinct from the [Number gallery's typed-zero declaration](../../ui/DATABASE_CREATION.md#adding-a-column-to-an-existing-saved-table).
It cannot enable ordinary Number creation by displaying a provisional zero or
borrowing the unsaved-table gallery's enabled choices.

The retained explicit-zero SDK target grows from 40,960 to 45,056 bytes, changes
existing pages 3, 4 and 7, and adds property LVAL page 10. Besides a new complete
property/allocation-preserving target proof, this requires resizable journal
preparation and matching original-request recovery: the current fixed V1
`prepare_expected` rejects unequal image lengths. Generic checked resizable V2
support already exists, but is not an add-column qualification. Frozen
`TCAC0001`/`TCAC0002` requests continue to mean their original no-default operations.

A complete user-facing slice still needs qualified source eligibility, exact
new-field declaration/name review, a dedicated schema-save owner, acknowledged
`ColumnAdded` receipt and guarded reopen. A separate insert must establish omitted
zero-default behavior while preserving explicit NULL and supplied values. Current
empty-source, field-count and file-profile limits remain; SDK byte comparisons
and zero dates do not establish native Access save timing or timestamp parity.
