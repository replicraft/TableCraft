# Borrowed checks for integral equijoins

Buffered joins can reject a pair using borrowed validated key cells before
allocating a combined intermediate row. This applies only when the entire ON
predicate is a bare equality between depth-zero fields from opposite inputs,
and both active result descriptors guarantee the same Boolean, Byte, Int16,
Int32 or Int64 carrier. Operand order can be reversed. Nullable fields retain
ordinary NULL Unknown and do not match each other.

```sql
SELECT Orders.Id, Customers.Label
FROM Orders INNER JOIN Customers ON Orders.CustomerId = Customers.Id;
```

The executor finishes both input relations and validates selected source cells
under their existing read/evaluation contracts, then visits pairs in the existing left-major/right-minor order. It calls the existing scalar
comparison service once for each pair. Accepted pairs use the original combined
row copy, retaining values, declarations and text policies. LEFT/RIGHT joins keep
their existing unmatched-row placement and nullable padding. No table, field or
provider is opened implicitly.

The proof uses the current materialized/declared result descriptor after field
pruning. Unknown descriptors, mixed carriers, Text, Currency, GUIDs, floats,
outer-field references, casts, function calls and compound ON predicates keep
the original general evaluator. In particular, an equality inside AND is never
extracted: doing so could suppress an eager failure elsewhere in that predicate.
No representational Value equality or new numeric promotion rule is introduced.

Previously, every pair allocated a complete candidate before ON evaluation,
including rejected pairs. A 1,100-row one-to-one self-join therefore exhausted
the hard one-million intermediate-row ceiling despite returning only 1,100 rows.
The borrowed check allocates candidates only for matches. This changes allocation
and row-quota consumption intentionally; cancellation and work remain checked
for every visited pair. It remains bounded nested-loop work, not a hash join or
an unlimited query path. Source snapshots, native allowances, result buffering,
structural ceilings and complete-result publication remain unchanged.

Microsoft's [Access SQL INNER JOIN contract](https://learn.microsoft.com/en-us/office/client-developer/access/desktop-database-reference/inner-join-operation-microsoft-access-sql)
establishes field equality joins and ordinary matching semantics. This slice
changes TableCraft's candidate materialization, not admitted SQL syntax or native
Access compatibility claims. Other comparison families retain their existing
engineering boundaries.

Eight focused query tests and 110 existing retrieval, grouped-subquery,
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
