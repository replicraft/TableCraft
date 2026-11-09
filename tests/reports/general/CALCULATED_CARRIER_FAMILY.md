# Additional cached calculated scalar carriers

Fresh public reference-builder/getter observations expose a cached-read admission
gap for calculated Byte, Int16, Int64, Single and Guid fields. The writer persists
the existing variable capacity-39 declaration, ordinary flags 0x02, extended flags
0xc0, a typed matching BYTE ResultType property and a nonempty Expression. Inert
stored getters return the authored values, while the earlier explicit cached
reader refused the declaration before reading its wrapper.

The engine now admits these five carriers through the existing calculation gates
and wrapper decoder. Integer payloads require their exact little-endian width;
Single preserves the raw IEEE binary32 representation; Guid uses the existing
native mixed-endian UUID interpretation. Unknown flags, fixed storage, nonmatching
ResultType, wrong widths and unsupported profiles remain refused, including empty
tables. An absent value or the existing qualified empty non-Boolean wrapper yields
NULL. Ordinary query sources still exclude calculated fields.

## Original corpus and consumers

[CalculatedCarrierFixture.java](CalculatedCarrierFixture.java) creates fresh ACE
2016/2019 files using reviewed Jackcess 5.0.4 public builder APIs. Each file has
six populated declarations and six matching empty tables, with seven stored
rows per populated table. Expression evaluation is disabled throughout creation
and [read-only getter inspection](CalculatedCarrierOracle.java). Original caches
are supplied as data; the expression `[ID]*2` never executes or coerces them.

Values cover unsigned Byte boundaries, Int16/Int64 extrema, an Int64 beyond the
exact binary64 integer range, NULL, signed zero, the smallest binary32 subnormal,
maximum finite Single, both infinities, a NaN payload and asymmetric Guid fields.
The independent oracle records Single raw bits and canonical Guid strings rather
than using a TableCraft decoder.

The [runner](compare_calculated_carriers.py) compares native declarations and
explicit cached inspection, row pages and finite-carrier queries. Separate
exceptional-Single tables retain infinities/NaNs for inspection and row pages;
queries explicitly retain their existing non-finite-source refusal, including
TOP, count and NULL-filtered queries. It checks
ordinary-source refusal even for empty tables, NULL selection/counts, unrelated
ordinary count pruning and separate native/query/output limits on finite sources,
with explicit error-category assertions. A unique late
cache wrapper is corrupted only on a disposable copy per populated declaration;
inspection, cached rows, TOP and restrictive cached queries must fail without
stdout. Ordinary counts remain readable. Original bytes are never modified.

CLI Single text round-trips to its original finite/infinite binary32 value.
Textual NaN cannot carry a sign or payload, so the CLI comparison checks its NaN
category. The public getter report and engine tests independently retain the
exact NaN payload bits; textual output is not represented as a raw-bit oracle.

Focused engine tests also check wrong payload widths, exact unknown ordinary and
extended flags, mismatched ResultType, profile refusal, metadata declaration and
empty-table gates. Adapter tests prove metadata-only binding, explicit stored
provenance, ordinary refusal and complete-cache reuse for all five carriers.

These checks cover the current stable/offline engineering cached-read profile.
They do not establish Microsoft Access calculated-field admissibility, native SQL
equivalence, formula behavior, freshness, shared-file coherence or writes.

## Accepted results

On 24 October 2026, the [accepted report](results/calculated-carriers-2026-10-24.json)
records 248 passing operation controls and 238 cached-cell comparisons. Across
ACE 2016/2019 there are 24 declarations, including twelve empty counterparts,
and 84 stored rows. Both inspection and row pages compare every stored cell;
finite cached queries compare seventy cells. Twelve disposable copies exercise
late malformed-wrapper refusal and ordinary projection pruning. All 21 pinned
input, recipe, getter, runner/helper, engine-source, reference JAR and executable
hashes remain unchanged after replay.

The immutable CLI SHA-256 is
`021a072d23e21e8d65fcf7e7331cc3100db24d4fda15337e31ded8cc01e34e11`.
The engine calculated-scalar target passes seven tests, the adapter cached-query
target passes eight, and scoped strict Clippy passes for both. The
[fixture ledger](FIXTURES.md#original-additional-cached-calculated-carrier-inputs)
retains original hashes and provenance; generated databases and binaries remain
ignored.

The initial combined finite/non-finite Single corpus and failed query replay are
excluded: they exposed the existing explicit query non-finite policy, which this
feature preserves. A preliminary 254-control replay repeated six quota attempts
on already unsupported non-finite query sources; that report remains ignored and
excluded. Accepted evidence uses the final split corpus and verifies resource
error categories only on admitted finite query sources.

```sh
python3 docs/testing/compare_calculated_carriers.py \
  --cli .cache/tablecraft-bin/calculated-carriers-20261024 \
  --jar .cache/tablecraft-fixtures/jackcess-5.0.4.jar \
  --directory .cache/tablecraft-fixtures/calculated-carriers-final-20261024 \
  --report /tmp/calculated-carriers.json
```
