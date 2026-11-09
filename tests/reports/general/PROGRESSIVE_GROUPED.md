# Original native grouped streaming replay

The original [runner](compare_progressive_grouped.py) checks buffered and explicit
progressive grouped queries against independent Python models over the registered
LargeDeployment originals. It reuses two immutable ACE 16 family native files
and their four original stored/MSZIP CAB packages. No fixture is generated or
modified. Existing read-only Jackcess getters validate every ID, Score, Label,
Memo and row position against the original public-builder recipe; bounded CAB
inspection checks complete expanded contents against each original image hash.
Only TableCraft executes SQL. These results are engineering evidence, not native
Access SQL compatibility or timing qualification.

## Accepted results and models

The [accepted report](results/progressive-grouped-2026-10-25.json) contains
96 matched commands: 72 successful queries comparing 1,374 rows and 5,364 cells,
and 24 intended refusals. Thirteen immutable pins and twenty-five scoped compiled
revision source blobs were independently rehashed after completion.

Each of the six inputs runs the same sixteen controls:

- `Id Mod 17` grouping with Count, Sum, Avg, Min and Max, buffered and progressive
  at batch sizes 1 and 128.
- Nullable two-component keys, buffered and progressive, preserving the first
  encountered group order and SQL NULL grouping.
- Per-group `Min(CStr(Id))` and `Max(CStr(Id))`, using explicit invariant number
  formatting and ordinal comparison, buffered and progressive.
- Prior projection aliases, final TOP 3, and Count/Sum/Avg carrier observations,
  buffered and progressive.
- Grouped UNION ALL promotions with original Min versus explicit Double Max,
  buffered and progressive; a further progressive control uses the declared
  arithmetic profile.
- A refused Text grouping key and three independently low query row, byte and
  work allowances, all with empty stdout.

The model visits IDs 1 through 1,100 and linearly searches previously encountered
key tuples. It does not import production hashing or SQL comparison code. Counts,
totals and extrema use ordinary Python integers. Average answers use Fraction
and independently verify the arithmetic-progression endpoint identity
`mean = (first + last) / 2`; all answers are exact integers or half-integers.
Text extrema use Python lexical ordering over independently generated decimal
spellings. The report compares literal expected rows, including carrier
observations, rather than treating buffered TableCraft output as its oracle.

Each CLI invocation supplies a 256 MiB cumulative native allowance and 10,000,000
native steps; query execution ceilings retain their defaults except in explicit
low-quota controls. CAB inputs exercise the working checked/spooled container
path. Successful output agreement covers these registered inputs and bounded
queries; it does not imply arbitrary grouping-key admission, unlimited group
state, native aggregate subtypes, or native collation equivalence. CLI refusal
controls observe exit status and buffered stdout, not internal cursor-entry
instrumentation; focused Rust tests establish the pre-scan admission boundary.

## Executable and immutable provenance

The frozen executable is
`.cache/tablecraft-bin/progressive-grouped-cli-20261025`, compiled revision
`6a0e5f209953bded9ecc5bbda8ee8497e68a2ea2`, SHA-256
`623e28dedb96b35c78a074e766271ae0938819902df49c169f36af5de6433144`.

The accepted report SHA-256 is
`546fc780f241b76a59c5ff79d9d20370a29909235442202ac45f102f987d3439`.
Session 36520 completed successfully; retained local output is
`.cache/progressive-grouped-replay-final-20261025.log`.

Before/after hashes cover the frozen executable, reviewed Jackcess JAR, registered
provenance JSON, all six original files, and four original Python/Java tools.
The source envelope uses raw `git show` blobs from the compiled revision;
mutable live checkout hashes are not used as evidence of executable contents.
The original corpus provenance remains
`.cache/large-deployment-provenance-20261025.json`, SHA-256
`e5c4e1f575d96bd8552ce05f610505b6987ee0e1946ac740886468cec094d56b`.
Existing public-builder/CAB recipes and notices remain unchanged.

## Excluded diagnostic-label attempt

Initial session 56243 ran the same 96 commands. All 72 successful queries and
18 low-quota controls matched. Six Text-key refusals correctly failed with empty
stdout but used the established diagnostic `query feature is not implemented`,
while the runner had expected the word `unsupported`. This was an expected-label
authoring error, not an output/model/runtime discrepancy.

The exact initial runner, report and log are preserved under
`.cache/progressive-grouped-exploration-20261025/`. Its report SHA-256 is
`0de24a3b69cf03ed4d58221003704fb4720a137c799cc01a93aa620a980be43d`.
Only that expected diagnostic fragment changed before the fresh accepted replay.
All 96 command argument arrays and expected rows were independently compared
between attempts and are identical. There was no production or model change.

## Reproduction

Run from the repository root with a fresh report destination. The runner refuses
an existing report, verifies the explicit executable digest and registered input
hashes, limits the plan to at most 100 commands, and uses 120-second child-process
timeouts. Each command retains expected rows or an error fragment, exit status,
stdout and stderr, including matched refusals and disagreements.

```sh
python3 docs/testing/compare_progressive_grouped.py \
  --cli .cache/tablecraft-bin/progressive-grouped-cli-20261025 \
  --cli-sha256 623e28dedb96b35c78a074e766271ae0938819902df49c169f36af5de6433144 \
  --revision 6a0e5f209953bded9ecc5bbda8ee8497e68a2ea2 \
  --jar .cache/tablecraft-fixtures/jackcess-5.0.4.jar \
  --directory .cache/tablecraft-fixtures/large-deployment-20261025 \
  --provenance .cache/large-deployment-provenance-20261025.json \
  --report docs/testing/results/progressive-grouped-2026-10-25.json
```
