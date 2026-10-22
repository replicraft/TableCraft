# Subqueries in grouped reads

Grouped SELECT outputs, HAVING conditions and supported sort expressions can now
contain scalar, EXISTS, IN and quantified subqueries. A nested query may use the
outer query's grouping keys; it cannot select an arbitrary member of a group by
capturing an ungrouped outer field. Preparation validates this from bound syntax
and declarations without reading records, including empty sources.

For example:

```sql
SELECT O.Category,
       (SELECT Count(*) FROM Orders AS I WHERE I.Category=O.Category) AS Total
FROM Orders AS O
GROUP BY O.Category
HAVING EXISTS (SELECT I.Id FROM Orders AS I WHERE I.Category=O.Category);
```

## Scope and evaluation rules

The validator tracks the particular grouped query frame as it enters nested
reads. It compares computed keys structurally after shifting correlated field and
collection-item depths. A grouped `Bucket Mod 2` permits that same expression in a
nested predicate; it does not permit bare `Bucket` or reordered arithmetic.
Aliases and pruning retain the already-bound field identities. Two nested levels
cannot accidentally refer to a same-position field from another query frame.

Nested SELECT projections, filters, grouping, HAVING, ordering, UNION arms and
join conditions are checked. Lowered parent-collection predicates are also checked,
including combined membership products and Boolean wrappers. Independent derived
and named definitions bind without lateral scopes; their internal correlations
belong to those definitions, not the enclosing grouped query. Named crosstabs
retain that isolated boundary. Inline TRANSFORM remains outside this profile.

An aggregate in the grouped outer query still evaluates its operand per input
record, so that operand may contain a row-correlated subquery. Aggregates inside
a nested query do not grant permission to capture an ungrouped outer field.
Aggregate ownership remains the existing lexical query-plan rule; wider native
Access ownership/inference is not established by these controls.

Execution reuses the existing nested-read evaluator. Scalar zero-row results are
NULL; multiple rows fail cardinality checks. NULL equality, IN/quantified truth,
eager operand evaluation and buffered publication are unchanged. HAVING in a
top-level crosstab can use the same qualified subquery predicates; unsupported
crosstab output syntax is not enabled. No correlated-result cache or early EXISTS
termination is introduced. Binding visits and every execution stage remain bounded
by existing work/depth/row/allocation limits and cancellation.

## Evidence

Focused tests cover scalar/HAVING/EXISTS/IN/ANY/ALL, computed keys, two nested
query frames, UNION, join predicates, lexical aggregate operands, derived-scope
isolation, NULL keys, cardinality and crosstab HAVING. Every observed cancellation
checkpoint of staged binding/execution refuses a result; byte/row/work exhaustion
also fails. A scan-panicking source proves that invalid captures fail during
preparation. Collection tests cover hidden captures after parent-membership
lowering.

Original synthetic native rows exercise these queries through profile bytes
1/2/5/6 (Jet 4, ACE 2007 and ACE 16-family layouts). These reuse the reviewed shared
fixture recipe and qualify only those tested read/query paths. No original binary
is changed, active content executed or native Access result recorded.

The original [partition-model CLI runner](testing/compare_grouped_subqueries.py)
checks 96 deterministic cases with duplicate rows, NULL amounts, positive/negative
values, correlated sums/counts and grouped EXISTS filtering. Python independently
partitions the inputs and derives expected outputs. It also checks that an invalid
capture returns a failure with no partial stdout. Run after building the CLI:

```sh
python3 docs/testing/compare_grouped_subqueries.py --cli target/debug/tablecraft
```

Microsoft's [Access SQL subquery reference](https://learn.microsoft.com/en-us/office/client-developer/access/desktop-database-reference/sql-subqueries-microsoft-access-sql)
documents SELECT-list and WHERE/HAVING subqueries, the predicate forms and outer
aliases for correlation. It supports the syntax direction, not a claim that our
precise grouping, ownership, error order or optimizer behavior matches native ACE.
Native Windows comparisons and broader application-query qualification remain open.
