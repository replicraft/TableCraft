# Original native Text grouping replay

The original [runner](compare_progressive_text_groups.py) checks buffered and
progressive Text grouping against independent ASCII, integer and Fraction models.
It reuses the two registered LargeDeployment ACE 16 family native files and their
four unchanged stored/MSZIP CAB packages. Existing read-only Jackcess getters
validate every original row; bounded CAB inspection verifies complete expanded
image hashes. No fixture is generated or modified. Only TableCraft executes SQL;
this is engineering evidence, not native Access SQL, general Unicode collation
or timing qualification.

## Accepted results and comparison scope

The [accepted report](results/progressive-text-groups-2026-10-25.json) contains
**96 matched controls**: 78 successful queries comparing 1,272 rows and 4,296
cells, plus 18 intended failures. Six failures also compare an exact earlier
progressive prefix of one row/two cells each; these six rows/twelve cells are
separate from successful results.

Each of six inputs runs sixteen controls. `Left(Label,8)` creates twelve ASCII
prefix groups from the original 1,100 ID/Label rows. Count/Sum/Avg/Min/Max answers
come from integers and Fraction, with the contiguous progression endpoint identity
independently checked for every mean. Buffered output and stream batch sizes 1
and 128 compare to the same literal models; buffered TableCraft is not the oracle.

The case/padding construction uses literal `ROW-`/`row-`, an original ASCII digit
substring, and optional trailing ASCII space. Ordinal equality is exact spelling.
The explicit General legacy/2010 model is limited to these ASCII inputs: case
folding plus trailing-ASCII-space removal under the existing engineering SQL
comparison contract. The first actual spelling is retained. This does not model
arbitrary General weights, locale-sensitive Unicode folding, native database
precedence or function-specific padding.

Other controls cover nullable Text/Decimal tuples, Text HAVING, False/NULL
suppression of projected `Sum(1/0)`, and valid retained UTF-16 prefix keys observed
through LenB. A batch-1 TOP query retains its first row before a later final scalar
error; a corresponding aggregate-operand error produces no output because all
operand replay precedes final projection. A low-work control remains a typed
empty-stdout refusal. Raw malformed/NULL/provider activation matrices have
separate focused Rust coverage; the original raw control here uses complete
ASCII UTF-16 units only.

All invocations supply 256 MiB native bytes, 10,000,000 native steps, invariant
numeric conversion and an explicit comparison profile. The six rich Ordinal
case/padding controls additionally supply 10,000,000 query steps. Other query
limits remain unchanged, including General cases and the explicit low-work test.
Text grouping retains paid input keys and may compare many representatives; this
replay does not claim default-work admission, constant memory, portable Text
hashing or a performance improvement for every population.

## Frozen provenance and bounded-work attempt

Executable: `.cache/tablecraft-bin/progressive-text-groups-cli-20261025`, SHA-256
`d19b3d30ac01c34aafe765cb0f8c0a2cd54c8ba234c6021f8c15065715caf041`.
Compiled revision: `05637859e69e9495b9d96e048dab06a89df9d140`.
Accepted report SHA-256:
`e8829985aa2806fd2e8b384cac228d0444f12c7e330c5e67a9212ec6e5fa5f1d`.

Session 52517 completed successfully. Local output remains at
`.cache/progressive-text-groups-replay-final-20261025.log`. All fifteen immutable
before/after pins and forty-five scoped revision source blobs were independently
rehashed afterward. Source provenance uses raw `git show` bytes from the compiled
revision, not mutable live checkout hashes. Registered corpus provenance remains
`.cache/large-deployment-provenance-20261025.json`, SHA-256
`e5c4e1f575d96bd8552ce05f610505b6987ee0e1946ac740886468cec094d56b`.

Initial session 67134 ran the same 96 controls under default query work, except
its explicit low-work case. Exactly six rich Ordinal controls exhausted Steps
with empty stdout: their 45 distinct case/padding classes and key-expression
work exceeded that cumulative ceiling. All other controls matched. This is a
preserved bounded-work observation, not a numerical/policy implementation bug.
The exact initial runner/report/log remain under
`.cache/progressive-text-groups-exploration-20261025/`; initial report SHA-256:
`b2760c64c4f8159bd9e9bbf2be627f04ade10bf5e72a284d30528804066963ea`.

The accepted retry adds `--execution-steps 10000000` only to those six commands.
Every SQL expression, expected row/prefix and other command argument was
independently compared between attempts and remains unchanged. There was no
production, comparator, fixture or numeric model change.

## Reproduction

The runner verifies the explicit frozen executable and original hashes, refuses
an existing report path, enforces at most 100 commands, and uses 120-second process
timeouts. Every control retains full arguments, expected rows/prefix/error
fragment, return status, stdout and stderr. Use a fresh report destination:

```sh
python3 docs/testing/compare_progressive_text_groups.py \
  --cli .cache/tablecraft-bin/progressive-text-groups-cli-20261025 \
  --cli-sha256 d19b3d30ac01c34aafe765cb0f8c0a2cd54c8ba234c6021f8c15065715caf041 \
  --revision 05637859e69e9495b9d96e048dab06a89df9d140 \
  --jar .cache/tablecraft-fixtures/jackcess-5.0.4.jar \
  --directory .cache/tablecraft-fixtures/large-deployment-20261025 \
  --provenance .cache/large-deployment-provenance-20261025.json \
  --report docs/testing/results/progressive-text-groups-2026-10-25.json
```
