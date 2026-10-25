# Contextual numeric and date comparisons

`Evaluator::compare_values` now supplies an explicit materialized scalar predicate
profile. Scalar SQL and bound comparisons use the same service, including WHERE,
join conditions, HAVING, BETWEEN, IN and quantified/subquery predicates. It also
applies to supported domain criteria and closed Eval expressions. Reusable query
consumers retain their existing statement/operation restrictions.

| Operands | Predicate behavior |
| --- | --- |
| Numeric and Text, in either order | Convert Text under the selected NumberFormat, then compare numerically |
| Text beside Currency | Checked four-place Currency conversion |
| Text beside Decimal | Checked Decimal conversion |
| Text beside ordinary integer/Boolean/Single/Double | Double conversion; LongLong is rounded to Double before comparison |
| LongLong beside Single/Double | Both operands convert to Double; Single widens rather than narrowing the LongLong |
| Currency beside integral/Boolean | Checked Currency conversion of both operands; an out-of-range LongLong fails Overflow |
| Currency beside Single/Double | Convert the floating operand through checked Currency conversion |
| Decimal beside Single/Double | Convert the floating operand through the existing CDec profile |
| Text beside Text | Existing collation and captured field-policy routing; no numeric reinterpretation |
| Either scalar NULL | Unknown, without parsing the other already-evaluated scalar |
| Legacy DATE beside ordinary numeric/Boolean | Checked CDate conversion of numeric operands, followed by temporal comparison; no Text policy needed |
| Legacy DATE beside Decimal | Existing CDec conversion and numeric comparison of raw serials |
| Legacy DATE beside Text | Convert Text through CDate under an explicit date policy, then compare temporally |
| Extended DATE, GUID or unsupported carriers mixed with Text | Existing refusal; no implicit narrowing or GUID parsing |

[Relational keys](NUMERIC_KEYS.md) retain a separate strict contract. Mixed
Single/Double keys widen Single exactly instead of narrowing Double; scalar
Single/Double predicates keep their effective Single rule. Key comparison does
not acquire numeric Text parsing or the wider scalar family conversions above.

Mixed numeric/Text predicates require NumberFormat; missing policy is an explicit
NumberFormatRequired failure. Malformed numeric Text fails, rather than sorting
by type name or becoming zero/NULL. Converted fractions are not rounded to an
integer field's width. LongLong/Single, LongLong/Double and LongLong/Text predicates now round the
LongLong through checked nearest-even Double conversion. Integer-only comparisons
remain exact; explicit Decimal conversion retains its separate fixed-point profile.
Checked Decimal/Currency range, rounding and underflow rules remain authoritative.

Lexical and floating inputs can differ at a Currency midpoint. Text 0.00015 rounds
to Currency 0.0002, while its binary64 spelling lies below that exact midpoint and
rounds to 0.0001 under the retained-binary conversion profile. The predicates use
the existing conversion services, not a second arithmetic or decimal parser.

```sh
tablecraft query eval "SELECT 2='2,0', 2 BETWEEN '3,0' AND '1,0'" --number-format de-de
tablecraft query run "SELECT ID FROM ScalarSamples WHERE ID>'1.5'" --database example.accdb --number-format en-us
```

## Legacy DATE and Text

Legacy DATE/Text predicates reuse [CDate](DATE_CONVERSIONS.md), in either operand
order. They require DateTextFormat (`--date-format iso|en-us|de-de`), including when
an explicit NumberFormat enables numeric serial fallback. The existing two-digit
year window and optional reference year apply; prepared plans use the current
execution settings. Missing policy/year, invalid syntax and range errors remain
explicit. No machine locale, clock or timezone is consulted. This is the bounded
CDate syntax profile, not full Windows regional parsing.

Comparison is temporal: negative Automation DATE aliases can compare equal despite
different stored bits. Extended DateTime/Text remains refused, so the predicate
cannot silently narrow its 100-nanosecond precision through legacy CDate.

Ordinary DATE/numeric predicates now reuse the existing effective-type service:
Date is selected beside integral/Boolean/floating/Currency values, while Decimal
selects Decimal. Without Decimal, numbers outside the Date envelope fail Overflow;
with Decimal, comparison does not impose that Date range on the numeric operand.
For example, CDate(-.25)=CDbl(.25) compares calendar aliases as equal, while
CDate(-.25)=CDec(.25) compares raw serials as unequal. This routing is independent
of declaration categories and now works without --declared-arithmetic in scalar
and bound queries. Other declaration-dependent conversions retain their existing
profiles. The relational effective-type table motivates this engineering profile;
native Access coercion, date ordering and failure precedence remain unqualified.

```sh
tablecraft query eval "SELECT #2025-01-01#='01.01.2025'" --date-format de-de
tablecraft query run "SELECT ID FROM ScalarSamples WHERE Stamp>'2025-01-01'" --database example.accdb --date-format iso
```

The [CDate conversion reference](https://learn.microsoft.com/en-us/office/vba/language/concepts/getting-started/type-conversion-functions)
supports the conversion mechanics. It does not establish that every Access SQL
operand shape chooses this conversion; native declared/Variant typing remains
unqualified. Both operands still evaluate eagerly before predicate conversion.

## Boundaries, ordering and budgets

This is a materialized numeric-comparison profile. It does not infer declared SQL
expression types or implement the complete distinction between typed operands and
two Variant expressions. In particular, native Variant numeric-before-Text ordering
is not supplied by this conversion rule. [Microsoft's comparison reference](https://learn.microsoft.com/en-us/office/vba/language/reference/user-interface-help/comparison-operators)
describes those distinctions and the fixed/floating conversion direction. These
references motivate the profile; native Access SQL qualification remains pending.

The pure `comparison::compare` contract is unchanged. Group keys, sorting,
duplicate removal and set equality continue using their existing strict service.
A successful mixed predicate does not authorize merging differently typed keys,
inventing a result schema or changing stored values. Source schemas and parameters
are still validated without coercion. Full subtype/declared/Variant inference,
additional temporal/coercion behavior and native operator precedence remain open.

Both written operand expressions evaluate before predicate conversion. BETWEEN
and membership retain their current eager/deferred-failure profiles; a matching
item cannot hide a later division-by-zero expression. All conversion scans,
normalization allocations, row work and comparison callbacks share execution
limits and cancellation. The query boundary preserves operation-level cancellation
and resource-limit errors from this service; other scalar failures remain typed
Evaluation errors. No partial result is published.

Only actual Text/Text comparisons activate field collation policies. A numeric
conversion does not silently select an unknown text collation. No coercive index
hint is introduced: the original filter remains, and a partly safe AND cannot
hide a bad value by discarding its row. Statement admissibility, physical write
qualification and preservation rules are unchanged. No new dependency, core
carrier, host runtime or storage codec is introduced.

## Evidence

Three expression tests cover regional numeric Text, exact Decimal and Currency
versus binary inputs, NULL/strict-key distinctions, range/precision refusal,
shared byte quotas and every observed conversion cancellation checkpoint. Four
query tests cover scalar/relational/domain predicates, joins, membership/quantifiers,
prepared policy changes, strict group/set equality and a source spy proving that
partial hints cannot hide a late bad value. Both CLI query commands retain buffered
failures; existing collation/cancellation tests remain passing.

The original [independent runner](testing/compare_numeric_text_predicates.py) passes
1,060 Decimal/rounding/predicate/native-field controls over four unchanged original
ScalarFixture inputs. Raw getters supply field values only. Only TableCraft
executes SQL; native Access typing, coercion, optimizer and error order remain
unqualified.

Three additional expression tests cover DATE/Text syntax, optional serial fallback,
year controls, NULL, strict/extended boundaries and every observed cancellation
checkpoint. Three query tests cover bound filters, joins, membership/subqueries,
domain criteria, closed Eval, per-execution settings and eager failures. Both CLI
query modes check buffered failures. The original [date predicate runner](testing/compare_date_text_predicates.py)
passes 310 Gregorian/serial/native-field controls over four unchanged priority-family
files; only TableCraft executes SQL. Native Access remains unrun.

## Rounded LongLong predicates

`Evaluator::compare_values` now handles LongLong beside Single, Double or numeric
Text by converting both operands to Double through the existing conversion service.
Both written orders and all six comparisons share this path. Converting Single to
Double is essential: leaving it as Single would cause the strict mixed-precision
comparer to narrow the rounded LongLong back to Single. For example,
`9007199254740995>CSng(9007199254740992)` remains true.

LongLong/Double equality can identify adjacent large integers after rounding:
`9007199254740993=CDbl(9007199254740992)` is true in this materialized profile.
Comparing those two integers directly remains false. The same service feeds
WHERE, joins, HAVING, BETWEEN, list/subquery membership, ANY/ALL and domain criteria.
Prepared parameters are converted anew on every execution. Numeric Text still
requires NumberFormat and retains the existing fractional Double route; this is
not an implementation of every declared/Variant Text coercion rule.

Pure `comparison::compare`, sorting, grouping and set-key equality are unchanged.
The source's exact stored values are never rewritten or merged by this predicate
conversion. Coercive floating/Text comparisons receive no exact integer index hint;
the original WHERE and whole-predicate safety rule remain mandatory. NULL returns
unknown before conversion; malformed Text, nonfinite values and eager expression
failures remain explicit. All conversions share work/byte limits and cancellation.

The [MS-VBAL relational table](https://learn.microsoft.com/en-us/openspecs/microsoft_general_purpose_programming_languages/ms-vbal/f8acd631-55c1-4199-bc1e-022aaab6d9c8)
selects Double for LongLong with Single/Double. That motivates this floating
extension, not native Access SQL qualification. The existing numeric-Text policy
remains a separate bounded materialized contract; it does not establish the table's
LongLong/String or declared-Variant branches.

Two expression tests cover endpoints, ties, Single widening, strict/integral
separation, required policies, nonfinite refusal and every observed cancellation
checkpoint. Two query tests cover bound parameter reuse, metadata without scans,
join/membership/quantified predicates, exact group keys and no coercive source hint.
The original [runner](testing/compare_rounded_predicates.py) passes 284 controls and
independently models
rounded significands and all six comparisons in both orders, including seeded
Int64 inputs. Native-source controls use raw ID getters from four unchanged
registered 20,000-row fixtures with sequential and both explicit ID-index directions.
Only TableCraft executes SQL; Windows Access equivalence remains unqualified.


## Ordinary Date/numeric predicate evidence

The shared Date/numeric service now runs from ordinary `compare_values` as well
as the explicit effective-type API. No declaration flag, numeric Text policy or
regional date parser is required for these numeric operands. Strict comparison
and set/group/sort keys retain their own service; native Date predicate pushdown
is not introduced. A partially supported AND cannot discard rows before a Date
conversion failure is evaluated. Extended Date mixtures remain unsupported.

Two new evaluator tests cover exhaustive quarter-day calendar/raw-serial
comparison models and checked Date envelopes. Two query tests cover ordinary
filters, reversed operands, membership/ranges, subqueries, joins, grouping/HAVING,
domain criteria and eager failures. A CLI test covers both eval/run and buffered
refusals. The original [comparison runner](testing/compare_date_numeric_predicates.py)
passes 236 query controls with independent Decimal day/fraction tuples and raw
native DATE getters on unchanged registered four-family inputs. It checks both
operand directions, nine numeric families, range errors, NULLs, and sequential/
ascending/descending index paths. Only TableCraft executes SQL; native Access
comparison/coercion and error precedence remain unqualified.

## Shared ordinary numeric effective types

Ordinary pure-numeric predicates now use the same checked effective-type service
as the explicit declaration profile. Decimal takes precedence over Currency;
Currency takes precedence over ordinary integral and floating carriers. Single
with Long/LongLong selects Double; Single with Double selects Single. The other
integral pairs retain their reviewed common width. Both operands convert before
comparison. This closes a Currency/LongLong gap: exact strict-key comparison could
previously compare an integer outside Currency's range without an overflow.

For example, `CCur(0)=CLngLng(922337203685478)` now fails Overflow, while
`CCur(0)<CLngLng(922337203685477)` succeeds. This follows the checked operand
conversion required by the [MS-VBAL relational effective-type table](https://learn.microsoft.com/en-us/openspecs/microsoft_general_purpose_programming_languages/ms-vbal/f8acd631-55c1-4199-bc1e-022aaab6d9c8).
It is a reference-backed materialized predicate profile, not native Access SQL
qualification. Numeric Text retains its existing separate default Double route
and explicit NumberFormat requirement; declared/Variant provenance is not inferred.
Strict sort/group/set-key comparison remains separate.

An independent checked scaled-integer property covers Currency/LongLong ordering
and range failure in both operand directions. Expression controls retain NULL and
strict-key distinctions. Query controls cover list/subquery membership, quantified
predicates, ranges, and both declaration settings. Source controls prove that a
partially safe AND cannot introduce a hint that hides overflow, including when the
other condition excludes every row. Empty sources still do not evaluate predicates.
Both CLI query modes retain buffered failures. Windows Access comparison remains
pending; no storage, index qualification or write gate changed.
