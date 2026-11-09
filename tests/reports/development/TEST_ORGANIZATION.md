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

Groups 2–5 now have live shared owners: finite independent key weights in
[`shared_keys/weights.py`](../../access/indexes/shared_keys/weights.py), raw map
cells/membership in [`shared_maps/cells.py`](../../access/storage/shared_maps/cells.py),
and metadata-only tree traversal in
[`shared_trees/metadata.py`](../../access/indexes/shared_trees/metadata.py).
Weights keep each feature's truncation policy bound explicitly. Policy-free map
and metadata traversal use direct dependency imports rather than forwarding
wrappers. The map owner retains the same reviewed 4,096-byte format constant.

All four bodies retain exact prior ASTs. Six finite weight inputs are checked
through five policy bindings (30 calls); inline/indirect literal maps and pinned
retained map rows check cell locations, complete cardinalities and file membership.
Thirty-six retained tree descriptions are checked through six consumers (216
calls). These are reused case/consumer combinations, not 216 independently
observed database cases. See the
[fresh primitive equivalence record](../../results/development/shared-primitive-model-equivalence-2026-11-09.json).

Groups 6–8, 12 and 13 now share their respective raw tree/renderer mechanics,
with distinct owners for
[model trees](../../access/indexes/shared_trees/model.py),
[reference trees](../../access/indexes/shared_trees/reference.py),
[integral framing](../../access/indexes/shared_trees/integral.py),
[candidate decoding](../../access/indexes/shared_split/candidates.py) and
[page rendering](../../access/indexes/shared_split/render.py).
Typed `decode_entry`/`decode_key`, numeric readers and source limits remain bound
by their original feature. Model and reference validation bodies remain separate;
this does not turn two independent proof paths into one decoder.

Fresh checks compare exact old/new bodies/bindings and decoded results over pinned
retained sources and expected images: 384 model-tree calls, 320 reference-tree
calls, 216 integral-framing calls and 216 candidate-tree calls. Those are consumer
and before/after combinations, not new native observations. Complete construction
also still matches 96 scalar/GUID/decimal image-and-fact goldens plus eight complete
child/general-height-one renderer goldens. See the
[fresh raw tree/render equivalence record](../../results/development/shared-raw-tree-render-equivalence-2026-11-09.json).

The final live groups, 14 and 19, now have shared
[optional Git-object protocol verification](../../access/shared/shared_provenance/git_objects.py)
and [saved-verifier CLI handling](../../access/shared/shared_provenance/saved_cli.py).
The Git helper keeps the same canonical checkout root and standard-library
bindings; callers import its actual owner directly. Each CLI still supplies its
own description, defaults and replay function explicitly.

The original bodies are AST-identical. Seven simulated bounded Git protocol
scenarios are compared through three consumers (21 comparisons), covering exact
blobs, framing/type/digest failures, injected query newlines and byte ceilings.
No Git process is executed. Eight CLI scenarios through three consumers (24
comparisons) preserve success, missing evidence, handled errors, help and unknown
error propagation, including return values and stdout/stderr. See the
[fresh protocol/CLI equivalence record](../../results/development/shared-provenance-cli-equivalence-2026-11-09.json).

## Disposition of all 22 original groups

| Original group | Disposition |
| --- | --- |
| 1 | Shared raw insertion body; feature codecs remain bound separately |
| 2 | Shared finite weights; original truncation callbacks remain bound |
| 3–4 | Shared raw map primitives; direct imports, no forwarding wrappers |
| 5 | Shared metadata-only tree traversal |
| 6 | Shared within model raw-tree family |
| 7 | Shared within reference raw-tree family; separate from group 6 |
| 8 | Shared integral raw framing |
| 9–11 | Shared split action inspection and complete membership |
| 12 | Shared within candidate tree family; separate typed decoder callbacks |
| 13 | Shared independent page renderer |
| 14 | Shared optional Git-object protocol verifier |
| 15 | Two immutable executed sources; no live duplicate to extract |
| 16–18 | One live body plus immutable witness; no second live owner |
| 19 | Shared saved-verifier CLI with explicit feature defaults/replay |
| 20–21 | Shared getter parsing and raw LVAL inventory |
| 22 | Separate public/native expected-transition oracles retained intentionally |

Seventeen live duplicate groups now have shared implementations. Five groups
retain the concrete execution/oracle boundaries above. Thin feature binders are
not alternate algorithm bodies: they retain their own typed codec, source policy
or CLI defaults. This concludes the bounded engineering-helper review; it does
not claim a semantic audit of every Rust test in the repository.

## Final batch and measured scope

The final affected Python batch passes all 42 tests with zero failures, errors or
skips. Fourteen current entry-point help checks pass, and all 775 engineering
Python files parse. The post-extraction scan finds eight long-function AST groups:
five are the retained witness/oracle boundaries already explained; three are thin
`derive`, `analyze` and `decode_tree` policy binders. Those binders select their own
feature-owned readers/codecs and preserve closed public signatures; there is only
one corresponding shared algorithm body. Replacing them with a generic mutable
module/context registry would hide that binding rather than remove another
algorithm implementation.

The 32 changed Python consumer files shrink from 10,591 physical lines to 8,843;
the 13 shared Python owners add 1,095 lines, giving a net reduction of **653
physical Python source lines**. This includes comments, docstrings and blank lines;
it excludes Markdown, JSON receipts, unrelated files and Rust test coverage. It
is not a Rust SLOC estimate or a count of tests removed. Final source hashes and
remaining-group dispositions are recorded in the inventory's
`post_deduplication` section.
