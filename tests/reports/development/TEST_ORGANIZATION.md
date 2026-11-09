# Engineering test organization

The former `docs/testing` engineering material now lives under repository-root
`tests/`. Access tools, reports and captured results are grouped by domain;
Access tools are further grouped by feature. The documentation area retains a
short [guide](../../../docs/testing/README.md).

Rust unit and integration tests retain their existing owning Cargo package paths.
No Cargo test targets were added or relocated. Private test visibility and shared
`tablecraft-testkit` ownership remain unchanged.

## Current tool execution

Use [`tests/run.py`](../../run.py) for Python entry points. Its shared
[current-path helper](../../../scripts/engineering_paths.py) discovers the checkout
root and configures the current feature import paths. Explicit Java recipes use
current repository paths; dynamic bare recipe names must have exactly one current
match. Missing, ambiguous and escaping paths are errors.

There is no relocation registry, old-path fallback, archived copy of each moved
tool, inverse source transformation or substituted hash. Existing executed/witness
sources remain their original evidence. Captured result records retain their
original bytes and hashes, including historical source paths. Moving a tool does
not make an old comparison a fresh execution; verifiers requiring old source pins
may need a new recorded run. Windows/Access observations require the actual
external application and were not rerun during this organization change.

## Duplication findings

A fresh AST inventory parsed 762 Python engineering files and found 22 groups of
identical complete top-level functions of at least 19 source lines. The
[current inventory](../../results/development/test-organization-2026-11-09.json)
records exact paths and line ranges. This identifies duplicated machinery, not
proof that the test cases exercise identical behavior.

Concrete candidates include:

- `derive` in the variable, GUID tuple and decimal index models: one long identical
  raw derivation body, with different surrounding source domains.
- `analyze` in repeated and integral split action models: identical tree analysis.
- `parse` in the global-bootstrap, long-value-map-growth and external-long-value
  insert source models: identical source parsing.

Some duplicates include intentionally retained executed/witness snapshots. Those
must not be replaced merely to reduce a count. No assertions or tests were removed;
no function was called tautological without demonstrating that its expected value
came from the same implementation being tested. Independent raw BEFORE/AFTER
oracles remain independent of production codecs.

## Fresh checks

The affected Python helper suites pass 35 existing tests, three new current-path
guard tests and four directly exercised primitive-model tests (42 total). Eleven
current entry-point help checks also pass. Current entry-point help checks and Python syntax checks
run without launching Java, a native application, Cargo or Git. Documentation
links are checked separately. These checks establish tooling behavior, not native
database or Windows/Access compatibility.
