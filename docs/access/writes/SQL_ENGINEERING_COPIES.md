# SQL-selected engineering copies

`tablecraft query preview-update` evaluates a bounded SQL UPDATE against one
original native snapshot, then prepares one fresh experimental engineering copy.
It uses `tablecraft_access::engineering::sql` and `AccessEngineeringEditor` over
shared DB/query contracts. It does not enable normal ACE saves, certify data-macro
absence, execute active content or produce a journalable `NativeEditPlan`.

```sh
tablecraft query preview-update 'UPDATE Contacts SET Name=Name & " Jr" WHERE Id=2' \
  --database contacts.accdb --output edited.accdb --variable --index-growth

tablecraft query preview-update "UPDATE Contacts SET Age=20, Name='Alice' WHERE Id=2" \
  --database contacts.accdb --output renamed.accdb

tablecraft query preview-update 'UPDATE Items SET Score=11 WHERE Id BETWEEN 1 AND 3' \
  --database offline.accdb --output scored.accdb

tablecraft query preview-update 'UPDATE Items SET [Group]=Score, Score=[Group] WHERE Id>0' \
  --database offline.accdb --output swapped.accdb

tablecraft query preview-update "UPDATE Contacts SET Name=NULL WHERE Name='Bob'" \
  --database contacts.accdb --output cleared.accdb --variable --collation ordinal
```

These examples require schemas admitted by the selected native mutation profile.
`NULL` is a nullable stored NULL; quoted text `'null'` remains literal text.
Text predicates require explicit `--collation ordinal`; no ambient locale or
native Access collation equivalence is inferred.

## Selection and assignment

The current profile permits one local table, an optional alias and one or more
SET assignments. Default metadata routing supports fixed fields together with
at most one Short Text/Binary field. Explicit `--variable` can also combine one
such field with fixed siblings under its joint DATA-placement profile. Explicit
LVAL retains exactly one target column. It reuses the existing pure scalar expression/WHERE profile:
field references, literals, unary/binary expressions, BETWEEN, IN lists and
three-valued filtering. The closed [deterministic mutation function set](../../query/MUTATION_SCALAR_FUNCTIONS.md)
adds existing conversions, eager IIf/Nz, selected text operations and numeric
functions. Only Boolean TRUE selects a row. Parameters, unlisted calls,
aggregates, subqueries, joins and external/saved targets remain refused. Syntax
and local-target shape are validated before opening files;
actual names, fields and mutation eligibility are checked against the source.

The editor captures one immutable image and one complete scalar snapshot. Target
names resolve to original column identities; repeated targets, including aliases
and case variants, are refused before WHERE or RHS evaluation. WHERE evaluates
once per original row. Every selected RHS then evaluates in written SET order
against the same unchanged row: `SET [Group]=Score, Score=[Group]` exchanges the
original values rather than reading an earlier assignment's replacement.

All selected replacements finish before one consuming backend call. Handles are
snapshot-local; the adapter recovers original expected values from those handles,
validates all groups under one aggregate assignment ceiling, and passes one
complete typed statement to one engine service. Shared NULL-mask bytes and composite
index keys are maintained together. Logical index aliases do not create separate
physical rewrites; ignored-NULL history follows the engine's original-association
law. Final Required, uniqueness and relationship checks remain engine-owned.
A late expression/carrier error produces no candidate. Even zero matches and
unchanged assignments still pass the selected native gates. Matched rows counts
logical selected rows, not assignment cells.

`EditableBatch` is the explicit neutral capability used by `plan_batch_update`.
The old single-column `plan_update` and ordinary `NativeEditor` still reject
multiple assignments. This extension neither changes their contract nor chains
one candidate into another.

The explicit `StoredScalar` profile accepts nullable NULL and exact stored
carriers under their shared type descriptors. Int32 targets retain the existing
checked Byte/Int16/Int32/Int64 assignment law, so ordinary SQL `Score=11` works.
Other types are not silently coerced. This does not broaden Variant promotion,
assignment coercions or the ordinary `ExistingInt32` carrier law.

## Native mode and publication

The default resolves the complete target set from the original snapshot and
selects one service before native planning. All-fixed targets use existing fitting
fixed/batch maintenance and retain its history policy. One Text/Binary target
uses the existing variable service without INDEX growth; its qualified DATA
placement may allocate pages. Fixed targets with exactly one Text/Binary target
use [joint fitted-record maintenance](../../testing/ACE16_FITTED_MIXED_UPDATES.md):
one original/final row projection, existing record capacity and retained INDEX
topology. There is no failed-preview retry or intermediate candidate.

Shared Text/Binary metadata selects a route; the engine still certifies actual
Short Text/Binary storage. It does not silently route Memo/OLE to the long-value
service. More than one variable target and mixed LVAL changes remain unsupported,
including when WHERE selects no rows. This routing `Default` is unrelated to
omitted INSERT fields or native `DefaultValue` evaluation.

`--index-growth` explicitly selects existing Int32 fixed INDEX allocation.
`--variable` retains its existing service for one Short Text/Binary target.
With fixed siblings it selects [joint DATA placement](../../testing/ACE16_MIXED_GROWING_UPDATES.md)
once, allowing a hidden target on the original or another owned DATA page, or
qualified represented EOF allocation. INDEX owners must still fit. For example:

```sh
tablecraft query preview-update "UPDATE Contacts SET Age=40, Name='Alice' WHERE Id=4" \
  --database contacts.accdb --output edited.accdb --variable
```

With fixed siblings, `--variable --index-growth` instead selects one
[joint DATA/INDEX continuation](../../testing/ACE16_MIXED_DATA_INDEX_GROWTH.md).
The engine uses one original source and one allocation ledger for DATA placement
and every affected physical INDEX owner. It derives each final tuple jointly;
logical aliases do not allocate independent trees. A single-variable statement
retains its existing selected INDEX-growth service. Variable mode still refuses
fixed-only groups or more than one variable target, including zero matches. Native Short
Text/Binary qualification remains inside the selected engine service; Memo/OLE
is never inferred from shared Text/Binary carriers. Default routing stays fitted
for joint fixed/variable updates; it does not retry with DATA growth.
`--long-value` selects one ordinary Memo/OLE column. `--cascade-updates` selects qualified fixed update
cascades and conflicts with variable, long-value and index-growth modes. These
explicit modes keep their existing profiles; unsupported shapes refuse without
fallback. Separate sequential previews are not an atomic mixed-update substitute.

Sources open read-only under the existing native host guard. Deployment packages
and encrypted rewrites are refused; passwords cannot authorize a plaintext
replacement of an encrypted source. One cumulative budget covers snapshot,
evaluation, engine work and publication: defaults are 256 MiB, 100,000 rows and
100,000,000 steps, adjustable with `--work-bytes`, `--work-rows`, `--work-steps`.
Native structural caps remain independent. Library callbacks support cancellation;
the CLI does not install a signal handler for this operation.

The bounded summary is prepared before publication. The common fresh-copy
publisher writes an owned same-directory temporary, syncs it and publishes with
`persist_noclobber`. Existing outputs, source aliases and symlinks are not replaced;
failed preparation/publication cleans only its owned temporary. No in-place
editing, directory durability, crash recovery or native save qualification is
claimed. A later stdout failure may leave the completed copy in place.

## Observed validation

The adapter source-only gate and five adapter functions passed their first actual
runs. The CLI source-only gate and four CLI functions also passed their first
actual runs. They exercise multirow Text/Int32/NULL changes, original-row selection,
handle identity, unchanged assignments, late/eager errors, mode refusals,
no-clobber publication and encrypted-source refusal. These source/read/selection
and no-op assertions are typed-row and identity checks, not complete independent
raw-image qualification or Microsoft Access comparisons.

Two neutral `StoredScalar` tests passed after a test-factory shadowing compile
error was corrected by renaming the factory. The first normal CLI check succeeded
with a private-interface warning, corrected by matching the error type visibility.
Both observations are retained; neither correction changed behavior or expected
values. Current broader checks are recorded below and in the owning results.

The multi-assignment continuation's source-only gate passed before its production
patch was applied. It independently qualifies six complete source/expected pairs
across both ACE headers: genuinely nullable nonprimary Group/Score fields, two
physical ordinary/ignored-NULL owners and four aliases. Expected images cover a
shared-mask NULL update with opaque payload retention, simultaneous partial-tuple
exchange, and an original omitted row introducing one historical family. The new
neutral batch tests passed their first actual run: seven functions, including
exact Bytes/Steps and final-cancellation bounds. Nine existing neutral edit tests
also passed. All 36 adapter library tests passed, including the batch API and
source controls. The two CLI batch functions passed on their first run, covering
six complete expected outputs and typed refusals; 31 CLI unit tests and the four
new SQL DELETE functions also passed. These literal expectations are separate
from the older typed-row tests and do not qualify native Access saves.

The combined CLI regression then stopped at an older pre-open test that treated
all multiple assignments as unsupported syntax. That obsolete expectation was
removed from its missing-source list: actual fixed-field batch positives are in
the new CLI tests. At that checkpoint its mixed fixed/text request remained an actual-source
refusal. The metadata-routing continuation below admits qualifying joint fitted
requests; the old statement is retained as an explicit fixed INDEX-growth refusal. Parameters/external
syntax, late errors and source preservation controls remain. The first failed
regression log and predecessor test are retained. The corrected CLI regression
passed: 31 unit functions and 20 integration functions, including the prior four SQL UPDATE, four normal Jet 4 UPDATE and six
typed write workflow functions. No producer or expected-image bytes changed to
accommodate the test.

Final strict checks for five packages passed on retry after an unrelated DELETE
test-only loop was changed to enumerate; four library minimal-feature checks,
formatting, all 6,446 documentation links and 15 dependency-boundary checks also
passed. The two focused DELETE relationship tests passed again. These checks
establish the recorded local engineering behavior, not Microsoft Access write or
save compatibility.

The [ordinary query editor](QUERY_EDITING.md) and its ACE refusal remain distinct.
[Explicit typed engineering commands](ENGINEERING_COPIES.md) expose INSERT,
DELETE and direct expected-value UPDATE requests without SQL selection.

## Metadata-routing continuation

Both frontend source gates passed before the new API checks. They reuse 39 original
engine source/expected pairs and four zero-slack raw one-character pairs without
changing their constructors or expected bytes. Typed adapter controls include
non-NULL Binary, shared NULL masks, both group orders and explicit-mode refusal;
SQL/CLI Text consumers compare complete frozen images. Single-variable and
all-fixed route-consistency checks are separate from those independent goldens.
The adapter's six focused functions passed on their first run, including 86 typed
whole-image comparisons in both group orders and eight SQL Text comparisons.
The CLI's four focused functions also passed on their first run, including eight
fresh copies matching the complete frozen images and source preservation. Its
first build warned that the shared Case.family field was unread in the CLI
consumer; the test now qualifies each case using that actual field rather than
the loop variable. Constructor and expected bytes did not change. The complete Access library
regression passed 48 functions; CLI regressions passed 31 unit and 23 integration
functions. Before the later routing-boundary test additions, strict ACE/Access/CLI
all-target/all-feature checks, four minimal libraries, two historical and four
normal Jet 4 CLI regressions, dependency boundaries and documentation links all
passed. The new genuine Memo/OLE source gate passed before routing tests. The old
long-value contract passed 15 functions after its extracted test module needed
an explicit path; all 22 Access editor functions then passed, including actual
Memo/OLE single/joint zero/nonzero refusal, shared stale handles, and cumulative
Default capture-through-finish exact pools, one-below pools and final cancellation.
All-target strict compilation subsequently exposed ambiguous source-helper glob
imports in another existing long-value test consumer. Only import ownership was
corrected: all 25 original fixture functions remain text-exact. Five existing source-owner integration targets then passed all 36 functions.
A further strict attempt found remaining nested private-helper references and
one unused local import; these were corrected to name the actual source owner.
Final all-target ACE/Access/CLI strict checks and scoped formatting of 23 Rust
files passed. The complete 48-function Access run predates these added boundary
tests; the later Access editor subset passed 22. No newer full-library total is
claimed. All three test-only compilation failures and corrections remain recorded. These checks do not establish Microsoft Access
write/save qualification.

## Explicit variable batch DATA placement

The engine, Access frontend and CLI source gates each qualified all 36 original
source/expected pairs before new frontend APIs. The actual pure constructor now
lives in `mixed_fitted_tests/growing/model.rs`; original constructor bodies and
expected bytes remain unchanged, and old engine gate/API consumers use that
actual owner. The first CLI source compile failed because its path-imported test
module needed an explicit child-module path. That path-only correction preceded
its successful source gate and every new frontend API.

The four Access controls passed on their first API run, including 72 whole-image
comparisons in both group orders, all three DATA destinations and non-NULL Binary.
Three CLI controls passed first, including 24 complete Text copies, source
preservation, fitted-default refusal and mixed INDEX-growth/multiple-variable
no-output refusals. At that checkpoint, the zero-match mixed INDEX-growth adapter control used a
zero-Bytes planning budget and received the then-current mode refusal. The joint
INDEX continuation below replaces that obsolete blanket refusal with a paid
request-allocation control. The targeted old/new native fixture regression passed all
16 functions, the Access library passed 55, and CLI regressions passed 31 unit
plus 36 integration functions (six DELETE, two historical, nine INSERT, fifteen
UPDATE and four normal Jet 4). Strict ACE/Access/CLI all-target/all-feature checks
and minimal Access checks also passed. These are scoped local engineering-copy
checks, not a full ACE library run or native Access save qualification.


## Explicit joint DATA and INDEX growth

The engine, Access frontend and CLI each passed a source-only gate over all 24
original/complete-expected pairs before frontend API execution. They use the
actual native `joint_split_tests/model.rs` literal owner and its four pure
children. Only test visibility and explicit module paths changed; reversing
those changes restores the original constructor file exactly. No candidate or
public AFTER image supplies expected bytes.

The qualified literal matrix covers both headers, Legacy/Modern Text and Binary,
both key directions, root-leaf growth and retained nonroot leaf growth. The
source histories remain 701 and 901; logical aliases share physical owners.
Default metadata routing remains fitted, `--variable` alone retains its prior
DATA-growth path with fitting indexes, and the old one-variable route remains
unchanged. Only the deliberately obsolete mixed-INDEX refusal controls migrate;
multiple variable fields, mixed LVAL and source/constraint failures still refuse.

The corrected all-features adapter API run passes 48 complete-image comparisons
in both group orders. Its initial command omitted the feature and ran zero tests;
that invocation is retained and excluded from passing API evidence. The CLI's
first focused run passes four functions, including 16 exact SQL Text copies and
24 exact typed copies with eight Binary cases. Stale expected values, existing
destinations and source aliases leave all original files unchanged. The full
adapter library passes 57 functions; the selected CLI round passes 37 unit and
29 integration functions (23 UPDATE and six typed CRUD). Strict ACE/Access/CLI
all-target/all-feature checks and minimal Access checks pass. See the
[frontend record](../../testing/results/mixed-data-index-growth-copies-2026-10-30.json).
These are experimental engineering copies, not native Access save qualification.
