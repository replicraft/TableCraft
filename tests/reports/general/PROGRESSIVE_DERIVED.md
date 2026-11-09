# Original static pipeline replay

The original [runner](compare_progressive_derived.py) checks native static
SELECT/WHERE/TOP pipelines against independent integer and Fraction models.
It reuses the two registered LargeDeployment ACE 16 family files and their four
unchanged stored/MSZIP CAB packages. Existing read-only public Jackcess getters
verify every authored row and ordinary value; the existing bounded CAB inspector
verifies complete decoded image hashes. No fixture is generated or modified.
Only TableCraft executes SQL: this is engineering correctness/resource evidence,
not native Access SQL or timing qualification.

The [accepted report](results/progressive-derived-2026-10-25.json) records
**96 matched commands**: 72 successful queries comparing 420 rows and 1,410 cells,
and 24 intended refusals. There were no disagreements or excluded attempts.

Each of six inputs runs sixteen controls. Nested inline stages exercise written
filter/alias order and separate positive TOP counters, buffered and progressive
at batch sizes 1 and 128. Further controls cover outer global/grouped aggregates,
flat UNION ALL carrier observations, and a declared typed parameter. A late
unused inner division error must still fail despite inner TOP 1 and outer WHERE
False; inner aggregate/order shapes and a low cumulative query work allowance
must refuse with empty stdout.

The named `Stage` route is an explicitly requested **ordinary-column host view**:
three `--recomputed-column Stage LargeDeployment COLUMN` selections expose Id,
Score and Label. It supplies a captured named SELECT without changing the file.
This is not a stored native saved-query definition or a newly computed formula.
Native saved-definition and external lexical-owner boundaries have separate Rust
and CLI coverage; this original replay does not claim to establish those cases.

Expected rows use the original ID progression, `Score = 17 * Id - 9` and literal
Label formatting. Linear first-encounter grouping and Fraction arithmetic provide
independent aggregate answers, with an arithmetic-progression endpoint check on
every mean. Buffered TableCraft output is compared to the same model and is not
the oracle. Every command explicitly supplies 256 MiB native bytes and 10,000,000
native steps; ordinary query ceilings remain unchanged except the low-work case.

## Frozen provenance

Executable: `.cache/tablecraft-bin/progressive-derived-cli-20261025`, SHA-256
`601802625ebf1448150856a7e2f724c0ae58f8118860f466b9c4034a0c3bf300`.
Compiled revision: `9882b9071d5dda849e575d0326c1eea9b62f967a`.
Report SHA-256:
`c8e8958ed119d90f7a83429a45629a6a9404081bea7dc450bba0b9e6b3910442`.

All fourteen immutable pins were rehashed before/after execution and independently
verified afterward: the executable, reviewed reader, original provenance, six
inputs and five original tools. Thirty-one scoped compiled-revision source blobs
were also independently verified using raw `git show` bytes. No mutable live
source inventory is used to claim what the executable contains. Original corpus
provenance remains `.cache/large-deployment-provenance-20261025.json`, SHA-256
`e5c4e1f575d96bd8552ce05f610505b6987ee0e1946ac740886468cec094d56b`.

Session 59868 completed successfully. Local output is retained at
`.cache/progressive-derived-replay-20261025.log`. Every CLI command retains its
arguments, expected rows/error fragment, exit status, stdout and stderr, including
matched refusals. Processes have a 120-second timeout and the runner enforces a
100-command ceiling before execution. CLI refusal output alone does not measure
cursor activation; instrumented Rust tests establish that internal boundary.

## Reproduction

Run from the repository root with a fresh report path; the runner refuses to
overwrite an existing report and checks the explicit frozen executable hash.

```sh
python3 docs/testing/compare_progressive_derived.py \
  --cli .cache/tablecraft-bin/progressive-derived-cli-20261025 \
  --cli-sha256 601802625ebf1448150856a7e2f724c0ae58f8118860f466b9c4034a0c3bf300 \
  --revision 9882b9071d5dda849e575d0326c1eea9b62f967a \
  --jar .cache/tablecraft-fixtures/jackcess-5.0.4.jar \
  --directory .cache/tablecraft-fixtures/large-deployment-20261025 \
  --provenance .cache/large-deployment-provenance-20261025.json \
  --report docs/testing/results/progressive-derived-2026-10-25.json
```
