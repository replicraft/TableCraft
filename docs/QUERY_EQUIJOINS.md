# Bounded directory checks for integral equijoins

Buffered joins use a bounded candidate directory and borrowed validated key
cells before allocating combined intermediate rows. This applies only when the entire ON
predicate is a bare equality between depth-zero fields from opposite inputs,
and both active result descriptors guarantee the same Boolean, Byte, Int16,
Int32 or Int64 carrier. Operand order can be reversed. Nullable fields retain
ordinary NULL Unknown and do not match each other.

```sql
SELECT Orders.Id, Customers.Label
FROM Orders INNER JOIN Customers ON Orders.CustomerId = Customers.Id;
```

The executor finishes both input relations and validates selected source cells
under their existing read/evaluation contracts. For nonempty proven inputs, it
checks every actual key against the descriptor, then indexes non-NULL right keys
without copying rows. Each left key probes candidate right rows in their original
encounter order. Every matching fingerprint candidate passes through the existing
scalar comparison service; fingerprints never establish SQL equality themselves. Accepted pairs use the original combined
row copy, retaining values, declarations and text policies. LEFT/RIGHT joins keep
their existing unmatched-row placement and nullable padding. No table, field or
provider is opened implicitly.

The proof uses the current materialized/declared result descriptor after field
pruning. Unknown descriptors, mixed carriers, Text, Currency, GUIDs, floats,
outer-field references, casts, function calls and compound ON predicates keep
the original general evaluator. In particular, an equality inside AND is never
extracted: doing so could suppress an eager failure elsewhere in that predicate.
Unexpected actual carriers or key widths disable the directory for the whole
operation before any joined rows are produced, retaining the original borrowed
comparison path and its errors. Empty inputs retain their existing behavior.
Resource, cancellation, allocation and selected-source failures are propagated;
they never trigger a silent retry. No representational Value equality or new
numeric promotion rule is introduced.

The earlier borrowed check removed combined-row allocations for nonmatching
pairs but retained all-pairs comparison work. The candidate directory now avoids
those proven nonmatching comparisons as well. Append-linked buckets and rebuilds
retain right duplicate order, preserving left-major/right-minor matched output.
RIGHT unmatched rows still follow all matched rows; LEFT unmatched rows follow
their corresponding left input. NULL keys are not indexed and never match.

Every actual-key check, fingerprint, bucket initialization, entry insertion,
rehash and probe is metered and cancellable. Bucket and entry capacities are
prepaid at their actual Rust widths before fallible reservation. Empty directories
allocate nothing; retained allocations are not refunded. The added index storage
and changed work can alter resource thresholds intentionally. This is not an
unlimited query path or a timing/performance claim. Full source materialization,
structural ceilings, complete-result publication and all unqualified predicate
paths remain unchanged.

Microsoft's [Access SQL INNER JOIN contract](https://learn.microsoft.com/en-us/office/client-developer/access/desktop-database-reference/inner-join-operation-microsoft-access-sql)
establishes field equality joins and ordinary matching semantics. This slice
changes TableCraft's candidate materialization, not admitted SQL syntax or native
Access compatibility claims. Other comparison families retain their existing
engineering boundaries.

## Earlier borrowed-pair verification

The earlier slice passed eight focused query tests and 110 existing retrieval, grouped-subquery,
named/external and alias regressions pass, along with strict query library/tests
Clippy. Controls cover all five admitted carriers, both arithmetic profiles,
swapped operands, duplicate/NULL keys, every join kind, empty inputs, unchanged
payloads, whole-predicate eager errors, mixed-carrier fallback, pruned/nested slot mapping, source
failures, cumulative quotas, every observed cancellation checkpoint and plan
reuse. An original generated 1,100-by-1,100 equijoin returns exactly 1,100 rows
within a 20,000-row query allowance. These are query-engine engineering checks;
no native Access execution or timing qualification is implied.

Three native controls pass across the four working layout profiles, including a
64-row self-join under a 1,024-row query allowance, nullable outer-join encounter
order and selected-corrupt/pruned payloads. Two new CLI controls and three SQLite
regressions also pass; the 1,100-row CLI join matches independent read-only SQLite
results under a 10,000-row query allowance. Scoped query/native/CLI Clippy and
formatting pass. See the [verification record](testing/results/read-query-borrowed-equijoins-2026-10-25.json).

## Directory implementation checks

Two focused implementation tests check the original 1,100-row default-work
self-join and 12,000-key joins under explicit bounded allowances, in both
arithmetic profiles. A declared-only integral descriptor also flows through
nested derived outputs without using the public conservative descriptor as a
runtime proof. Five private units exercise forced fingerprint collisions,
duplicate ordering through rebuilds, actual storage charges, every growth
cancellation checkpoint and whole-operation fallback for unexpected carrier,
Error or missing-key cells. The original eight borrowed-join tests remain
unchanged and pass. Ten independent conformance tests and 110 related
retrieval/named/external/alias regressions also pass: 135 distinct query tests,
including 17 new tests. Strict query library/tests Clippy and the standalone
minimal-feature check pass. The
[query verification record](testing/results/read-query-directory-joins-2026-10-25.json)
retains exact commands, raw log hashes and excluded test-authoring/lint attempts.
Native/CLI integration and replay evidence are recorded separately.

Four native and three CLI integration controls also pass with strict linting and
formatting, bringing the scoped checkpoint to 142 unique tests and 19 new tests.
The new native control checks 1,100-row joins with default query work across the
priority layouts and both arithmetic profiles; CLI SQLite controls keep exact
read-only reference results with the previous large work override removed.
Native Access and timing qualification remain separate.

A separate [whole-CLI benchmark](testing/DIRECTORY_JOIN_BENCHMARK.md) on the two
original 1,100-row ACE 16 inputs measures 0.87–1.01 seconds before and about
15 milliseconds after, with identical exact results and explicit equal allowances.
All 28 measured samples are retained. This is local development-build evidence,
not release, kernel, arbitrary-workload or native Access optimizer qualification.
