# Explicit database Name AutoCorrect options

`NativeImage::plan_disable_name_auto_correct` now constructs an unpublished native
candidate for an explicit request to disable Track/Perform Name AutoCorrect.
This does not change constructor defaults, imply that missing metadata is zero,
or implicitly save a file. The native candidate and separate original-intent
lane retain their own explicit qualification below.

The first source language is the existing canonical seven-system-role creator
ledger with exactly one user table and no MSysDb. Source admission reuses the
complete append-table creator qualification, including native roles, original
catalog records, header/system pages, schema/row/index checks and exact bounded
allocation memberships. Ordinary manual/keyless and generated-primary originals
must independently enter that existing language. The reserved native MSysDb
identity and name must be unused. Existing database properties, eight-role
catalogs, imported application dependencies and broader allocation layouts
remain unsupported by this first operation.

The explicit request appends a real MSysDb row under the Databases container and
one 152-byte unnamed MR2 map containing the two exact LONG/DDL zero properties.
Creation/update dates and the owner carrier are absent rather than invented.
The map uses one external LVAL row on a new EOF page. The existing distinct,
empty LvProp owned/candidate map records must represent that page below their
64-page boundary. The operation sets only those two allocation bits and updates
the catalog definition's count plus its checked dense DATA append scope. It
does not relocate original catalog rows, reuse free pages, expand maps, rebuild
tables/indexes or author NameMaps.

The shared native17 catalog serializer owns the new database-record variant;
the checked dense catalog-page helper and existing DATA-page serializer own
physical append/LVAL framing. Candidate verification checks the precise original
byte complement, unchanged identities, new native containment role and exact
database-property decoding. All original user TDEF/DATA/INDEX bytes, physical
row slots, counters, properties and opaque tails remain unchanged.

The [SDK reference and independent full-image model](../../testing/ACE16_DATABASE_OPTIONS_SOURCES.md)
establish the external representation on two current creator originals. Separate
raw fixtures author six empty/populated keyless/manual/generated-primary pairs
with retained sentinels for the native candidate tests. Authored tests and SDK
write observations remain distinct from executed native planner checks.

A separate [OFF-after-ADD SDK experiment](../../testing/nac_after_field_add/README.md)
now compares five first-party Number/ShortText ADD sources with complete raw
images frozen before SDK mutation. All five match exactly. The explicit new
MSysDb role appends its 152-byte body to the existing catalog-column LvProp page;
old user descriptors, property rows and all map bytes stay unchanged. Forwarded
user catalog sources append database physical slot 9, while the ordinary source
uses slot 8. These observations require object/role-specific history accounting;
the number of LVAL rows alone is not the number of user ADD events. This is SDK
engineering evidence, not native Access UI qualification or expanded native
option admission. In particular, frozen `TCDO0001` still rejects these 11-page
inputs. The portable wrapper's later classpath adjustment is explicitly unrun.

The focused native checks pass all three candidate tests, including six complete
independent images with retained catalog/map/definition sentinels. Initial runs
exposed two verification mistakes: complete Header equality included page count
derived from actual EOF, and the LVAL check interpreted the complete slot word as
flags instead of masking its offset bits. A temporary test-only diagnostic proved
zero complete-image differences from the independent oracle and identified the
exact final verification branch. The corrections keep every stored header byte
in the exact complement proof, require exactly one additional physical page and
check only actual slot flags. Temporary diagnostics were removed before the
passing rerun; no native source/profile guard was broadened.

`plan_recoverable_disable_name_auto_correct` now binds a qualified candidate to a
new closed `TCDO0001`/profile 1 original request. Its fixed 64-byte record contains
the complete BEFORE length/digest and the explicit disable-both action; it stores
no target path, payload patch or AFTER. The bounded 9/10-page wire envelope never
qualifies source ownership by itself. Fresh requalification compares the entire
original, rebuilds the same closed native candidate under a separate native pool,
then compares both complete images under the proof allowance.

Persistence uses the existing expected-image V2 journal with an explicit
resizable target. Cold recovery verifies original intent, freshly captures the
journal BEFORE and reconstructs the complete option candidate before consuming
that same checked V2 session. Pending/torn writes restore original bytes and
length; committed writes require the full replacement. Settled confirmation
checks bytes/barriers without repair or another option action. All six focused
wire/requalification/recovery tests pass, including two independently authored
full-image reconstructions and eight pending/committed/torn-length/partial-marker
recovery cases. These and the three candidate tests form the nine-test passing
options batch. The shared memory storage models protocol behavior, not physical
filesystem durability. Initial failure, diagnostic and corrected logs remain in
ignored cache records. Existing TCAT/TCNT/TCAC/TCRN languages are unchanged.

The Access adapter now stages reviewed explicit OFF requests through
`stage_reviewed_disable_name_auto_correct_with_budgets`. It freshly captures the
same retained exclusive target, checks the complete reviewed original and uses
the consuming native requalification capability under separate allowances.
Abandoning staging retains that target without creating a journal. Explicit
apply uses the existing resizable pair lifecycle without replacing the file.
Cold/retained recovery and write-free settled confirmation decode only TCDO and
reconstruct the same native request through its exact checked V2 session.

The allocation-free receipt has a separate
`DatabaseOptionsChange::NameAutoCorrectDisabled` marker for acknowledged commits
or verified Replacement images. Original images and rollback retain no such
marker. Matched/changed rows and table-schema/query changes remain absent.
Existing cancellation, setup, recovery/finalization and release failures retain
the same owner and terminal outcomes. All six focused adapter tests pass.

The shared runtime now has a separate explicit option review/save checkpoint.
`ReviewDisableNameAutoCorrect` requires the current document revision and issues
`DatabaseOptionsReview` plus the consuming `ReviewedDatabaseOptions` capability.
The unlocked review retains complete original/candidate bytes and a full-image
read-only reopen witness. `SaveWorker::start_disable_name_auto_correct` consumes
it through the same nonreplaceable save owner, rechecks source validity and
stages the existing adapter operation under the retained exclusive target.
Only the separate database-options completion fact is reported; row counts and
schema/query facts remain absent. The existing resizable recovery lifecycle
handles retained/cold requests without a new host recovery implementation.

After acknowledgement, `OpenDatabaseOptionsSaved` verifies the entire expected
replacement before creating a fresh read session. It neither replays the setting
operation nor grants permission to rename. Unsupported hosts return explicit
refusals and unconsumed save capabilities. A shared native helper owns the same
fallible, budgeted expected-image copy used by option, rename, column and query
reviews. Five focused runtime tests are authored; compilation/execution remains
pending the coordinated batch.

`DatabaseProperties::name_auto_correct` provides an inert typed observation:
`Absent` when neither named setting occurs in an unnamed default map,
`ExplicitOff` only for one occurrence each of native LONG/DDL zero, and `Other`
for partial, duplicated, differently typed/flagged or other values. A missing
MSysDb remains the outer inspection's `None`. No missing or nonzero value is
called enabled, and this observation alone never qualifies a mutation. The two
focused observation tests are authored but unrun at this checkpoint.

The [earlier completed SQL/creator batch](../../testing/results/sql-creator-focused-2026-11-08.json)
passes 72 tests, including 35 engine rename, four runtime rename and six long-value
cases. Its V3 positives establish bounded native option-then-rename composition.
It predates this new runtime options service, metadata observer and expected-image
copy extraction; the five service tests, two observer tests and rewired-consumer
regressions remain unrun.

No GUI option command, native Microsoft Access comparison or Windows execution
is supplied here. Existing add-table source admission still assumes the original
seven-role ledger. Rename V1/V2 retain their exact inline source grammar;
[the separately sealed creator V3 source](TABLE_RENAME.md) now qualifies bounded
rename after explicit OFF, with independent source/image/recovery tests passing.
The setting change remains an explicit operation and never changes constructor
defaults or runs automatically during rename. General CRUD composition remains
incomplete.
