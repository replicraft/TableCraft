# Shared application-object metadata

`crates/foundation/tablecraft-model` owns the common catalog descriptors consumed
by the native reader and CLI. It depends only on core, schema and thiserror; it
has no native decoder, SQL parser, UI, runtime, serialization or network dependency.

## Implemented contracts

- `ObjectDescriptor`: ObjectId, validated SchemaName, ObjectKind and optional parent.
- `ObjectKind`: Table, Query, Form, Report, Macro, Module, Container, Relationship
  and Unknown. A category identifies metadata, not decoded contents or executable behavior.
- `CatalogScope`: caller-issued 64-bit namespace combined with a 64-bit local token
  into core's existing ObjectId. Callers own namespace uniqueness and persistence.
- `ObjectCatalog`: validates a caller-selected object ceiling, scoped identities
  (including parent references) and unique IDs. It preserves input order and names.
  ID lookup is exact; name lookup uses an explicit schema NameComparison policy.
  Duplicate names are legal; ambiguous filtered lookup returns a typed error.

Parent references can remain unresolved because native inventories may omit root
containers. They express containment, not a discovered dependency graph. `None`
for a lookup filter means any category/parent; it does not filter for root objects.
Unknown categories remain in the inventory with their source metadata retained
by the adapter. Name equality is not SQL equality or Access locale collation.
Descriptors/catalogs do not implement Debug or Serde; applications explicitly
choose what to display and protocols need their own validated exchange contract.

## Ownership and actual consumers

[The native reader](../access/NATIVE_READING.md) constructs descriptors from five selected
MSysObjects fields. Signed native IDs are mapped without losing their 32-bit
representation into source-local tokens. Native type codes, flags and physical
page/slot references remain in ACE's NativeObjectOrigin, alongside the catalog.
Its CLI consumer displays both shared and native metadata with escaped output.
The CLI uses one invocation-local namespace; those IDs are not persistent file
identities or safe handles for future edits.

Core owns ObjectId and budgets. Schema owns reusable names and relational type
contracts; a Relationship object descriptor does not decode foreign keys. SQL
owns syntax. Detailed form/control/report definitions, macro/VBA models, object
properties and dependency discovery belong to their domain owners as they become
real. None is implemented by categorizing a catalog row. No empty application
packages or speculative cross-domain object enum was added.

## Verification

Focused tests cover scoped identities, duplicate-ID rejection, ambiguous names,
explicit comparison/filter behavior, unresolved parents, unknown categories and
object ceilings. The public API has a runnable doctest; ACE integration tests
consume it without an application host, including a registered fixture copy.
See [implementation evidence](../IMPLEMENTATION.md) for observed checks and limits.

## Canonical project definitions direction

Future native packed/unpacked serializers consume one versioned, backend-independent
application model with stable object identities and provenance. Shared definitions
can grow here with actual consumers; domain-specific form/report behavior stays
with its owners. Distinguish editable semantic definitions, preserved opaque
native content and derived rendering/runtime state. This does not add serialization,
persistent project IDs, forms or dependency graphs to the current catalog API.
See [project format](../formats/TCRAFT_FORMAT.md) and
[ownership rules](../architecture/DEPENDENCY_BOUNDARIES.md).
