# Fresh native database creation

## Current catalog and normal-session consumption

The native artifact now contains the seventeen-field catalog used by existing
ordinary source admission, including eleven variable slots and seven distinct
catalog Memo/OLE map pairs. Required publication, public APIs, user declarations,
counts and failure ownership are unchanged. The artifact and publication receipt
still grant no mutation permission.

After publication, eligible files can be opened by the real retained
`OfflineWriteSession` and freshly qualified for normal INSERT/DELETE. The session
receives current file bytes, not a creator certificate. Typed and SQL acceptance
publishes two fresh files and performs four normal transactions under retained
leases, matching independently authored complete images. The separate CLI
workflow adds one publication, two normal transactions and two exact row
readbacks. Focused tests and the strict/broad regression batch pass; exact receipts are
in the [execution record](../../testing/results/ace16-native17-creation-writes-2026-11-01.json).

The [source qualification](../../testing/ACE16_NATIVE17_CREATION_SOURCES.md)
precedes these producers. Complete-row INSERT now has its separate
[wider scalar and represented EOF profile](NORMAL_COMPLETE_INSERT.md). Explicit
Required/AllowZeroLength creation is described below; generated/default policies
and represented future FREE requirements remain explicit normal gates. No future GLOBAL bits are changed by
this upgrade, and native Access/Windows conformance is not inferred.

The optional `tablecraft-access/ace16-create` feature composes the
[native creator](../../testing/ACE16_DATABASE_CREATION.md) with required
fresh-file publication. The Int32 convenience and typed scalar path accept
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

## Typed scalar preparation

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
Access declaration DTO. Explicit Short Text, Short Binary and Decimal declarations now use the same
preparation and required publication path. Their capacities, exact retained scale
and NULL/empty rules follow the [engine contract](../../testing/ACE16_DATABASE_CREATION.md#explicit-variable-declarations).
Extended and long-value families remain unsupported.

Both preparation methods retain one user table, 1–16 columns, at most 64 rows and
an independent one-DATA-page fit check. NULL input does not author a Required,
AllowZeroLength or other native constraint property. Native eligibility and exact
encoding remain in the engine; no additional permission is implied by the host.

The CLI `create` command keeps `--column` as the present-Int32 convenience path.
Its exclusive `--field NAME DECLARATION` mode forwards explicit native metadata
and exact typed TSV values to `prepare_typed`, retaining the same required
publication and receipt handling. See the [CLI syntax and NULL/empty rules](../../cli/NATIVE_CREATION.md#explicit-typed-schema).

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

The variable creation acceptance adds five exact publications through the same
service. All seven creation-service tests pass, retaining the ten fixed/IEEE
and six Int32 images, cancellation, namespace retargeting and refusal controls.
See the [variable result](../../testing/results/ace16-native-variable-creation-2026-10-31.json).

## Multiple-table preparation

`CreationDestination::prepare_tables(tables, budget, cancelled)` consumes borrowed
engine `CreateTable` specifications and prepares one complete fresh image. Native
metadata, per-table row fit, distinct table names and aggregate layout qualify
before staging. It returns the same `PreparedCreation` as the single-table
methods; no publisher, retry or receipt family is added.

`PreparedCreation` and the acknowledged `CreationReceipt` expose `table_count()`.
Their `row_count()` and `column_count()` are aggregate user-table totals for this
operation. Existing single-table methods retain their counts and report one user
table. Hosts can allocate their full aggregate completion report before calling
`publish`, which retains the existing required file/captured-directory barriers,
no-clobber behavior and known-publication failure facts.

The engine admits 1–16 user tables, each with 1–16 explicit columns and at most
64 rows plus its own DATA-page fit check. One cumulative budget covers all tables
and publication. A malformed late table produces no staging or final entry.
The CLI remains a single-table consumer: `--column` and exclusive typed `--field`
continue to call their existing preparation methods. This API grants no normal
editing, journal or macro-absence permission for imported ACE files.


## Required and empty-Text source qualification

Eighteen independent native17 source views now qualify explicit Required and
AllowZeroLength Boolean properties: fourteen healthy sources and four readable
constraint violations. Native declaration/constraint reads, an explicit export,
eighteen exact read-only SDK reports and saved verification pass before any
matching creator is run. The [source record](../../testing/results/ace16-creation-property-sources-2026-11-01.json)
retains receipts. These are authored metadata sources, not native Access creation
or save/reopen qualification. Creator support is tested separately.


## Explicit Required and AllowZeroLength creation

`CreateColumn::with_required(bool)` authors an explicit Required property.
`CreateColumn::short_text(...).with_allow_zero_length(bool)` authors an explicit
empty-Text rule; other declaration families reject that option. Omission remains
distinct from false. Both single-table and multiple-table creators serialize
bounded native MR2 column properties into the catalog and validate initial rows
before allocating the image. Aggregate catalog capacity is checked before staging.
No-policy creation retains its existing complete bytes.

Five focused functions pass, including fourteen full-image creator/source
comparisons, four initial-row refusals, three two-table component comparisons
and invalid-policy/early-allocation controls. The existing creation regression
filter passes 31 functions with one ignored exporter. The
[batch record](../../testing/results/ace16-sparse-actions-and-creation-properties-2026-11-01.json)
separates these from source qualification and broader regression. The two-table
checks qualify independent catalog and user-page components, not whole-image
source equality. The CLI currently has no field-policy flags. DefaultValue and
AutoNumber creation remain separate work; native Access save/reopen is unrun.


The combined local batch passes 1,148 ACE units, 79 adapter units, 102 CLI units
and 90 default recovery functions. Strict all-feature, journal-only, preview-only
and minimal checks pass. Fixture-dependent ignored functions are covered only by
the separately executed focused receipts. Native Access and Windows qualification
remain unrun.
