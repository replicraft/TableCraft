# Joined action query planning

`tablecraft-query` supplies explicit joined UPDATE preparation over a frozen
editable snapshot and a separately supplied read source. This is a working query
planner whose qualified native adapter/runtime route now feeds the existing
consuming native save owner. Normal GUI Run uses that issued review and explicit
confirmation. This is not automatic write admission for arbitrary joined reads.
Joined DELETE remains unsupported.

Microsoft documents joined sources in its [update-query workflow](https://support.microsoft.com/en-us/access/create-and-run-an-update-query).
Updateability and native constraints remain separate from a join's read results.
This implementation admits a restricted profile rather than claiming every
Access updateable query or ambiguous-match behavior.

## Target identity and evaluation

`JoinedMutationOptions` identifies the exact original `SnapshotId` and written
target qualifier or alias. Empty snapshots have real identities without invented
row handles. The selected occurrence's rows and handles come from the editor's
snapshot. Other occurrences, including the other side of a self-join, use the
read source; equal cell values never establish target identity.

The planner admits scalar local tables, INNER/LEFT/RIGHT joins and comma products.
All SET targets must resolve to the selected occurrence, with no duplicate target
column. Derived, external and collection relations remain refused. The existing
deterministic mutation expression profile and explicit parameter/comparison
contracts apply. Captured schema facts are checked before row evaluation; the
ordinary source stability contract remains in force for the supplied reader.

Outer-join absence retains an absent target handle independently of nullable
cells. Such a frame cannot invent a target row. Qualifying assignments use
unchanged original values, and staged updates reach the backend in original
target order rather than joined result order.

Multiple qualifying join matches for one target row remain explicitly unsupported.
The planner does not choose a first or last replacement. It evaluates selected
expressions before refusing repeated targets, preserving late eager failures.
No backend finish occurs on expression, ambiguity, schema, cancellation or budget
failure.

## Two-phase ownership

`prepare_joined_batch_update_with_options` returns an owned
`PreparedJoinedUpdate<E>` containing the editor and complete staged replacements.
It retains no read-source or budget borrow. `finish` consumes that owner and uses
the existing checked batch finish under the caller's backend budget. The
convenience `plan_joined_batch_update_with_options` delegates through these same
two phases.

This permits a native host to release its read/source budget borrow before
borrowing that pool for native planning. Preparation does not reset either pool
or persist a partial result. Native source proof, constraints, programs, original
intent and journal/recovery behavior remain the backend's responsibility.

Focused tests cover actual target identity in self-joins and empty snapshots,
both outer directions, source drift before scanning, original target ordering,
ambiguous matches and eager errors, parameters, cumulative work and shared-budget
phase separation. Local tests do not establish Windows or Microsoft Access parity.

## Native source integration and correlated predicates

The native adapter pairs the editor and predicate reader with one immutable
original image. Read callbacks share the bounded native pool only while complete
selection/assignment staging runs; no mutable native-budget borrow survives into
those callbacks. The consuming native finish reacquires that pool afterward and
retains the original program, constraint, ownership and byte-preservation checks.
Late errors and repeated UPDATE targets produce no reviewed save capability.

Local UPDATE also admits this explicit predicate-read profile through
`prepare_read_source_batch_update_with_options` and
`PreparedReadSourceUpdate::finish`. Its `ReadSourceUpdateOptions` binds the
original `SnapshotId` and existing mutation policies. Captured target scalar
columns and text-comparison policy identities must match the bound source before
predicate evaluation. Outer target values and every SET expression use the
original snapshot frame; read-side duplicates inside EXISTS/IN do not duplicate
that target. NULL NOT IN can select no rows; the query layer still invokes backend finish.
For actual ordinary unindexed LONG targets in the complete retained source,
the adapter selects the separate TCNI24 zero-assignment service before native
planning. It retains named target groups and full source/target proof with
changed rows zero and every AFTER byte equal to BEFORE. TCNI17 retains its
single-cell language; no arbitrary row is synthesized as a matched no-op.
Broader zero-assignment scalar/opaque source profiles remain separately qualified. Scalar or quantified subqueries in SET remain
refused, without retrying a different planner.

Microsoft explicitly permits UPDATE predicates with subqueries in its
[SQL subquery contract](https://learn.microsoft.com/en-us/office/client-developer/access/desktop-database-reference/sql-subqueries-microsoft-access-sql).
This implementation retains the closed local scalar read context below; that
reference does not qualify arbitrary Access updateability or native writer types.

A separate local DELETE profile admits direct scalar EXISTS/IN SELECT predicates
with existing correlation and NULL semantics. Selected handles always come from
the original editor snapshot, not equal read-side cell values. Syntax chooses this
mode before preparation; a failed legacy planner is never retried through it.
Scalar/quantified subqueries, aggregate/grouped/ordered or external/named/collection
read contexts remain refused in this profile. This does not establish joined
DELETE or ambiguous-match/UniqueRecords behavior.

The GUI keeps the actual runtime-issued capability through No/Yes, save and ACK;
SQL is not reevaluated while saving. Headless cases preserve independent complete
AFTER images and sibling data, stale-source refusals and prior results on errors.
See the [SQL workspace](../ui/SQL_WORKSPACE.md#reviewed-local-action-queries).

## Local verification

The correlated-predicate Query gate passed 1,904 tests across 272 harness summaries
(including 167 library tests and one rustdoc test) before the empty native action
was added; that unchanged query graph was not rerun for the TCNI24 phase. The
preceding Desktop action/parameter gate passed 32 tests.

The TCNI24 phase passed 62 selected native tests, four journal-only tests (three
data cases and one typed constructive refusal), 52 Access tests, 397 Runtime tests
with one ignored, and 33 Desktop action/parameter tests. Strict lint checks for
eight packages and no-create/minimal feature checks passed. These are local and
headless results with independent raw images, not Microsoft Access or Windows
execution/save parity evidence.
