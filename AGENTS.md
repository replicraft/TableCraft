# Repository guidance

## Current implementation

TableCraft currently has a CLI package, `tablecraft-cli`, at
`apps/tablecraft-cli`, and the shared library `tablecraft-core`
at `crates/foundation/tablecraft-core`. `crates/query/tablecraft-sql` provides a bounded, syntax-only Access
query parser; its supported syntax is in [SQL_PARSER.md](docs/SQL_PARSER.md).
`crates/query/tablecraft-expressions` implements the explicit scalar profile in
[EXPRESSIONS.md](docs/EXPRESSIONS.md); do not infer complete Variant promotion,
collation or native compatibility from it.
`tablecraft-schema` owns the shared names, declared scalar type constraints and
explicit policies in [SCHEMA.md](docs/SCHEMA.md). Consumer limits and SQL syntax
lowering remain at their boundaries. `tablecraft-query` now binds
and executes the read-only profile in [QUERY_EXECUTION.md](docs/QUERY_EXECUTION.md)
over explicit host sources. Keep native I/O and application hosts out of that layer.
The CLI executable is `tablecraft`. Argument handling uses
clap's `Parser` derive. The CLI provides help, version, `query parse` output and
`query eval` for source-free scalar SELECT expressions. Eval rejects clauses
requiring query execution and buffers results before stdout. `query run` uses the
retrieval layer for inline rows, typed parameters and explicitly chosen collation;
it opens no database files and buffers all results before stdout. Database
inspection, editing and GUI hosts have not been implemented.

Core values, identity scopes, budgets and their reference documents are described
in [CORE_TYPES.md](docs/CORE_TYPES.md). Value equality is representational; do not
use it as SQL equality or silently coerce expression-only states into stored NULL.
Core errors use `thiserror`; generated tests use `proptest`; `serde` support is
optional and must preserve constructor validation. Keep format-specific output
policies at the application/protocol boundary. The requested libraries and their
intended future uses are recorded in [LIBRARY_POLICY.md](docs/LIBRARY_POLICY.md).
Use those libraries when the corresponding functionality is introduced, with
target-appropriate features and verification; do not add unused dependencies.
The SQL parser selection and adapter boundaries are in
[TECH_STACK.md](docs/TECH_STACK.md). Keep upstream parser AST types inside the adapter;
Access semantics and the query model belong to TableCraft.

Read [README.md](README.md) and [implementation status](docs/IMPLEMENTATION.md)
before making changes. Follow [development policy](docs/DEVELOPMENT.md) for
toolchain, package boundaries and verification commands, and update
[dependency records](docs/DEPENDENCIES.md) when adopting dependencies.

## Working practices

Preserve existing repository conventions and user changes. Prefer small,
reviewable changes with meaningful acceptance checks. Organize code by feature
or domain concept, with its implementation and focused tests together. Keep
module entry points focused on composition and re-exports; avoid collecting
unrelated concepts in one file. Keep interfaces narrow
and introduce crates only when they own working functionality or a useful
dependency, testing or platform boundary. Avoid empty scaffolding. Use typed
errors in libraries and contextual reporting at executable boundaries. Do not
hide failures through silent defaults or unconditional retries.

Group library packages by domain under `crates/`, following
[ARCHITECTURE.md](docs/ARCHITECTURE.md). The implemented parser is `tablecraft-sql`;
`tablecraft-query` owns working binding/planning/read-only execution functionality.
The target package map is guidance for progressive extraction, not authorization
to generate empty packages. Keep feature slices inside their owning crate until
a useful independent boundary exists.

Follow [NATIVE_ENGINE.md](docs/NATIVE_ENGINE.md) for the standalone engine boundary:
future `tablecraft-ace` owns native Jet/ACE behavior; `tablecraft-access` is its
TableCraft adapter. Engine and reusable query code must not depend on application
hosts, UI, runtime or that adapter. Pure headless macro/automation contracts may
live in the application domain without importing those hosts. Backend-neutral contracts must not import
concrete engines/adapters. Introduce engine/FFI/driver crates only with working
functionality and keep compatibility claims tied to observed evidence.

Follow [VBA_COMPATIBILITY.md](docs/VBA_COMPATIBILITY.md) for separate VBA, macro
and automation-host ownership. Preserve native code without implicitly executing
it. Required validation/data macros cannot be skipped on writes. SQL UDFs need
narrow supplied callable contracts, not direct query-to-runtime dependencies;
the current scalar evaluator still rejects function calls.

[WINDOWS_COMPATIBILITY.md](docs/WINDOWS_COMPATIBILITY.md) owns future per-API
provider requirements. Keep wincompat separate from the VBA engine and from
engineering oracle tooling. No silent path mapping, DLL loading, denied-call
fallback or successful defaults for unsupported behavior. Replication must not
repeat user-triggered external effects or bypass required database validation.

Keep the CLI free of GUI and network runtime dependencies. Libraries belong
under `crates/` and must not depend on application packages. Document unsafe
code, FFI and platform assumptions where they matter. Check cancellation and
resource limits when introducing long-running operations.

## Documentation and private planning

The architecture pack under ignored `docs/temp/` is private planning material.
Consult it locally when relevant, but do not track it, copy it wholesale into
public docs, or make CI depend on it. Extract only guidance that applies to
current work. Public documentation describes implemented behavior, observed
results and explicit current limitations. Promote subsystem documentation as
its implementation becomes real; planned APIs and tests are not working features
or passing evidence.

Keep this file focused on working guidance. Put implementation details and
verification results in their owning documents under `docs/`. Update those
documents and relevant tests when behavior changes.

## Fixtures and verification

Use the [compatibility roadmap](docs/ROADMAP.md) and feature-local contracts for
application work. Prioritize observed application outcomes; the
[corpus policy](docs/testing/ACCESS_APPLICATIONS.md) separates open, preservation,
execution, editing and native save/reopen evidence. A candidate inventory or
requirements document is not a qualified fixture or passing application test.

Do not copy third-party code or fixtures without provenance and license review.
Record fixture sources, hashes and notices in [FIXTURES.md](docs/FIXTURES.md).
Preserve original fixture bytes and perform mutation experiments on copies.
Do not execute active database content implicitly. Keep private data,
credentials and machine-specific paths out of commits.

Run applicable formatting, lint, test and build checks from the development
guide. Report unavailable environments and unrun checks accurately. A native
Access compatibility claim requires actual Access comparison evidence; a
headless build does not qualify database behavior or another platform.

Use explicit path-scoped staging and focused commits. CI actions use version
tags rather than commit hashes. Do not push unless requested.
