# LIKE matching

`tablecraft-expressions::predicates::like` supplies bounded Text matching for
Text/NULL values. Its optional SQL evaluator exposes `with_like_mode` and
`apply_like` / `apply_expression_like`; `tablecraft-query` binds LIKE/NOT LIKE in projections, predicates,
aggregate expressions and nested queries. Syntax remains TableCraft-owned and
the upstream parser AST stays private. No pattern matching belongs in the CLI or
native storage engine.

The evaluator applies String Let-coercion to both operands in ordinary and
explicit declared query modes. Numeric/Boolean and legacy Date operands use the
existing metered String conversion with explicit number/date policies; Empty
becomes empty Text. NULL propagates before conversion or comparison-provider
activation, although the wildcard mode remains explicit. Both source expressions
still evaluate eagerly. Error data is not implicitly rendered using CStr's
special Error spelling. Missing, stored GUID/Binary and Extended DateTime remain
outside this coercion profile.

Hosts can call `prepare_expression_like` before selecting a provider, then
`apply_prepared_like` without repeating conversion. `PreparedLikeOperands`
borrows original Text/raw Strings and owns only freshly converted Text. Its
conversion markers distinguish original String policies from fresh Default
provenance; a numeric operand's latent field policy does not become a String
policy. The low-level Text matcher remains strict and performs no coercion.

This follows [MS-VBAL §5.6.9.6](https://learn.microsoft.com/en-us/openspecs/microsoft_general_purpose_programming_languages/ms-vbal/792d4f80-72c1-49da-98a7-37d5783a8615)
and the existing explicit [String conversion](CSTR.md) profile. Direct query
availability is documented by Microsoft's [Access LIKE operator](https://support.microsoft.com/en-us/access/like-operator).
These language and query contracts do not establish native Windows formatting,
collation or error-order qualification.

A host may select `LikeProfile::Unsupported` when it supplies ordering without
matching. Non-NULL LIKE then returns `UnsupportedExpression`, including wildcard-only
patterns that would otherwise need no ordering callback. Mode checks, NULL
propagation and operand preparation/coercion retain their existing phases; the
unsupported check precedes raw String publication and pattern execution. Existing
UnicodeScalar and Utf16Weights providers retain their matching paths. Exact
French legacy now supplies the separate
[weighted LIKE service](../../access/indexes/FRENCH_LIKE.md) rather than the
unsupported profile; unknown/unqualified providers retain explicit refusal.

`predicates::like_utf16` also accepts borrowed `LikeString::Text` and
`LikeString::Utf16Le` inputs under an explicitly selected UTF-16 weight provider.
Complete raw units, including isolated surrogates and ranges starting at odd byte
offsets, reach the existing literal/class callbacks without Unicode publication.
The matcher builds checked unit buffers with explicit little-endian conversion;
it performs no alignment casts, replacement, padding or truncation. Odd byte
lengths remain UnrepresentableText. This non-NULL service rejects scalar-only
providers. The expression evaluator preserves their existing strict Unicode
publication path and keeps the separate NULL gate ahead of decoding.

Query routing uses the retained field/runtime String policy to select the
provider. Unknown/conflicting required policies fail, with no fallback or fake
Text placeholder. The provisional General profiles expose their existing raw-unit
callbacks through the CLI:

```sh
tablecraft query run "SELECT ChrW(55296) LIKE '?' AS Matches" --like-mode ansi89 --collation general-2010
```

This produces TRUE under the explicit UTF-16 profile. It does not establish native
Access equivalence or make an isolated surrogate publishable as stored Text.
Raw unit buffers and pattern/matching states consume the same cumulative byte,
step and cancellation controls. The per-Text UTF-8 ceiling still applies to
decoded Text operands; raw unit buffers have no fabricated UTF-8 byte size.

The host must explicitly select `LikeMode::Ansi89` or `LikeMode::Ansi92` through
`ExecutionOptions::like_mode`. Default options do not choose a convention, even
for NULL operands. Non-NULL text matching also requires a supplied collation.
The CLI exposes these controls through `query run`:

```sh
tablecraft query run "SELECT 'apple' LIKE 'a*' AS Matches" --like-mode ansi89 --collation ordinal
tablecraft query run --database example.accdb "SELECT ID FROM Customers WHERE Name LIKE 'A%'" --like-mode ansi92 --collation ordinal
```

The database's ANSI query property/provider convention is not inferred. Parameters
can supply patterns, including NULL; reused plans evaluate their current values.
The optional SQLite worker forwards its owned `AsyncOptions::like_mode`, keeping
pattern semantics in TableCraft. `query eval` has no text-policy controls, and the
restricted UPDATE planner continues to reject LIKE before backend planning.

## Pattern error timing

The matcher follows the element-by-element error rule in
[MS-VBAL §5.6.9.6](https://learn.microsoft.com/en-us/openspecs/microsoft_general_purpose_programming_languages/ms-vbal/792d4f80-72c1-49da-98a7-37d5783a8615).
An earlier mismatch returns FALSE before an unreached malformed class or reversed
range. For example, `'x' LIKE 'a['` is FALSE, while `'ab' LIKE 'a['` raises
InvalidPattern. NOT LIKE negates successful truth values; it does not catch errors.
A reached `*` immediately validates the remaining pattern grammar, so
`'x' LIKE '*a['` raises InvalidPattern even though no `a` matches. The ANSI-92
`%` wildcard uses the same explicit timing profile.

Grammar validation is separate from provider-dependent range ordering. Thus
`'x' LIKE '*a[z-a]'` is FALSE without activating the reversed range's provider;
`'ab' LIKE '*a[z-a]'` reaches that class and raises InvalidPattern. Typed provider
failures propagate at the first reached class and are never relabeled as syntax
errors. The existing whole-class range validation remains, rather than adding a
new rule for stopping inside a list after one member matches.

The exhaustion rule precedes an additional consuming class: `'' LIKE '['` and
`'a' LIKE 'a['` are FALSE. Zero-width `[]` and reached stars still operate:
`'' LIKE '[]*'` is TRUE, `'' LIKE '*['` raises InvalidPattern, and
`'' LIKE '*[z-a]'` is FALSE without a range comparison. This is an explicit
interpretation of the formal rule, not an observed native Access error-order
result. Microsoft's [Access SQL LIKE page](https://support.microsoft.com/en-us/access/like-operator)
establishes query use and wildcard matching, but does not specify these error
frontiers. Windows/ACE qualification remains open.

Both matchers retain bounded iterative position frontiers. The scalar matcher
now reserves two byte-indexed input frontiers, each of UTF-8 input length plus
one, instead of two pattern-token frontiers. The UTF-16 path keeps its existing
three unit-indexed frontiers. Pattern storage, input conversion, frontier buffers,
scanning and transitions remain charged to the original cumulative pools. An
unreached pattern error does not suppress cancellation, byte/work limits, eager
operand failures or required String conversion. No budget or performance claim
is implied by the timing correction.

## Pattern profile

| Feature | ANSI-89 | ANSI-92 |
| --- | --- | --- |
| Zero or more selected units | `*` | `%` |
| Exactly one selected unit | `?` | `_` |
| ASCII digit | `#` | `[0-9]` |
| Character list/range | `[abc]`, `[a-c]` | `[abc]`, `[a-c]` |
| Negated list | `[!abc]` | `[^abc]` |

Bracketed wildcard literals and an opening bracket are supported; a closing
bracket is literal outside a list. A leading/trailing list hyphen is literal,
while interior hyphens form ranges. Reversed ranges, unterminated lists and
misplaced interior hyphens fail with a payload-free `InvalidPattern`. `[]` denotes
a zero-length sequence. Patterns match the entire string; newline and trailing
space are ordinary characters. No regex syntax or escape-clause extension is
introduced. Wildcards from the other mode remain literal.

The default `LikeProfile::UnicodeScalar` consumes Unicode scalar values, not graphemes or UTF-16 code units.
Literal equality and range order use the host collation on individual scalars.
There is no implicit case folding, normalization, numeric/date formatting or
multi-character expansion/contraction. In particular, plugging in a locale string
comparator cannot supply Access ligature/digraph matching. This is an explicit
scalar profile; Access locale fidelity needs a broader matcher and native evidence.
The CLI ordinal policy is case-sensitive Unicode scalar ordering.
Explicit `general-legacy`/`general-2010` CLI policies now select the separate
`LikeProfile::Utf16Weights` matcher. Each `?`/`_` consumes one raw UTF-16 unit,
so an emoji takes two; `#` remains ASCII-digit-only. Complete literal runs and
character-list elements can consume unequal-length equivalent spans such as
`ss`/`ß`, with spaces retained. Empty `[]` groups do not split adjacent literal
runs. The supplied `compare_like_units` and `is_like_unit` contracts keep weights
and single-element recognition out of the expression layer. A string merely
sorting inside a range is insufficient: the span must represent a single element.

Positive lists retain every matching element span. Negated lists first check
all element prefixes at a start; any excluded prefix blocks that start before
shorter nonexcluded alternatives are admitted. Thus `ss LIKE '[!ß]*'` is FALSE
under this explicit model. This excluded-prefix choice and raw UTF-16 semantics
remain provisional native compatibility rules, not observed Access results.
Expansions are compared within a contiguous literal run or one list element;
wildcards/lists separate runs, with no hidden splitting of one character's weight
stream across tokens. Full SQL text comparison's trailing-space trimming is not
reused for LIKE. Exact French legacy also selects this full grammar, with its
own reversed-secondary complete keys and separately derived dictionary. See
[General matching](../../access/indexes/NATIVE_TEXT.md#general-utf-16-like-matching)
and [French matching](../../access/indexes/FRENCH_LIKE.md).

The UTF-16 path compiles literal arenas and list ranges, then iterates tokens
over budgeted position vectors. It uses no recursive/exponential backtracking.
Pattern syntax is scanned under the shared work/allocation budget, but malformed
class errors remain deferred markers until matching reaches them. Range-order
providers run only for classes with a reachable, non-exhausted input position. A final literal needs only complete-tail comparisons, so long literal
matches do not encode every shorter prefix. Weighted providers may return a
conservative span state: complete match, possible extension, or proof that no
extension at that start can match. Only the last state prunes candidate ends;
default callbacks never prune. General implements this with append-only primary
weight prefixes and immutable FST traversal, retaining complete raw-key checks
for acceptance. Provider normalization/FST work uses
the same cumulative General budget as other comparisons. Expression scans,
buffers and transitions retain their separate byte/work/cancellation controls.
Large ambiguous patterns can still exhaust these allowances and must publish
no partial query result. Other host collations stay on the scalar path unless
they explicitly opt in and provide the raw-unit callbacks.

With a selected mode, either NULL operand produces unknown; NOT retains unknown.
Retained expression Strings use this gate before Unicode publication, including
odd byte ranges and unpaired UTF-16 units. They still require an explicit mode;
non-NULL raw matching requires complete units and a UTF-16 provider, or strict
Unicode publication for scalar providers. See
[retained predicate boundaries](RAW_PREDICATES.md).
Remaining unsupported non-NULL operand kinds fail with `TypeMismatch`. Both operand expressions
are evaluated eagerly before matching, including errors in a pattern expression
when the tested expression is NULL. Missing policy, bad-pattern, collation,
resource and cancellation failures never publish partial query results. As with
existing runtime scalar operations, policy/pattern errors arise when evaluated;
an empty input need not evaluate a predicate. Unsupported child syntax still
fails binding even on an empty relation.

## Work bounds and evidence

Each match compiles its pattern under the shared byte/step budget and runs an
iterative dynamic program with two state buffers. It has no recursive/exponential
backtracking. Pattern parsing, every input/state transition, list comparisons and
completion check cancellation/consume steps. Buffer reservations are fallible and
charged before allocation. Evaluator text ceilings still apply. Worst-case work
includes list sizes; budget exhaustion is an explicit failure. Constant patterns
are currently recompiled per match; no unbounded compiled-pattern cache is added.
Host collation callbacks own their internal time/work bounds.

Literal goldens cover wildcard modes, Unicode, lists/ranges, escaped characters,
NULLs/types, missing policies, collation failures, cancellation and exhausted
buffers/work. A fixed-seed 256-case property compares short generated wildcard
patterns against an independent exhaustive matcher. SQL evaluator, query, CLI and
async worker tests cover eager behavior, parameter reuse, grouping/aggregate
traversal, correlated subqueries, errors and atomic output.

[Native-file comparisons](../../testing/compare_like.py) use independent inert Jackcess
rows and explicit Python regular-expression expectations on fresh Jet 4 and both
ACE 16-family files. These test TableCraft's ordinal pattern profile over native
text; they are not native Microsoft Access SQL/locale comparisons.
[Fixture provenance](../../testing/FIXTURES.md#native-like-query-inputs) records inputs and scope.

## Original native-source coercion replay

The [136-control replay](../../testing/results/native-like-coercion-2026-10-25.json)
checks the registered original Scalar V2000, V2007, V2016 and V2019 files in
ordinary and declared arithmetic modes. All 944 modeled result cells agree.
Nine native numeric/Boolean/date carriers appear in both LIKE operand positions;
NOT LIKE, parent filtering, NULL suppression, missing formatting/comparison
policies, Error states, eager expression failures and invalid patterns are covered.

Expected spellings come from previously pinned inert ordinary getter transcripts,
exact integers/fractions and Gregorian calendar facts under invariant numeric and
ISO date policies. Neither CStr nor another SQL evaluator supplies the answers.
The [runner](../../testing/compare_native_like_coercion.py) rechecks the getter hashes
and preserves all four original hashes before and after execution. The initial
unbracketed Money field-name authoring failure is retained separately; its parser
failure and repeated controls are excluded from the accepted count.

The frozen executable uses revision `612bd74a` for the relevant query/expression
sources and includes eight pending native check-only production changes. Their
exact snapshots and the frozen executable/build-manifest hashes are recorded;
this is not a clean-full-tree build claim. Scoped committed source snapshots are
not a complete compilation manifest. Native Microsoft Access execution and
locale qualification remain unrun; this replay establishes the explicit
TableCraft profile over original native stored values.

## Primary references

Reviewed 19 October 2026:
[Access wildcard conventions](https://support.microsoft.com/en-us/access/access-wildcard-character-reference)
describes the two mode families and database setting distinction.
[Microsoft LIKE rules](https://learn.microsoft.com/en-us/office/vba/language/reference/user-interface-help/like-operator)
describes NULL, lists/ranges, zero-length brackets and locale expansions. These
references guide the explicit profile above; they do not establish conformance.

## Pattern timing verification

The [timing verification record](../../testing/results/read-query-like-timing-2026-10-25.json)
records 219 unique tests, including ten new focused tests. The scalar and
weighted matchers share explicit element-frontier goldens; independent generated
valid-pattern models remain passing. Query and CLI checks cover both ANSI modes,
NOT LIKE, raw intermediate Strings, NULL, eager operand failures and buffered
publication. Provider failures, byte/work limits and cancellation remain typed.

The first matcher batch retained one obsolete raw-empty upfront-error assertion;
it was changed to the documented exhaustion result, without changing positive
matching goldens. A separate initial lint attempt found only unnecessary clones
of Copy expected values. Both attempts remain recorded. The earlier coercion
implementation record and frozen original-file replay above remain historical
artifacts; neither is relabeled as native evidence for the new timing rules.
