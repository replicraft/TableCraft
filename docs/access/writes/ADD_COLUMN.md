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
