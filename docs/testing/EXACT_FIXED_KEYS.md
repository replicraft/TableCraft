# Original exact fixed-key replay

The original [runner](compare_exact_fixed_keys.py) checks Currency/Decimal
relational key behavior against independent scaled-integer and Fraction models.
It reuses the two registered LargeDeployment ACE 16 family files and four
unchanged stored/MSZIP CAB packages. Existing read-only public Jackcess getters
validate the original rows; bounded CAB inspection verifies complete expanded
image hashes. No fixture changes or native Access SQL execution are involved.
This is engineering correctness/resource evidence, not a timing claim.

## Accepted results

The [accepted report](results/exact-fixed-keys-2026-10-25.json) records 90 matched
commands, separated into:

- 84 new-executable successes: 14,088 rows and 41,802 cells.
- Four old-executable baseline commands: two default-work Steps refusals and
  two explicit-10,000,000-work successes, totaling 2,200 rows and 6,600 cells.
- Two source-free old/new representation probes, four cells each.

Each of six inputs supplies depth-zero outer fields through ordinary derived
CCur/CDec projections. Nullable fractional fixed keys exercise buffered and
progressive GROUP, DISTINCT, written-prefix UNION, and progressive grouped HAVING.
Unique-key queries produce 1,100 retained groups under default query work on the
new executable. Explicit Decimal scale-1/scale-2 inputs verify that DISTINCT and
GROUP retain the first actual representative's `1.0` spelling. Hash canonical
views do not authorize changing stored/retained scale.

The model linearly searches groups in first encounter order. Counts/totals use
integers; key identity uses exact Fraction values. Currency rendering is checked
at four fractional digits. VarType observations remain literal expected Integer
codes: Currency 6, Decimal 14 and NULL 1. These are the current explicit profiles,
not newly qualified native aggregate or DAO metadata rules.

Only key column zero in the calculated Decimal quotient `group`, `distinct` and
`union-prefix` cases uses exact Fraction(PythonDecimal) comparison. Its complete
actual wire spelling is retained in the report. Headers, every other cell and
all other controls remain exact wire comparisons, including the separate strict
first-scale controls. This avoids making the owning division kernel's quotient
scale a new key-equivalence requirement; no float approximation or epsilon is
used. Query SQL and independent numeric expected values are unchanged.

Every native query supplies 256 MiB native bytes, 10,000,000 native steps and
explicit invariant numeric conversion. Query work remains at its default except
in the two old explicit-work baseline commands. The same old 1,100-unique Decimal
query fails at default work and succeeds with the larger work allowance. This
establishes improved admission for that workload, not universal memory bounds,
a change to strict mixed-family equality, or a performance timing result.

## Provenance and excluded attempt

New executable: `.cache/tablecraft-bin/exact-fixed-keys-cli-20261025`, revision
`9b8d5280ce5ab4d15225d630d8a1d2135835644a`, SHA-256
`89f66dcbc51349598137ed40356d711f2a7798828aaad69b8be43df3cd58687d`.
Old executable: `.cache/tablecraft-bin/progressive-having-cli-20261025`, revision
`e28fd42cf33a64489106d9514fbc227b8ae1f9e4`, SHA-256
`f6a86b005512d52520c9398853843be1d3e47d19b34f1758e09b0f6f40f1a89c`.

The accepted report SHA-256 is
`a5820e6f891d18f81bffb2a0b200064541bc30316f933c5663cb3bda75a72010`.
Session 55665 completed successfully; local output is retained at
`.cache/exact-fixed-keys-replay-final-20261025.log`.
All sixteen immutable before/after pins were independently rehashed: both frozen
executables, the reviewed reader, original provenance, six inputs and six tools.
Thirty-seven new-revision and thirty-four old-revision source blobs were also
independently verified with raw `git show` bytes. No mutable live source inventory
is used as executable provenance.

Initial session 6360 ran 88 commands. Twenty-four comparisons rejected successful
Decimal quotient outputs solely because the authored wire model expected `0.1`
rather than retained `0.10`, and similarly `1.3` rather than `1.30`. Keys, counts,
sums, types and order agreed numerically; every Currency, unique-key, explicit
first-scale and old baseline control matched. The exact initial runner/report/log
remain under `.cache/exact-fixed-keys-exploration-20261025/`; initial report SHA-256:
`b4ec17491e1d9073b41b77be14cd8af6662a10476fc59b72f829681d891676b2`.

The correction declared the narrow exact-rational comparison above, preserving
all original 88 command arrays and numeric expected rows. Two additional old/new
representation probes establish the unchanged existing division spellings
`0.10`, `0.20`, `1`, `1.30`. They explain the final count of 90 commands and eight
additional cells. The old executable is not the numerical oracle; those probes
only pin preexisting display/representation behavior. No production or arithmetic
normalization model changed.

## Reproduction

Use a fresh report destination; the runner refuses overwrite, verifies the frozen
executable and registered original hashes, enforces at most 100 commands, and
uses 120-second child-process timeouts. Commands retain expected rows, comparison
policy, return status, stdout and stderr.

```sh
python3 docs/testing/compare_exact_fixed_keys.py \
  --cli .cache/tablecraft-bin/exact-fixed-keys-cli-20261025 \
  --cli-sha256 89f66dcbc51349598137ed40356d711f2a7798828aaad69b8be43df3cd58687d \
  --revision 9b8d5280ce5ab4d15225d630d8a1d2135835644a \
  --jar .cache/tablecraft-fixtures/jackcess-5.0.4.jar \
  --directory .cache/tablecraft-fixtures/large-deployment-20261025 \
  --provenance .cache/large-deployment-provenance-20261025.json \
  --baseline-cli .cache/tablecraft-bin/progressive-having-cli-20261025 \
  --report docs/testing/results/exact-fixed-keys-2026-10-25.json
```
