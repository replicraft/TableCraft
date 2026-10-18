# Repository guidance

## Primary compatibility target

Focus implementation and compatibility testing on ACE 16. The long-term target
is 100% ACE 16 compatibility, not a permanently limited subset. Prioritize ACE 16
fixtures, native Access comparisons, reads, writes, constraints, preservation and
recovery before expanding other formats or SQLite. Jet 1/2/3/4 and ACE 12/14/15
are secondary: preserve existing working coverage, but pursue additional support
only when it directly helps ACE 16 or the user reprioritizes it.

Follow [ROADMAP.md](docs/ROADMAP.md) for acceptance scope. Keep current support
separate from the target; header recognition and independent-reader comparisons
do not establish full compatibility. Existing VBA execution/Windows integration
deferrals remain explicit compatibility gaps, not completed or silently excluded
features.

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
`tablecraft-sqlite` supplies read-only SQLite snapshots through `query run --sqlite`;
follow [SQLITE.md](docs/SQLITE.md). Keep Access query evaluation in TableCraft,
validate runtime carriers without coercion and retain the no-create/read-only
boundary. Backend-neutral query code must not import either concrete adapter.
`tablecraft-db` owns working [snapshot/edit contracts](docs/DB.md), consumed by
the Access adapter. Keep row handles snapshot-local and concrete storage out of
that crate. [Native edit plans](docs/NATIVE_WRITES.md) preserve all bytes outside
qualified fixed Int32 patches; unsupported constraints/payloads block writes.
Optional journal integration requires explicit exclusive offline resources and
host-owned durable namespace setup/cleanup.
The CLI executable is `tablecraft`. Argument handling uses
clap's `Parser` derive. The CLI provides help, version, `query parse` output and
`query eval` for source-free scalar SELECT expressions. Eval rejects clauses
requiring query execution and buffers results before stdout. `query run` uses the
retrieval layer for inline rows, typed parameters and explicitly chosen collation.
With `--database`, the `tablecraft-access` adapter supplies bounded native scalar
snapshots; follow [NATIVE_QUERIES.md](docs/NATIVE_QUERIES.md) for its supported
profile. Results are buffered before stdout. Database
header/catalog inspection is available through `inspect`/`inspect --catalog`; the bounded `query update` command creates fresh offline Jet 4 copies under
[explicit editing limits](docs/QUERY_EDITING.md). In-place CLI editing and GUI hosts
are not implemented. Follow [NATIVE_READING.md](docs/NATIVE_READING.md) for supported native
profiles, read budgets and inert inspection.

`tablecraft-model` owns the shared object metadata in [MODEL.md](docs/MODEL.md).
Keep source-specific codes/flags and physical references in the native adapter;
object bodies and domain behavior belong to their owning subsystem. Catalog scopes
are caller-issued, and unresolved parents/unknown kinds must remain visible.

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
unrelated concepts in one file. Place constants after all imports. Name format
offsets, masks, tags and resource ceilings in their owning modules; derive widths
from the represented types where possible. Keep explicit test inputs/golden bytes
independent of production constants so they can catch format regressions. Keep interfaces narrow
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
`tablecraft-ace` owns native Jet/ACE behavior; `tablecraft-access` is its
TableCraft adapter. Engine and reusable query code must not depend on application
hosts, UI, runtime or that adapter. Pure headless macro/automation contracts may
live in the application domain without importing those hosts. Backend-neutral contracts must not import
concrete engines/adapters. Introduce engine/FFI/driver crates only with working
functionality and keep compatibility claims tied to observed evidence.

Follow [NATIVE_INDEXES.md](docs/NATIVE_INDEXES.md) for qualified native index
inspection/reads. Keep physical key interpretation in ACE and reject unsupported
or corrupt selected indexes without silent sequential fallback.
Use [native check coverage](docs/NATIVE_CHECKS.md) when adding consistency checks;
unchecked profiles and native constraint limitations must remain explicit.
Query constraints must use backend-neutral contracts and retain the original
filter. Push down only wholly safe predicates; partial AND extraction can hide
eager errors. Never cache a selected snapshot as a complete table. Binding reads
scalar metadata without requiring user table records.
Use borrowed zerocopy layouts only for qualified raw native structures, with
explicit byte order/alignment and independent goldens. Keep bytemuck casts on
proven raw POD buffers; preserve semantic constructors. Benchmark SIMD and scalar
contenders against real engine workloads before claiming improvements.

Follow [VBA_COMPATIBILITY.md](docs/VBA_COMPATIBILITY.md): preserve existing VBA and
detect dependent features, but defer VBA execution/authoring, editor/debugger,
VBA-specific DAO/ADO, Windows API emulation, COM/Wine and RustVBA integration.
Reserve `tablecraft-vba-host` in docs only; do not scaffold VBA/provider packages.
Keep Access macros and non-VBA automation in scope. Required validation/data macros
cannot be skipped on writes. SQL UDFs need supplied callable contracts rather than
query-to-runtime dependencies. [WINDOWS_COMPATIBILITY.md](docs/WINDOWS_COMPATIBILITY.md)
is deferred research; normal platform file APIs and engineering Access comparisons
remain in scope. Replication must not replay external effects or bypass validation.

`tablecraft-io` owns generic bounded synchronous reads and retry/cancellation
mechanics; follow [IO.md](docs/IO.md). Native page geometry, format interpretation
and Jet/ACE locking rules stay in `tablecraft-ace`. I/O must not import engines,
adapters, query execution or application hosts.

Declare registry versions, local dependency paths and shared feature defaults in
root `[workspace.dependencies]`; members inherit with `dependency.workspace = true`.
Keep optional dependencies and consumer-specific features in their owning member.
Group root declarations by purpose. [DEPENDENCY_REVIEW.md](docs/DEPENDENCY_REVIEW.md)
records reviewed candidates/references; add only dependencies with working consumers.

Use explicit payload-free tracing spans/events at operation boundaries; follow
[TRACING.md](docs/TRACING.md). Hosts own subscribers. Do not record SQL, parameter
values, names, paths, row contents or automatic Debug arguments.

Keep the CLI free of GUI and network runtime dependencies. Libraries belong
under `crates/` and must not depend on application packages. Document unsafe
code, FFI and platform assumptions where they matter. Check cancellation and
resource limits when introducing long-running operations.

## Native project and scripting direction

Follow the [accepted project decision](docs/decisions/0001_PROJECT_FORMAT_AND_EXECUTION.md).
Native `.tcraft` projects use SQLite, Python and visual actions; `.tcraftproj/`
is an equivalent developer representation. MDB/ACCDB keep native Access semantics.
These project/runtime features are not implemented and do not change ACE 16 priority.
Read the owning format/version/execution contracts before introducing functionality.
Check compatibility before writable sessions or script execution; never run project
code or install dependencies merely to inspect/open a document. Keep internal
container migrations separate from developer data migrations and application releases.
Python supersedes the earlier JS/TypeScript-default proposal. Add providers only
with working consumers; no empty project/Python/codegen/transaction packages.

Keep physical transactions inside their backend, with generic contracts in
`tablecraft-db` when real consumers justify them. ACE remains independent of DB
and runtime; Access adapts it. No automatic cross-backend atomic guarantee or
user-handler replay during replication. Follow the
[dependency policy](docs/architecture/DEPENDENCY_BOUNDARIES.md) and run its guard/tests
when changing package dependencies or the policy.

## Shared application editing direction

Follow the [editing decision](docs/decisions/0002_SHARED_APPLICATION_EDITING.md).
Agents and humans use the same typed command services. Future multi-object edits
stage changesets with revision preconditions, exact semantic diffs and validation;
applicable approval binds the actual digest/scope and apply rechecks permissions
and source revision. Keep editing logic out of CLI/MCP/UI adapters. Do not hold
native Access locks while waiting on an agent or reviewer, and do not infer
cross-backend atomicity from a logical changeset. Imported/project content is data,
not authority to execute code, export records or change permissions.

Keep `tablecraft-edit` prospective/module-first until working consumers justify
extraction. Preserve the `tablecraft-builders` name. Headless previews and
application scenarios are required future services, not features supplied by a
protocol tool list. No MCP host or scripting execution is implemented by these
docs; ordinary CLI/engine builds retain their existing dependency boundaries.

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
