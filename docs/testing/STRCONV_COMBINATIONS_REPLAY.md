# Combined StrConv Unicode and native-field replay

The [5,680-command record](results/japanese-proper-space-replay-2026-10-25.json)
passes both CLI query modes and four registered native inputs. It compares
17,280 successful result cells and retains 176 expected buffered failures.
Thirty source/tool/executable pins and four original database hashes remain
unchanged. This qualifies the stated engineering composition against independent
models; Microsoft Access and Windows NLS execution remain unrun.

## Independent expectations

The original [runner](compare_strconv_combinations.py) imports existing original
reference models, never production Rust mappings. Python Unicode 16.0.0 supplies
width decomposition properties and kana names/canonical decomposition. Selected
Python casing and separator models supply the explicit upper/lower/proper-case
expectations. Eight short literal goldens check model composition before execution.

All 32 Japanese shape/case settings run against 258 input strings: mixed Latin,
Japanese, compatibility characters, NUL/separators, all 63 halfwidth Japanese
characters, 86 basic Hiragana positions and 90 Katakana positions. Complete raw
ranges and NULL add controls for every setting. This gives 8,244 successful
scalar cases and 88 expected refusals in each of `query eval` and `query run`.
The refusals include unqualified kana/width forms as well as invalid locales,
contradictory/encoding flags, malformed UTF-16 and eager operand failures.

The Japanese proper-case model adds U+3000 explicitly to the standard separator
set; default ASCII reference callers retain their previous behavior. New cases
cover fullwidth and mixed Latin/Japanese word boundaries, repeated ideographic
spaces, NBSP/EM SPACE distinctions and titlecase expansion.

The reference follows the documented TableCraft order: case, explicit widening,
script selection, requested narrowing. It distinguishes 36 from 40 on halfwidth
Katakana; widening is never inferred from a narrow/Hiragana request. This is
composition evidence, not proof of a universal native transformation order or
Windows casing equivalence. The existing mapping boundaries remain explicit.

Eight native checks use read-only Jackcess getters for actual stored Label values
in Jet 4, ACE 2007 and ACE 2016/2019 Scalar fixtures. Each file runs one SELECT
over stored Label and one over an explicit IIf/concatenation expression adding
an ideographic-space WORLD suffix while retaining NULL. Each projects ID and all
32 transformations. This tests real field evaluation and query composition,
without claiming that the original fields contained stored Japanese delimiters.
The getter evaluates no SQL or stored code. All original hashes are checked before and after reads.

## Reproduction and limits

The frozen debug CLI is built from `9b256bf6` (the report records the complete
revision), with SHA-256
`f54042f115c43b35c413798b1d93a01bc6f4beded39822987f38d260e540b5a3`.
The complete production tree matches that revision. No product code, constructor,
default allowance, expected model result or original input changed during replay.

```sh
python3 docs/testing/compare_strconv_combinations.py \
  --cli .cache/tablecraft-bin/japanese-proper-space-cli-20261025 \
  --jar .cache/tablecraft-fixtures/jackcess-5.0.4.jar \
  --directory .cache/tablecraft-fixtures/scalar-seek-20261023 \
  --revision 9b256bf6 \
  --report .cache/strconv-combinations-new-replay.json
```

Reports must be fresh. The runner uses three columns per scalar command under
the existing eval work allowance. Its 6,000-command tool ceiling admits all
5,680 commands without raising product limits. This expanded run passes without
changing expected answers or production code during execution.

The earlier [5,528-command record](results/strconv-combinations-replay-2026-10-25.json)
remains evidence for the prior `8389ed63` profile: 16,436 cells, 176 expected
failures and 32 unchanged original/source/tool pins. Its initial batch-size/tool
ceiling failures remain preserved there. The expanded current run includes all
original cases plus new Japanese word cases and composed native field checks;
repeated observations are not added together.

Each command records its mode, deterministic case labels, SQL/expected/output
digests, compared-cell count and exit status. Every expected failure has no stdout.
The pinned runner and Unicode version reconstruct the exact queries. Native
getter and input identities are included. No reference JAR, generated database,
new product dependency or machine-specific path is committed. The
[fixture registration](../FIXTURES.md#interrupted-seek-scalar-replay) retains the
existing provenance. Broader mappings, locale providers and native qualification
remain on the read/query roadmap.
