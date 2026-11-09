# Native exact-key directory join replay

The original [runner](compare_directory_joins.py) compares TableCraft joins with
an independent integer-equality model over six immutable, publicly authored
native databases. Only TableCraft executes SQL. Jackcess ordinary getters verify
the original input rows; this is engineering correctness and resource-admission
evidence, not native Microsoft Access SQL qualification or a timing result.

## Accepted results

The [accepted report](results/directory-joins-2026-10-25.json) contains 76 matched
commands: 62 successful queries, 12 intended refusals on the new executable and
two intended refusals on the old executable. Successes compare all 51,848 result
rows and 105,896 cells exactly. There were no disagreements or excluded attempts.

The four registered Decimal originals cover Jet 4, ACE 2007 and the two ACE 16
family profiles. Their existing inert getter supplies all ID values in encounter
order. The two registered LargeDeployment originals contain 1,100 rows each;
their existing read-only getter verifies every ID, Score, Label, Memo and row
position against the original public-builder recipe, then emits a checked summary.
The Python model independently computes Score as `17 * ID - 9` and Label as
`row-` followed by the six-digit ID. No database bytes or fixtures are created or
modified during this replay.

For each original, the replay checks INNER, LEFT and RIGHT self equality joins
with both written operand directions, left/right filtered unmatched rows, and a
small duplicate-producing derived source. The model preserves left-major,
right-minor matching order and appends unmatched right rows in original right
encounter order. NULL-extended IDs are observed through `Nz(...,-1)`.

A small mixed Double/integral equality checks the existing general fallback.
An expression equality under an explicit 10,000-step ceiling must refuse with
empty stdout; a compound ON predicate containing division by zero must fail
eagerly, also without stdout. These controls preserve unsupported fast-path
boundaries rather than expanding fingerprint coercion or extracting partial AND
predicates. The two large-file queries additionally compare every ID, Score and
Label in native encounter order.

The large joins succeed on the new executable without an `--execution-steps`
override. The exact same large SQL and resource arguments on the old executable
fail with `Steps resource limit exceeded (0 remaining)`. Other allowances are
explicit: 5,000 native rows, 10,000 query rows, 256 MiB native/query bytes and
10,000,000 native steps. Therefore this demonstrates admission within the
existing default **query work** allowance, not every default consumer allowance
or a runtime speedup.

## Frozen executables and provenance

The accepted executable is `.cache/tablecraft-bin/directory-join-cli-20261025`,
compiled revision `35b6ce8dca59590d316e62bf59ec9511a16d14ca`, SHA-256
`9b5b64282049461ca5590f16d9746ff8fdfcb465801b6ef69f56928e0ed4350f`.
The baseline is `.cache/tablecraft-bin/progressive-extrema-cli-20261025`, revision
`ef6d7f50acf9d6068c7ea14bc26f8270a2d8c446`, SHA-256
`b46bd18f59033360e5fcdb4e6b5e562e78698353016a8f9ea3a7c61e7e87aeee`.

All sixteen immutable pins were rehashed before and after the run: both frozen
executables, the reviewed Jackcess JAR, six original databases and seven original
tool/recipe files. Twenty-one scoped source blobs from the new compiled revision
and sixteen from the baseline revision were independently rehashed using raw
`git show` bytes after completion. Mutable live checkout hashes are not used to
claim what either executable contains. Existing fixture provenance and notices
remain with the registered Decimal and LargeDeployment recipes; no new
third-party data or code is copied by this runner.

The report SHA-256 is
`c23a4651ed662f5a54f4767358b5bb0393df313027e10ccd4f67844006cf7a2b`.
Session 60738 completed successfully; local output is retained at
`.cache/directory-joins-replay-20261025.log`. Every CLI command retains expected
rows or an error fragment, exit status, stdout and stderr. Child processes have
a 120-second timeout; the planned command count must stay within 100.

## Reproduction

Run from the repository root with the registered originals and reviewed reader.
Choose a fresh report path: the runner refuses to overwrite an accepted report.
No build or fixture-generation command is invoked.

```sh
python3 docs/testing/compare_directory_joins.py \
  --cli .cache/tablecraft-bin/directory-join-cli-20261025 \
  --cli-sha256 9b5b64282049461ca5590f16d9746ff8fdfcb465801b6ef69f56928e0ed4350f \
  --revision 35b6ce8dca59590d316e62bf59ec9511a16d14ca \
  --jar .cache/tablecraft-fixtures/jackcess-5.0.4.jar \
  --decimal-inputs \
    .cache/tablecraft-fixtures/decimal-seeks-V2000-20261025.mdb \
    .cache/tablecraft-fixtures/decimal-seeks-V2007-20261025.accdb \
    .cache/tablecraft-fixtures/decimal-seeks-V2016-20261025.accdb \
    .cache/tablecraft-fixtures/decimal-seeks-V2019-20261025.accdb \
  --large-inputs \
    .cache/tablecraft-fixtures/large-deployment-20261025/V2016.accdb \
    .cache/tablecraft-fixtures/large-deployment-20261025/V2019.accdb \
  --baseline-cli .cache/tablecraft-bin/progressive-extrema-cli-20261025 \
  --baseline-cli-sha256 b46bd18f59033360e5fcdb4e6b5e562e78698353016a8f9ea3a7c61e7e87aeee \
  --baseline-revision ef6d7f50acf9d6068c7ea14bc26f8270a2d8c446 \
  --report docs/testing/results/directory-joins-2026-10-25.json
```
