# Original-file progressive DISTINCT replay

The [original Python runner](compare_progressive_distinct.py) passes all 184
controls on four unchanged, 701-row `Ignore-V2000/V2007/V2016/V2019` inputs:
160 successful queries produce 10,432 rows / 18,208 cells, and 24 intended
refusals produce no stdout. The first complete run passed without SQL, model,
golden or allowance changes. [The accepted report](results/progressive-distinct-2026-10-25.json)
retains every command, expected result, actual stdout/stderr and exit code.

The files and ordinary-getter transcripts come from the existing
[ignore-NULL index corpus](results/ignored-null-index-replay-2026-10-25.json),
authored by the repository's original [public Jackcess recipe](IgnoreNullIndexFixture.java).
The getter opens read-only, disables expression evaluation and rejects linked
opens. This replay reuses those reviewed transcripts; it does not generate files
or run Java again. Only TableCraft executes SQL. Neither Jackcess metadata nor
these independently modeled answers establish native Access SQL qualification.

## Independent models and controls

Linear first-encounter models use the captured integer/NULL cells and ordinary
row encounter order. The five positive shapes cover numeric/NULL DISTINCT,
two-column tuples, Currency/Decimal conversion with TOP after deduplication,
a derived TOP before terminal DISTINCT, and grouped counts followed by an empty
global UNION ALL arm. Each runs buffered and progressively with batches 1, 2 and
128, under both arithmetic profiles. Each command compares directly with its
model; buffered execution is not a reference SQL oracle.

Currency conversions use exact integer keys rendered with four fractional places;
Decimal conversions retain scale zero. VarType controls independently require
Currency 6 and Decimal 14. The only modeled Mod operation uses getter IDs asserted
to be exactly 0–700, so its nonnegative remainder agrees with truncation-toward-zero
semantics. No float-based key equality or approximate numeric comparison is used.

The three refusal shapes run in both profiles with batch size 1: unsupported Text
DISTINCT, a late division failure despite TOP 1, and an unsupported later Double
DISTINCT arm before any earlier arm can publish. All keep empty stdout. Default
native/query limits remain unchanged; only explicit invariant number syntax and
stream batch sizes are supplied.

## Frozen artifacts and reproduction

The frozen executable is
`.cache/tablecraft-bin/read-query-distinct-wide-cli-20261025`, SHA256
`121d76588ed4e6c9497e23b9f42c6f9ca5e729aceb0a0e9491f153865fac3e4c`.
The selected query revision is
`b44494d4cd5a9350cfef1f1484148f16d5362bb7`.
The report independently pins 47 raw query-source objects from that revision,
with matching scoped live hashes before and after execution. This is a selected
query-source record, not a full executable source inventory. The frozen binary
hash identifies the actual executable, including the accompanying native changes.

All 18 immutable pins remain unchanged: the executable, reviewed JAR, prior
provenance report, seven original tool/recipe files, and four database/getter
pairs. The report SHA256 is
`0bfb5c4b7538f49a191e5c49fad5e27fe60832687294f565c1c33ff33138e6d4`.
The private command log is `.cache/progressive-distinct-replay-20261025.log`,
SHA256 `f6b66cb33f6b89e12c491f2e56a864518230e104d0aa3b2224b117c265c20c21`.
Session 83438 completed with exit 0. No failed replay attempt was discarded.

Use fresh output paths; the runner refuses overwrites and checks the original,
getter, JAR, tool and selected query-source hashes before and after replay:

```sh
python3 docs/testing/compare_progressive_distinct.py \
  --cli .cache/tablecraft-bin/read-query-distinct-wide-cli-20261025 \
  --cli-sha256 121d76588ed4e6c9497e23b9f42c6f9ca5e729aceb0a0e9491f153865fac3e4c \
  --revision b44494d4cd5a9350cfef1f1484148f16d5362bb7 \
  --originals .cache/tablecraft-fixtures/ignore-null-indexes-20261025 \
  --jar .cache/tablecraft-fixtures/jackcess-5.0.4.jar \
  --report .cache/progressive-distinct-reproduction.json \
  --log .cache/progressive-distinct-reproduction.log
```

The [implementation checks](results/read-query-progressive-distinct-2026-10-25.json)
separately cover exact representatives, logical EOF, source/sink failures,
cancellation and quotas. This replay adds original-file CLI composition evidence;
it does not expand the admitted key families, native comparison claims or writes.
