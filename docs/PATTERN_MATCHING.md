# LIKE matching

`tablecraft-expressions::predicates::like` supplies bounded scalar matching for
Text/NULL values. Its optional SQL evaluator exposes `with_like_mode` and
`apply_like`; `tablecraft-query` binds LIKE/NOT LIKE in projections, predicates,
aggregate expressions and nested queries. Syntax remains TableCraft-owned and
the upstream parser AST stays private. No pattern matching belongs in the CLI or
native storage engine.

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
| Zero or more scalar units | `*` | `%` |
| Exactly one scalar unit | `?` | `_` |
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

Matching consumes Unicode scalar values, not graphemes or UTF-16 code units.
Literal equality and range order use the host collation on individual scalars.
There is no implicit case folding, normalization, numeric/date formatting or
multi-character expansion/contraction. In particular, plugging in a locale string
comparator cannot supply Access ligature/digraph matching. This is an explicit
scalar profile; Access locale fidelity needs a broader matcher and native evidence.
The CLI ordinal policy is case-sensitive Unicode scalar ordering.
Explicit `general-legacy`/`general-2010` CLI policies instead supply the
provisional General weights for each scalar comparison, under their own
cumulative budget. They do not change the matcher units or supply native
expansion/contraction behavior; full-string equality and LIKE remain separate
operations. See [the selected policy](NATIVE_TEXT.md#general-query-policy).

With a selected mode, either NULL operand produces unknown; NOT retains unknown.
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
