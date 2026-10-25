# Scalar String and Variant comparison replay

The [accepted independent replay](results/scalar-string-2026-10-24.json) passes
224 CLI controls: 144 source-free and 80 native integrations across original
Jet 4 (`V2000`), ACE 2007 (`V2007`) and ACE16-family (`V2016`, `V2019`) files.
There are 196 successful commands and 28 expected buffered refusals. Successful
results check 6,433 modeled Boolean cells and 718 NULL cells. All 1,869
pinned hashes remain unchanged, including every entry in the parent's 1,849-file
Rust source inventory, the executable, JAR, original files and getter captures.

This is engineering evidence for TableCraft's explicit `--declared-arithmetic`
profile, not Microsoft Access execution qualification. The comparison is only
modeled as ordinal String comparison after an explicitly requested rendering
policy. It does not establish native Windows NLS, LCID precedence or general
numeric/Text coercion. Default arithmetic and strict relational keys retain
their existing separate contracts.

## Independent model

Microsoft's [MS-VBAL §5.6.9.5 relational operators](https://learn.microsoft.com/en-us/openspecs/microsoft_general_purpose_programming_languages/ms-vbal/f8acd631-55c1-4199-bc1e-022aaab6d9c8)
distinguishes declared String/Variant effective String comparison from the
both-Variant numeric/String rank exception. The latter ranks a numeric value
before any String regardless of spelling; Boolean and Date have separate
contracts. Those clauses are a language-semantic reference, not proof of a
particular native Access query result.

[compare_scalar_string.py](compare_scalar_string.py) independently compares
Python strings with six operators, in both operand directions. The corpus uses
BMP/ASCII characters for which Python code-point order and the selected ordinal
engineering model agree. Complete strings are compared: trailing spaces are
retained. The model does not parse input spellings as numbers for the new branch.
`02 < 2` and `10 < 2` as formatted strings distinguish the new behavior from
numeric comparison. NULL returns NULL before formatting or comparer use.

Peer rendering uses these explicit short goldens:

| Peer | Selected format | Rendered String |
| --- | --- | --- |
| Integer 2 | invariant | `2` |
| Boolean True / False | no numeric format | `True` / `False` |
| Currency 1.25 | invariant / de-de | `1.25` / `1,25` |
| Midnight 2000-01-02 | ISO / en-us / de-de | `2000-01-02` / `1/2/2000` / `02.01.2000` |

Seven standalone CStr controls verify the same non-NULL rendered goldens against
the existing renderer. Expected comparisons are computed from the literal
goldens, not from CStr output. Native NULLs remain actual NULLs through CVar;
CStr/CVDate conversion of NULL is not used to manufacture a comparison operand.

Direct source-free and native vector projections test all six operators in both
directions for the seven peer/format combinations. The 13 source-free String/NULL
cases and 17 native rows yield 6,804 direct modeled comparison values, separately
from category, composition and refusal controls.

Additional controls preserve both-Variant rank, numeric Scalar/Variant String
numeric comparison, default-profile numeric behavior and malformed-input errors.
Unknown numeric/String aggregate categories refuse when branch selection depends
on their declarations; explicit CVar(Max(Number)) succeeds, while known numeric
Scalar with unknown String retains numeric conversion. These controls do not
infer categories from the returned scalar value.

Native integrations also cover IN, BETWEEN, joins, ANY subqueries, DCount
criteria and derived predicate projections. Missing number/date policies,
missing comparison policy, eager division errors and positive execution/output
limits retain their specific refusal diagnostics and empty stdout. The runner
checks positive byte/step/output limits; it does not independently qualify every
resource or cancellation path.

## Original ordinary-table corpus

[ScalarStringFixture.java](ScalarStringFixture.java) uses public Jackcess table
and column builders, expression evaluation disabled, and fresh output paths.
Each profile contains one TextOperands table with Int32 ID/Number, Short Text
Spelling, Boolean Flag, Currency Money and legacy Date D. Values include `02`,
`10`, `bad`, empty, `é`, negative/equal strings, trailing space, regional/date
spellings and NULL; a final row adds nullable numeric/Currency/Date peers and
Boolean False. There are 17 rows per original, 68 overall.

[ScalarStringOracle.java](ScalarStringOracle.java) reopens read-only, disables
expression evaluation and installs a throwing link resolver. It records public
column types and exact ordinary cell getters; text is UTF8/base64 so empty and
NULL remain distinct. Currency is recorded as exact BigDecimal, and Date uses
explicit LOCAL_DATE_TIME getters. The independent runner validates column types,
known peer values and nullable cells before deriving expected results. No
reference SQL, saved queries, imported expressions or external target are run.

The reviewed Jackcess 5.0.4 JAR is Apache-2.0, SHA-256
`19a7494a56f439a57fda85c059b019b665802085cec5ae8186d1e028cc1bdfbc`.
These original recipes/oracles follow the repository MIT license; no third-party
source or fixture bytes are copied. Original binaries, exact producer/getter
commands, stdout/stderr captures and source hashes remain under ignored
`.cache/tablecraft-fixtures/scalar-string-20261024/`. `generation.json` records
the actual fresh producer/getter commands and unchanged original hashes.

## Artifacts and reproduction

The accepted frozen executable is
`.cache/tablecraft-bin/declared-string-cli-20261024`, SHA-256
`8aa578c621fe989b5034853e946a942f3700cb90aee8be12ee3d4970866cc5ce`.
The complete source inventory is
`.cache/declared-string-rust-source-20261024.manifest`, SHA-256
`ec3f7808c617daf80477f51af569cdcde4d6e5c7b1cf9826b8de223b720a7024`.
Every listed source file is verified before and after the replay.

Compile the tools and generate each profile at a fresh path:

```sh
javac -cp .cache/tablecraft-fixtures/jackcess-5.0.4.jar \
  -d .cache/scalar-string-classes-20261024 \
  docs/testing/ScalarStringFixture.java docs/testing/ScalarStringOracle.java
java -cp .cache/tablecraft-fixtures/jackcess-5.0.4.jar:.cache/scalar-string-classes-20261024 \
  ScalarStringFixture .cache/tablecraft-fixtures/scalar-string-20261024/V2016.accdb V2016
java -cp .cache/tablecraft-fixtures/jackcess-5.0.4.jar:.cache/scalar-string-classes-20261024 \
  ScalarStringOracle .cache/tablecraft-fixtures/scalar-string-20261024/V2016.accdb
```

Repeat for V2019/V2007 and V2000 (`.mdb`). Capture getter stdout as
`V2016.accdb.getters.tsv`, stderr as `.getters.stderr`, and similarly for every
profile. Fresh file timestamps may change binary hashes; the report pins the
actual preserved originals.

```sh
python3 docs/testing/compare_scalar_string.py \
  --cli .cache/tablecraft-bin/declared-string-cli-20261024 \
  --cli-sha256 8aa578c621fe989b5034853e946a942f3700cb90aee8be12ee3d4970866cc5ce \
  --jar .cache/tablecraft-fixtures/jackcess-5.0.4.jar \
  --directory .cache/tablecraft-fixtures/scalar-string-20261024 \
  --report docs/testing/results/scalar-string-2026-10-24.json
```

The runner refuses an existing report. Accepted JSON preserves every command,
exit status, stdout, stderr and all hashes. The terminal log is
`.cache/scalar-string-final-20261024.log`. An excluded first probe used
CVDate(D) on the deliberately NULL native Date row and correctly failed with
invalid NULL conversion; native operands were corrected to CVar(D), which keeps
the original Date/NULL identity. Passing exploratory/source-inventory checkpoints
remain private and are not added to accepted counts. No original input was
patched and no production behavior was changed for those probes.

Broader Variant conversion, Error/Empty runtime scope, raw malformed String
providers, native comparisons and Windows qualification remain separate. Parent
Rust tests cover callback counts, raw String policy propagation, cancellation
and other engine/query contracts beyond this CLI corpus.
