# Atomic DELETE and SET NULL engineering previews

Microsoft's [Access CONSTRAINT contract](https://support.microsoft.com/en-us/access/constraint-clause)
defines `ON DELETE SET NULL`: deleting a parent sets corresponding foreign keys
in surviving child rows to NULL. This is separate from VBA and from recursive
self/cyclic cascade behavior. The contract also describes UPDATE SET NULL;
that action is not admitted by this delete continuation.

This work extends the separate `preview_ace16_cascading_deletes` engineering
service. It does not authorize normal ACE saves, journal application or execution
of required data macros. Independent byte and reader checks remain separate from
native Access save/reopen qualification.

## Original associations and one final image

The service captures complete original tables and relationship metadata before
expansion. Explicit row requests retain exact raw before-images, including when
another seed would also delete the row. Cascade closure uses original parent/child
associations. SET NULL assignments then target surviving original rows; a deleted
child does not also receive a NULL edit. Repeated assignments to the same cell
coalesce. Foreign-key matching never uses keys changed earlier in the operation.

Only checked cross-table enforced actions are eligible. Self references and
cyclic DELETE action components remain refused. DELETE lookup requires a genuine required
primary key with a qualified matching key profile. Unknown rules, unchecked
constraints and active content keep their existing write refusals.
The [downstream UPDATE continuation](ACE16_DELETE_NULL_UPDATE_CASCADES.md) now
propagates effective surviving NULLs through established UPDATE CASCADE declarations
using original associations and checked nullable UNIQUE parents. Its UPDATE-only
cycles and Restrict policy have separate engineering checks; stored UPDATE SET NULL
encoding and native SQL action timing remain unqualified.

NULL transitions preserve opaque fixed payload bytes and clear only the stored
presence bits. Different columns sharing one mask byte compose from the same
original byte; structural tombstone/count patches retain structural validation.
Required fields and indexes must still reject a surviving NULL assignment.

Each physical index receives one combined transform for removed rows and changed
surviving keys, regardless of logical aliases. Existing NULL-membership policies,
ordering, uniqueness, page framing and complete final constraints apply. A NULL
row omitted by an Ignore NULL index remains a surviving table row. Retirement and
global-map changes compose once across tables. `deleted_rows()` counts removed
logical roots and excludes surviving NULL edits.
The transform retains existing index storage and partitions final sorted keys by
original surviving leaf capacity. It rebuilds parent bounds and composes actual
retirements when eligible keys disappear. If prefix loss makes a retained page
overflow, the entire candidate refuses; this continuation adds no index allocation.

## Independent acceptance corpus

The repository-authored `ace16_set_null_delete_contract` constructs literal
source and complete expected images without mutation planners or encoders. Its
read-only source/expected gate runs before the first mutation API call. Cases
cover both ACE 16 header variants, nullable integral foreign keys, reversed
two-component keys sharing one mask byte, unchanged NULL children, logical index
aliases and mixed explicit child deletion with parent cascade actions.
Additional literal variants cover Ignore NULL index omission and movement of NULL
keys across two existing leaves with both parent bounds rebuilt.

Expected images preserve every unrelated byte, including inactive index tails,
old fixed payloads, allocation maps, historical counters and unrelated tables.
Separate controls cover stale explicit children, empty seeds, incoming-only
deletion, final Restrict membership, resource exhaustion and cancellation.

Three read-only source/expected test functions pass before mutation calls. The
initial alternate-parent checkpoint refused a surviving referenced-key NULL with
`ConstraintCoverage`, while empty seeds and deleted children remained no-ops.
That deliberate refusal is superseded by the separate downstream continuation
above, with complete positive source/expected controls.
That fixture initially lacked an ordinary UNIQUE alias; stronger relationship
coverage assertions exposed the gap before it could count as qualified evidence.
Adding the real alias corrected the source without changing production gates or
expected action outcomes. The failed fixture and diagnostic compile attempt are
retained separately.

All thirteen integration functions pass, including complete expected-image
comparisons. Four shared-patch controls and 76 existing regression functions pass.
The initial production compile found three missing limit-error conversions;
correcting their typed error mapping changed no expected images. Minimal build
warnings exposed a missing preview feature gate on the extracted patch module;
the gate correction keeps the ordinary library free of unused preview code.

The full engine library passes 617 functions with two existing generators ignored.
Seven additional direct planner controls pass separately after their source/expected
gate: seven real fixed-key carriers, both headers and directions, Ignore NULL
membership, required-before-omission, stale/conflicting intents, empty eligible
streams, prefix-loss overflow and measured terminal cancellation. Their first
compile exhausted disk space before tests; the log is retained and generated
build artifacts were reclaimed before the passing execution. Test-only lint
corrections changed no constructed database or expected image bytes.

Strict Clippy across all engine targets/features, strict minimal-library Clippy,
workspace formatting and documentation/dependency guards pass. The
[verification record](results/ace16-set-null-delete-preview-2026-10-28.json)
retains scoped source hashes, passing groups and earlier failed attempts. This
keeps the separate original-file study below distinct from Windows Access execution
and normal saves.


## Original-file DELETE and SET NULL comparison

The [maintained original-file proof](ace16_set_null_deletes/README.md) now passes
on two immutable public-created V2016 originals. SOURCE5 capture is followed by
two saved raw gates that bind genuine required parent primaries, reciprocal
foreign indexes and raw SET NULL/CASCADE relationship actions. Two complete
SOURCE/request-only images are frozen before public AFTERs, the committed producer
build or native candidates. Two inert expected-image reads pass before REF4.

Public REF4 passes both transitions. Each public file differs from its earlier
engineering image in five bytes: two newly NULL fixed-payload low bytes and
three vacated Child FK leaf wire bytes. These are observed representation
choices, not model corrections. Both native candidates from the fresh isolated
`13c075bb` build match the earlier complete engineering files exactly, followed
by two separately bound fresh reader proofs. Across the two cases, three logical
roots are deleted, four surviving FK cells become NULL, and the final snapshots
contain 18 user rows and 30 physical key/ROW pairs. Maps, historical counters,
unrelated rows and Sentinel remain unchanged.

The first pure saved gate and first constructor failed before source qualification
and image publication respectively; their programs/freezes and correction notes
remain retained. Root tool output recorded the exceptions, with no standalone
traceback file retained. Saved default replay passes with subprocess creation
blocked; optional Git metadata mode passes, and missing evidence returns status
2. The [result](results/ace16-set-null-delete-originals-2026-10-28.json) and
[build receipt](results/ace16-set-null-delete-example-build-2026-10-28.json) keep
those histories separate from the first passing runtime phases. The public
reference uses one parent-delete call per original and does not qualify a
multi-call transaction, crash recovery, data macros or native Access save/reopen.
