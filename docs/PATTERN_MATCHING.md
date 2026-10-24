# LIKE matching

`tablecraft-expressions::predicates::like` supplies bounded Text matching for
Text/NULL values. Its optional SQL evaluator exposes `with_like_mode` and
`apply_like` / `apply_expression_like`; `tablecraft-query` binds LIKE/NOT LIKE in projections, predicates,
aggregate expressions and nested queries. Syntax remains TableCraft-owned and
the upstream parser AST stays private. No pattern matching belongs in the CLI or
native storage engine.

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
reused for LIKE. See [General matching](NATIVE_TEXT.md#general-utf-16-like-matching).

The UTF-16 path compiles literal arenas and list ranges, then iterates tokens
over budgeted position vectors. It uses no recursive/exponential backtracking.
All pattern ranges validate before matching; only reachable transitions invoke
providers. A final literal needs only complete-tail comparisons, so long literal
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
Other non-NULL operand kinds fail with `TypeMismatch`. Both operand expressions
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

[Native-file comparisons](testing/compare_like.py) use independent inert Jackcess
rows and explicit Python regular-expression expectations on fresh Jet 4 and both
ACE 16-family files. These test TableCraft's ordinal pattern profile over native
text; they are not native Microsoft Access SQL/locale comparisons.
[Fixture provenance](FIXTURES.md#native-like-query-inputs) records inputs and scope.

## Primary references

Reviewed 19 October 2026:
[Access wildcard conventions](https://support.microsoft.com/en-us/access/access-wildcard-character-reference)
describes the two mode families and database setting distinction.
[Microsoft LIKE rules](https://learn.microsoft.com/en-us/office/vba/language/reference/user-interface-help/like-operator)
describes NULL, lists/ranges, zero-length brackets and locale expansions. These
references guide the explicit profile above; they do not establish conformance.
