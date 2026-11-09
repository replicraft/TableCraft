# Saved queries with up to 32 declared sources

The saved SELECT/crosstab adapter now admits 32 declared table sources using a
32-bit forest membership mask. It retains its separate limit of 16 native join
fragments. Microsoft's [Access specifications](https://support.microsoft.com/en-us/access/access-specifications)
list 32 tables and 16 joins per query, with lower maxima possible for multivalued
lookup fields. This slice does not establish every query shape at those product
limits or native Access compatibility.

[SavedManySourcesFixture.java](SavedManySourcesFixture.java) creates four fresh
Jet 4/ACE 2007/ACE 16-family files through the reviewed Jackcess 5.0.4 artifact.
Its public table builder writes one ordinary Int32 table containing ID 7. Original
inert MSysObjects/MSysQueries rows declare repeated references to that table under
distinct aliases. This exercises declared-source identity and the last mask bit;
it is not a fixture with 32 different physical user tables.

Seven definitions cover 17/32-source comma SELECTs, a 32-source forest with 16
disconnected INNER joins pairing the first and last aliases, and a 32-source
comma crosstab. Additional 33-source SELECT/crosstab and 17-join definitions are
explicit out-of-profile controls, not claims that Access accepts those shapes.
Every fixture uses original repository code and inert data. No third-party
fixture bytes or implementation source are copied.

The read-only getter mode disables expression evaluation and rejects linked
opens. It records the ordinary Int32 getter, query family, exact stored table/join
fragment counts and the reference renderer's SQL. Reference SQL never executes.
The [runner](compare_saved_many_sources.py) compares TableCraft reconstruction
with that independently assembled SQL normalized by TableCraft's existing parser.
It separately models execution from the single ordinary getter: each self-source
has one row, the joins match ID 7, and the crosstab counts one combination. This
normalization comparison is not a second independent SQL parser.

The original private 17-source V2016 probe reproduced the old refusal before the
runtime change: the getter rendered 17 sources, while the frozen CLI returned
unsupported metadata without stdout. Private probe files remain separate from the
final four-file corpus. No byte mutations are used to qualify these declarations.

Focused Rust coverage checks complete owned models and execution for 17/32 comma
sources, all 32 membership bits across 16 disconnected joins, the 17th-join
refusal, shared crosstab behavior, the 33rd-source refusal, syntax/allocation/work
ceilings and early/mid/final cancellation. The existing source-boundary test now
includes 17, 32 and 33. The source count rejects 33 before constructing a member
mask. Separate query depth, projection, binding, execution and native read limits
remain active.

The first scoped test attempt was exploratory: its two new execution expectations
hit the previous query layer's 16-FROM-root ceiling after successful reconstruction.
A subsequent temporary test expectation documented that independent ceiling.
The query layer now separately admits 32 FROM roots; accepted final checks use
actual successful comma execution. Neither exploratory attempt is final evidence.

The corpus, compiled helper, exact generation commands and getter captures remain
under ignored `.cache/tablecraft-fixtures/saved-many-sources-20261024-qualified/`. The runner
requires a fresh report, verifies the reviewed reference JAR, compares fresh getter
captures, checks exact error classes and empty refusal stdout, and verifies pinned
original/source/executable hashes before and after every replay. No database bytes
are modified by reading, rendering or TableCraft query execution.

```sh
python3 docs/testing/compare_saved_many_sources.py \
  --jar .cache/tablecraft-fixtures/jackcess-5.0.4.jar \
  --cli path/to/frozen/tablecraft \
  --directory .cache/tablecraft-fixtures/saved-many-sources-20261024-qualified \
  --report path/to/fresh-report.json
```

Add `--generate` with a fresh directory to compile the original Java helper,
author all four files and capture getters before running the same checks. Without
that flag, existing original bytes and getter captures are reused unchanged.

## Accepted engineering results

The [final replay](results/saved-many-sources-2026-10-24.json) passes 60 CLI
commands: 16 independent-rendering comparisons, 16 getter-modeled executions and
12 boundary refusals, plus the 16 parser normalization calls used by the rendering
comparisons. These are 48 successful commands and 12 intended refusals, with 20
modeled result cells and zero disagreements. All 17 scoped pins remain unchanged,
including the four original databases, fresh getter captures, compiled helper,
source recipes, three runtime files and frozen executable. Report SHA-256 is
`739894e8c88c4346b6e45ac4913839594e254c0bece91c3fc0cafc38c06a15ec`.

The immutable CLI is `saved32-selector-cli-20261024`, SHA-256
`2762809bf145adcfc3631a713ee36b3008e63503d9c94c19570dec9bc6322d7f`.
The parent separately verified its 1,897-file Rust source inventory against
committed archive `4443cd37`; the compact replay pins only its three directly
relevant runtime files and does not claim to rerun the entire Rust suite.

The final saved adapter target passes 89 tests, including the four new source
boundary tests, and strict adapter-library/target Clippy passes. The independent
query-layer change also passes its focused FROM/crosstab/retrieval checks. Final
adapter logs remain in ignored `.cache/saved-many-sources-tests-composed-final-20261024.log`
and `.cache/saved-many-sources-clippy-composed-final-20261024.log`.

The first CLI probe used output alias `Last`. The reference renderer emitted it
bare, outside TableCraft's reserved-name parser policy, so normalization refused
before checking reconstruction. Its four originals, compiled helper, exact
recipes and failure log remain untouched in a separate exploration directory.
Fresh qualified originals use neutral alias `Chosen`. A subsequent successful
report with one absolute runner pin label is likewise retained privately; only
that label was normalized before the fresh accepted replay over unchanged inputs.
These probes are excluded from the accepted 60 commands.

Windows Access acceptance, query execution comparison and native save/reopen are
unrun. Header-family coverage and independent getters remain engineering evidence.
