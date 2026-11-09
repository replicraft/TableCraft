# Exact Decimal index selection evidence

The 2026-10-25 [accepted replay](results/decimal-index-selections-2026-10-25.json)
passes 788 controls across fresh Jet 4, ACE 2007 and ACE 16-family originals.
Its 696 successful queries compare 38,772 returned IDs with independent exact
Fraction expectations. The 92 buffered refusals preserve empty stdout: 72
structural-limit controls, 12 eager division errors, four native resource failures
and four output failures. These are engineering comparisons; Microsoft Access
execution, optimizer behavior and native saves remain unqualified.

The implemented [Decimal selections](../access/indexes/NATIVE_INDEXES.md#exact-decimal-equality-and-null-selections)
retain the existing physical codec and qualify exact single-key equality/NULL
reads. Typed Decimal parameters can use automatic or required indexes after
lossless checked field-scale normalization. The original filter remains active.
CDec calls, other numeric/Text operands, ranges and partial unsafe conjunctions
produce no Decimal equality hint. Microsoft's
[PARAMETERS contract](https://learn.microsoft.com/en-us/office/client-developer/access/desktop-database-reference/parameters-declaration-microsoft-access-sql)
provides the typed-query declaration/use boundary; no new query function was
introduced for this optimization.

## Original inputs and independent model

The unchanged original MIT [DecimalIndexFixture.java](DecimalIndexFixture.java)
recipe uses reviewed Jackcess 5.0.4 public builders to author fresh files, with
expression evaluation disabled. Its existing reference-format check distinguishes
Jet 4 legacy numeric indexes. The complete recipe also retains its unique/primary,
precision-four and empty/composite controls. The new replay selects its three
1,000-row ordinary tables, with precision 28 and scales 0/4/28. Each contains 16
non-NULL numeric boundary values and 91 NULL records. Boundary coefficients include
both signed 28-digit extrema and neighbors beyond i64 and floating exactness.

The read-only [IndexRowsOracle.java](IndexRowsOracle.java) returns stored cells
through public index cursors; no reference SQL or active database content runs.
The Python model constructs exact `Fraction` values from getter Decimal strings,
checks their retained scale, and derives matching IDs without floating conversion.
Each reference value is tested against automatic, required ascending and required
descending paths. Getter hashes agree across profiles at each scale:

| Table | Raw getter stdout SHA-256 |
| --- | --- |
| Decimal0 | `67559c3cd53d31bcc0cfc08e8589507269cd08141450d30a419718a306628d3f` |
| Decimal4 | `ede9a228e39462288251fa9205415eae6b3a301221fcbc69712e5a5bc903f1f9` |
| Decimal28 | `747a385506567f54c64634b47c474b92bcfb4b8834ce20e4a942041722719d37` |

| Fresh original | Bytes | SHA-256 |
| --- | ---: | --- |
| `decimal-seeks-V2000-20261025.mdb` | 618,496 | `0ec45083fd4aea8cde561ecac5a6bec472a019d9a62249914120c9ad6fc1782a` |
| `decimal-seeks-V2007-20261025.accdb` | 847,872 | `7d6bf4d00e6da1a674e2ac8dbc5fea9e6ab84a26fc09ae7e98010c22cc6ffc21` |
| `decimal-seeks-V2016-20261025.accdb` | 888,832 | `be4d1f2d6bc9005263ff1ae90f442897f2d63b7d02f7e2b2bfcbfc00a26dc63c` |
| `decimal-seeks-V2019-20261025.accdb` | 958,464 | `555d1544d87d54d82648a36a2e46148dc1c9911d07b2070909dfd3710799cb37` |

These ignored originals supplement the earlier registered complete-index corpus.
The producer refuses an existing output path. Fresh generation commands are:

```sh
for decimal_profile in V2000 V2007 V2016 V2019; do
  decimal_extension=accdb
  if [ "$decimal_profile" = V2000 ]; then
    decimal_extension=mdb
  fi
  java -cp .cache/tablecraft-fixtures/jackcess-5.0.4.jar \
    docs/testing/DecimalIndexFixture.java \
    ".cache/tablecraft-fixtures/decimal-seeks-$decimal_profile-20261025.$decimal_extension" \
    "$decimal_profile"
done
```

## Accepted replay and scope

The indexed modes use native row ceiling 500, below each 1,000-row declaration;
sequential controls use 2,000. Tests include every boundary, absent points, NULL,
reversed predicates with mathematically equal differently scaled parameters,
Jet 4 descending sign groups, lossy/out-of-precision declines and unoptimized
conversion/integral/Double/not-NULL forms. Positive native row allowance one and
output allowance one are runtime failures; they are not argument-parser refusals.
The eager-error control uses an unsafe conjunction, so no partial hint can suppress
its division failure. Complete reused-source cache behavior is covered separately
by the focused Rust tests, since CLI controls start independent operations.

```sh
python3 docs/testing/compare_decimal_seeks.py \
  --cli .cache/tablecraft-bin/read-query-decimal-alias-cli-20261025 \
  --jar .cache/tablecraft-fixtures/jackcess-5.0.4.jar \
  --inputs .cache/tablecraft-fixtures/decimal-seeks-V2000-20261025.mdb \
    .cache/tablecraft-fixtures/decimal-seeks-V2007-20261025.accdb \
    .cache/tablecraft-fixtures/decimal-seeks-V2016-20261025.accdb \
    .cache/tablecraft-fixtures/decimal-seeks-V2019-20261025.accdb \
  --report docs/testing/results/decimal-index-selections-2026-10-25.json
```

The immutable executable SHA-256 is
`d1588956781f8162d96b3db8e7383476954c5d390d7270ad826d6ff49f7bb259`.
The accepted report SHA-256 is
`96d00011059fcb1b84e974e31e878911d0f914394d8a01ca8e97d532e42204b9`.
The terminal log is `.cache/decimal-selection-replay-final-20261025.log`.
All 19 scoped input/reader/runner/executable/runtime pins agree before/after and
were independently rehashed afterward. The runner requires the reviewed JAR hash
before Java, records repository-relative labels and gives each subprocess a
60-second ceiling. Reported stdout hashes follow complete ID-sequence comparisons;
large row outputs are not committed. No benchmark claim follows from this replay.

## Focused Rust checks

The final batch passes 84 tests: 14 Decimal-index tests, eight existing Decimal
mutation-preview regressions, 59 query retrieval tests and three new adapter tests.
Eleven are new: five engine, three backend-neutral and three adapter tests. They
cover typed eligibility, all four profiles, both directions, 28-digit boundaries,
scale/precision refusals, compressed duplicates, NULL/zero, zero-width projections,
pruning, key/row agreement, metadata-only preparation, retained cache isolation,
resource limits and cancellation. Preview initialization remains complete-only;
no write gate or permit changed.

```sh
cargo test --locked -p tablecraft-ace --features ace16-write-preview \
  --test decimal_indexes --test decimal_updates
cargo test --locked -p tablecraft-query -p tablecraft-access \
  --test retrieval --test decimal_selection
cargo clippy --locked -p tablecraft-ace -p tablecraft-query -p tablecraft-access \
  --features tablecraft-ace/ace16-write-preview --lib \
  --test decimal_indexes --test decimal_updates --test retrieval --test decimal_selection \
  -- -D warnings
```

All commands completed successfully. Logs are
`.cache/decimal-selection-engine-tests-20261025.log`,
`.cache/decimal-selection-query-adapter-tests-20261025.log` and
`.cache/decimal-selection-clippy-20261025.log`. Formatting, Python compilation and
whitespace checks also passed. The full workspace suite was not run for this slice.

The earlier getter-only preflight remains
`.cache/decimal-getter-probe-20261025.json`, outside the 788 CLI controls. A separate
baseline query against frozen Boolean CLI
`013203c54f1235727e73c56f702ed70498acd1ac924a05fc4f46ef350849b8b1`
refused the V2016 typed Decimal equality at native row cap 900 with a structural
limit and empty stdout; its `.cache/decimal-selection-baseline-20261025.{stdout,stderr}`
captures are excluded from accepted controls. Originals remain byte-for-byte unchanged.
