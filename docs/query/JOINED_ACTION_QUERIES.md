# Joined action query planning

`tablecraft-query` supplies explicit joined UPDATE preparation over a frozen
editable snapshot and a separately supplied read source. This is a working query
planner, not automatic native write admission or GUI execution. The normal native
SQL action route still requires integration with this preparation API. Joined
DELETE remains unsupported.

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
