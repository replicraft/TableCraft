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
Int16, Int32 or Int64, with ordinary NULL values permitted. A known NULL-only
output also qualifies. GROUP BY and hidden DISTINCTROW keys initially require
ordinary depth-zero fields whose types come from the pruned available source
slots. Computed grouping keys remain supported through the original general path.
DISTINCT/UNION use the active materialized or declared output descriptors, so a
conservative public Dynamic descriptor does not erase a separately proven active
profile. Binding still performs no record scans.

The whole tuple must qualify. No first-row type inference or partial-column
index is used. Unknown/Dynamic descriptors, mixed carriers, Error, raw Strings,
Text/provider-dependent equality, Date, Currency, Decimal, floats, GUIDs and
Binary retain the original comparison path in this slice. This does not change
those existing query/key contracts or claim their absence from ordinary SQL.

The directory hashes exact typed scalar bits and distinct NULL markers, using
an execution-local standard-library hasher. It does not use representational
Value equality/hashing, normalize values, manufacture declaration provenance or
create persistent native/index keys. Hash collisions call the existing SQL key
comparator. NULL tuples retain key equality with NULL, separately from scalar
NULL predicates. Retained result carriers and the first selected cells remain
unchanged.

A successful-result descriptor is not permission to reinterpret an unexpected
runtime cell. If a row cannot be fingerprinted under the proof, execution disables
the directory for the rest of that operation and resumes the original comparator
over **every** retained representative. It does not introduce a new conversion,
error, NULL value or category. Existing singleton key validation stays at its
original position. Failed executions publish no complete result or named cache.

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
rehash entry, each fingerprint column and every candidate probe check cancellation
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

The focused batch passes 223 unique query tests: ten independent conformance
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

The final-executable native exact-key replay is a separate pending check. These
Rust controls establish the bounded engineering profile; native Access SQL,
optimizer behavior and timing comparisons remain unrun.
