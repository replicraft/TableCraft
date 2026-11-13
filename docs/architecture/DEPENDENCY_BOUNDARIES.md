# Enforced dependency boundaries

[The checker](../../scripts/check_dependency_boundaries.py) uses locked/offline
Cargo metadata and an explicit allowlist for the existing packages. It
checks every declared workspace edge, including optional, renamed, target-specific,
build and development dependencies. Libraries cannot import application packages;
unreviewed packages/local dependencies fail rather than silently inheriting access.
Indirect workspace paths necessarily contain at least one checked forbidden edge.

Current rules keep core/schema/model independent of hosts; I/O imports only core;
DB contracts import core/schema; SQL has no workspace dependency; expression
semantics may import SQL/core; query uses shared contracts but no concrete adapter
or engine. ACE may use reusable query/semantic contracts and I/O/foundation,
but never DB, adapters or hosts. Access/SQLite adapt engines into shared/query
contracts, and CLI composes these libraries. The allowlist is slightly broader
than today's graph for already accepted directions; it is not a feature claim.

`tablecraft-display` under `crates/presentation/` owns renderer-neutral regional
formatting, options and validated scalar presentation facts. It consumes Core's
validated DATE constructor for exact-bit whole-second presentation checks rather
than duplicating Automation encoding. It has no workspace state, GUI,
platform-window, database-service or native-engine dependencies. Runtime, UI and desktop import
this owner directly; neither runtime nor UI re-exports its types as compatibility
aliases. The guard rejects GUI/window registry dependencies in display, including
optional, target-specific and test/build declarations.

`tablecraft-builders` owns headless query-design models and checked SQL
construction/import. Its local dependencies are SQL syntax and schema; the guard
rejects GUI/window dependencies, including optional, target-specific and test/build
edges. Native metadata and execution remain runtime services.

`tablecraft-ui` owns egui widgets, styling and accessibility. Its normal local
dependencies are display, the headless builders model and schema's validated
Decimal field constraints; it receives owned state
and emits typed actions without opening databases. `tablecraft-runtime` owns headless backend workers, document
reads and application operations, and can share the pure formatter without
importing UI or desktop. Testkit remains development-only. The binary-only
`tablecraft-desktop` composes runtime, UI, display and builders directly; backend services
remain behind runtime. Desktop acceptance tests may import core value types and
the expression owner's clock contracts through development-only dependencies,
to inject and observe a real provider in Runtime journeys. Normal, optional normal
and build edges remain forbidden by the guard. Neither runtime nor desktop imports CLI or SQLite. These
edges grant no permission for foundation, engine, adapter, query or CLI packages
to import UI/runtime. Libraries still cannot depend on application packages.

`tablecraft-formats` owns bounded CAB and ZIP/OPC/XML/template decoding, consuming
core/I/O/schema/model without importing ACE, DB, query or hosts. Access retains
native payload qualification and the `TemplateSource` query adapter. CLI package
inspection imports formats directly. Generic I/O gains no format knowledge.

`tablecraft-crypto` owns checked shared cryptographic primitives without local
package dependencies. ACE retains native encryption policy, KDF/page mapping,
header rewriting and preservation. The engine consumes crypto directly.

`tablecraft-testkit` owns independent raw fixtures and shared test machinery.
Default `fixtures/access` uses Core carriers and the UUID byte-order utility;
independent expected bytes stay separate from production encoders. Optional `formats-fixtures` supplies the
working CAB fixture builder. Feature-gated `support/ace`, `support/query`,
`support/access` and `support/cli` own shared observations and scenarios.
Reviewed subsystem dependencies must be optional normal dependencies of the
testkit. The guard rejects arbitrary subsystem edges and mandatory/build/dev
support dependencies. All incoming testkit edges remain development-only,
including renamed/optional declarations; libraries cannot import the CLI app.

These test dependencies may point back to the subsystem under test. They do not
enter its production dependency closure. Access forwards its existing offline
feature to its development helper so minimal tests keep the helper disabled;
ACE's base reader observer enables no journal feature. Query Source fixtures
serve integration tests. Unit tests retain private glue for their own crate's
types; external normal-crate types cannot replace those private types implicitly.
See the [testkit guide](../../tests/reports/general/TESTKIT.md) for features and platform scope.

Access also consumes the existing expression crate directly for its working
`GeneralCollation` implementation of the backend-neutral `Collation` contract.
Native weights remain in ACE; query/expression crates gain no concrete adapter
dependency. The adapter owns the supplied comparison budget and cancellation.

```sh
python3 scripts/check_dependency_boundaries.py
python3 -m unittest discover -s scripts -p 'test_*.py'
```

Python 3.11+ is an engineering-only prerequisite; no application Python runtime
is installed by this check. CI runs these commands before Rust build checks.
The guard checks local package ownership, not third-party transitive/feature
closures, source-level API exposure, unsafe code or runtime correctness. CLI
GUI/network/runtime isolation and upstream AST privacy still require feature/tree
review and code review; passing this check is not proof of every architectural rule.

Future packages need reviewed rules with their first working implementation:
project serializers sit above shared definitions and SQLite services; Python sits
behind application services, never in the reusable engine/query dependency closure.
Pure automation/macro contracts may serve engines but cannot import concrete
runtime/UI/adapters. SQL UDFs use supplied lower-level contracts. Domain directory
placement alone does not make a reusable contract an application host dependency.

Keep semantic editable definitions, preserved opaque native payloads and derived
runtime/render state distinct. The future canonical application model must not
become a central crate importing all application domains or upstream SQL ASTs.

The prospective edit boundary depends on shared models/lower contracts, not
runtime, GUI, MCP or application hosts. Runtime orchestrates apply and protocol/
UI adapters consume the same services. `tablecraft-edit` has no manifest and
requires a policy review at introduction like any new package. The existing
checker was not relaxed to permit speculative agent/runtime dependencies.

## Provider ownership

The local `tablecraft-mcp` application depends on runtime and headless
builder/value/display contracts, with testkit restricted to development use.
Its SDK, Tokio transport and Schemars wire schemas do not enter library, desktop
or ordinary CLI closures. It must not import desktop/UI or native adapter internals
to bypass shared service admission. The executable owns only protocol/session
lifetimes, scoped file arguments and actual runtime review/report handles.

Shared DB contracts reside in `foundation/`; Access and SQLite integrations
reside in `backends/`. Future `adapters/` packages connect external systems rather
than owning native storage. This relocation changes no allowed dependency edges.
Shared task/link services compose providers through narrow contracts; provider
packages must not import runtime or UI. The standalone ACE engine remains
independent of DB and all concrete providers. See [External Data](../features/EXTERNAL_DATA.md).

`tablecraft-update` is a native platform service with no workspace dependencies.
It owns HTTP/release identity and signature-verified staging, with no database,
runtime, presentation or host imports. Its blocking operations require a
host-owned worker. Concrete desktop integration may consume this service;
shared UI and eventual browser hosts must not acquire native HTTP/replacement
mechanics. The ordinary CLI dependency closure remains unchanged.
