# Native saved-query join forests

Native local SELECT and crosstab reconstruction now accept up to sixteen declared
sources and sixteen ordered join fragments. Sources may also be
[stored derived reads](NATIVE_SAVED_DERIVED.md). These are current adapter ceilings,
not Microsoft Access limits. Parser byte/token/depth and execution budgets may
reject smaller queries. Source-free queries remain supported. No dependency,
native storage layout, writer or backend-neutral query contract changed.

## Reconstruction

Sources start as ordered independent roots. Native join kind 1/2/3 means
INNER/LEFT/RIGHT. Name1 is the left endpoint; Name2 is the right endpoint. Endpoint
identity is the raw stored alias, or table name when unaliased, including any
[stored bracket delimiters](NATIVE_SAVED_NAMES.md). Logical labels must be unique
under ASCII-insensitive, otherwise exact matching. Multi-source labels may contain
Unicode table names or aliases, including one stored bracket delimiter pair. No
Unicode normalization or non-ASCII case equivalence is inferred. This is metadata identity, not SQL
data collation. Undeclared endpoints, duplicate labels and an edge to the same
source fail explicitly. Self-joins require distinct aliases. Anonymous derived
sources have no endpoint label; they remain roots unless explicitly aliased.

Each join fragment consumes its two current roots and appends the resulting join,
retaining the order of other roots. A later edge between endpoints already inside
one root adds its predicate to the innermost containing join with the same kind.
If a deeper containing join has another kind, a matching ancestor may receive it.
No matching kind is an explicit failure. Reversed endpoint labels on an additional
condition do not reverse an established outer join. These rules follow the
reviewed [Jackcess 5.0.4 metadata renderer](https://github.com/jahlborn/jackcess/blob/be0a2139b087a227d14cafd4ea231306a5a3a862/src/main/java/com/healthmarketscience/jackcess/impl/query/QueryImpl.java); native Access qualification remains pending.

Conditions prepend in stored order with their original expression grouping.
They are neither sorted nor deduplicated. An added condition inside a LEFT join
must remain there: moving it to an enclosing ON or WHERE could discard rows that
should survive with NULL-extended fields. Parenthesized nested joins preserve the
constructed topology. Disconnected sources stay separate comma roots, including
two sources without an edge. The shared [native fragment ordering](NATIVE_SAVED_ORDERING.md)
view supplies join order before forest construction: key order when all keys are
present, stable physical ties, or the whole physical sequence when any key is NULL.

An original arena of borrowed source names/condition text and guarded derived
read carriers builds this forest.
Every predicate passes independent original/guarded expression parsing. Bounded
SELECT parsing supplies the owned source tree; verification checks all roots,
children, kinds, names, aliases and condition expressions before publication.
Unaliased join parentheses normalize out of the syntax model while preserving
tree structure. Allocations charge before fallible reservations, construction and
verification check cancellation, and final publication retains the shared check.

## Retrieval and limits

`query saved` remains inert: it does not bind names, scan user rows, resolve
providers or call functions. Explicit `query run --saved` and named SELECT sources
use existing binding, dependency cycles, snapshot budgets, eager expression/error
semantics and buffered stdout. Crosstabs share the forest adapter; qualified named crosstab schemas follow
their separate binding/discovery contract. Unknown functions can reconstruct as syntax but
remain execution failures unless an explicit callable contract supplies them.
Source bytes, native identity and original fragments remain unchanged.

External native table qualifiers, native Unicode/NLS label equivalence,
unqualified stored name forms and graphs exceeding consumer limits remain gaps.
This work does not implement Access's full mixed outer-join admissibility,
optimizer behavior, native collation, error order or Windows qualification.
Ordinary explicit SQL execution retains its existing independent limits.

## Evidence

Focused Rust tests compare independent expected trees for nested/mixed/reversed
edges, disconnected roots and late predicates. Authored native layouts exercise
Jet 4 and ACE 2007/2016/2019 headers with unchanged bytes; a crosstab regression
checks the shared adapter and missing intersections. Every measured construction/
publication cancellation checkpoint refuses; byte/work and parser depth ceilings
are checked. CLI tests execute saved and named sources and refuse inert unknown
calls without partial stdout. Existing same-pair condition and malformed metadata
tests remain in the saved-query suite.

The original [SavedForestFixture.java](testing/SavedForestFixture.java) recipe
creates fourteen inert definitions in each of four fresh files. The independent
[comparison](testing/compare_saved_forests.py) passes 240 SQL-assembly, table-getter,
direct/saved/named execution and mutation-refusal controls. Jackcess renders SQL
but never executes it. Separate Python relational models control exact nullable
rows for three/four-table joins and comma products. Copies with conflicting or
unknown kinds refuse reconstruction/execution. Original and mutation-copy bytes
remain unchanged. [Fixture provenance and hashes](FIXTURES.md#saved-join-forest-inputs)
record the observed files; regenerated files can differ in incidental metadata.

```sh
python3 docs/testing/compare_saved_forests.py \
  --jar path/to/jackcess-5.0.4.jar --cli target/release/tablecraft \
  --directory path/to/fresh-join-forests --generate
```

These are independent renderer/row-model comparisons, not native Microsoft Access
execution or application save/reopen evidence. Windows comparisons remain unrun.


## Unicode source labels

Unicode labels now use the same ASCII-insensitive, otherwise exact policy as
owned query binding. Sources and join endpoints preserve raw stored delimiters;
duplicate logical labels compare after removing one bracket pair. Exact composed
and decomposed spellings remain distinct. A differently cased non-ASCII endpoint
is not silently rebound to another source. Native Unicode/NLS identifier matching
still needs its own provider/evidence; this change does not qualify that behavior.

The original [Unicode fixture recipe](testing/SavedUnicodeFixture.java) extends our
existing inert forest recipe, adding seven named/derived/comma/inner/outer/nested
source definitions. [Independent comparison](testing/compare_saved_unicode.py)
passes 112 normalized rendering and direct/saved/named execution controls across
Jet 4, ACE 2007 and both ACE 16-family layouts. Table getters and explicit nullable
row models supply expected data; Jackcess renders but never executes SQL.
Original hashes remain unchanged. [Fixture records](FIXTURES.md#unicode-saved-source-inputs)
retain the generated inputs and provenance separately from native Access qualification.

```sh
python3 docs/testing/compare_saved_unicode.py \
  --jar path/to/reviewed/jackcess-5.0.4.jar --cli target/debug/tablecraft \
  --directory path/to/fresh-unicode-sources --generate
```
