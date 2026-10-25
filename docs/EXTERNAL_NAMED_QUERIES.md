# Named queries in explicitly selected external databases

An external `IN` source can expose supported named SELECT, UNION and crosstab
queries through the existing `Source::query` contract. The query layer binds and
executes those definitions in their owning namespace. This extends explicit
[external sources](NATIVE_EXTERNAL_QUERIES.md); it does not grant permission to
open a database named inside SQL or activate an external provider.

```sh
tablecraft query run --database frontend.accdb \
  --external remote backend.accdb \
  'SELECT Id FROM SavedPeople IN "remote" ORDER BY Id'
```

This example requires an already supported SavedPeople definition in the chosen
backend. The existing repeatable `--external DATABASE_TOKEN PATH` option selects
the file; no additional CLI option is needed. Exact token matching, password
handling, selected indexes and stable/offline source requirements remain those
of the external-source host. Inventory and `query saved` reconstruct definitions
inertly; execution and schema discovery occur only through explicit retrieval.

## Identity and lexical ownership

A named dependency is identified by its source namespace and its name under the
existing schema-name policy. An external namespace carries the host-issued
immutable `SourceKey`; the root has its own namespace. Equal names in the root
and two external bindings are distinct dependencies. Active cycle detection and
per-operation crosstab caches use this scoped identity. A cycle within a namespace
still fails, and nesting/structure ceilings remain cumulative across namespaces.

Each bound read plan captures the definition's lexical owner. Consider a root
query selecting from SavedPeople IN "remote":

| SQL location | Default namespace when there is no explicit IN |
| --- | --- |
| Root caller, including its own nested SELECTs and domain calls | Root |
| SavedPeople's definition and its local named dependencies | The selected remote namespace |
| Nested SELECTs and domain calls written inside that definition | The definition's remote namespace |

Selecting an external FROM relation does not move the caller's entire expression
scope into that database. Entering an external named definition does establish
that definition's owner. Runtime read and crosstab entry points install the
captured owner and restore the caller's owner on both success and failure, so a
later sibling expression cannot inherit a preceding dependency's namespace.

Ordinary correlated references retain the existing explicit outer-row scopes.
A nested SELECT's local FROM source follows its lexical owner; references to an
outer alias still resolve through the supplied outer frame. Named definitions
remain independent, non-lateral relations and cannot capture a caller's fields
or undeclared parameters.

## Explicit redirection and closed Eval

An explicit IN inside an external definition uses the operation root's authorized
binding registry. The selected target is not asked to open or resolve another
provider. A second token must already have an explicit host binding; an unknown
token fails. This permits deliberate composition across supplied sources without
turning stored SQL, paths or connection strings into authority.

The original closed Eval boundary is unchanged. Its written argument expression
is evaluated in the surrounding query environment, so a domain call used to
construct that argument follows the lexical owner. The resulting Eval text still
runs with NoBindings and cannot acquire row fields, named sources, domain access
or a provider merely because its caller is an external definition.

## Metadata, discovery and retained revisions

Ordinary preparation remains metadata-only. Named SELECT/UNION dependencies and
qualified constant-heading crosstabs bind without reading user rows. Dynamic or
policy-dependent named crosstab schemas require the existing explicit
[execution-phase discovery](NAMED_SCHEMA_DISCOVERY.md) service; standalone prepare
continues to refuse them.

A schema request retains both the scoped dependency identity and the external
bindings observed before binding stopped. Discovery preserves those binding keys
before it resolves or materializes the dependency, then seeds and revalidates
subsequent binding attempts. A changed binding revision fails rather than
substituting another target into a partially prepared operation. Execution also
checks captured external bindings before ordinary scans. If a revision changes
during discovery, completed earlier reads are not undone, but cached results and
outer output are not published as a successful result.

Completed discovered results are cached only within that operation, separately
for each namespace/name pair. Definition changes retain the existing
SchemaChanged checks. No failed or incomplete crosstab prefix becomes a complete
cache entry. Runtime domain targets use the same owner, discovery state and
binding registry; parameterized domain targets remain refused.

Named parameter scopes, compatible parameter merging, exact input carriers and
undeclared-name refusals retain their existing contracts. Default and explicit
declared arithmetic result descriptors propagate through named dependencies;
public conservative metadata and runtime-selected types remain distinct. Actual
field text-policy identities follow returned cells across namespaces. Named
expansion neither invents a collation nor combines unrelated policies implicitly.

## Native adapter and resource boundaries

The native external target wrapper forwards inert saved-query lookup from its
already-open NativeSource handle. Its table callbacks still enforce local base
tables. A target saved query that reaches a linked or ODBC table fails the same
local-table guard; named expansion does not authorize link recursion. Further
external selection is possible only through explicit IN and the operation-root
registry described above.

Root, linked and external native handles retain the shared cumulative native
budget requirement. Query binding, discovery, retained cells, repeated cache
copies and final execution share the query allowance; comparison and output
allowances remain separate. Adding another namespace or retrying after a schema
request does not reset budgets. Cancellation, source failures, selected-index
errors, unavailable fields and collection/calculation-view restrictions remain
explicit. Buffered query modes publish no partial result on failure.

## Verification scope

The focused query batch passes ten tests covering same-named definitions and
fixed/dynamic crosstabs across three namespaces, local dependencies, correlated
and domain contexts, parameter isolation, cycles, paired arithmetic types,
policy retention, revision changes, cancellation and resource failures. The
focused native external-source batch passes ten tests, including target saved
SELECT/crosstab lookup and continued refusal of nonlocal target tables. Broader
regression and corpus results are recorded separately after their checks finish.

These are TableCraft engineering contracts and original-fixture results. They do
not establish Microsoft Access execution equivalence, complete saved-query
support, coherent live multi-file reads or a transaction across databases.
Native Access qualification remains separate; full backend/read-query completion
is not implied by this feature.
