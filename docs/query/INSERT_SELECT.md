# INSERT SELECT staging

The backend-neutral query layer stages `INSERT INTO … SELECT …` through the
existing retrieval engine and real atomic batch contracts. Native/GUI admission
is recorded separately in [reviewed INSERTs](../access/writes/REVIEWED_INSERTS.md).
Generic query staging alone does not qualify a native file profile, journal,
default program or GUI command.

`prepare_insert_select` consumes a `CompleteInsertableBatch` editor and returns a
`PreparedInsertSelect`. `prepare_insert_select_with_omissions` consumes an
`OmittingInsertableBatch` and returns a `PreparedOmittingInsertSelect`. Both take
borrowed TableCraft syntax, a stable host `Source`, typed parameter bindings, execution
options and the caller's cumulative query allowance. They do not open sources or
create durable resources.

Each candidate privately owns the original editor and the complete staged rows.
Its consuming `finish` takes the backend allowance and invokes exactly one real
batch finish. This separation lets source callbacks use a host's shared native
allowance during retrieval; the host borrows that same allowance for native
finish only after preparation has returned. Neither phase resets or replenishes
an allowance. Empty retrievals still invoke backend admission once.

Destination table/column identities, duplicate fields and explicit positional
mapping are checked before source scans. Positional appends require the complete
target width. Named omissions remain omitted for every row; a supplied NULL stays
a value. Static source width is checked after metadata binding and before source
execution. Qualified execution-phase named-schema discovery may read dependencies
before their dynamic width is available, under the same query allowance. Original
root parameters are validated before those discovery reads. Named/domain scopes,
external binding identities and cycle checks retain their retrieval contracts.

The source is the original stable view throughout the operation, including a
self-append. No inserted prefix becomes a source row. The complete retrieval,
including UNION promotion, aggregates, sorting and required eager work, succeeds
before destination assignment begins. Assignment retains the shared stored-value
law: nullable NULL, checked integral assignments, exact integral Currency scaling
and otherwise exact destination carriers. Text representations do not become
implicit numeric or date inputs. Non-storable expression states do not
become NULL, zero or error codes. Lossless String cells survive intermediate read
stages; valid Unicode is produced only at final stored Text assignment. Invalid
Unicode refuses that assignment. TOP-omitted values do not undergo destination
assignment, while operational errors from required omitted work still propagate.

All rows and assignments succeed before a candidate can be returned. A late read,
conversion, cancellation or resource failure publishes no batch prefix and never
calls the backend finish. A cancellation after preparation also refuses the
consuming finish. Backend refusal is terminal for that candidate: there is no
query replay, implicit retry or switch between complete and omission contracts.

The existing VALUES planners and read-only `prepare` API retain their boundaries.
The append binder borrows the original `ReadQuery` and outer parameter declarations;
it neither interpolates SQL nor clones the syntax tree to invent a read statement.
