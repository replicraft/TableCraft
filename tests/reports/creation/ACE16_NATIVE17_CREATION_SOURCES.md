# Native17 creation source qualification

The native17 source family defines the catalog upgrade independently of the
TableCraft creator. All 31 recipes start with immutable, independently authored
canonical originals: six Int32, five fixed, five IEEE, five variable, two capacity,
four catalog-name and four multiple-table cases. Original source owners and
previous fifteen-field goldens remain unchanged.

The [new source owner](../../crates/engineering/tablecraft-testkit/src/fixtures/access/canonical_native17_creation.rs)
reconstructs the catalog declaration, catalog DATA and map container from literal
seventeen-field laws. It retains native fixed prefix 32, eleven variable slots,
bf0000 presence, seven distinct Memo/OLE declaration pairs and 22 initial map cells.
Appended user-map references shift by six; all other original user/header/system
bytes remain exact. Raw leading-FEFF catalog names, unaligned fixed values,
nonfinite IEEE payloads, Decimal scale, NULL/empty values and multiple-table role
placement stay source facts rather than inferred writer output.

Both header codes 5 and 6 produce 62 complete source images. The
[read-only native gate](../../crates/engines/tablecraft-ace/src/table/mutation/normal_ordinary_sources/native17_creation.rs)
checks 98 complete user reads, native table checks and Source snapshots. Its six malformed
controls cover shortened catalog footer, missing Memo pair, duplicated physical
pair, present-empty Extra, wrong descriptor flags and a distinct live object
aliasing one TD. The corrected source gate passes 2 tests with 1 explicit exporter
ignored; the create-new exporter separately passes.

The first source run passed the positive 62-view check but failed the malformed
control test. Diagnostic index 2 was a real duplicate-LVAL-map alias: field-number
bijection alone did not establish physical map ownership. The strict target-local
declaration consumer now checks every actual column-owned/candidate storage role
against the complete captured census. Distinct records may share an ordinary map
container, but different owners cannot share a cell, record span or bitmap page.
The reader's general framing and explicit initialization policies are unchanged.
Guard commit `9ce5b171` retains the failure and passes the corrected six controls
plus five declaration/resource regressions. No expected bytes were retuned.

Independent SDK qualification reused the four unchanged bounded read-only Java
observers and their original class/jar closure, with expressions disabled and
links refused. Predictions changed only the declared catalog metadata/cells and
complete-file digest bindings; every original user and unrelated observation
remained exact. All 62 complete reports matched on the first run in 10.737 seconds,
with 0 compiler calls, 0 writer calls and 0 retries. Saved-report verification also
passed with 0 child calls. Every source, expected report and closed reader input
remained unchanged.

The [result record](results/ace16-native17-creation-sources-2026-11-01.json)
pins all 62 images and terminal receipts. These are source/read qualifications.
Subsequent creator and normal-session acceptance is separate from these source
results; see the [creation contract](ACE16_DATABASE_CREATION.md). No creator
provenance or stored permission grants write admission. Native Access
creation/save/reopen and Windows conformance remain unrun.

The source grammar deliberately uses the existing authored minimal envelope,
13-byte base 0 inline maps and already qualified direct user records. It does not
claim general imported-file compatibility. Future GLOBAL FREE bytes are retained
unchanged; no EOF allocation authority, property/default/identity generation or
new user scalar write carrier is introduced by this source qualification.


## Literal normal-session companion

A separate source owner,
[canonical_native17_sessions](../../crates/engineering/tablecraft-testkit/src/fixtures/access/canonical_native17_sessions.rs),
authors the explicit `(1,10)`→INSERT `(2,20)`→DELETE row1 sequence before any
creator or session producer runs. Its six header 5/6 views reuse the two exact
original core source images and add four unique complete AFTER images.

The new 11-byte row occupies DATA 8 slot 1 at 4074, with free space 4056 and TD 7 live
count 2. Deletion changes only live count 1 and slot 1 word 0xcfea; original/new raw
payloads, directory count, free space, maps, system/catalog bytes and EOF remain
exact. The read-only companion gate passes 1 test over all six views, and its
separate create-new exporter passes. The six unchanged-observer SDK reports match
on their first run in 1.011 seconds; saved verification passes with 0 child calls.
There are 0 compiler/writer/retry calls in the SDK phase and all input pins remain
unchanged.

These source/read results precede actual creator and normal-session acceptance.
They qualify literal full-image expectations; they do not record a performed
TableCraft create/INSERT/DELETE workflow or native Access save/reopen comparison.
