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
`UpdateProfile::StoredScalar` is a separate opt-in for nullable NULL and exact
stored carriers, retaining checked integral assignment to Int32. `ExistingInt32`
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
engineering editor implements it for one fixed-field batch. Variable/LVAL modes
retain one target; ordinary NativeEditor does not acquire this capability.
The contract does not authorize sequential candidate chaining, cross-backend
atomicity, durable changes or journal conversion.
Seven neutral batch-planner functions passed, including original-row exchange,
duplicate-target refusal, zero backend finishes on late failures, unchanged
single-column admission, exact Bytes/Steps limits and final cancellation.

## Transaction contract direction

[Transaction ownership](../architecture/TRANSACTIONS.md) keeps physical transactions
inside ACE/SQLite and reserves shared begin/commit/rollback, capability, scope,
nesting and uncertain-outcome contracts here when working consumers exist. The
Access adapter maps ACE's standalone API; ACE does not import DB. No general
transaction API or cross-backend coordinator is implemented by today's snapshots
and candidate edit plans. No `tablecraft-transactions` crate is added.
