# Backend-neutral snapshots and editing contracts

`tablecraft-db` owns complete validated scalar `Snapshot` values and the
`EditableTable` contract. It imports core values/budgets, shared schema and typed
errors, with no concrete engine/adapter, SQL, async runtime or application host.
Connection discovery, native lock protocols and cursor/session runtime are not
implemented here. The package exists for working edit consumers rather than a
complete speculative database API.

The shared borrowed `ProjectedRow` and `MultivalueList` views also live here.
Consumers import them directly from `tablecraft-db`; query has no compatibility
re-exports. Projection validation returns a storage-neutral `ProjectionError`.
The query boundary converts it to its existing source error. Query-specific
visitors, named SQL, collation provenance and `Source` remain in query.

A snapshot consumes owned name/schema/rows under an explicit SchemaPolicy. It
validates exact carriers and nullability without copying records, charging work
and checking cancellation. Producers retain acquisition limits and allocation
charging. Snapshot construction does not infer SQL equality or apply defaults.

`RowHandle` is an opaque process-local position issued by one exact snapshot. It
is not a persistent backend RecordId. Another snapshot rejects it even when all
displayed values and names are identical. Identity issuance is atomic and fails
before wrap/reuse. There is no unchecked ordinal constructor or serialization.

`CellUpdate` binds an exact replacement Value to such a handle. `EditableTable`
exposes its frozen snapshot, candidate update profile and consuming batch planner.
`UpdateProfile::StoredScalar` is a separate opt-in whose SQL consumer applies
[target-aware stored assignment](../query/QUERY_EXECUTION.md#sql-stored-assignment):
nullable NULL, exact stored carriers and checked integral Byte/Int16/Int32/Int64/Currency
conversion. The typed DB contract still carries exact replacement Values. `ExistingInt32`
keeps its earlier non-NULL behavior. Neither capability authorizes every field: adapters
must validate carriers, identities, constraints and unavailable behavior. Planning
returns a backend-owned candidate and performs no durable writes. Persistence
and recovery remain concrete plan responsibilities.

`tablecraft-access::NativeEditor` is the first implementation. It reads one table
from an immutable native image, moves scalar data into a shared snapshot, validates
handles and exact Int32 replacements, and delegates preservation/constraints to
[the engine](../access/writes/NATIVE_WRITES.md). ACE reports Unavailable updates; Jet 4 still passes
all native gates. No engine dependency on this contract or adapter was introduced.
Existing read-only query Source interfaces and SQLite behavior remain unchanged.

Two contract tests check stale identities, exact carriers, limits and cancellation;
two adapter integration tests drive native planning through the generic trait,
reject cross-snapshot handles/wrong carriers and check unavailable ACE capability.
These are headless engineering checks, not native Access compatibility evidence.

The [query update planner](../access/writes/QUERY_EDITING.md) now consumes EditableTable directly.
It resolves fields and evaluates all selected replacements before invoking the
backend once. Read-only query Source/SQLite adapters retain their existing APIs.

The separate [SQL engineering editor](../access/writes/SQL_ENGINEERING_COPIES.md)
also implements this consuming contract. It binds one original NativeImage to
one snapshot and finishes once into a non-journalable EngineeringCopy. This does
not change NativeEditor admission or introduce persistence into DB.

`EditableBatch: EditableTable` is an explicit capability for a single consuming
multi-column finish. Its borrowed `ColumnUpdates` groups hold canonical target
names and snapshot-issued `CellUpdate` handles. The caller retains each complete
column vector through the finish; the backend validates duplicate targets,
repeated/stale handles, aggregate limits and actual native eligibility. The Access
engineering editor implements fixed batches and fixed fields with one qualified
Short Text/Binary field. Default mixed routing fits the original record;
explicit variable mode permits joint DATA placement but no mixed INDEX growth.
LVAL retains one target; ordinary NativeEditor does not acquire this capability.
The contract does not authorize sequential candidate chaining, cross-backend
atomicity, durable changes or journal conversion.
Seven neutral batch-planner functions passed, including original-row exchange,
duplicate-target refusal, zero backend finishes on late failures, unchanged
single-column admission, exact Bytes/Steps limits and final cancellation.

## Separate DELETE and INSERT capabilities

`DeletableTable` owns a complete original snapshot and consumes selected
`RowHandle` values in one `plan_deletes` finish. Handles are valid only for that
same snapshot, not persistent row identities. The Access engineering deleter
resolves them to original ordinals and captures exact source records from the
same immutable image before native deletion. The [SQL DELETE copy consumer](../access/writes/SQL_DELETE_COPIES.md)
evaluates all predicates before that single finish; a zero-match selection still
passes backend admission. No normal ACE save capability is implied.

`InsertableTable` exposes a canonical target name, complete supplied-column
metadata and explicit schema/name policy without scanning original records to
invent a schema. Its consuming `plan_insert` receives one complete owned row in
backend column order and returns a backend candidate without durable writes.
There is no neutral default evaluator, automatic identity mode or implicit omitted
field. A backend may explicitly select a generated-column companion schema and
require a full field list; engine-owned declaration checks and generation policy
remain at that backend boundary.

The [SQL VALUES consumer](../access/writes/SQL_INSERT_COPIES.md) binds every supplied
field exactly once before evaluating values, evaluates in written VALUES order,
then moves results into backend column order and finishes once. It shares the
actual UPDATE [stored-assignment owner](../query/QUERY_EXECUTION.md#sql-stored-assignment):
nullable NULL, exact carriers and checked integral Byte/Int16/Int32/Int64/Currency conversion. INSERT does not broaden
ordinary UPDATE's `ExistingInt32` law. Nine neutral INSERT controls passed their first actual run. The full query
regression passed 78 library and 16 existing editing integration tests; broader
checks also passed: 296 INSERT regressions, 36 Access adapter tests, 31 CLI unit
and 23 integration functions, strict checks for five packages and minimal checks
for four libraries. These results do not weaken metadata,
carrier, source or publication admission and are not native Access qualification.

## Explicit INSERT omission intent

The separate `OmittingInsertableTable` capability consumes a complete-width
`Vec<InsertValue>` over that same exposed schema. `Supplied(Value::Null)` retains
explicit NULL; `Omitted` retains absence. It carries no SQL expression, native
property, storage code or default evaluator. The concrete backend resolves only
needed defaults under its explicit policy, then enforces its constraints.
A generated companion schema excludes only its backend-bound generated field;
absence never guesses that field or selects identity generation.

`CompleteInsertableBatch` and `OmittingInsertableBatch` add optional consuming
ordered batch capabilities. The former receives complete `Vec<Value>` rows; the
latter retains each original supplied/omitted field through `InsertValue`.
Neither grants a schema, default, generated-value or SQL-syntax permission.
The concrete backend must qualify every original row and return one plan for
the complete batch. Planning does not persist a first-row prefix.

The native journal inserter and retained offline inserter consume both contracts.
They check all row widths before allocating native envelopes, move owned payloads
without cloning, charge allocations and poll cancellation, then delegate once to
the existing TCNA4 or TCNA2 engine planner. A successful retained preparation
returns the existing `StagedInsert`; failures use the same consuming session
failure path. One session, lease, journal and receipt cover the complete action.
The single-row SQL/CLI grammar remains unchanged. These interfaces do not imply
that arbitrary native table layouts or concurrent Access clients support writes.

The [presence-bearing planner](../query/INSERT_OMISSIONS.md) allows named subsets,
binds identities before evaluating expressions in written order, and invokes one
consuming finish. Positional VALUES remain complete-width. The original complete
trait and planner retain their old strict behavior. Six new neutral tests and all
nine existing complete INSERT tests passed on their first run. Standalone neutral strict
and minimal DB/query checks also passed; no native default, adapter, CLI or native save
qualification is inferred from this neutral capability.

## Transaction contract direction

[Transaction ownership](../architecture/TRANSACTIONS.md) keeps physical transactions
inside ACE/SQLite and reserves shared begin/commit/rollback, capability, scope,
nesting and uncertain-outcome contracts here when working consumers exist. The
Access adapter maps ACE's standalone API; ACE does not import DB. No general
transaction API or cross-backend coordinator is implemented by today's snapshots
and candidate edit plans. No `tablecraft-transactions` crate is added.
