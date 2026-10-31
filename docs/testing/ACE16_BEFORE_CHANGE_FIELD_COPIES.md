# Explicit BeforeChange field-update candidates

`NativeImage::preview_ace16_before_change_updates` consumes original fixed UPDATE
intents and explicitly selected BeforeChange programs, then creates one fitting
ACE16 engineering candidate. `Ace16BeforeChangePreview` exposes complete original/
candidate bytes, distinct changed-row count and program/trace metadata. It has no
normal-plan conversion or journal/commit operation. This native API introduces no
CLI command and does not enable normal ACE saves. A [complete explicitly selected
before-event collection](ACE16_BEFORE_EVENT_COLLECTIONS.md) may also contain
dormant BeforeDelete definitions; all are preflighted before the UPDATE runs.

## One original source, one jointly final update

The method performs [logical field preparation](../access/schema/NATIVE_BEFORE_CHANGE_FIELDS.md)
itself. It accepts original selectors, column and exact `FixedScalarUpdate`
requests; a public logical report cannot grant physical permission. Current
changes sequentially, Old remains frozen and RaiseError returns a typed rejection
with no candidate, including when earlier rows already completed logically.

A fresh source/program/request reconstruction proves every canonical native-column
group against original values and jointly final rows. The existing fixed batch
then stages all columns against that same source and rewrites each affected
physical index once with the complete final tuple. Complete after-image rules,
index checks and byte preservation finish before returning the candidate. No
intermediate candidate or preview retry supplies authority.

The initial physical profile is ordinary Int32/NULL fixed UPDATE under Standard
capture. Existing record/index capacity must suffice; no DATA/INDEX allocation,
map rewrite, defaults, generated fields, cascades or general record blocks are
introduced. Logical INSERT preparation remains a separate read-only capability;
physical INSERT SetField handling is unfinished.

All native columns participate, including no-ops. The current consumer ceilings
are 256 column groups and 16,384 total cell assignments: selected rows multiplied
by full source width. Named empty operations still compile every program and run
complete original-source admission; they do not bypass source gates.

## Exact Extra permission

A sealed internal witness binds this immutable image, table and exact original
programs. Every parsed Extra property must be selected and compiled. Unselected,
opaque, unrepresented, nontext, malformed or unsupported potential programs
refuse, even for no rows or unreachable branches. Fresh recompilation rechecks
these original witnesses at physical admission.

The purpose-specific gate retains body/module/children, source-origin, field,
property, index, relationship and constraint checks. Actual catalog role capture,
complete logical-root/final-record ownership, physical INDEX inventories and
original GLOBAL allocation protect the source before staging. These read-only
checks grant no map-write capability. Existing ordinary preview and normal-save
entries retain their inert-payload policy.

Complete Extra, catalog, maps, EOF, unselected rows and ignored NULL payloads stay
protected. This explicit profile does not discover native event storage or prove
required/global macro coverage. Native dispatch, Updated timing, logging, handlers,
after-events and Microsoft Access save/reopen qualification remain unfinished.
See [data-macro acceptance](DATA_MACROS.md).

## Observed evidence

Ten source pairs passed before this consumer ran: eight complete expected images
across headers 5/6, both key directions and unique/nonunique profiles, plus two
late-rejection sources with exact unchanged images. Two physical INDEX owners and
four aliases include a nullable composite tuple. Original constructors and all
expected bytes remain unchanged.

The first source attempt correctly exposed a test-reader scope mistake: ordinary
All reads refuse the Ignore-NULL composite's 200-of-500 physical subset. The
corrected gate retains that public refusal, uses the existing complete
`check_table` qualification and independently decodes every literal KEY/ROW wire.
No reader or production qualification was widened.

The first ACE/CLI normal compilation and all four focused API functions pass.
They compare all eight prefrozen candidates plus eight exact empty-operation
images; test Group as a metadata name, late RaiseError in both request orders,
stale/duplicate requests, unchanged ordinary gates, exact resource pools/one-below
and final cancellation. Both causal authority controls pass: reconstructed groups
remain bound to the actual original request, and source/program witnesses reject
substituted images or altered program bytes. The full mutation regression passes
568 functions with four ignored helpers. Library strict checks pass for IO, ACE,
formats and Access. The combined host strict attempt retained a separate CLI
journal-error size diagnostic; the corrective CLI strict round passes after its
separate retained-error-owner correction. Exact logs and phase
boundaries are in the [focused record](results/ace16-before-change-field-copies-2026-10-30.json).


## Shared SQL-selected preparation

The Access service `engineering::guards::before_change::preview_update` plans one
local single-column SQL UPDATE through the existing shared query planner over one
captured original snapshot. Its specialized `EditableTable` reuses the existing
validated CellUpdate-to-FixedScalarUpdate converter, then finishes once through
the native physical consumer. SQL expressions see original rows; SetField receives
proposed Current and frozen Old values. Selectors remain original borrowed caller
inputs; their order is explicit engineering policy, not discovered native dispatch.

The result is `UpdatePlan<Ace16BeforeChangePreview>`, retaining exact images,
changed rows, program revisions, trace and assignment locations. Typed backend
errors distinguish snapshot conversion and native preview failures. RaiseError or
any error exposes no candidate. This service publishes no files and grants no
normal-save, journal, default/generation or cascading permission.

The shared source gate initially qualified eight ordinary read pairs/sixteen views.
The first API compile stopped before execution on three test-only ParsedQuery
boundaries. After those accessor corrections, five controls passed and three
physical positive/empty/resource controls refused: the reused read-only fixture
had no GLOBAL map, legacy owned-container headers and incomplete declaration tails.
This was a fixture admission gap, not a reason to weaken the physical gate.

An additive derivative retains the original source/helper and failed run. It adds
the literal page1/slot0 GLOBAL map (self-owner1), owned-container owner0 headers,
and independently derived FFFF declaration terminators. A stronger source gate
passes eight derivative pairs/sixteen views, checking exact map roles/memberships,
allocated pages, complete tails and unchanged original properties before service
execution. The eight API functions then pass: complete candidates/noops, empty
selection preflight, late rejection, original selectors, unchanged old admission,
closed SQL shape, late SQL error, exact/one-below pools and final cancellation.
No production gate or literal value outcome changed for these corrections.

Strict Access/CLI all-target/all-feature checks, the full 15-function guard target,
69 Access unit functions, minimal Access, scoped formatting and documentation
links (6,697 targets, no broken links) pass. CLI adoption is a separate consumer.
The [shared-service record](results/ace16-before-change-sql-service-2026-10-30.json)
keeps original read pairs, physically qualified derivatives and failed attempts
separate. Required discovery, native timing and normal ACE saves remain open.

## Rich-schema original fixed batches

`NativeImage::preview_ace16_before_change_batch_updates` is an additive native
engineering service for preservation-qualified rich schemas under Standard
capture. It accepts the existing `FixedColumnUpdates` groups and exact borrowed
program selectors. User targets may be ordinary unindexed, calculation-independent
Boolean, Int32, Int64, Currency or legacy DateTime fields. Macro operands and
SetField remain Int32/NULL only; wide values and Boolean user intents stay outside
that frame. This does not broaden the existing single-column physical service or
its public complete logical final-row contract.

All original user assignments are checked against the same captured source. Their
row union, including rows selected only by wide or no-op intents, supplies one
BeforeChange event per original logical row in source order. Every Int32 Current
replacement is seeded before the event; Old stays frozen. Sequential SetField
writes update Current. Final output targets are the union of user declarations
and potential active-event SetField targets, rather than every readable column.
Empty declarations still receive full metadata and program qualification.

Ordinary indexed or AutoNumber Int32 operands can be read without granting write
permission. Complex association keys, calculated capsules and other opaque
fields are not fabricated as Int32 values. All selected and dormant definitions
compile before rows run; indexed, dependent, counter or otherwise ineligible
SetField targets refuse even under false conditions or with no selected rows.
BeforeDelete definitions remain dormant for this UPDATE and cannot contain
SetField. Unknown, opaque, named or unselected Extra content, body/module payloads
and separately owned children retain their refusals.

One source-bound admission retains complete actual Extra coverage and exact
selectors; equal bytes in a different captured image do not reuse its authority.
The final fitting batch preserves indexes, catalog, maps, EOF, opaque siblings,
calculated caches, inactive NULL payloads and Int32 padding. There is no DATA/INDEX allocation,
record growth, cascading or intermediate candidate. Required and other qualified
rules apply after the complete transformation, so a transient user NULL can be
repaired by SetField; an unrepaired final NULL still refuses. Changed rows count
the final original-root union, including zero changes when programs restore the
original values. RaiseError returns no candidate, including after earlier rows.

An independent physical proof reacquires selected programs and complete Source,
replays original user intents, and compares exact program receipts, traces,
native-position assignments, projected row results and derived groups. Only its
own replayed groups reach the existing independent fixed-byte auditor and final
original-property validation. Fresh candidate program/Source admission follows.
No staged group or partial logical row grants physical authority.

### Authored qualification

The original catalog corpus was committed as `d78cacb0`; healthy Required
originals were separately read-qualified before their expectations were authored.
The 29-case corpus has 15 complete header-6 expected images and 14 refusal
controls. Its header-5 rich siblings retain their unsupported Extended Date
profile boundary; this is not a general header-5 BeforeChange refusal. The exact
expectations were committed as `bd35525f` before this batch producer ran.

All ten focused native functions passed on their first execution, including 60
complete-image comparisons across group and row orders, original Current/Old
semantics, final restoration to the original image, readonly counter operands,
empty-operation preflight and late rejection. Causal controls cover source
identity, well-formed Extra changes, alternate original intents, byte/mask/count
forgeries and final Required enforcement. Exact Bytes/Rows/Steps pools, each
one-below limit and final cancellation pass.

After the fixture-owner extractions, three read-only source functions passed
again, covering catalog originals, healthy Required originals and all 30 positive
before/expected views through actual selected Source admission. The full mutation
suite passed 629 functions with 11 ignored helpers. The focused Extra regression
passed 36 functions, including legacy complete-final-row SetField coverage.
The public preserved contract passed 54 functions with two ignored helpers.
Combined all-target/all-feature strict checks passed for ACE, Access, CLI and
testkit; workspace formatting and 6,828 documentation link targets passed.
No-default-feature builds pass for ACE, Access and testkit. Access/CLI consumer
qualification remains separately tracked; it is not implied by native results.

These authored engineering observations do not establish genuine-file or
Microsoft Access save/reopen compatibility, required-event discovery, native
ordering/timing, normal ACE saves, journal permission or macro absence. See
[the complete collection contract](ACE16_BEFORE_EVENT_COLLECTIONS.md) and
[data-macro acceptance](DATA_MACROS.md).
