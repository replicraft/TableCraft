# Declared numeric/String predicates

With `--declared-arithmetic`, a known Scalar Boolean, Byte, Integer, Long,
LongLong, Single or Double operand selects the conversion width for a String
peer. The String can have Scalar, Variant or unknown declaration provenance.
Both operand orders and all six relational operators share this path. Default
materialized comparisons retain their existing Double-based numeric Text route.

The [MS-VBAL relational effective-type table](https://learn.microsoft.com/en-us/openspecs/microsoft_general_purpose_programming_languages/ms-vbal/f8acd631-55c1-4199-bc1e-022aaab6d9c8)
specifies numeric/String Let-coercion and the separate both-Variant exception.
The [comparison reference](https://learn.microsoft.com/en-us/office/vba/language/reference/user-interface-help/comparison-operators)
also distinguishes Scalar/Variant comparison families. These are language-level
references for the explicit engineering profile; native ACE SQL declaration
inference, parsing and error precedence remain unqualified.

```sh
tablecraft query run --declared-arithmetic --number-format en-us \
  "SELECT CInt(2)='2.5' AS Rounded,CLngLng('9007199254740993')='9007199254740992' AS Exact"
```

The result is True/False. Removing the declared option gives False/True under
the default materialized policy. Selected integer conversion uses the existing
half-even rounding and checked range constructors. LongLong Text converts exactly
without an intermediate Double. Single selects Single precision; Boolean uses
the existing Boolean conversion. Supply an exact stored field, parameter or Text
conversion for a large integer: the declared literal-preparation profile may
already select a rounded Double for an untyped decimal integer literal.

## Boundaries and composition

The query predicate boundary checks declaration-sensitive String branches, then
uses `Evaluator::compare_effective_numeric_values` over already-evaluated scalar
operands. Retained String inputs use the existing strict publication adapter.
Declaration categories are not invented from a value. Currency, Decimal and legacy DATE retain their
existing effective-type services. Both-Variant numeric/String ordering and
Scalar String/Variant String-coercion keep their existing branches. Unknown
numeric provenance still refuses when it could change the comparison family.
A successful comparison does not invent a declaration for an unknown operand or
result; a later declaration-sensitive operation retains that uncertainty.

The selected numeric parser uses explicit NumberFormat, existing text ceilings,
normalization allocations, cancellation and the operation's cumulative budget.
Retained Strings publish strictly once for numeric conversion; invalid Unicode
does not acquire a numeric value. No String comparer or field collation activates
for this numeric path. NULL and expression Error handling remain separate; both
written operands still evaluate before conversion. Overflow, malformed Text and
late operand failures remain typed errors, without buffered CLI output.

Filters, HAVING, list/range membership, quantified subqueries and ordinary derived
or scalar subqueries use the same predicate boundary. Prepared queries convert
current parameter Text on each execution without changing captured declarations.
Relational keys, sorting/group identity, source carrier validation and native
index hint eligibility remain unchanged. This is not a new query builtin, native
compatibility qualification or completion of P0 value semantics.

## Verification

Six retained regression tests and 73 existing controls pass: 35 expression tests,
40 query tests and four CLI tests. Controls include raw UTF-16/decoded inputs, half-even boundaries,
LongLong values beyond 2^53, both directions, declaration combinations, all
relational operators, composed predicates, reusable parameters, eager failures,
every observed conversion cancellation checkpoint and zero-byte allocation
refusal. Strict expression/host linting, independent minimal-feature expression
and query checks, and formatting pass. See the
[baseline audit and current verification](testing/results/declared-numeric-text-baseline-audit-2026-10-25.json).

The initial minimal-feature test filter selected zero tests because evaluation
requires the SQL feature; it is not counted as test coverage. The initial CLI
precision control used an untyped large literal that had already rounded under
literal preparation; the final control uses exact Text-to-LongLong conversion.
No product result was adjusted to make that assertion pass. The independent
[source-free/native replay](testing/DECLARED_NUMERIC_TEXT_REPLAY.md) now passes
128 controls comparing 2,592 Boolean/NULL cells and 32 buffered overflow failures
with all originals and artifact/source pins unchanged. Native Access execution
remains separate evidence work.

## Baseline correction

The earlier assessment incorrectly called this new conversion behavior. The
frozen pre-change CLI from `3b1e2344` passes all 128 independent replay controls
with identical outputs and error statuses. Its existing declared predicate
service already selected these widths for decoded and retained String inputs.
The extra helper introduced in `79f4c2cb` was redundant and has been removed,
along with three helper-only tests. Production expression/query sources match
the pre-change tree; the six useful query/CLI tests and independent replay remain.
This is additional verified coverage, not closure of a previously missing P0
coercion branch. Historical helper-build records remain explicitly superseded.
