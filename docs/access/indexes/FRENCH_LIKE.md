# French legacy LIKE

Exact French legacy descriptor `1036/0/0` supplies the provisional UTF-16 weighted
LIKE policy. The Access adapter selects the existing `Utf16Weights` matcher;
there is no second grammar or French-only subset. The host still chooses
ANSI-89 or ANSI-92 explicitly. Unknown descriptors, Windows Text/LCID selection
and equality-prefix hints keep their separate boundaries.

## Pattern semantics

The existing [pattern contract](../../expressions/text/PATTERN_MATCHING.md) owns
wildcards, lists, ranges, negation, empty `[]`, eager operands and reached-error
timing. `?`/`_` consume one raw UTF-16 unit, so a supplementary pair takes two.
ANSI-89 `#` tests one ASCII digit; an ANSI-92 `[0-9]` is a weighted class.
Complete literal runs and class elements can consume equivalent expansions.
Negated classes retain the existing excluded-prefix rule: an excluded expansion
at a start blocks that start before shorter nonexcluded alternatives can pass.

French pattern callbacks encode complete, **untrimmed** operands. Trailing
spaces remain literal, including inside class candidates. For example, SQL
comparison and the separate substring policy can equate `a ` with `a`, while
`'a' LIKE 'a '` is FALSE. Physical index prefixes/digests and the SQL/search
trimming helper never supply pattern equality or order. Raw units, isolated
surrogates and checked little-endian String ranges retain the matcher’s existing
input/publication boundaries.

Full French keys decide literal equality and class range order, including the
reversed secondary stream. Class eligibility additionally requires equality to
some single raw unit’s complete key; sorting between two endpoints alone is
insufficient. The immutable French dictionary is derived separately from the
untrimmed full keys of all 65,536 units. It is not the General Legacy dictionary
reused by assumption, and it requires no per-call construction or runtime cache.

## Bounds and ownership

Only the append-only Legacy primary stream permits pruning. French reverses the
secondary stream after primary construction, so secondary differences never
justify dropping a candidate extension. Literal steps compare complete French
keys first, then use the shared primary-prefix proof. Class steps require exact
dictionary membership or traverse only the primary prefix to establish that no
single-unit key can extend it. This shares working primitives with General
without changing the expression parser or matcher.

Key buffers, encoding, comparison and FST traversal use one cumulative provider
budget and cancellation callback. The adapter retains the existing shared pool
with comparisons and substring searches, and refuses reentrant mutable access.
Expression parsing, raw-unit buffers and iterative matcher frontiers retain
their own cumulative allowances. Large ambiguous patterns can exhaust those
limits; they do not trigger an exhaustive unmetered fallback or publish partial
query results. No per-candidate budget is reset.

## Evidence and remaining scope

The ignored dictionary generator passed its first run. Its retained output
produced the 33,228-byte original derived artifact with SHA-256
`a8584c05037d6695926c93ea3c3ffe0c6a332454acdfb863901d5ca5151808f1`.
The first dictionary-only gate passes all 65,536 raw-unit memberships, exact
distinct-key inventory and CRC validation. The subsequent first focused batch
passes all seven engine functions, including the artifact hash, independent
pattern oracle and pruning/resource controls. The new Access target passes six
functions; all six existing provider and seven query-contract functions pass
with the new policy. The dictionary gate overlaps the seven engine functions
and is not counted as an additional distinct test. General regressions pass six
engine functions and three adapter functions; its two engineering generators
remain ignored in the ordinary run. All 263 INSERT functions also pass.
The full ACE library passes 613 functions with two ignored engineering generators
(the existing General generator and the new French generator). The full Access
run passes 506 functions across 63 result groups. These totals include the
focused functions; they are not additional independent populations.

The first strict lint run found three redundant String conversions in the new
adapter tests. Only those conversions were removed, retaining the first source
freeze and all literal expectations; no runtime behavior changed. Corrected
strict all-target/all-feature Clippy, minimal builds and formatting pass for
both crates. Document links and dependency boundaries for all fifteen workspace
packages also pass. Native execution or fixture qualification is not inferred
from these repository checks.

Independent tests were frozen before runtime. A separate recursive pattern
oracle owns its atoms, mode-specific rendering and exhaustive span enumeration;
it shares neither the production parser/frontiers nor FST/pruning. Its element
reference is a test-only complete-key set over all raw units. Literal controls
and generated raw patterns cover both ANSI modes, spaces, expansions, ranges,
negation and raw-unit boundaries. Resource controls include measured one-short
pools, cancellation at every observed callback checkpoint and long-input work
ceilings. Those tests qualify the stated engineering policy, not the native
meaning of every stored weight or Access pattern.

The existing [weight-data provenance](../../../crates/engines/tablecraft-ace/data/general/README.md)
and notices apply. The dictionary and regeneration script are original derived
project artifacts; no new third-party code or database fixture is copied.
Native Access LIKE results, Windows/NLS equivalence, other stored sort orders
and function-specific native error timing remain unqualified. French comparison
and substring observations do not establish those outcomes.
