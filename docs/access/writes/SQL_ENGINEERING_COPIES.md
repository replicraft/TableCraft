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

tablecraft query preview-update "UPDATE Contacts SET Name=NULL WHERE Name='Bob'" \
  --database contacts.accdb --output cleared.accdb --variable --collation ordinal
```

These examples require schemas admitted by the selected native mutation profile.
`NULL` is a nullable stored NULL; quoted text `'null'` remains literal text.
Text predicates require explicit `--collation ordinal`; no ambient locale or
native Access collation equivalence is inferred.

## Selection and assignment

The current profile permits one local table, an optional alias and exactly one
SET assignment. It reuses the existing pure scalar expression/WHERE profile:
field references, literals, unary/binary expressions, BETWEEN, IN lists and
three-valued filtering. Only Boolean TRUE selects a row. Parameters, calls,
aggregates, subqueries, joins, external/saved targets and multiple assignments
remain refused. Syntax and local-target shape are validated before opening files;
actual names, fields and mutation eligibility are checked against the source.

The editor captures one immutable image and one complete scalar snapshot. All
selected replacements are evaluated against original rows before one consuming
backend finish. Handles are snapshot-local; the adapter recovers original expected
values from those handles and passes typed requests to the engine. A late
expression/carrier error produces no candidate. Even zero matches and unchanged
assignments still pass the selected native gates.

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
with variable, long-value and index-growth modes. Unsupported shapes refuse;
there is no fallback or sequential candidate chaining.

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
values. Broader final checks remain separately reported by the owning results.

The [ordinary query editor](QUERY_EDITING.md) and its ACE refusal remain distinct.
[Explicit typed engineering commands](ENGINEERING_COPIES.md) expose INSERT,
DELETE and direct expected-value UPDATE requests without SQL selection.
