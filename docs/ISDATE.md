# IsDate scalar inspection

Unqualified `IsDate(value)` returns Boolean in scalar SELECT, bound retrieval and
qualified saved SELECT execution. It takes one actual expression, evaluates eagerly,
and uses the existing backend-neutral Inspect node. No runtime code executes.

| Materialized input | Current result |
| --- | --- |
| Legacy DATE | TRUE, without changing stored bits. |
| Extended date | TRUE if representable as legacy DATE; FALSE outside that envelope. |
| Numeric carriers | TRUE if checked CDate conversion succeeds; FALSE for range overflow or nonfinite values. |
| NULL, Boolean, binary, GUID | FALSE; NULL does not propagate. |
| Text | Whole calendar/time recognition using explicit DateTextFormat, two-digit-year window and optional ReferenceYear. |

Text uses the [date-conversion grammar](DATE_CONVERSIONS.md), including supported
English month names, regional numeric dates and 12/24-hour times. It validates both
date and time components. Missing date format or omitted-year context is an error,
not FALSE. There is no clock, locale discovery or numeric Text fallback: number
format does not turn a bare numeric string into a recognized calendar spelling.
Typed numeric and Boolean behavior is an explicit current profile awaiting native
qualification; IsDate is not an alias for general CDate convertibility.

```sh
tablecraft query eval "SELECT IsDate('04/28/2014'), IsDate('13/32/2014'), IsDate(NULL)" --date-format en-us
tablecraft query run "SELECT IsDate('Feb 29')" --date-format en-us --reference-year 2024
```

The first returns TRUE, FALSE, FALSE; the second returns TRUE. Reusing the same
prepared plan with reference year 2023 produces FALSE. Prepared plans do not retain
execution settings. DateValue/TimeValue and IsDate share their parsing policy,
but failed conversions remain errors outside inspection.

Only expected invalid-value failures become FALSE. Callback errors are tracked
before interpreting conversion failures, including callbacks returning TypeMismatch,
Overflow or NonFinite. Resource, input-size, allocation and cancellation errors
propagate. Query/CLI output remains buffered on failure. IsDate does not short
circuit AND or IIf: `IsDate(Label) AND CDate(Label)>#2020-01-01#` can still fail.
There is no new UPDATE support or predicate pushdown.

## References and remaining scope

Microsoft's [IsDate guide](https://learn.microsoft.com/en-us/office/vba/language/reference/user-interface-help/isdate-function)
and [Access reference](https://support.microsoft.com/en-us/access/functions/isdate-function)
define date/time recognition and the Windows calendar envelope. The guide's dotted time example (`04.28.14`) is supported under en-us; its
four-digit final component (`04.28.2014`) is rejected. Dotted times require three
components and do not accept fractional seconds; de-de retains dotted date ordering.
Full regional grammars, other calendars, Variant/runtime states, numeric Text
recognition and native coercion/error precedence remain unqualified. The
[built-in tracker](ACCESS_BUILTINS.md) therefore marks IsDate **Partial**.

Kernel tests cover materialized types, calendar/range failures, missing policies,
leap-year reference context, generated date spellings and injected host failures.
Evaluator tests cover shared work/input limits and cancellation. Query tests cover
request-local settings, grouping identity and correlations; CLI tests cover both
execution paths, argument rejection and buffered failures. The independent inspection
recipe uses Python Gregorian validation and optional Jackcess-decoded input rows.
These are model/reader comparisons; native Windows Access execution remains unrun.


## Expression-state recognition (23 October 2026)

Like [IsNumeric](ISNUMERIC.md#expression-state-recognition-23-october-2026), IsDate
now receives explicit states after eager operand evaluation. Error data and host
Empty return FALSE; host Missing remains NonScalarExpression. The recognition
boundary does not apply CDate's Empty-to-DATE-zero coercion, nor does it interpret
an Error code as a date serial. Explicit `IsDate(CDate(CLng(CVErr(1))))` remains a
different, successful request. No stored carrier or SQL Empty constructor is added.

The reference is the Variant recognition signature in
[MS-VBAL revision 3.0, IsDate](https://learn.microsoft.com/en-us/openspecs/microsoft_general_purpose_programming_languages/ms-vbal/d5418146-0bd2-45eb-9c7a-fd9502722c74),
read alongside the Access reference above. Existing materialized numeric/Boolean,
regional and precision profiles are unchanged. Recognition still checks work and
cancellation and does not suppress operational operand failures. Error data can
arrive through derived/grouped sources, subqueries or encounter/extrema/domain
aggregates without requiring a stored Error column or aggregate declaration guess.
Native Access admission/error precedence and full Variant recognition remain
unqualified.
