# Implementation status

## Headless build foundation

Implemented the minimal Cargo workspace, pinned Rust toolchain, clap-based
headless CLI, build/feature/licensing policy and three-platform CI configuration.
The MIT license is preserved.

The CLI only implements help and version output. Integration tests launch it
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
Database commands and a desktop host do not exist yet.

## Shared core types

`crates/tablecraft-core` implements distinct object/table/operation/cursor IDs,
table- and generation-scoped record locators, cursor bookmarks, scalar values,
typed validation failures and checked resource budgets. Errors use `thiserror`;
optional `serde` support is limited to scalar value exchange. The CLI does not
yet consume these types.

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
Access oracle comparison and SQL evaluation remain unimplemented.

The user-requested future library choices and decimal candidate assessment are
recorded in [LIBRARY_POLICY.md](LIBRARY_POLICY.md); those future libraries have
not been added as unused dependencies.

## Bounded SQL parser

`crates/tablecraft-query` implements a bounded Access SELECT syntax adapter using
sqlparser 0.63.0. The owned model, original-source retention, normalized formatter,
typed payload-free errors and caller-lowerable limits are implemented separately
from upstream parser types. Single-table projections, aliases, ALL/DISTINCT/
DISTINCTROW/TOP, a small WHERE expression subset including BETWEEN/NOT BETWEEN,
and field ORDER BY are supported. BETWEEN keeps the tested expression, both
written bounds and negation flag without sorting or comparison rewrites.
[SQL_PARSER.md](SQL_PARSER.md) records the precise subset, source mapping and
rejected constructs. [TECH_STACK.md](TECH_STACK.md) records adapter boundaries.

The corpus uses original synthetic SQL derived from Microsoft grammar references.
Feature-local tests check acceptance/rejection, owned structure, normalized round
trips, original text, Unicode locations, malformed inputs and resource limits.
Four fixed-seed properties use 256 cases each. SQLParser's recursive-protection
feature is enabled after a stress test exposed a debug stack overflow without it;
input, nesting and expression limits remain independently enforced. Dependency
records describe the resulting native stacker/psm boundary.

Local verification passed on aarch64-apple-darwin with Rust 1.99.0 on 17 October
2026: standalone query check, workspace formatting and Clippy with warnings
denied, minimal workspace tests (48 including doctests), core Serde checks/tests,
workspace release build, rustdoc with warnings denied and CLI version smoke.
The query suite has 20 tests and one public-API doctest; its corpus covers 33
accepted and 70 rejected statements. BETWEEN tests cover reversed/equal bounds,
logical grouping, malformed/unsupported operands, depth limits on each operand
and generated i64 bounds; no evaluation or Access comparison is implied.
Both single- and double-quoted WHERE text
parse into the same Text node; formatting uses escaped single-quoted text while
original source remains unchanged. Double-quoted identifiers remain rejected.
The CLI production dependency tree remains
clap-only. Local documentation links and Git whitespace checks passed.

The CLI does not depend on or expose the query library. Binding, name resolution,
Access expression evaluation, query execution, SQL translation and extraction
from ACCDB remain unimplemented. No real Access comparison or Windows/Linux
execution has been observed for this slice.

## Access example

The supplied [Access example](examples/Access_Example_VBA.accdb) is registered in
[FIXTURES.md](FIXTURES.md) with its
size and SHA-256. It matches upstream Access-projects/Access-examples commit
`d0ffd681d431f38d91c138559763c30df0699c55` byte for byte, and its original MIT
notice is retained. It has not been parsed, executed or qualified against Access.

Real Access comparison still requires a separately provisioned Windows oracle.
The fixture has not established native format or application compatibility.
