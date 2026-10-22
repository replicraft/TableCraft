# Projected ordering for DISTINCT reads

`SELECT DISTINCT` and the qualified `DISTINCTROW` profile now accept ORDER BY
expressions that structurally match a supported stable projection. Bare output
aliases and ordinals retain their existing behavior. For example:

```sql
SELECT DISTINCT Len(Label) AS Width FROM Items ORDER BY Len(Label);
SELECT DISTINCT T.ID FROM Items AS T ORDER BY T.ID DESC;
SELECT DISTINCT Count(*) FROM Items GROUP BY Category ORDER BY Count(*);
```

The binder resolves source fields first and compares bound expression identities.
Matching scalar and aggregate trees become output references carrying that output's
text policy. Sorting therefore uses the projected value, without evaluating the
expression again. The existing duplicate-removal, encounter/tie, NULL, TOP and
DISTINCTROW complete-record rules remain unchanged. Ordinary non-DISTINCT sorting
keeps its existing independent expression evaluation.

Identity is structural, not algebraic: `N+1` does not match `1+N`. Fields retain
their query frame and pruned ordinal. Grouping still disallows aggregates as keys;
projection identity allows matching aggregate kinds/operands, including aggregate
subexpressions. This shared identity service does not enable aggregate GROUP BY
or treat read plans as grouping keys.

Repeated clock/Rnd, opaque domain calls and subqueries do not acquire reusable
identity, including inside a larger expression or aggregate argument. Use an
explicit selected alias to order those values. Sort expressions that use output
aliases inside a larger expression, unselected DISTINCT keys and broader read-plan
identity remain unsupported. New matching does not execute providers or scan rows
while preparing, coerce result carriers or infer collation. Unsupported forms fail
even for empty sources. All structural comparisons share existing bind work/depth
and cancellation limits.

Focused tests cover qualified fields, computed/NULL outputs, aggregates and
aggregate subexpressions, DISTINCTROW/TOP, metadata-only refusal and retained late
errors. Per-field collation controls verify inherited policy routing; an explicit
Rnd alias proves one draw per input rather than a second draw for sorting. The
original [set/order CLI runner](testing/compare_projected_sort.py) independently
models 144 deterministic arithmetic/NULL and grouped-count cases with Python sets
and counters. The existing original native grouped-query fixture also checks
computed and aggregate DISTINCT ordering across Jet 4/ACE 2007/ACE 16-family
read paths. None of these execute SQL through Microsoft Access.


```sh
python3 docs/testing/compare_projected_sort.py --cli target/debug/tablecraft
```

Microsoft documents [ORDER BY](https://support.microsoft.com/en-us/access/order-by-clause)
and the [DISTINCT/DISTINCTROW/TOP predicates](https://support.microsoft.com/en-us/access/all-distinct-distinctrow-top-predicates).
These establish the query concepts, not native Access optimizer evaluation counts,
expression identity, inferred type or complete sort compatibility. Native Windows
comparisons remain unrun; current independent evidence is separate.
