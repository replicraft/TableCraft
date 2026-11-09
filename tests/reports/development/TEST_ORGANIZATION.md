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

## Live helper deduplication

The variable, GUID tuple and decimal index insertion models now share the raw
image construction body in
[`shared_insert/derive.py`](../../access/indexes/shared_insert/derive.py). Each
feature keeps its own request, record, key, eligibility and tree policy. Its
named constructor binds thirteen callbacks/constants explicitly; the shared
algorithm imports neither those feature modules nor production encoders.

The extracted body has the same AST as all three former bodies. Wrapper keyword
bindings were checked against the actual prior function globals. A fresh replay
compares old/new complete images and facts for 118 retained source/request cases
(38 variable, 32 GUID tuple, 48 decimal), then checks the complete results against
pinned cached image and fact goldens. All agree. This is model extraction
regression evidence, not a new JVM, Rust writer, Windows or native Access run.
See the [fresh equivalence record](../../results/development/shared-insert-derive-equivalence-2026-11-09.json).

The GLOBAL-bootstrap, long-value-map-growth and external-long-value source models
now share only tagged-getter parsing and raw LVAL inventory traversal in
[`shared_long_values/source.py`](../../access/storage/shared_long_values/source.py).
Each source model still binds its own label decoder, numeric reader and page size;
its source-specific qualification, expected rows, map ownership and allocation
requirements remain separate. Both extracted bodies are AST-identical to their
former implementations. Sixteen retained source/getter cases (4 bootstrap,
8 map-growth, 4 external) yield identical complete old/new qualification states;
unknown getter tags also retain exact refusal categories. See the
[fresh source equivalence record](../../results/development/shared-long-value-source-equivalence-2026-11-09.json).

Repeated and integral split actions now share index inspection, complete-map
membership decoding and action analysis in
[`shared_split/actions.py`](../../access/indexes/shared_split/actions.py).
The families still supply their own typed source model, numeric readers and
qualification functions. The three bodies retain their original ASTs, including
the complete row, key, counter, ownership and untouched-byte checks. Four repeated
and eight integral retained actions produce identical complete old/new facts and
match their pinned original source/reference/getter analysis. See the
[fresh action equivalence record](../../results/development/shared-split-actions-equivalence-2026-11-09.json).

### Concrete retained boundaries

- Group 15 has two executed nullable-split `tree` sources and no live duplicate.
  Replacing either would change the actual code recorded for that execution.
- Groups 16–18 each have one live macro-XML verifier body and one retained witness.
  There is no second live implementation to consolidate; extracting the single
  live body would only add forwarding code while leaving the witness unchanged.
- Group 22 has separate public-reference and native-transition `ordered_intent`
  expectations. They are different evidence paths: the native proof must not
  obtain its expected ordered write/refusal/warning results from the same helper
  as the public-reference proof. Their separate ownership is intentional even
  where the present algorithms agree. Common transport primitives may be shared;
  expected-transition authority remains separate.
