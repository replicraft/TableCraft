# Fresh native database creation

The optional `tablecraft-access/ace16-create` feature composes the
[native Int32 creator](../../testing/ACE16_DATABASE_CREATION.md) with required
fresh-file publication. It accepts one explicit user-table name, ordered
ordinary Int32 column names and borrowed present Int32 rows. It accepts no
database template, imported image or engineering candidate.

`CreationDestination::capture(path, budget, cancelled)` holds the output's actual
parent directory and preflights its barrier without creating a temporary or final
entry. The host maintains the namespace preconditions of the
[IO publication service](../../foundation/IO.md#explicit-required-fresh-copy-publication).
Unsupported required-durability backends refuse before staging; no default
publisher is tried afterward.

`destination.prepare(table, columns, rows, budget, cancelled)` creates the complete
sealed native artifact. Invalid names, row shapes, carriers or physical fit
produce no output entry. The returned `PreparedCreation` exposes `row_count()`
and `column_count()`, allowing the host to allocate and format its full completion
report before publishing.

`prepared.publish(budget, cancelled)` consumes the artifact and selected output
once. IO stages and synchronizes its bytes, publishes without clobbering and
finishes the captured-parent barrier. `CreationReceipt` retains the counts,
`namespace_synced()` and captured `parent_identity()`. Directory identity is
independent of later textual path resolution; actual filesystem/device barrier
compliance remains outside the acknowledgement itself.

`CreationError::Create` preserves the typed native input/resource failure.
`CreationError::Publication` preserves IO's `NotPublished`, `VisibilityUncertain`,
`PublishedUnconfirmed` and `PublishedCleanupFailure` facts. A pre-existing
destination may exist even after `NotPublished`. Other failures can retain a
visible final file or an owned temporary. The service neither retries publication
nor deletes a final entry to resolve uncertainty.

The CLI's [create command](../../cli/NATIVE_CREATION.md) consumes this shared
service. It finishes input parsing and complete report preparation before
publication. Report write/flush failure retains the successful receipt and
returns failure; a failed diagnostic cannot invoke read-only BrokenPipe success
handling. Libraries do not publish reports themselves.

The current schema is one table, 1–16 ordinary Int32 columns and at most
64 rows of present values, with a separate single-DATA-page fit check. NULL input
is refused; no Required property or NOT NULL constraint is authored. Columns are
manual values;
there is no implicit AutoNumber, primary index, default or constraint authoring.
The engine owns native eligibility and exact bytes. Shared work budgets and
cancellation pass through construction and publication.

This service creates an unencrypted fresh file. It grants no normal update,
macro-absence or recovery permission for arbitrary existing ACE files. Native
Access creation/save compatibility and broader schemas remain unqualified.
Existing [normal-write gates](NATIVE_WRITES.md) are unchanged.

The [local acceptance record](../../testing/results/fresh-native-creation-host-2026-10-31.json)
keeps complete-file checks, publication controls, process observations and
unqualified native Access behavior separate.
