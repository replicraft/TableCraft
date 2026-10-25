# Explicit external query sources

The query library executes the existing Access `SELECT ... FROM ... IN`
syntax through an explicit host resolver. An external database token is opaque
metadata, not a filename to open. The library has no filesystem, provider,
credential or password discovery. This is a read-only engineering contract;
it does not establish native Access execution qualification.

Microsoft documents `IN` as identifying the external database containing the
tables in a table expression, including a join:
[IN clause](https://support.microsoft.com/en-au/access/in-clause).

## Host contract

`Source::external_source` receives an `ExternalSourceRequest` with the exact
parsed database token, optional connection text and connection-bracket marker.
Missing connection text and empty connection text remain distinct. The default
implementation returns no binding. A host returns a `ResolvedExternalSource`
containing an already authorized source and a caller-issued `SourceKey`.

The key identifies one immutable selector binding and source revision for the
operation. It is not evidence of physical file identity. A different selector
must receive a different key. The host must change the key when the authorized
binding, metadata or source revision changes, and keep the source stable during
preparation and execution. Provider callbacks retain the ordinary `Source`
requirements for bounded work, cancellation and complete scans. A resolver must
only resolve metadata; it must not authenticate, open external resources or scan
records as a side effect.

Preparation records bounded owned selectors and keys, without reading records.
Execution revalidates all captured bindings before scanning either root or
external records. Missing bindings or changed keys fail with `SchemaChanged`.
Runtime schema discovery retains successful binding revisions across its stages,
so a completed named crosstab cannot silently supply cached cells after remapping.
Query work and allocation ceilings include registry lookup, retained selectors
and runtime provider references; the registry has an independent 256-binding cap.
Native I/O and comparison budgets remain separate host responsibilities.

## Namespace and callback behavior

An external clause selects the source for every direct table leaf in that
SELECT's FROM expression, including comma sources and joins. Each nested SELECT
has its own namespace: without an external clause it uses the root source; with
its own clause it resolves that selector. Correlated references retain their
actual captured outer frame.

Named definitions and domain targets are resolved in the root namespace. A root
named definition can itself contain an external clause. Direct external FROM
names resolve only base tables in this first slice: the external provider's
`query` callback is never invoked. External named targets require future
namespaced definition, domain and discovery-cache contracts; they are not
silently interpreted as root definitions.

Schemas, unavailable columns, text policies, equality hints, projected and
constrained scans, multivalue values, collection members and parent-presence
scans all use the selected provider. Original query filters remain in the
executor even when a provider receives a safe constraint hint. Prepared public
result metadata and active arithmetic-profile result descriptors retain their
existing behavior across these source boundaries.

Results remain buffered. A late provider, cancellation, quota, schema or
expression failure returns an error, never a successfully completed prefix.

## Focused evidence

The `external_sources` query integration target uses independent root and target
values, metadata-only scan traps and distinct policy identities. It covers joins,
correlated and nested namespaces, root named/domain composition, dynamic heading
discovery and revision invalidation, collection alignment and joined presence,
constraint forwarding with residual filtering, exact selectors, schema drift,
pre-scan binding rejection, late failures and quota/cancellation controls.
These controls exercise explicit host sources; they do not qualify native saved
query execution or authorize automatic external access.
