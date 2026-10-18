# Implementation status

## Numeric-contract cleanup

A workspace source audit replaced unexplained native offsets/masks/tags/page sizes,
query/binding/evaluation limits, Pratt binding powers, CLI budgets and scalar
scale/calendar/identity widths with names or type-derived widths. Format facts and
structural policies remain separately owned; constants follow all imports.
CLI native inspection/retrieval share one explicit host-budget/scope policy.
Public limit/carrier constants expose existing contracts; their values, validation,
accepted SQL/native profiles and output remain unchanged. Independent fixture
bytes and ordinary arithmetic/test inputs remain explicit regression evidence.
No dependency or workspace-package change was introduced.

Local macOS / Rust 1.99.0 checks passed workspace Clippy and 169 tests/doc tests,
standalone schema/query/model/ACE/adapter checks, expression minimal/all-feature
and core all-feature lint/tests, release build, warning-denied rustdoc, formatting
and whitespace. Import ordering was checked across changed Rust modules and the
original fixture hash is unchanged. Native Access comparison, other targets and
remote CI were not rerun for this refactor.

## Native scalar retrieval CLI

`tablecraft-access` is a working storage adapter over ACE snapshots and the existing
query Source interface; the workspace now has nine packages. ACE and query retain
their independent dependency boundaries. The CLI's `query run --database PATH`
opens only that explicit stable/offline source read-only and uses the existing
retrieval/expression engine. [NATIVE_QUERIES.md](NATIVE_QUERIES.md) records supported
formats/carriers, eager table loading, caching, budgets and failure behavior.
Without --database, inline retrieval is unchanged. No active content or writes run.

Three adapter tests cover real query execution, caching/read counts, immediate
visitor rejection, unsupported/missing and empty tables, limits and cached-scan
cancellation. Three new CLI tests cover native TOP/filter/parameter/aggregate,
self-join/empty/text-collation and payload-free error results with unchanged copies.
The existing help test now describes explicit --database capability.

Local macOS / Rust 1.99.0 verification passed all standalone package checks,
workspace Clippy and 169 tests/doc tests, expression minimal/all-feature and core
all-feature lint/tests, release build, warning-denied rustdoc, formatting and
whitespace. The release binary's documented native TOP example matched exactly.
Dependency metadata verified nine packages, independent engine/query closures and
no GUI/network/provider runtime in the CLI. Public links across 33 Markdown files
and unchanged original/comparison-copy fixture SHA-256 passed. Eighteen ACE tests
include three 256-case fixed-seed properties. Native Access, remote CI and
Windows/Linux execution were not observed.

## Native scalar row snapshots

ACE's `read_table` now returns a bounded complete snapshot over the supported
scalar schema profile. [NATIVE_TABLES.md](NATIVE_TABLES.md) describes exact
carriers, NULL/empty/Boolean handling, physical order, structural rejection and
shared budget/cancellation behavior. No registry dependency was added.

Seventeen ACE tests passed locally, including new fixed/variable row fixtures,
exact representation/NULL/empty tests, unsupported/calculated/corrupt inputs,
mid-operation cancellation and a 256-case mutation property. An external Rust
consumer matched Jackcess 5.0.4 for 1,488 rows across seven explicitly named
candidate tables; format-specific source review and normalized result hash are
recorded in NATIVE_TABLES.md. Original fixture bytes remain unchanged. The initial
row API initially had no native query adapter; the adapter milestone above adds
that consumer. No write path or Microsoft Access qualification was added.
ACE Clippy/tests passed on macOS / Rust 1.99.0; other targets were not observed.

## Native table schemas

ACE now exposes bounded local-table schema inspection through `inspect_table_schema`
and CLI `inspect --table NAME`. [NATIVE_TABLES.md](NATIVE_TABLES.md) records the
implemented native metadata and shared Column mapping, unsupported types and
conservative nullability policy. Shared page/layout/Unicode code remains internal;
catalog-specific validation is separate. No registry dependency was added.

All 36 local-table schemas / 187 columns matched Jackcess 5.0.4 for name, native
type, normalized byte length and order on a disposable candidate copy. No active
content or table rows were evaluated. Original fixture bytes are unchanged.
Focused ACE/CLI Clippy and tests passed locally on macOS / Rust 1.99.0.
Native Access comparison and remote CI were not run.

## Shared object model and native catalog inspection

`tablecraft-model` now provides scoped object descriptors and validated read-only
catalog lookup, with real consumers in `tablecraft-ace` and the CLI. [MODEL.md](MODEL.md)
records its identity/name/category/parent contracts and ownership boundaries.
The workspace has eight packages. No placeholder application models were added.

`inspect --catalog` reads five selected MSysObjects fields through dynamic column
metadata, owned-page allocation maps and bounded relocation following. It preserves
unknown native types and unresolved parent references, rejects unsupported/malformed
structures explicitly, and buffers escaped output before stdout. Native origins
stay in ACE rather than entering the shared model. [NATIVE_READING.md](NATIVE_READING.md)
records format profiles, limits, pinned research and the manual comparison recipe.

A disposable copy of the registered candidate yielded 146 metadata records; all
146 `(Id, ParentId, Type, Flags, Name)` tuples matched a read-only Jackcess 5.0.4
scan exactly. The original SHA-256 remains unchanged. No active content, saved
query, linked source or object body was executed/interpreted. This is independent
reader evidence, not a Microsoft Access oracle result. General table data, object
contents, edits, locking and native application execution remain unavailable.

Local verification on aarch64-apple-darwin / Rust 1.99.0 passed standalone
model/ACE/schema/query checks, minimal workspace Clippy and 154 tests/doc tests,
expression minimal/all-feature and core all-feature lint/tests, release build,
warning-denied rustdoc, formatting and whitespace checks. ACE has ten tests,
including two 256-case fixed-seed properties; model has two focused tests and a
runnable public doctest; the CLI has 22 integration tests. The release catalog
command produced the expected 146 rows. Public links across 31 Markdown files,
eight-package metadata and the fixture hashes were checked.

An unrelated temporary Rust package outside the workspace also consumed ACE/core/
model, decoded all 146 objects and resolved AutoKeys as a Macro. Its normal
closure contained only these foundation/engine libraries and thiserror's macro
closure, without CLI, SQL, GUI, network, Java or provider dependencies. No new
registry dependency was added to TableCraft. Native Access, remote CI, Windows
and Linux execution were not observed.

## Initial native header inspection

`tablecraft-ace` now performs bounded read-only header inspection independently
of application packages. The CLI exposes it through `tablecraft inspect <path>`.
[NATIVE_READING.md](NATIVE_READING.md) records exact supported fields, limits and
pinned MDB Tools/Jackcess research references. A copy of the registered example
reports Ace2007/subversion 0, 4 KiB pages and 827 pages with a zero encoding-key
field. No active content ran and the original fixture remains unchanged. Native
catalog/records, writes, locking, runtime execution and Access oracle evidence
were unimplemented at that header-only milestone. The workspace then had seven packages.

Local macOS/Rust 1.99.0 verification passed workspace Clippy, native/CLI tests,
formatting, whitespace and public local links. Three engine tests include a
256-case fixed-seed arbitrary-prefix property; two new CLI tests cover read-only
output and payload-free failures. The copy's observed header was checked and the
original SHA-256 is unchanged. Native Access comparison was not run.

## Shared schema ownership

[SCHEMA.md](SCHEMA.md) records the extracted SchemaName, TypeDescriptor and explicit
SchemaPolicy/NameComparison contracts. Columns, memory table identities and bound
parameters now share validated names. Fields and parameters share declared Text
constraints and exact carrier checks; nullability and runtime resource/finiteness
checks remain separate. SQL type-spelling conversion stays in the query adapter.

Generic schema construction no longer imposes the query profile's 255-byte names
and 256-column ceilings. Query binding and MemoryTable still enforce those same
limits through an explicit query-owned policy. The standalone validate_columns
API now requires an explicit comparison policy. Column::new retains its carrier
constructor, while Column::with_type supports reusable declared constraints.
No new crate, dependency, Serde support, storage behavior or native claim was added.

Verification on aarch64-apple-darwin with Rust 1.99.0 on 18 October 2026 passed
workspace minimal Clippy/tests, standalone schema/query checks, the release build,
rustdoc with warnings denied, formatting and whitespace checks. Five schema tests,
the schema API doctest and three new query/schema integration tests cover the
extracted contracts; existing query/property/CLI tests also passed. The Unicode
Text parameter CLI example returned its expected result. Local links across 29
public Markdown files, unchanged six-package metadata/schema dependency boundary
and the original fixture hash passed. Other targets and native Access comparison
were not run for this refactor.

## Current retrieval milestone

`tablecraft-schema` and `tablecraft-query` now provide validated scalar relation
contracts, explicit read-only sources, prepared slot binding and bounded retrieval
execution. [QUERY_EXECUTION.md](QUERY_EXECUTION.md) defines the implemented profile:
projection/filter/order, joins, grouping/aggregates, DISTINCT/TOP/UNION, parameters
and derived/correlated retrieval subqueries. The workspace has six packages. No
native file source, write execution, full Access expression profile or native
comparison is implemented. Later sections retain evidence from earlier milestones.

Local verification on aarch64-apple-darwin with Rust 1.99.0 on 18 October 2026
passed workspace minimal Clippy/tests, standalone expression minimal/all-feature
Clippy/tests, serde-enabled core Clippy/tests, release build and rustdoc with
warnings denied. The new suites have 14 retrieval acceptance tests, three
fixed-seed properties (256 cases each), one schema test and a runnable public
API doctest. Public local links, formatting, diff checks and the original fixture
hash passed. No Windows/Linux, file-backed source or native Access comparison
was run. CLI retrieval integration is recorded separately below.


## Inline retrieval CLI

`tablecraft query run` now consumes prepared retrieval over an empty host catalog,
with inline SELECT/UNION/derived sources, filtering/sorting, joins, grouping and
subqueries. Repeated typed `--param` arguments validate exact declared carriers;
`--collation ordinal` selects the explicit non-locale text profile. The shared
formatter emits one header and zero or more rows under its 1 MiB logical ceiling.
Execution and formatting finish before stdout; errors exit 2 without partial
results or query/value payloads. Parse/eval behavior is unchanged.

No native file/source adapter, VBA, GUI/network runtime or new registry dependency
was added. CI checks standalone scalar-schema and query builds using the existing
version-tagged actions. The CLI suites now have 18 tests, including seven new
retrieval tests for queries, bindings/collation, failures and output exhaustion.

Local verification on aarch64-apple-darwin on 18 October 2026 passed standalone
schema/query checks, minimal workspace Clippy/tests, the release build, rustdoc
with warnings denied, formatting and whitespace checks. Both documented examples
ran against the release executable with exact expected output. Public local links
across 28 Markdown files, six-package metadata, the normal CLI dependency closure
and the original ACCDB hash were checked. No new runtime dependency enters the
CLI. Native Access comparison, remote CI and other targets were not observed.

## Domain-oriented workspace

The workspace now groups `tablecraft-core` under `crates/foundation/` and the
SQL parser under `crates/query/tablecraft-sql`. The parser was renamed from
`tablecraft-query` to `tablecraft-sql`, including Rust imports, its CLI dependency,
lockfile and CI package selection. Parsing APIs and CLI behavior are unchanged.
The existing feature modules, tests and proptest regression seed moved with their
owning packages. No unused packages or dependencies were added.

[ARCHITECTURE.md](ARCHITECTURE.md) records the implemented package layout,
the target domain map and boundaries for progressive extraction. The future
`tablecraft-query` package is reserved for binding, planning and execution;
`tablecraft-expressions` now implements the initial scalar profile described
below. The planner was absent at that relocation milestone; the bounded retrieval
implementation is now described above. The private architecture
pack remains ignored.

Local verification on aarch64-apple-darwin with Rust 1.99.0 on 17 October 2026
passed locked/offline standalone core and SQL checks, minimal workspace
Clippy/tests, serde-enabled core Clippy/tests, the release build and rustdoc with
warnings denied. Formatting, CLI version/parsing smoke checks and local public
documentation links passed. The suites retain 45 SQL tests plus its doctest,
five CLI tests, and the existing core contracts and optional serde tests. All 71
relocated files retain their contents except for the intended parser manifest
and doctest import rename. Remote CI and other native targets were not observed.

## Headless build foundation

Implemented the minimal Cargo workspace, pinned Rust toolchain, clap-based
headless CLI, build/feature/licensing policy and three-platform CI configuration.
The MIT license is preserved.

The CLI implements help/version, syntax-only `query parse` and source-free scalar
`query eval`. Integration tests launch it
without display environment variables and check output streams, version identity
and rejection of unsupported commands/extra arguments.

Local verification passed on aarch64-apple-darwin with Rust 1.99.0 on 17 October 2026:

- rustfmt check and Clippy with warnings denied.
- All three CLI integration tests (help, version and invalid arguments).
- Locked, offline release build and executable version smoke check.
- Dependency tree contains clap and its derive dependencies; the CLI remains
  free of graphics, network and database runtimes. Versions, licenses, features
  and minimum toolchains are recorded in DEPENDENCIES.md.
- Git whitespace check.

Remote CI and native Windows/Linux execution have not yet been observed.
Database inspection/editing commands and a desktop host do not exist yet.

## Shared core types

`crates/foundation/tablecraft-core` implements distinct object/table/operation/cursor IDs,
table- and generation-scoped record locators, cursor bookmarks, scalar values,
typed validation failures and checked resource budgets. Errors use `thiserror`;
optional `serde` support is limited to scalar value exchange. The CLI consumes
values and resource budgets for scalar evaluation.

Exact Currency, 28-digit field Decimal, width-preserving integers, bit-preserving
floats, GUIDs and distinct legacy/extended timezone-free dates are implemented.
NULL stays separate from empty text, and conversion rejects expression-only
Empty/Missing/Error states. Value and locator diagnostic output redacts payloads.
[CORE_TYPES.md](CORE_TYPES.md) maps the implemented contracts to Microsoft primary
references and records source inconsistencies and remaining boundaries.

Local core verification on aarch64-apple-darwin with Rust 1.99.0 on 17 October
2026 includes 17 feature-local unit/property tests, three public-API contract
tests, four compile-fail doctests and the three existing CLI tests. With the
optional serde feature, seven additional serialization tests check typed round
trips and rejection of invalid constructor inputs. Eight proptest properties
across both configurations use 256 cases each with a fixed seed and regression
persistence. The exhaustive small-coefficient decimal sweep remains in place.
Formatting, Clippy in both feature configurations, minimal and serde-enabled
release builds, and rustdoc with warnings denied also passed locally.

Serde preserves scalar integer/bit representations without JSON-specific string
policies. JSON is a test-only format, with direct typed decoding for wide numeric
values. IDs, generations, expression states, locators, budgets and errors have
no serde implementations. Native codecs, production serialization protocols,
Access oracle comparison and native database query execution remain unimplemented.

The user-requested future library choices and decimal candidate assessment are
recorded in [LIBRARY_POLICY.md](LIBRARY_POLICY.md); those future libraries have
not been added as unused dependencies.

## Access data-query parser and owned model

`tablecraft-sql` implements `parse_query` for retrieval, parameter declarations,
unions, crosstabs, append/update/delete, make-table and schema queries. `parse_select`
remains a single-SELECT convenience entry point. Expressions, clauses, relations,
parameter/type syntax and statement variants are TableCraft-owned; upstream
parser/token types do not escape the adapter. Source is retained independently
from normalized SQL. See [SQL_PARSER.md](SQL_PARSER.md) for precise grammar,
limits, reference mapping and unsupported forms.

The Access Pratt grammar implements operator precedence explicitly, including
ampersand, integer division, Mod, Eqv and Imp. Numeric/date literals retain syntax
without floating conversion or calendar/locale interpretation. Function calls
preserve qualified names and wildcard/omitted arguments; GUID literals and
@@IDENTITY have distinct syntax nodes; LIKE keeps pattern syntax without assuming
wildcard mode. Joins, ordered UNION ALL modifiers, subqueries, fixed pivot headings
and external source/destination metadata survive round trips. Make-table queries
are rejected inside retrieval operands and distinguished as root action syntax.
CREATE/ALTER TABLE, indexes and named/inline constraints retain syntax without
applying schema changes. Saved views/procedures retain ordered parameter headers
and one parsed body; procedure recursion shares the parser budget. Identifier
classification uses Access SQL reserved words instead of upstream multi-dialect
keyword status, including ordinary ID/Ref names. No SQL is executed, and external
source strings are never opened.

Local parser verification on aarch64-apple-darwin with Rust 1.99.0 on 17 October
2026 passed formatting, Clippy with warnings denied, 45 query tests and the API
doctest. Tests cover each query family, structure/precedence, original source,
normalization, malformed/foreign rejection and bounded recursion across nested
queries. Six fixed-seed properties use 256 cases each with regression persistence.
The grammar uses SQLParser std only; recursive-protection and native stack helpers
were removed after moving expression grammar behind an explicit shared budget.

The CLI integrates the parser through clap Parser/Subcommand derives and
`tablecraft query parse "<SQL>"`. Integration tests check normalized output,
parameter/action syntax without execution, malformed/multiple-query rejection,
byte limits, help/version and invalid arguments. All five parse/help/version CLI tests pass locally.
The CLI dependency closure remains free of GUI, network and database runtimes.

The final local workspace checks also passed minimal workspace tests/Clippy,
serde-enabled core tests/Clippy (including seven serde round trips), the release
build, rustdoc with warnings denied, CLI version output, local public-document
links and diff whitespace checks. These checks cover this checkout on macOS.

Binding, schema/type validation, function evaluation, collation, wildcard matching,
native query execution and ACCDB query extraction remain unimplemented. No native Access
comparison or Windows/Linux execution evidence is available for this slice.

## Initial scalar expression semantics

`tablecraft-expressions` implements Boolean/NULL logic, exact numeric comparison,
explicit half-even integral coercion, checked Int64/Double arithmetic, exact
Currency addition/subtraction and multiplication by integers, and bounded
Text/NULL concatenation. Its direct scalar functions consume core values;
they do not use representational equality as SQL equality. Text comparisons
require an explicit collation. The supported carrier profile and deferred
Variant/locale/Decimal behavior are in [EXPRESSIONS.md](EXPRESSIONS.md).

Local formatting, Clippy with warnings denied and 12 scalar tests pass on
aarch64-apple-darwin with Rust 1.99.0 on 17 October 2026. Four properties use 256
fixed-seed cases each. No native Access comparison or query execution is claimed.
The package reuses core, thiserror and development-only proptest; no new registry
dependencies, serialization or runtime libraries were introduced.

The optional `sql` feature now evaluates supported owned expression trees over
host-supplied borrowed values. It supports arithmetic/comparison/logical/text
operators and IS NULL, with explicit unsupported errors for calls, subqueries and
other deferred forms. Binary operands evaluate eagerly in written order. The
evaluator checks cancellation per node, charges core step/Text-allocation budgets
before work, and enforces nesting and per-value byte limits. Its numeric literal
carrier choices are explicit, without claiming full native type inference.

With `sql`, nine more tests (one generated property) and an API doctest pass;
default and SQL-enabled Clippy checks pass with warnings denied. See
[EXPRESSIONS.md](EXPRESSIONS.md) for reference mapping and current limitations.

Final local checks passed locked/offline minimal workspace Clippy/tests,
SQL-enabled expression Clippy/tests, serde-enabled core Clippy/tests, the minimal
release workspace build, rustdoc with warnings denied, CLI version output,
formatting and public-document link/whitespace checks. Dependency-tree inspection
confirms that the default expression package excludes SQLParser/log. The CLI's
subsequent evaluator integration is described below. Other platforms and native
Access evaluation remain unverified.

## CLI scalar evaluation

`tablecraft query eval "<SELECT>"` now invokes the existing evaluator for one
source-free SELECT's scalar projections. Optional aliases label a two-line,
tab-separated result table; Text is quoted/escaped and NULL remains distinct
from empty text. The command rejects database sources, query clauses, parameters,
UNION, action/definition statements and unsupported expressions. It supplies no
field bindings or text collation. `query parse` retains its syntax-only behavior.

One shared budget covers all columns, with 1,024 steps and 65,536 cumulative
Text bytes; depth, per-value limits and the parser input ceilings still apply.
Output is buffered with a 1 MiB logical cap and fallible allocation. Any parse,
validation, evaluation or formatting failure exits 2 without partial stdout.
Projection errors identify the column index without exposing values or SQL.
No query clause is ignored or external content opened.

The CLI now consumes core values/budgets and the expression crate's `sql` feature.
No new registry dependencies were introduced. CI checks both standalone minimal
expressions and the SQL-enabled profile explicitly, since the CLI enables SQL
evaluation in the workspace's unified feature set. CLI command handling and output
policy live in feature modules; library semantics remain in the expression crate.

All 11 CLI integration tests pass locally: the five existing tests and six eval
tests covering results, NULL/text formatting, controls, unsupported query clauses,
eager errors, limits, diagnostics, help and absence of partial output.
The documented arithmetic/concatenation/NULL example was also run successfully.

Local verification on aarch64-apple-darwin with Rust 1.99.0 on 17 October 2026
also passed formatting, standalone minimal and SQL-enabled expression
Clippy/tests, workspace Clippy/tests, serde-enabled core Clippy/tests, release
build, rustdoc with warnings denied, CLI version output and public-document
links/whitespace checks. Dependency inspection confirms the CLI adds only the
existing local core/expression packages, with no Serde, GUI, async/network or
database runtime. Remote CI, other targets and native Access comparison were
not observed.

## Standalone native engine architecture

[NATIVE_ENGINE.md](NATIVE_ENGINE.md) now makes standalone Rust engine usability
an architectural requirement. The target map adds `engines/tablecraft-ace` and
a later `tablecraft-ace-ffi`, while `tablecraft-access` becomes the TableCraft
adapter. Reusable query components and backend-neutral contracts must not import
that adapter, application services or runtime; hosts compose concrete adapters.
The document separates Rust-first implementation, later C ABI/ODBC, engine scope,
publication requirements and native compatibility evidence.

This is a documentation/guidance change. The current four-package workspace is
unchanged. No engine, adapter, FFI, driver or public transaction API has been
implemented or published. The proposal's positional-parameter example is not a
working API; the current SQL parser still rejects positional `?` placeholders.
Private plans and the original ACCDB fixture remain untouched.

Documentation verification passed local links across 14 public documents, target
map counts (33 distinct package names across 13 domains), acyclic proposed
dependency arrows, rustfmt and whitespace checks. Locked/offline Cargo metadata
still lists the same four implemented packages. Rust runtime tests/builds were
not rerun for this documentation-only change; it introduces no native execution
or compatibility evidence.

## VBA and macro architecture

[VBA_COMPATIBILITY.md](VBA_COMPATIBILITY.md) adopts preservation, documented
execution and native authoring as separate compatibility tracks. The target map
adds `tablecraft-vba` and `tablecraft-macros`, with `tablecraft-automation` narrowed
to reusable host contracts. It records interpreter-first language work, project
metadata/unknown-stream preservation, module absence, DAO/DoCmd and form/report
events, cancellation/transaction boundaries, SQL callable injection and debugger
ownership. Required native data macros must work headlessly or affected writes
must fail explicitly. Execution authorization remains separate from preservation.

Microsoft primary references were reviewed on 18 October 2026 and linked in the
owning document. These are requirements and research inputs, not implemented
VBA, macros, module discovery/writing, DAO objects, callable registry or execution
evidence. The current four-package Cargo workspace, runtime behavior and original
fixture are unchanged; no interpreter dependencies or empty packages were added.

## Windows compatibility architecture

[WINDOWS_COMPATIBILITY.md](WINDOWS_COMPATIBILITY.md) adds the planned
`compatibility/tablecraft-wincompat` product boundary, distinct from engineering
oracle tooling. It records per-API Rust/emulated/native/service providers through
shared automation contracts, with VBA independent of concrete providers. Mechanism,
coverage and authorization are separate facts; partial, blocked and unsupported
calls cannot return fabricated success or retry around a denial.

The document covers inert dependency discovery, object dispatch, explicit virtual
paths, filesystem/environment/registry boundaries, qualified Declare signatures,
optional bridges, compatibility reporting and WIN-0 through WIN-5 acceptance.
It separates replicated data validation from replayed UI events/external side
effects, without allowing required data macros to be skipped. Microsoft primary
references were reviewed on 18 October 2026 and linked in the owning document.

No provider, COM/Win32 bridge, DLL loader, isolation helper or new runtime/library
dependency was implemented. The four working packages, CLI behavior, fixtures
and private planning material are unchanged. VBA and Windows tracks are design
requirements, not passing native or cross-platform compatibility evidence.

Documentation verification on 18 October 2026 passed local links across 16 public
documents, the target map (36 distinct package names in 14 domains), acyclic
arrows in all three engine/VBA/provider dependency diagrams, rustfmt and whitespace
checks. Locked/offline Cargo metadata still lists the same four working packages.
Rust runtime tests/builds and native comparisons were not rerun for these
documentation-only changes.

## Application compatibility contracts and corpus policy

The application review now has explicit future contracts for startup/session
state, imported Ribbon XML, DAO/ADO recordsets, advanced field behavior and native
distribution/signatures, linked from [ARCHITECTURE.md](ARCHITECTURE.md). Microsoft
primary references were reviewed on 18 October 2026, including build-qualified
form/report limits and the distinction between code and package signatures.

[ACCESS_APPLICATIONS.md](testing/ACCESS_APPLICATIONS.md) defines separate
open/preserve/execute/edit/save-and-reopen results, provenance/privacy handling,
oracle profiles and representative application selection. The existing MIT
example is its sole candidate, with every native operation unrun. No application
harness, native format support, new fixture, crate or dependency was added. Private
planning and the original fixture bytes remain unchanged.

Documentation verification passed local links across 22 public Markdown files,
the unchanged 36-package/14-domain target map, locked/offline metadata for the
four actual packages, original fixture SHA-256, rustfmt and whitespace checks.
Rust runtime suites and native Access comparisons were not rerun for this
documentation-only change.

## Application workflow priorities

[ROADMAP.md](ROADMAP.md) records P0–P4 priorities and bounded native inspection as
the next native implementation slice. Feature contracts cover reusable expression
editing, saved import/export versus linked tasks, dependency/refactoring analysis,
Documenter/Compare/search/application tests and templates/wizards. The roadmap
also deepens forms/reports, linked-source, native shared-file laboratory, P2P UX
and browser requirements, with explicit scope limits. Existing engine/VBA/Windows
documents remain canonical; the engine document adds public API and ODBC
qualification requirements.

This is documentation only. The parser and initial scalar evaluator already work;
visual editing, application tools and native database features remain future work.
There are no new crates, dependencies, application test runs or fixture changes.

Verification passed links across 27 public Markdown files, three acyclic proposed
dependency diagrams, the unchanged target map and four-package Cargo metadata,
fixture SHA-256, rustfmt and diff whitespace checks. Runtime suites/builds,
diagram rendering and native Access comparisons were not run for this doc-only
revision.

## Access example

The supplied [Access example](examples/Access_Example_VBA.accdb) is registered in
[FIXTURES.md](FIXTURES.md) with its
size and SHA-256. It matches upstream Access-projects/Access-examples commit
`d0ffd681d431f38d91c138559763c30df0699c55` byte for byte, and its original MIT
notice is retained. It has not been parsed, executed or qualified against Access.

Real Access comparison still requires a separately provisioned Windows oracle.
The fixture has not established native format or application compatibility.
