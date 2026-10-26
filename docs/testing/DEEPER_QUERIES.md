# Deeper compound-query replay

The [explicit binding-depth implementation](../QUERY_BINDING_LIMITS.md) is exercised
by the original [replay runner](compare_deeper_queries.py) over literal constructors
and unchanged native stored fields. Only TableCraft executes SQL; exact Python
Fraction/IEEE relations and independent Jackcess getters supply expected values.
This is engineering composition evidence, not native Access semantics.

Fifteen literal cohorts and eight stored-field cohorts in each of the four existing
Jet 4/ACE 2007/ACE 2016/ACE 2019 scalar originals cover decimal tenths, dyadic
equality, signed zeros, Boolean/integer peers, 96-bit Decimal boundaries,
subnormals and values beyond exact Double integer precision. Six additional
`CVar` wrappers retain each selected conditional value before UNION composition.
Group/count, ordered identity/value observations and Min/Max run over all six
encounter permutations in both arithmetic profiles with explicit depth 32.
Default-depth controls use the identical SQL for the first permutation/profile.

Literal values are actually constructed in these queries, unlike the previous
VALUE-parameter replay. Double constructors beyond the signed LongLong envelope
use explicit floating spellings. Fixed values retain exact decimal spellings;
stored values come from the unchanged native fields. The model reconstructs
Single transport at binary32 precision before comparison and never rounds fixed
coefficients through Double. No source rows or database bytes are modified.

The [machine-readable report](results/deeper-query-replay-2026-10-25.json) records
the observations, default refusals, getter transcript hashes, executable/tool/input
pins and selected committed source hashes. Selected source snapshots are not an
exhaustive compilation manifest. Private diagnostic attempts remain identified
separately; runtime code and expected relations did not change during the replay.

## Observed results

| Observation | Count |
| --- | ---: |
| Successful explicit-depth literal queries | 540 |
| Successful explicit-depth native-field queries | 1,152 |
| Identical-SQL default-depth refusals | 141 |
| Total CLI observations | 1,833 |
| Returned rows checked against the model | 3,660 |
| Returned cells checked against the model | 11,544 |
| Independent read-only getter invocations | 4 |
| Immutable artifacts rehashed | 11 |
| Selected committed/live source snapshots checked | 66 |

All default controls refuse with empty stdout and the structural-limit diagnostic;
all depth-32 controls succeed. The audit checks the complete unique command grid,
identical SQL hashes for paired controls and recomputes expected returned counts
from the exact value cohorts. Getter transcript hashes agree with the prior
independent mixed-key replay. No larger depth or default limits were enabled
implicitly.

The first diagnostic run exposed an intended Double written as an unsupported
large integer literal. A second, successful workload crossed the default depth
only six times; the final workload adds retained wrappers to exercise actual
depth refusals for every native-field cohort as well. Both private reports remain
identified by hashes in the final record. Neither adjustment changed engine code
or expected numeric relations.

## Reproduction

The frozen executable was built at `b9e26076` with SHA-256
`f9e51ed3b8bfd0500ed8f650cec6e2bdbf27ee4d93fccfab2e176454f6277433`.
The registered originals, reviewed getter and Jackcess JAR must already exist
under the ignored cache. The runner downloads and builds nothing, checks the
reviewed fingerprints and requires a fresh report path.

```sh
python3 -B docs/testing/compare_deeper_queries.py \
  --cli .cache/tablecraft-bin/binding-depth-cli-20261025 \
  --cli-sha256 f9e51ed3b8bfd0500ed8f650cec6e2bdbf27ee4d93fccfab2e176454f6277433 \
  --jar .cache/tablecraft-fixtures/jackcess-5.0.4.jar \
  --directory .cache/tablecraft-fixtures/scalar-seek-20261023 \
  --revision b9e26076 \
  --report FRESH_REPORT.json
```

The CLI observation ceiling is 2,000, commands have individual timeouts and
getters have a 256 MiB Java heap ceiling. This is not a performance benchmark.
Default depth remains 16. Wider SQL syntax, full Variant/text semantics and
native Access qualification remain separate acceptance work.
