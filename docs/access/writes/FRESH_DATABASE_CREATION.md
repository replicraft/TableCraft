# Fresh native database creation

The optional `tablecraft-access/ace16-create` feature composes the
[native creator](../../testing/ACE16_DATABASE_CREATION.md) with required
fresh-file publication. The Int32 convenience and typed fixed-scalar path accept
explicit user schemas and borrowed rows. Neither accepts a database template,
imported image or engineering candidate.

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

## Typed fixed-scalar preparation

`CreationDestination::prepare_typed(table, columns, rows, budget, cancelled)`
accepts borrowed engine `CreateColumn` declarations and exact shared `Value`
rows. It returns the same `PreparedCreation`; report preparation, required
publication, counts, receipts and typed publication failures use the existing
service unchanged.

The engine's qualified fixed families are Boolean, Byte, Int16, Int32, Int64,
Currency, Single, Double, legacy DateTime and Guid. Declarations remain explicit
for empty tables and all-NULL columns. NonBoolean fields may receive NULL;
Boolean requires a Boolean value. Numerically equal values with another carrier
are refused. There is no type inference, coercion, native flag input or copied
Access declaration DTO. Variable/extended families remain outside this path.

Both preparation methods retain one user table, 1–16 columns, at most 64 rows and
an independent one-DATA-page fit check. NULL input does not author a Required,
AllowZeroLength or other native constraint property. Native eligibility and exact
encoding remain in the engine; no additional permission is implied by the host.

The CLI `create` command continues to use the Int32 convenience method. Its
`--column` arguments are names, and its TSV cells remain present decimal Int32
values. It gains no typed-schema syntax, NULL token or additional scalar parsing
from this library entry point.

Two typed service tests publish all ten independent fixed/IEEE images exactly
and reject invalid declarations/carriers before staging. The complete six-test
creation suite also retains Int32, directory-retargeting and publication-failure
coverage. See the [typed acceptance record](../../testing/results/typed-native-creation-host-2026-10-31.json).

This service creates an unencrypted fresh file. It grants no normal update,
macro-absence or recovery permission for arbitrary existing ACE files. Native
Access creation/save compatibility and broader schemas remain unqualified.
Existing [normal-write gates](NATIVE_WRITES.md) are unchanged.

The [local acceptance record](../../testing/results/fresh-native-creation-host-2026-10-31.json)
keeps complete-file checks, publication controls, process observations and
unqualified native Access behavior separate.
