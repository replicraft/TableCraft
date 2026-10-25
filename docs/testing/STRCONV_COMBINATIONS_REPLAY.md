# Combined StrConv Unicode and native-field replay

The [5,528-command record](results/strconv-combinations-replay-2026-10-25.json)
passes both CLI query modes and four registered native inputs. It compares
16,436 successful result cells and retains 176 expected buffered failures.
Twenty-eight source/tool/executable pins and four original database hashes remain
unchanged. This qualifies the stated engineering composition against independent
models; Microsoft Access and Windows NLS execution remain unrun.

## Independent expectations

The original [runner](compare_strconv_combinations.py) imports existing original
reference models, never production Rust mappings. Python Unicode 16.0.0 supplies
width decomposition properties and kana names/canonical decomposition. Selected
Python casing and separator models supply the explicit upper/lower/proper-case
expectations. Six short literal goldens check model composition before execution.

All 32 Japanese shape/case settings run against 251 input strings: mixed Latin,
Japanese, compatibility characters, NUL/separators, all 63 halfwidth Japanese
characters, 86 basic Hiragana positions and 90 Katakana positions. Complete raw
ranges and NULL add controls for every setting. This gives 8,020 successful
scalar cases and 88 expected refusals in each of `query eval` and `query run`.
The refusals include unqualified kana/width forms as well as invalid locales,
contradictory/encoding flags, malformed UTF-16 and eager operand failures.

The reference follows the documented TableCraft order: case, explicit widening,
script selection, requested narrowing. It distinguishes 36 from 40 on halfwidth
Katakana; widening is never inferred from a narrow/Hiragana request. This is
composition evidence, not proof of a universal native transformation order or
Windows casing equivalence. The existing mapping boundaries remain explicit.

Four native checks use read-only Jackcess getters for actual stored Label values
in Jet 4, ACE 2007 and ACE 2016/2019 Scalar fixtures. Each SELECT projects ID and
all 32 transformations, preserving empty Text and NULL. The getter evaluates no
SQL or stored code. All original hashes are checked before and after reads.

## Reproduction and limits

The frozen debug CLI is built from `8389ed63` (the report records the complete
revision), with SHA-256
`2a143bd8cab69b2fca85ba60533a06d7ebd23addb6e46479b0dd371f996fd35d`.
The complete production tree matches that revision. No product code, constructor,
default allowance, expected model result or original input changed during replay.

```sh
python3 docs/testing/compare_strconv_combinations.py \
  --cli .cache/tablecraft-bin/strconv-combinations-cli-20261025 \
  --jar .cache/tablecraft-fixtures/jackcess-5.0.4.jar \
  --directory .cache/tablecraft-fixtures/scalar-seek-20261023 \
  --revision 8389ed63 \
  --report .cache/strconv-combinations-new-replay.json
```

Reports must be fresh. The initial 32-column batch exceeded the existing scalar
eval work allowance; the final runner retains every case and uses three columns
per command. A subsequent preflight rejected the expanded command count against
an author-selected 5,000-command tool ceiling. Its final 6,000-command ceiling
admits the complete 5,528-command run; product limits stay unchanged. Both initial
logs are preserved in the record. Earlier/failed attempts are not added to totals.

Each command records its mode, deterministic case labels, SQL/expected/output
digests, compared-cell count and exit status. Every expected failure has no stdout.
The pinned runner and Unicode version reconstruct the exact queries. Native
getter and input identities are included. No reference JAR, generated database,
new product dependency or machine-specific path is committed. The
[fixture registration](../FIXTURES.md#interrupted-seek-scalar-replay) retains the
existing provenance. Broader mappings, locale providers and native qualification
remain on the read/query roadmap.
