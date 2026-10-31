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

The separate [`query preview-before-change`](../../cli/BEFORE_CHANGE_COPY.md)
command invokes an explicitly selected qualified BeforeChange property subset
through the shared service before creating a fresh fitting engineering copy.
Its caller-selected order does not certify native dispatch, required-program
coverage or normal saves. Ordinary `preview-update` retains its active-content
refusal.

## Selection and assignment

The current profile permits one local table, an optional alias and one or more
SET assignments. Default metadata routing supports multiple Short Text/Binary
fields with optional fixed siblings in one fitted row. Explicit `--variable`
selects their joint DATA-placement profile. Explicit
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
selects one service before native planning. A qualified mixed-schema projection
with one ordinary, unindexed Short Text target uses the
[preserved fitting service](../../testing/ACE16_PRESERVED_SHORT_TEXT_UPDATES.md)
under Default mode, retaining opaque companions and original record capacity.
Explicit `--variable` selects the separate
[preserved Short Text growth service](../../testing/ACE16_PRESERVED_TEXT_GROWTH.md)
for one ordinary, unindexed target with no calculated dependency. It preserves
the original logical row reference while placing the replacement on the original
or another owned DATA page, or allocating a represented EOF DATA page. Default
remains fitting; neither mode retries a failed candidate. INDEX growth,
map-storage initialization, larger-image modes, Memo/OLE targets and mixed-target
requests remain refused for this rich-schema Text profile. It grants no normal
save or required-macro bypass. All-fixed targets use existing fitting
fixed/batch maintenance and retain its history policy. For one Text/Binary target,
the retained native declaration selects the existing ordinary Memo/OLE service
when it matches that engine storage shape. Otherwise it uses the existing variable
service without INDEX growth. Their qualified DATA placement may allocate pages. Multiple variable targets, or variable targets
with fixed siblings, use [joint fitted-record maintenance](../../testing/ACE16_MULTIPLE_VARIABLE_UPDATES.md):
one original/final row projection, existing record capacity and retained INDEX
topology. There is no failed-preview retry or intermediate candidate.

Shared Text/Binary metadata alone does not establish native storage kind. The
single-column Memo/OLE selection uses the engine's ordinary LVAL declaration
predicate; ownership, properties, constraints and payloads still require the
selected engine service's checks. Multiple-column LVAL changes remain unsupported,
including when WHERE selects no rows. Projected rich schemas keep their separate
qualified Boolean, nullable Int32 and single Short Text preservation profiles;
these do not admit arbitrary rich-schema fields. This routing `Default` is unrelated to
omitted INSERT fields or native `DefaultValue` evaluation.

`--index-growth` explicitly selects existing Int32 fixed INDEX allocation.
`--variable` retains its existing service for one Short Text/Binary target.
With multiple variable targets or fixed siblings it selects [joint DATA placement](../../testing/ACE16_MIXED_GROWING_UPDATES.md)
once, allowing a hidden target on the original or another owned DATA page, or
qualified represented EOF allocation. INDEX owners must still fit. For example:

```sh
tablecraft query preview-update "UPDATE Contacts SET Age=40, Name='Alice' WHERE Id=4" \
  --database contacts.accdb --output edited.accdb --variable
```

With multiple variable targets or fixed siblings, `--variable --index-growth`
instead selects one [joint DATA/INDEX continuation](../../testing/ACE16_MIXED_DATA_INDEX_GROWTH.md).
The engine uses one original source and one allocation ledger for DATA placement
and every affected physical INDEX owner. It derives each final tuple jointly;
logical aliases do not allocate independent trees. A single-variable statement
retains its existing selected INDEX-growth service. Variable mode still refuses
fixed-only groups, including zero matches. Native Short Text/Binary qualification remains inside the selected engine service; Memo/OLE
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

## Explicit larger fixed-update copies

`--image-bytes N` selects only the existing fixed fitting UPDATE profile under a
validated, retained complete-image ceiling, currently at most 80 MiB. Without
this flag, capture retains its 64 MiB ceiling and current Default metadata routing.
The flag does not resize a file or authorize variable/LVAL edits, INDEX growth,
cascades, normal saves or data macros. Those mode flags conflict before opening
the source; nonfixed target columns still require actual source metadata and
refuse even when WHERE selects no rows. No failed preview retries another mode.

Image retention and cumulative operation work are separate. Defaults remain
256 MiB Bytes, 100,000 Rows and 100,000,000 Steps; selecting a larger image does
not raise them. For the qualified 80 MiB fixed-copy profile, this explicit example
provides independent byte and step allowances:

```sh
tablecraft query preview-update 'UPDATE Items SET [Score]=11 WHERE [Id]=1' \
  --database offline.accdb --output fixed.accdb --image-bytes 83886080 \
  --work-bytes 2147483648 --work-rows 100000 --work-steps 1000000000
```

The host captures the original once through `capture_for_fixed_updates`, passes
it to the existing SQL batch editor in `Fixed(Fitted)` mode, and uses the common
fresh-copy publisher. Actual native field, index, ownership and preservation
checks remain mandatory. See the [native image contract](../../testing/ACE16_LARGER_FIXED_IMAGES.md)
for its separately recorded limits and native qualification.

The inert CLI source gate qualified twelve source/expected pairs, including ten
native-eligible recipes and two header-5 rich recipes that remain refused. A
separate ordinary Memo source passed metadata/constraint/LVAL reads before the
host change. The first host API round passed five functions: ten complete frozen
copies, ten occupied-output controls, default/one-below/independent-budget
refusals, four conflicting modes and four invalid limits before opening a source,
two header-5 rich refusals, and a small Memo zero-match contrast. Default routing
publishes that Memo source unchanged; the explicit fixed profile refuses it.
Source and expected constructors remain unchanged. Affected regressions pass
48 CLI unit and 46 integration functions, and all-target/all-feature CLI strict
checks pass. This is authored engineering-copy evidence, not a native Access
save comparison or a general larger-write claim.

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
request-allocation control. The later [multiple-variable extension](../../testing/ACE16_MULTIPLE_VARIABLE_UPDATES.md)
also replaces the historical multiple-variable refusal. The targeted old/new native fixture regression passed all
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

## Default ordinary Memo/OLE routing evidence

One-column SQL updates to qualified ordinary Memo/OLE fields now use the existing
LVAL service without requiring `--long-value`. Explicit modes retain their prior
profiles. The adapter moves the native schema into its captured state without
cloning it, selects one service before planning and never retries a failed preview.
This grants no normal-save or data-macro authority.

Before producer installation, Access and CLI each qualified six source/expected
pairs, and the reused old direct-engine recipe passed. The first Access positive
run exposed an authored OLE expectation that compacted a 39-byte root into a
40-byte record. The existing stable-root contract instead preserves its original
capacity and appends a hidden forwarding target. That failed compacting expectation
remains unchanged in its original owner. A separate literal forwarding expectation
was then source-qualified in both frontends before API replay; no routing or
encoder change occurred during this correction.

The corrected Access run passed two functions with twelve complete default/explicit
images and six empty-batch copies. The first CLI run passed two functions with
eight exact SQL copies, eight existing-output refusals and three native-profile
no-output refusals. The existing routing boundary filter passed three functions,
including unchanged joint-LVAL refusal. Broader checks passed 15 native LVAL and
41 Extra functions, Access 63 library plus four LVAL and two preserved integration
functions, and CLI 43 unit plus 107 integration functions. Minimal ACE/Access,
formatting, documentation and dependency boundaries passed. Strict compilation
exposed a test-only reuse-context name collision in the extracted recipe; naming
its actual `source::source` owner fixes that ambiguity with an exact inverse check
of the remaining constructor bytes. The affected ordinary/mixed LVAL contracts
then passed 15 and eight functions, and Access/CLI LVAL targets passed four each.
The strict retry found two obsolete wrapper imports; removing only those imports
allowed final all-target/all-feature ACE/Access/CLI strict checks and formatting
to pass. Both compile-only hygiene failures remain recorded separately from the
earlier authored expectation failure; no production or encoding policy changed.

## Explicit secondary-index creation

`query preview-create-index` lowers one owned CREATE INDEX definition into the
Access engineering service and publishes a fresh experimental copy. It adds one
ordinary nonunique single-Int32 secondary index, including NULL entries, on a
previously unindexed field:

```sh
tablecraft query preview-create-index \
  'CREATE INDEX ScoreLookup ON Items (Score DESC)' \
  --database offline.accdb --output indexed.accdb
```

The existing SQL parser owns the statement. The adapter borrows its original
index, table and field names without SQL interpolation. One field and optional
ASC/DESC are accepted; UNIQUE, WITH policies, parameters and other statement
kinds refuse before source open. Native qualification owns actual names,
metadata, properties, constraints and complete source ownership. Parser
acceptance alone does not establish write eligibility.

The native profile requires a qualified ordinary fixed Int32 table, supported
existing single-column indexes, a single-page definition and represented inline
allocation maps. New index names are ASCII, at most 64 bytes, beginning with a
letter and containing only letters, digits or underscore. Wider fields, existing
selected-field indexes, unique/primary/foreign policies, larger tree profiles
and unsupported properties remain refused. The command uses Standard complete
capture, retaining its 64 MiB ceiling; there is no `--image-bytes` mode here.

The service returns one non-journalable `EngineeringCopy` with complete before
and candidate bytes. Its `created_index()` metadata reports the native-audited
name, separate logical and physical numbers and indexed source-row count.
`affected_rows()` is zero: index population does not modify data rows. The CLI
prints only that audited ASCII name and numeric metadata after successful
publication. It performs no row expression evaluation and no failed-preview
fallback.

The command retains the read-only source guard, refuses encrypted/deployment
rewrite profiles, formats the complete report before publication and uses the
shared synced temporary/no-clobber publisher. Existing outputs, source aliases
and symlinks are not replaced. It does not claim directory durability, normal
ACE save permission, required data-macro qualification or in-place editing.
Defaults remain 256 MiB cumulative work bytes, 100,000 row work and 100,000,000
steps; the separate Standard image ceiling still applies.

Explicit `--durable-output` selects the
[required captured-directory publication path](../../cli/DURABLE_OUTPUT.md)
before source I/O, after closed CREATE INDEX shape validation. All report
allocation and fallible source release complete before either publication mode;
this corrects the earlier silent source Drop ordering. Default publication
contents and metadata reports otherwise stay unchanged. Required success adds a
captured-directory barrier acknowledgement; uncertain or visible failures can
retain the final copy and never trigger fallback, retry or final-file deletion.
This option changes filesystem publication only, not index admission or normal
native-save/macro authority.

The [native creation contract](../../testing/ACE16_SECONDARY_INDEX_CREATION.md)
and [host result](../../testing/results/secondary-index-sql-copies-2026-10-30.json)
separate literal engineering checks from native Access conformance. The shared
pure source owner preserves eight original images and sixteen complete expected
images across both qualified headers, both directions and four populations.
Indexed-row counts 0/3/6/600 remain distinct from fresh family counters
0/3/4/541. Existing aliases, counters, Sentinel and unrelated bytes are preserved.

The normal CLI check and three Access lowering/three CLI preopen tests pass
after extraction-related test-import corrections. Both initial compile-only
failures are retained. Separate Access and CLI SOURCE gates each pass one
function covering all sixteen source/expected pairs before host API calls.
The first complete Access target passes four functions, including sixteen exact
SQL-service images, one typed-request parity image, native-profile refusals and
cumulative capture/lowering/finish budget controls. The first CLI target passes
three functions, including sixteen exact fresh copies, sixteen occupied-output
refusals and native-profile/source-alias no-output controls. Index metadata and
unchanged original files are checked independently. An additive guard SOURCE
test then qualifies a supported encrypted reader and both stored/MSZIP deployment
envelopes before the new guard API test. Its four calls refuse encrypted input
with and without the correct password and both deployment profiles, preserving
the original envelope bytes with no output or temporary artifact. These are
repository-authored envelopes, not native Access encryption/signature evidence.

Native all-target strict checks pass. The first broader host strict compilation
stopped on eleven old fixture-import issues exposed by shared source extraction;
those diagnostics are retained separately from the passing new host tests.
After those import corrections, final Access/CLI all-target/all-feature strict
checks and minimal Access pass. Broader regressions pass 67 Access library
functions, 51 CLI unit functions and 46 existing CLI integration functions
(four preserved-Boolean, four Memo/OLE, 27 SQL UPDATE, four Jet 4 UPDATE and seven
typed-write functions). No new host production changes were needed after the
passing API checks. Final repository formatting, links and dependency boundaries
are checked by the commit owner after this documentation freeze.
