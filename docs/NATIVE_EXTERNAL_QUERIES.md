# Explicit native external IN sources

`ExternalNativeSource` resolves an Access query's external `IN` database token
through explicit host bindings to already-open `NativeSourceHandle` values.
`NativeExternalMapping::new(database, key, handle)` borrows the exact token and
an immutable host-issued `SourceKey`. Neither token nor key is a filesystem
identity, a credential or authority to open a file. The adapter opens no paths,
activates no provider and performs no authentication.

Only requests with absent connection syntax are admitted. A present connection,
including an empty connection, or a retained bracketed-connection signal fails
explicitly. Database tokens compare exactly and case sensitively;
unknown tokens return no binding. The query layer owns the syntax boundary,
prepared binding-key checks and namespace routing.

## Native and namespace boundaries

`ExternalNativeSource::new(root, mappings)` keeps the native root namespace.
`ExternalNativeSource::with_links(&linked_root, mappings)` preserves an existing
`LinkedNativeSource` for ordinary root tables and saved queries. Root names do
not acquire target table aliases merely because an external mapping exists.
An explicit SQL query can combine a qualified root saved-query dependency or
mapped link with an external IN subquery through this composed facade. Native
saved-query reconstruction still refuses stored external-source fragments;
this facade does not qualify or enable those persisted definitions.

An external target exposes local base tables only. Each table lookup validates
its captured native storage classification before forwarding. Linked/ODBC target
tables fail; unknown names stay unknown. The wrapper exposes no target named
query definitions and resolves no further external requests. A matching table
name is not permission to follow a link chain or execute a target saved query.
This does not expand the engine's existing native format/readability profile.

The target wrapper forwards all existing table capabilities directly: scalar
and unavailable columns, text comparison policy IDs, multivalue/named collection
metadata and policies, equality hints, projected and constrained reads, scalar
rows and aligned collection visitors. Target flags, selected-index failures,
source order, visitor errors and complete-cache boundaries remain intact. There
is no fallback to a weaker source method and no recomputed calculation value
substituted for a cached field.

## Identity, lifetime and resources

All distinct native handles require distinct caller-issued catalog scopes and
must borrow the exact same cumulative native `RefCell<ResourceBudget>`. The same
actual handle can be reused, including the root handle, without inventing a
second scope. Reusing a root handle still exposes only local base tables through
its external wrapper. Linked-root composition checks external handles against
all existing linked target scopes as well as the root scope.

Exact database tokens and binding keys must be unique within the external
facade. Keys must change when the host changes the binding or source revision;
creating another facade does not establish a file revision automatically.
Handles, mappings and source bytes must remain stable for preparation and
execution. This is not a cross-file snapshot or transaction guarantee.

Mapping records and token-comparison work consume the shared native pool.
The pool is never replaced, reset or refunded by the facade. Native schema,
payload and cache allocation charges continue through the same target handle;
zero-I/O cached views retain their existing accounting. Query work and comparison
pools remain separate. Resolver work checks cancellation and releases its native
budget borrow before any source method or visitor callback. Borrow collisions
return typed errors, not a panic. Setup failure publishes no partial facade.

The focused `external_sources` tests cover metadata-only binding with user reads
forbidden, exact token and connection refusals, nonlocal targets, key/pool/scope
checks, root reuse, linked-root composition, cancellation, borrow collisions,
quota exhaustion and failed row reads. These are original synthetic engineering
controls; they are not native Access compatibility comparisons or permission to
open an embedded database path.
