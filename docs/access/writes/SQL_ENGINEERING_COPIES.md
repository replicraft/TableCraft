# SQL-selected engineering copies

`tablecraft query preview-update` evaluates a bounded SQL UPDATE against one
original native snapshot, then prepares one fresh experimental engineering copy.
It uses `tablecraft_access::engineering::sql` and `AccessEngineeringEditor` over
shared DB/query contracts. It does not enable normal ACE saves, certify data-macro
absence, execute active content or produce a journalable `NativeEditPlan`.

```sh
tablecraft query preview-update 'UPDATE Contacts SET Name=Name & " Jr" WHERE Id=2' \
  --database contacts.accdb --output edited.accdb --variable --index-growth

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
SET assignments in fixed-field modes. Variable and LVAL modes retain exactly one
target column. It reuses the existing pure scalar expression/WHERE profile:
field references, literals, unary/binary expressions, BETWEEN, IN lists and
three-valued filtering. Only Boolean TRUE selects a row. Parameters, calls,
aggregates, subqueries, joins and external/saved targets remain refused. Syntax and local-target shape are validated before opening files;
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
complete fixed-column batch to the engine. Shared NULL-mask bytes and composite
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
functions or the ordinary `ExistingInt32` editor.

## Native mode and publication

The default selects existing fitting fixed-field maintenance. `--index-growth`
selects the existing Int32 fixed INDEX allocation profile. `--variable` selects
one Short Text/Binary column and can combine with `--index-growth` for its existing
variable/index service. `--long-value` selects one ordinary Memo/OLE column.
`--cascade-updates` explicitly selects qualified fixed update cascades and conflicts
with variable, long-value and index-growth modes. A multi-column variable/LVAL
request refuses, including when WHERE selects no rows. Atomic multi-column
variable/LVAL maintenance remains a backend gap; separate sequential previews
are not a substitute. Other unsupported shapes refuse with no fallback.

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
the new CLI tests. Its mixed fixed/text request remains an actual-source refusal,
because no single current native mode admits that mixture. Parameters/external
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
