# Backend-neutral snapshots and editing contracts

`tablecraft-db` owns complete validated scalar `Snapshot` values and the
`EditableTable` contract. It imports core values/budgets, shared schema and typed
errors, with no concrete engine/adapter, SQL, async runtime or application host.
Connection discovery, native lock protocols and cursor/session runtime are not
implemented here. The package exists for working edit consumers rather than a
complete speculative database API.

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
Candidate ExistingInt32 capability is not authorization for every field: adapters
must validate carriers, identities, constraints and unavailable behavior. Planning
returns a backend-owned candidate and performs no durable writes. Persistence
and recovery remain concrete plan responsibilities.

`tablecraft-access::NativeEditor` is the first implementation. It reads one table
from an immutable native image, moves scalar data into a shared snapshot, validates
handles and exact Int32 replacements, and delegates preservation/constraints to
[the engine](NATIVE_WRITES.md). ACE reports Unavailable updates; Jet 4 still passes
all native gates. No engine dependency on this contract or adapter was introduced.
Existing read-only query Source interfaces and SQLite behavior remain unchanged.

Two contract tests check stale identities, exact carriers, limits and cancellation;
two adapter integration tests drive native planning through the generic trait,
reject cross-snapshot handles/wrong carriers and check unavailable ACE capability.
These are headless engineering checks, not native Access compatibility evidence.

The [query update planner](QUERY_EDITING.md) now consumes EditableTable directly.
It resolves fields and evaluates all selected replacements before invoking the
backend once. Read-only query Source/SQLite adapters retain their existing APIs.
