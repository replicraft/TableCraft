# Empty saved-table column addition

The normal desktop supports restricted Number-column additions through reviewed
native saves and complete-image reopen. The engine also supplies lower-level
Int32 candidates and original-intent recovery profiles, plus a separate typed
Number/Short Text candidate. General saved-column editing remains unfinished;
the typed candidate alone does not enable Text saves in the GUI.

## Typed Number and Short Text candidate

The separate `prepare_created_add_field_candidate` API now constructs a typed
Number or Short Text candidate from a positively reconstructed empty ACE16
source. Short Text explicitly selects 1–255 UTF-16 units and Unicode compression
permission, Required(false), AllowZeroLength(true), and no stored default. Existing
creation defaults and TCAC1–4 source/request languages remain unchanged.

The proof reconstructs a canonical generated-primary seed and every appended
field/property-history stage. Dictionary order derives from that history; an
arbitrary observed permutation cannot authorize itself. Original field widths,
compression, variable ordinals, index bytes, property rows and the full image
complement remain protected. Only the qualified one-EOF property rollover is
available. This candidate supplies no normal persistence authority or GUI Text
activation; the existing Number save service remains separate.

The [candidate workflow batch](../../testing/results/typed-field-candidate-workflows-2026-11-08.json)
passes 336 unique selected tests, including ten new candidate cases covering
18 independent SDK AFTER images. Three existing ignored cases are excluded.
Strict engine/testkit all-feature lint, creation+journal lint and creation-only
lint pass. Initial feature/lint failures remain recorded separately. Native
Microsoft Access and Windows execution remain unqualified.

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
NULL and supplied-value observations. TCAC0003 retains its original inline/absent
property source; the repeated external-property route below is a separate captured
purpose. Required true, broader properties, populated sources and native Access
timestamps remain outside both profiles. SDK comparisons and macOS checks do not
qualify Microsoft Access or Windows execution.

## Repeated Number addition with existing external properties

The distinct `RepeatedDefaultZeroAddColumnCandidate` and `TCAC0004`/profile 4
original intent admit the same explicit nullable LONG, stored Required(false)
and typed zero declaration on the output of the first Number save. The
[three independent SDK observations](../../testing/number_column_repeat/README.md)
match complete native candidates: the file stays 45,056 bytes, only catalog page 4,
table-definition page 7 and existing property LVAL page 10 change, and allocation
maps and the primary INDEX page remain exact. The new complete MR2 body appends
as row 1; the prior row 0 body remains byte-exact. Dictionary/group order and absent
versus explicit Required metadata remain distinct.

Source admission is positive complete-image reconstruction, not a creator flag,
fixture hash or normalized copy. The exact empty ACE16 generated-key layout
retains owned property pages [10] in an eleven-page file, or [10, 11] in a
twelve-page file after one rollover. Only the last owned page is a candidate.
For n current fields and r occupied property rows summed across that actual
ledger, reconstruct the canonical n-r field prefix, apply the frozen first
Number constructor, then replay each historical append, including its exact
fitted or one-rollover placement. Every historical appended field requires exact
Int32, stored Required(false) and zero policies. The complete resulting source must equal the actual original,
including catalog logical/hidden roots, descriptor selection of the last body,
old property rows, maps, index ownership and untouched bytes. Work and allocations
consume one cumulative native allowance across all stages.

Candidate construction uses the existing declaration/MR2 serializer, catalog
logical-root/record-placement owner and shared `owned_lval` append framing. Exactly
one new body appends to the last owned page when it fits. If page 10 cannot
fit it, exactly one immediate-EOF page 11 may be allocated; subsequent bodies
must fit page 11. The old page 10 stays entirely exact during rollover. A full
page 11 refuses: no second allocation, reuse, reclaim or history retirement is
admitted. The existing sixteen-field bound remains independent: a qualified
source with fifteen fields can yield sixteen; a sixteen-field source cannot
add another field.

The represented EOF policy is explicit. Preview allocation still requires its
original free-cell state. This canonical property route requires the represented
EOF free bit clear, while separately proving physical file length, immediate EOF,
ordinary/LVAL nonownership and the complete constructive source. The bit alone
grants no ownership. Rollover derives exactly two selected owned/candidate map
byte edits, rejects role-byte aliasing and leaves global allocation bytes exact.
Fresh native reopen verifies the exact complete property body and field/index
identity; every earlier row's slot word, span and payload remains unchanged.
Complement proof permits only the selected definition/catalog/property scopes
and those derived map bytes. Additional tables/options insertion, unknown
histories, populated sources, chained/other external storage and growth beyond
page 11 remain unsupported.

TCAC0004 binds the original complete length/digest and exact names; decode returns
request data only. Fresh reconstruction and both-image qualification use separate
normal native/proof allowances and one cumulative recovery allowance. It reuses
the checked resizable V2 journal for both fitted equal-length images and the
one-page rollover; request version and journal version are independent. Extending
source/placement admission changes neither the original declaration nor its
TCAC0004 framing. TCAC0001/0002/0003 retain their exact previous languages and reconstruct only their captured constructors.
Cold/settled recovery dispatches the exact version with no decoder fallback.

The public runtime/UI service stays `ReviewAddNumberColumn`,
`ReviewedNumberColumn`, `start_number_column` and `OpenColumnAdded`. An engine-owned
metadata observation resolves the actual catalog logical root (including
forwarding), native field framing and LvProp carrier. Review and availability
select one constructor from that fact, then fully qualify it; failed qualification
never retries another profile. Known older-format Source refusal remains
unavailable, while malformed/cancel/resource errors propagate. The worker retains
the actual sealed purpose, same exclusive offline lifecycle, schema-only
ColumnAdded receipt and full-image witness before publishing the fresh document.

The [coordinated repeated-add record](../../testing/results/repeated-number-workflows-2026-11-08.json)
retains 127 selected passing checks across engine, adapter, runtime and desktop.
It includes three ordinary Click-to-Add/name/Save operations in one session,
followed by actual GUI insertion of a row with generated ID and three omitted
fields stored as zero. A near-capacity history save reaches fourteen readable
fields using the existing runtime native pool; no resource allowance changed.
Strict affected-package lint passes with all features and for normal
creation+journal without preview. This is bounded repeated ADD, not saved-field
rename/drop, general column CRUD or Access 97–2019/native Microsoft Access
qualification. The earlier native and consumer checkpoints, zero-match selection
attempt and corrected test-helper build failure remain separately recorded.

### One-rollover recovery and consumer evidence

The earlier three second-ADD SDK observations above remain same-page evidence.
The later [rollover SDK record](../../testing/results/number-column-rollover-sdk-2026-11-08.json)
separately pins the complete N13→N14 image, which grows from 45,056 to 49,152
bytes. N15's fitted page-11 construction has independent raw-model/native proof;
it is not an additional SDK execution observation.

The [rollover workflow record](../../testing/results/number-column-rollover-workflows-2026-11-08.json)
retains 135 selected passing checks: 83 engine, 16 adapter, 18 runtime and 18
desktop checks. Strict normal creation+journal lint without preview passes.
Native checks cover the complete SDK image, positive reconstructed page-11
sources, exact map refusals and full-page refusal.
Checked V2 tests cover committed growth, partial-growth rollback, write-free
settled confirmation and refusal of tampered foreign/global/owned map bytes even
when a generic journal digest is valid. Preview's original RequireFree placement
policy remains separate.

Runtime and ordinary `Desktop::new` headless checks both save N14 using unchanged
normal native/proof/persistence budgets, acknowledge the real actor and pass the
complete-image reopen. The actual GUI then uses New row, omits all fourteen
Number fields and saves generated ID1 plus fourteen non-NULL zeros. No viewport
widening or capability override is used. These checks qualify this bounded
engine/application route; they do not establish native Microsoft Access,
Windows execution, other fields or older-format writes.


## Short Text and mixed-source evidence checkpoint

The [new independent reference](../../testing/short_text_add/README.md) records
eighteen complete SDK/raw AFTER matches for explicit TEXT255 with Required(false),
AllowZeroLength(true), no DefaultValue property and separate compression-off/on
twins; repeated Text and Number-after-Text compositions retain all original
property rows and field facts. A separate third Number SDK image closes the
earlier raw-only history observation. These are new source/target byte oracles,
not native Short Text candidate, persistence or GUI admission.

Actual current TableDraft metadata has the same declared width/policies/default
absence but the existing creator leaves UnicodeCompression off. Microsoft
documents the Access UI compression default as on. A future typed native opt-in
can preserve old defaults and images while making that new target explicit;
source reconstruction must preserve each original field's actual compression
and AZL/default presence. No generic raw flags, silent global default flip or
new wire version is introduced by this evidence checkpoint.
