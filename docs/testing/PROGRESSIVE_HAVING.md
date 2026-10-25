# Original native HAVING replay

The original [runner](compare_progressive_having.py) compares native/CAB queries
with independent linear grouping, integer and Fraction models. It reuses both
registered LargeDeployment ACE 16 family files and four unchanged stored/MSZIP
CAB packages. Existing inert public Jackcess getters validate every authored row;
the existing bounded CAB inspector verifies complete decoded image hashes. No
fixture is generated or modified, and only TableCraft executes SQL. This is
engineering evidence, not native Access SQL/error-order or timing qualification.

The [accepted report](results/progressive-having-2026-10-25.json) contains
**96 matched commands**: 72 successes comparing 336 rows and 696 cells, plus
24 intended failures. Six failures independently compare an earlier progressive
prefix of one row/two cells each; those six rows/twelve cells are recorded
separately from successful results. There were no disagreements or excluded runs.

Each input runs sixteen controls. Common modulo-17 grouped HAVING runs buffered
and progressively at batch sizes 1 and 128. Other controls cover five aggregates
used only by HAVING, the empty global population, exact mean predicates, False
and NULL suppression of projected `Sum(1/0)`, TOP counting accepted groups, and
HAVING over a static derived input under the declared arithmetic profile.

The failure controls distinguish suppression from eagerness. A later accepted
group must still evaluate a dividing projection after TOP 1 has emitted a prior
accepted group; batch size 1 makes that exact prefix visible. A HAVING aggregate
operand containing division by zero remains eager even beside False. An
unsupported Last aggregate and a low cumulative work allowance refuse without
stdout. Buffered output is checked against independent expected answers, not
used as the numerical oracle. CLI output alone does not instrument cursor entry;
focused Rust tests establish pre-scan admission and provider suppression.

Expected groups are built by a linear first-encounter search over IDs 1–1,100.
Counts/totals use integers; means use Fraction and an independent arithmetic-
progression endpoint identity. The original formula is `Score = 17 * Id - 9`.
Each invocation explicitly supplies 256 MiB native bytes and 10,000,000 native
steps; query ceilings remain unchanged except the low-work control. The replay
does not imply constant-memory HAVING: deferred selected operands may retain
paid candidate rows until the predicate accepts or rejects their group.

## Immutable provenance

The executable is `.cache/tablecraft-bin/progressive-having-cli-20261025`, SHA-256
`f6a86b005512d52520c9398853843be1d3e47d19b34f1758e09b0f6f40f1a89c`,
compiled revision `e28fd42cf33a64489106d9514fbc227b8ae1f9e4`.
The accepted report SHA-256 is
`6e66f857a015f5266ed812d851eb516b208170efc6fcaa75c5354ef282e3d556`.

All fifteen immutable pins were checked before/after the run and independently
rehashed afterward: executable, reviewed reader, original provenance JSON, six
inputs and six original tools. Thirty-four scoped compiled-revision source blobs
were independently verified using raw `git show` bytes. No live full-tree source
inventory is used. Original corpus provenance remains
`.cache/large-deployment-provenance-20261025.json`, SHA-256
`e5c4e1f575d96bd8552ce05f610505b6987ee0e1946ac740886468cec094d56b`.

Session 75659 completed successfully. Local output is retained at
`.cache/progressive-having-replay-20261025.log`. Every command retains its exact
arguments, expected rows/prefix/error fragment, exit status, stdout and stderr.
The runner refuses existing report destinations, verifies the explicit executable
hash, enforces a 100-command ceiling and applies 120-second process timeouts.

## Reproduction

Run from the repository root using a fresh destination:

```sh
python3 docs/testing/compare_progressive_having.py \
  --cli .cache/tablecraft-bin/progressive-having-cli-20261025 \
  --cli-sha256 f6a86b005512d52520c9398853843be1d3e47d19b34f1758e09b0f6f40f1a89c \
  --revision e28fd42cf33a64489106d9514fbc227b8ae1f9e4 \
  --jar .cache/tablecraft-fixtures/jackcess-5.0.4.jar \
  --directory .cache/tablecraft-fixtures/large-deployment-20261025 \
  --provenance .cache/large-deployment-provenance-20261025.json \
  --report docs/testing/results/progressive-having-2026-10-25.json
```
