# Native saved-query fragment ordering

SELECT, UNION and crosstab reconstruction now share an ordered view of their
native fragments. The engine still exposes exact sequential rows and original
nullable Order bytes. The adapter never mutates, clones or rewrites that native
definition. `inspect --query` continues to report physical row order; explicit
`query saved` and saved/named execution consume the ordered view.

## Reference rules

The reviewed Jackcess 5.0.4
[QueryImpl.sortByOrder/compareOrder](https://github.com/jahlborn/jackcess/blob/be0a2139b087a227d14cafd4ea231306a5a3a862/src/main/java/com/healthmarketscience/jackcess/impl/query/QueryImpl.java)
sorts only when every fragment has a non-NULL Order value. In that case it sorts
by signed native attribute byte, then unsigned lexicographic Order bytes. Prefixes
sort before longer keys; an empty non-NULL key is distinct from NULL. Equal keys
retain physical encounter order. A NULL key anywhere preserves the entire original
row sequence, including otherwise keyed columns, sources, joins and parameters.
This is a whole-definition policy, not per-clause partial sorting.

The adapter follows these rules rather than treating Order as an increasing u32
counter or requiring physical rows to be in key order. Metadata carrier validation
still applies: an ordering payload cannot exceed the native column's declared
capacity or the read budget. Unknown/NULL tags, duplicate singleton clauses,
conflicting roles, duplicate logical parameter/source names and unsupported
payloads remain errors. Order ties do not authorize dropping a fragment or
overwriting metadata. UNION operand labels still determine left/right arms.

Clause extraction, source forest construction, parameter declarations and final
ORDER BY keys all use the same ordered reference vector. Multiple ON conditions
still prepend in their resulting native order and attach to the proper containing
join. Sorting happens before that semantic reconstruction, not after constructing
a join tree or binding fields. No provider, function or active application content
runs merely to inspect or reconstruct a definition.

## Resources and integrity

The adapter charges a fallibly reserved reference vector. Already ordered inputs
avoid a scratch allocation. Other fully keyed inputs use a stable bottom-up merge
with one charged scratch vector and O(n log n) comparisons. Every scan/merge and
comparison checks cancellation/work; byte comparisons charge their conservative
prefix allowance. Parsing and final publication retain existing independent
syntax/memory/resource limits. There are no new dependencies or format writes.

The fixture writer's native system primary index requires unique, non-NULL keys.
Tie/NULL cases are therefore deliberate mutation copies with inconsistent system
index metadata. Their reader results test the reference ordering policy; they do
not prove that native Access creates, accepts, executes or saves such databases.
Reconstructing these rows is not an index-consistency check or write qualification.
Ordinary index selection retains its independent validation/no-fallback contract.

## Evidence

Five feature tests cover all ordered clause families, unchanged physical fragments,
unsigned/prefix/empty/tied keys, whole-definition NULL behavior, native outer joins,
every measured construction/publication cancellation checkpoint, resource limits
and 128 generated key layouts against an independent stable sort. The generator
respects the authored fixture's four-byte binary column capacity. Existing SELECT,
UNION, crosstab, name, join and parameter checks continue to pass. One CLI test
compares physical inspection with saved/named output header and row ordering.

The original [SavedOrderingFixture.java](testing/SavedOrderingFixture.java) adds
eleven inert definitions to our fourteen-query forest baseline on fresh local
Jet 4 / ACE 2007/2016/2019 files. All inserted ordering keys satisfy the system
primary index. [compare_saved_ordering.py](testing/compare_saved_ordering.py)
passes 252 getter/rendering/direct/saved/named/refusal controls, including explicit
row/header expectations and separate copy-only tie/NULL cases. TSV raw inspection
is compared after CSV decoding; getters inspect physical system rows while the
reference SQL renderer orders them independently. Originals and mutation-copy
bytes remain unchanged. [Fixture records](FIXTURES.md#native-fragment-ordering-inputs)
retain provenance and observed hashes.

```sh
python3 docs/testing/compare_saved_ordering.py \
  --jar path/to/jackcess-5.0.4.jar --cli target/release/tablecraft \
  --directory path/to/fresh-native-ordering --generate
```

These are independent metadata/row-model comparisons. Native Microsoft Access
execution, system-index admissibility, error order and application save/reopen
remain unrun; this does not complete the backend roadmap.
