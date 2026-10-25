# Bounded exact-key lookup for grouping and sets

Ordinary GROUP BY, DISTINCT, qualified DISTINCTROW and UNION duplicate removal
can use a bounded fingerprint directory instead of comparing every incoming key
against every retained representative. Existing vectors still own the rows,
complete key cells and first-encounter order. The directory owns only fingerprints
and representative indices; it never supplies SQL equality or output order.

This addresses a concrete larger-query limit: a table containing 12,000 distinct
integers previously exhausted even the maximum query work allowance for GROUP BY
or DISTINCT, despite successful linear retrieval of those same rows. The indexed
path executes these queries under a finite five-million-step allowance without
raising defaults or removing any quota.

```sql
SELECT Category, Count(*) FROM Items GROUP BY Category;
SELECT DISTINCT Category FROM Items;
SELECT Category FROM Items UNION SELECT Category FROM ArchivedItems;
```

## Proven key profile

Every key column must have a known uniform successful carrier: Boolean, Byte,
Int16, Int32, Int64, Currency or Decimal, with ordinary NULL values permitted. A known NULL-only
output also qualifies. GROUP BY and hidden DISTINCTROW keys initially require
ordinary depth-zero fields whose types come from the pruned available source
slots. Computed grouping keys remain supported through the original general path.
DISTINCT/UNION use the active materialized or declared output descriptors, so a
conservative public Dynamic descriptor does not erase a separately proven active
profile. Binding still performs no record scans.

The whole tuple must qualify. No first-row type inference or partial-column
index is used. Unknown/Dynamic descriptors, mixed carriers, Error, raw Strings,
Text/provider-dependent equality, Date, floats, GUIDs and
Binary retain the original comparison path in this slice. This does not change
those existing query/key contracts or claim their absence from ordinary SQL.

The directory hashes exact typed scalar views and distinct NULL markers, using
an execution-local standard-library hasher. It does not use representational
Value equality/hashing, change retained values, manufacture declaration provenance
or create persistent native/index keys. Hash collisions call the existing SQL key
comparator. NULL tuples retain key equality with NULL, separately from scalar
NULL predicates. Retained result carriers and the first selected cells remain
unchanged.

A successful-result descriptor is not permission to reinterpret an unexpected
runtime cell. If a row cannot be fingerprinted under the proof, execution disables
the directory for the rest of that operation and resumes the original comparator
over **every** retained representative. It does not introduce a new conversion,
error, NULL value or category. Existing singleton key validation stays at its
original position. Failed executions publish no complete result or named cache.

## Exact fixed-point fingerprint views

Currency has one fixed scale of four decimal places. Its signed scaled Int64
therefore supplies an exact fingerprint input, including the signed minimum;
there is no absolute-value overflow, float conversion or rounding.

Decimal retains a signed coefficient with a 96-bit magnitude and a scale from zero through 28.
Representational equality is not numeric key equality: `1.0` and `1.00` have
different retained scales but the existing strict key comparator equates them.
For hashing only, a local coefficient/scale view removes fractional trailing
zeros until the scale reaches zero or the coefficient is no longer divisible by
ten. Every zero maps to `(0,0)`. For example `(100,2)` and `(10,1)` both hash as
`(1,0)`, while `(-1200,3)` hashes as `(-12,1)`. No coefficient multiplication or
scale increase is needed, so maximum coefficients and tiny fractions remain
exact. Each normalization iteration checks work and cancellation.

The original cells keep their first coefficient, scale, carrier, declaration and
policy/completion lineage. Canonical views are not constructed core values and
are never published as result replacements. Hash collisions still invoke the
ordinary SQL key comparator. The proof is uniform per column: a Decimal column
requires actual Decimal or NULL, and Currency requires Currency or NULL. A mixed
Currency/Decimal column does not gain an implicit conversion or a new mixed
profile, even when the existing general comparator can equate its values.

Existing active paired group descriptors also admit these two key families in
explicit progressive grouped/HAVING execution. Actual-key validation precedes
insertion; complete numeric/NULL policy cohorts, selected operand replay,
checked EOF and final key cells retain their existing contracts. Buffered
directories retain their whole-operation disable-and-compare fallback for an
unexpected actual carrier. This does not authorize native index encodings,
constraint pushdown or additional arithmetic conversions.

## Existing evaluation and policy stages

Grouping still evaluates each key once at its existing position. With a resolver,
it retains and unifies the complete key-column policies before partitioning.
Without one, it preserves incremental evaluation. Existing group policy and
completion-marker merging remains unchanged after a membership match.

DISTINCT retains complete output/record and sort-key policy unification before
discarding duplicates. Hidden record ownership is unchanged. UNION keeps its
written promotions and policy-prefix stages, creating a fresh directory at each
duplicate-removal boundary. UNION ALL does not deduplicate and a later arm cannot
retroactively replay an earlier DISTINCT boundary. ORDER BY/TOP and named/derived
scope behavior are unchanged.

## Cumulative work and storage

The key profile, bucket heads, entry storage and every growth/rebuild use checked
sizes, prepaid cumulative allocation and fallible reservation. Directory storage
is lazy; empty input does not allocate an index. Bucket initialization, each
rehash entry, each fingerprint column, Decimal normalization iteration and every
candidate probe check cancellation
and consume work. Adversarial collisions can still require many comparisons;
limits stop that work rather than granting an unchecked hash-table callback.

The implementation uses two private vectors, not a new general-purpose map or a
new dependency. Existing row/group vectors already own insertion order. A
fingerprint directory makes collision probes and rebuild accounting explicit
without giving a container's Eq/Hash implementation authority over SQL keys.
No IndexMap/SmallVec dependency is introduced solely to rewrite bounded vectors.

Reduced comparison work adds index storage, so allocation/quota thresholds can
differ from the general path. All native/source, row, output, structural and query
limits remain separate. Budgets are never reset or refunded after buffers drop.
This is working bounded-query functionality, not a universal complexity or
throughput claim.

Microsoft's [GROUP BY contract](https://support.microsoft.com/en-us/access/group-by-clause)
and [DISTINCT contract](https://learn.microsoft.com/en-us/office/vba/access/concepts/structured-query-language/all-distinct-distinctrow-top-predicates-microsoft-access-sql)
establish grouping and duplicate-removal behavior. The directory changes
TableCraft's execution strategy; it does not qualify native Access subtypes,
optimizer behavior or the full read/query roadmap.

## Verification

The Currency/Decimal extension passes 394 distinct query tests, including 21 new
controls: 29 directory/value/policy tests, 33 runtime units and 332 related
buffered/progressive grouping, HAVING, named, crosstab and UNION regressions.
Independent BigInt cross-products establish exact fixed-value models without
using production normalization. Representation, late policy cohorts, nullable
keys, full-node aggregate consumers, collision/fallback, quotas and cancellation
are checked alongside 12,000-row fixed-key populations in both profiles. Strict
query library/tests Clippy and the standalone minimal-feature check pass. The
[fixed-key record](testing/results/read-query-exact-fixed-keys-2026-10-25.json)
retains exact commands and hashes; its excluded initial regression used an obsolete
Decimal-refusal control, corrected to the still-unsupported Single profile.
Parent native/CLI integration adds 57 passing controls, including three new tests.
The scoped total is 451 distinct tests with 24 new controls; strict native/CLI
linting and repository formatting also pass. The [original native/CAB replay](testing/EXACT_FIXED_KEYS.md)
adds 84 new-query successes comparing 14,088 rows and 41,802 cells, four old-work
baselines and two representation probes. Actual Access qualification remains separate.

The initial integral-directory checkpoint passed 223 unique query tests: ten independent conformance
tests, three implementation scale controls, five private directory/profile tests
and 205 existing regressions. The independent integer/NULL model checks first
encounter and counts across all five admitted carriers, signed limits and exact
LongLong neighbours beyond binary64 precision. It covers multicolumn nullable
keys, 12,000-row descending populations, active paired descriptors through derived
queries, complete selected DISTINCTROW records and written UNION boundaries.
Dynamic mixed-integral rows, Error/raw refusals, every observed cancellation
checkpoint and cumulative quotas retain their existing consumers.

Private tests force fingerprint collisions, failed directory growth and unexpected
runtime carriers. The SQL comparer remains authoritative and a disabled directory
does not lose any retained representative or refund charges. Existing policy,
group, key, alias and set targets pass beside these controls. Strict Clippy covers
the library and all query test modules. Exact commands, observed counts and log
hashes are retained in the [verification record](testing/results/read-query-exact-keys-2026-10-25.json).
The initial lint attempt is excluded: it flagged a complex tuple type in the new
test helper, corrected with readable aliases. No test-answer or runtime correction
was required.

The [native exact-key replay](testing/NATIVE_EXACT_KEYS.md)
passes 76 controls: 64 successes and 12 intended buffered refusals, comparing
48,304 rows and 72,584 cells. Ordinary read-only Jackcess getters supply the IDs
and encounter order from four immutable registered originals. Independent integer
models cover field/derived grouping, DISTINCT, NULL keys and written UNION
boundaries in both execution profiles. The 1,000-row native populations fit the
explicit 500,000-step query allowance; work, native-row and output ceilings still
refuse without partial stdout. All ten immutable tool/input pins remain unchanged,
and scoped source hashes come from the frozen executable's committed revision.
Only TableCraft executes SQL. Native Access SQL, optimizer behavior and timing
comparisons remain unrun.
