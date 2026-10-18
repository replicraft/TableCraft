# Repository guidance

## Current implementation

TableCraft currently has a CLI package, `tablecraft-cli`, at
`apps/tablecraft-cli`, and the shared library `tablecraft-core`
under `crates/`. `crates/tablecraft-query` provides a bounded, syntax-only Access
query parser; its supported syntax is in [SQL_PARSER.md](docs/SQL_PARSER.md).
The CLI executable is `tablecraft`. Argument handling uses
clap's `Parser` derive. The CLI provides help, version and `query parse` output without executing SQL; database
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
