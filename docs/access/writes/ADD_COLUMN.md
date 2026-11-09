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
page. It is outside both existing no-default candidate target declarations. The
separate default-zero candidate below passes its focused byte-preservation checks;
normal persistence still needs an explicit original request and matching
resizable journal/recovery qualification. Existing
default-zero preservation does not authorize inventing a new field default or
enabling the ordinary GUI Number-field workflow. All observed SDK dates are
`1899-12-30T00:00`; this does not qualify native Access timestamp changes.

## Separate explicit-default-zero candidate checkpoint

The authored `prepare_created_add_default_zero_int32_column_candidate` is a
separate candidate-only operation, with its own opaque candidate/error types.
The native candidate slice is connected. All six focused Rust cases pass in the
coordinated batch, including both full SDK AFTER-image pins. Seven existing
created-source cases, five keyless candidate cases and nine frozen-intent cases
also pass. The candidate itself supplies no original intent or save authority.
The separate Number route below now supplies native intent, journal/recovery
and runtime services; GUI controls remain disabled. Existing `TCAC0001`/`TCAC0002` admission and
wire meanings remain unchanged.

Its authored source proof requires the complete canonical empty header-5
creator image, a first LONG Increment primary, one through fifteen source fields
and zero native dates. Properties must be absent or consist only of constant
zero defaults and explicit Required false. Existing external property storage,
other dictionaries, extra objects, populated rows, unknown history and nonzero
dates refuse. The complete
target MR2 body must prefer one external LVAL row (65 through 4,076 bytes);
shorter inline and chained targets remain outside this checkpoint.

The [new SDK record](../../testing/results/add-column-default-zero-sdk-source-2026-11-08.json)
qualifies two distinct independent observations. Adding `Score` with explicit
DefaultValue `0` to `Table1(ID)` grows its 73-byte catalog root into an 85-byte
hidden target, retaining logical root slot 7 through a forwarding pointer to
hidden slot 8. Its logical catalog row count remains unchanged. Adding `Extra`
with the same default to `Items(Id, Score DefaultValue0)` instead keeps the
151-byte physical record, replaces its inline property descriptor and zeroes the
reclaimed inline body. Both grow from 40,960 to 45,056 bytes, change existing
pages 3, 4 and 7, and append LVAL page 10. Only the selected LvProp owned/candidate
map cells gain page-10 membership; the global free map remains exact.

The candidate composes existing declaration/MR2 encoding, variable-record
replacement, represented allocation and audited stable-root placement; it does
not introduce an ordinary system-catalog UPDATE permission. Independent raw
fixtures retain literal record/map/LVAL construction without production codecs.
The separately authored Python literal AFTER reconstruction matches both entire
SDK AFTER images byte for byte. The six Rust cases cover those full image pins,
old no-default behavior and refusal/cancellation/resource boundaries, but have
not been compiled or executed at this checkpoint. No native Microsoft Access
comparison or timestamp/event parity is inferred from the SDK observations.

### Reproduce the independent SDK operations

The maintained [first-column helper](../../testing/CreatedFirstDefaultColumnOracle.java)
is the executed original first-party probe source, promoted unchanged; the
[existing-inline helper](../../testing/CreatedTableColumnOracle.java) keeps its
frozen recipe. Both use the reviewed Jackcess 5.0.4 JAR, Apache-2.0, SHA-256
`19a7494a56f439a57fda85c059b019b665802085cec5ae8186d1e028cc1bdfbc`.
See [fixture provenance](../../testing/FIXTURES.md#explicit-default-zero-saved-column-add-reference--8-november-2026).

Set `JACKCESS_JAR` to that reviewed artifact, `ORACLE_CLASSES` to a local generated
class directory, and `ID_BEFORE`/`INLINE_BEFORE` to the hash-pinned original inputs
in the SDK record. The independently authored sources are
`canonical_created_default_zero::source(Source::IdOnly)` and
`source(Source::ExistingZero)` in the testkit fixture owner. Set `ID_AFTER` and
`INLINE_AFTER` to fresh output filenames that do not exist. Run from the repository
root in a POSIX shell:

```sh
mkdir -p "$ORACLE_CLASSES"
javac -cp "$JACKCESS_JAR" -d "$ORACLE_CLASSES" \
  docs/testing/CreatedFirstDefaultColumnOracle.java \
  docs/testing/CreatedTableColumnOracle.java
java -cp "$JACKCESS_JAR:$ORACLE_CLASSES" CreatedFirstDefaultColumnOracle \
  "$ID_BEFORE" "$ID_AFTER" Table1 Score --default-zero
java -cp "$JACKCESS_JAR:$ORACLE_CLASSES" CreatedTableColumnOracle \
  "$INLINE_BEFORE" "$INLINE_AFTER" Items Extra --default-zero
```

Each helper copies its original, disables expression evaluation, calls public
SDK ADD, reopens read-only, prints complete BEFORE/AFTER SHA-256 and checks that
the original remains byte exact. Database, JAR and generated class bytes remain
local and untracked. These commands replay SDK evidence; they do not execute the
new native candidate or establish normal save/recovery qualification.

## Existing-table UI boundary

Saved native datasheets connect the exact Number declaration through shared UI
naming, runtime review and the dedicated desktop save owner. Host support and a
positive name-independent native source inspection are both required. The older
absent-default primitive remains distinct from the [Number gallery's typed-zero
declaration](../../ui/DATABASE_CREATION.md#adding-a-column-to-an-existing-saved-table).
Painting a provisional zero cannot substitute for persisted default metadata.

The explicit-zero target grows from 40,960 to 45,056 bytes, changes pages 3, 4
and 7, and adds property LVAL page 10. TCAC0003's complete property/allocation
proof uses checked resizable V2 journal recovery. Frozen TCAC0001/TCAC0002 retain
their original no-default meanings. An acknowledged ColumnAdded receipt alone
does not publish a refreshed document: the complete expected image must reopen.
A separate insert checks omitted zero-default behavior while retaining NULL and
supplied values. SDK comparisons and zero dates do not establish native Access
save timing or timestamp parity.

## Exact Number declaration and persistence

The typed `CreatedDefaultZeroColumnRequired` declaration keeps absent Required
metadata distinct from stored Boolean false. The Number service explicitly
requests `StoredFalse` together with an Int32 zero default. Three independent
[SDK observations](../../testing/results/add-column-number-sdk-source-2026-11-08.json)
cover an ID-only source, an existing default-only source and a source with
Required false plus default zero. Complete candidate images match all three.
The property writer retains dictionary order: newly introduced Required follows
an existing DefaultValue entry, while an existing Required-first dictionary
remains in that order. Ordinary constructive creation keeps its original order.

`TCAC0003` binds the exact declaration, original image length/digest and bounded
names. Its native plan reconstructs the entire candidate before a checked
resizable V2 journal; cold and settled recovery decode that exact language before
reconstruction. Frozen TCAC0001/0002 remain separate no-default languages.
The Access adapter preserves the retained exclusive owner, original lengths,
explicit cancellation, separate normal native/proof pools and schema-only
ColumnAdded receipts. Recovery spends its original cumulative caller allowance.

Runtime exposes `ReviewAddNumberColumn`, a consuming `ReviewedNumberColumn`,
`SaveWorker::start_number_column` and the existing complete-image guarded
`OpenColumnAdded` read. Review holds no write lease. Actual native review/save
rechecks the source and requested declaration; a terminal receipt alone cannot
publish a refreshed document. The [batch record](../../testing/results/number-column-runtime-batch-2026-11-08.json)
retains 65 passing checks and strict affected-package lint. The actual GUI draft
producer, not only hand-authored declarations, matches both source classes.

Saved-datasheet naming/save, multi-tab checked publication, close confirmation,
readonly inspection cancellation and explicit failed-refresh Return are connected.
Return preserves the field name, save outcome and cleanup notice without rollback
or replay, and blocks mutation against the stale document until a fresh open.
The actual GUI Number-save/New-row case verifies generated ID and an omitted
Score stored as zero after native reopen; separate runtime cases retain explicit
NULL and supplied-value observations. Required true, broader properties,
populated sources, external source properties and native Access timestamps remain
outside this profile. The [repeated-addition SDK reference](../../testing/number_column_repeat/README.md)
records expected same-page property history, but does not yet qualify its source.
SDK comparisons and macOS checks do not qualify Microsoft Access or Windows
execution.
