# CDate, CVDate, DateValue and TimeValue

These unqualified one-argument calls now run in scalar SELECTs, bound/nested
queries and qualified saved SELECTs. They return the legacy DateTime carrier or
propagated NULL as specified below. They never read the clock, machine locale or
timezone, and do not execute VBA. Full regional/Variant/native behavior remains
incomplete; all four are Partial in the [built-in tracker](ACCESS_BUILTINS.md).

| Function | Current behavior |
| --- | --- |
| CDate | Retains an existing legacy DATE's exact bits. Converts supported numeric/Boolean carriers through the existing explicit Double conversion and checked Automation serial range. Parses complete Text under explicit date syntax, with optional numeric fallback. NULL is InvalidNullConversion. |
| CVDate | Same materialized DATE conversion as CDate, including NULL failure and explicit policies. The explicit declared-arithmetic profile retains Variant provenance, while CDate retains Scalar provenance; stored/public DATE carriers remain identical. |
| DateValue | Returns midnight on the date portion. Existing DATE values use their exact truncated day, without a second/calendar round trip. NULL propagates in the current SQL profile. |
| TimeValue | Returns the time portion on 1899-12-30. Existing DATE values retain their absolute stored fraction, canonicalized onto the epoch. NULL propagates. |

Numeric/Boolean DateValue/TimeValue inputs use the checked serial conversion then
project its component; they do not gain general implicit coercion elsewhere.
Binary/GUID inputs fail. Existing legacy CDate inputs preserve signed zero and
negative aliases. DateValue/TimeValue produce canonical component values, so zero
need not retain the original sign bit. Temporal SQL comparison remains separate
from representational equality.

Extended date inputs use exact validated components: extraction occurs before
narrowing into legacy DATE. Thus TimeValue can extract a valid time from an extended
year-one date, while CDate/DateValue cannot produce a year-one legacy date. The
legacy result can lose extended fractional precision through its existing IEEE
calendar encoding. Native extended-type, numeric/Boolean coercion, NULL and
range/error-order behavior still require Access comparison.

## Date and time Text

Text requires `--date-format iso|en-us|de-de` or the corresponding DateParseContext/
Evaluator/ExecutionOptions setting. DateTextFormat now lives in the date-parsing
slice and remains re-exported from the existing CStr path. This also controls
[CStr output](CSTR.md); it does not alter [SQL literals](SQL_DATE_LITERALS.md), parameter transport,
ordinary CLI date rendering or stored date fields.

| Profile | Numeric date order |
| --- | --- |
| iso | Year-month-day, with hyphens; omitted-year month-day needs a reference year. |
| en-us | Month/day/year, with slashes. |
| de-de | Day.month.year, with periods. |

An unambiguous four-digit-year ISO date is also accepted in the other profiles.
English full/abbreviated month names use Jiff, including `February 12, 1969` and
`12-Feb-69`; named-date tokens can use spaces, commas or hyphens. This is not a
complete localized month-name table. Weekday-bearing long dates, unsupported
regional separators/patterns and Hijri/alternate calendars remain unsupported.

Times accept hour:minute, optional seconds, and up to nine fractional-second digits
with a period. English AM/PM is case-insensitive, can immediately follow the digits,
and permits an hour-only spelling; 12-hour values require hours 1..12. Without a
suffix, hours are 0..23. Leap seconds, 24:00, offsets/timezones and invalid components
fail. A date can precede a time with whitespace; ISO profile also accepts `T`.
The en-us profile additionally accepts H.MM.SS dotted times, optionally with
AM/PM, without fractional seconds. Dotted dates in de-de retain day.month.year
semantics. Both components are validated completely before DateValue or TimeValue discards one.
Time-only input uses the Automation epoch; date-only input uses midnight.

Text component extraction precedes legacy encoding, avoiding avoidable loss of time
precision from adding a distant date first. DateValue of a valid final-day datetime
can discard its time without first encoding an unrepresentable rounded endpoint.
CDate still checks full legacy encoding. None of these functions borrows CStr's
nearest-second output rounding. Whole-second input can nevertheless have subsecond
IEEE representation error in a full DATE result, as with existing date literals.

CDate first recognizes date/time Text. With an explicit NumberFormat, it can then
try whole numeric Text under that separate profile. A missing reference year is
not retried as numeric input. Malformed/overflowing numeric Text or out-of-date-range
numeric Text fails with TypeMismatch under the current conversion profile. Host
failures retain their origin and propagate, including a callback's Overflow error.
DateValue/TimeValue do not enable this numeric Text fallback.

## Year controls and deterministic execution

Explicit one/two-digit year tokens use the existing DateSerial window (default
1930..2029), overridden by `--two-digit-year-max`. Longer tokens are literal years
within 100..9999. An omitted year requires a validated ReferenceYear in that range,
supplied through `--reference-year`; omission raises ReferenceYearRequired. A host
can supply its current year explicitly. The library never picks a hidden year or
reads a clock. Month/day possibility is validated before requesting a missing year,
and the actual year is then validated by Jiff, including leap days.

```sh
tablecraft query eval "SELECT DateValue('February 12, 1969'), TimeValue('2:24PM')" --date-format en-us
tablecraft query eval "SELECT Year(DateValue('Feb 29'))" --date-format en-us --reference-year 2024
tablecraft query eval "SELECT Year(CDate('1/2/49'))" --date-format en-us --two-digit-year-max 2049
tablecraft query eval "SELECT CDate('-1.25')" --date-format iso --number-format invariant
```

DateParseContext carries format, year window, reference year and optional numeric
fallback syntax. Prepared plans capture no execution settings. New DateConvert
nodes retain fields through pruning/remapping, grouping/aggregate detection,
correlations and qualified multivalue predicates. Optional worker requests forward
and isolate the controls. CLI execution flags remain unavailable on inert parse
or inspection. Bounded UPDATE still rejects these calls.

All written arguments remain eager; invalid shapes fail before operand execution.
Input-size/work/cancellation limits apply before scans and fallible numeric scratch
allocation. Date tokenization uses bounded borrowed arrays, and month recognition
uses bounded Jiff input rather than an unbounded format/string guesser. Partial
results are discarded and failed commands publish no partial stdout. No dependency,
fixture or host runtime is added.

## References and evidence

Microsoft's [CDate conversion guidance](https://learn.microsoft.com/en-us/office/vba/language/concepts/getting-started/type-conversion-functions)
and [MS-VBAL CDate/CVDate](https://learn.microsoft.com/en-us/openspecs/microsoft_general_purpose_programming_languages/ms-vbal/1f287742-e07f-4169-8ce7-5ddfe0f951fb)
guide serial/date conversion. [DateValue](https://support.microsoft.com/en-us/access/functions/datevalue-function)
and [TimeValue](https://learn.microsoft.com/en-us/office/vba/language/reference/user-interface-help/timevalue-function)
describe component extraction and validation of discarded parts. DateValue's
omitted-year behavior refers to the system year; the explicit caller-issued year
above provides that context deterministically. TimeValue's NULL behavior is documented;
DateValue NULL behavior remains an explicit current profile awaiting native evidence.

Eight kernel tests cover raw bits/aliases, numeric bounds, region/month/time grammar,
reference/window controls, dotted-time boundaries, extended projection and resource/error provenance,
including generated whole-day models. Evaluator, query, multivalue, CLI and worker
regressions cover dependencies, settings, buffering and failure reuse. The
[independent recipe](testing/compare_date_conversion.py) uses Python Gregorian and
Automation-serial models plus optional sequential/indexed native-row controls,
checking the source hash. Independent models/readers do not establish native Access
compatibility; Windows comparisons remain unrun.

## CVDate query alias

Microsoft's [sandbox query list](https://support.microsoft.com/en-us/access/functions-and-properties-in-access-blocked-by-sandbox-mode)
explicitly includes CVDate. The MS-VBAL CDate/CVDate contract above specifies the
same conversion with different declared return types. Both materialize the same
legacy DateTime carrier here; VarType returns the Date subtype code 7. Their
resolved function identities now remain distinct so [explicit declared arithmetic](DECLARED_ARITHMETIC.md#date-conversion-return-declarations)
can honor Scalar CDate versus Variant CVDate. Conversion policies, exact bits,
NULL/errors and default materialized arithmetic remain unchanged. No Variant
variable runtime, object conversion or new stored type is introduced. Existing
regional/coercion/precision limits remain; this does not qualify native Access
results. The independent runner accepts `--conversion CVDate` to exercise the
same Gregorian/Automation models under the alternative SQL spelling.

The SQL [date component functions](EXPRESSIONS.md#date-component-source-coercion)
now consume this same bounded CDate conversion for numeric/Text source values.
Existing DateTime/Extended/NULL sources bypass conversion to preserve the component
kernel's established precision and NULL behavior. [DateAdd/DatePart/DateDiff](DATE_INTERVALS.md) now use the same conversion after
their owning interval/control validation; no parser or policy default is added.


Legacy DATE/Text [predicate comparisons](NUMERIC_PREDICATES.md#legacy-date-and-text)
now use the same contextual CDate service. Extended DateTime/Text stays refused;
strict set/group/sort keys and stored values are unchanged.


## Single-date expression states (23 October 2026)

Scalar and relational SQL now retain `ExpressionValue` until CDate/CVDate,
DateValue/TimeValue or component extraction deliberately admits it. Error data
fails TypeMismatch rather than becoming its numeric code; explicit `CLng(CVErr(...))`
remains a separate requested conversion. Host Missing fails NonScalarExpression.
These are expression states, not stored field carriers, and no SQL Empty constructor
is introduced. Query `Nz(NULL)` retains its existing empty Text result.

Host Empty becomes DATE zero for CDate/CVDate and the date source of
Year/Month/Day/Hour/Minute/Second, yielding 1899-12-30 midnight. DateValue/TimeValue
retain an explicit TypeMismatch refusal for host Empty: their String-parameter
coercion is distinct, and this change does not reinterpret it as numeric zero.
NULL rules, numeric/Text policies, legacy bits and Extended precision/range rules
are unchanged. Operational failures and cancellation remain failures, never Error
data. Value-only entry points continue sharing the existing conversion kernels.
The public retained-value CDate/CVDate/DateValue/TimeValue APIs now also accept
valid active String ranges through strict, metered publication followed by those
same kernels; see [retained conversion parity](RETAINED_CONVERSIONS.md).
Malformed selected units and missing conversion policies remain explicit failures.

The reference is [MS-VBAL revision 3.0](https://learn.microsoft.com/en-us/openspecs/microsoft_general_purpose_programming_languages/ms-vbal/d5418146-0bd2-45eb-9c7a-fd9502722c74):
sections 5.5.1.2.9/11 distinguish Error and Empty Let-coercion; section 6.1.2.4
specifies the individual date signatures. This is a reviewed expression profile,
not proof of native Access SQL admission/error precedence or complete regional/
Variant/Hijri support. Independent component models and the Weekday runner cover
retained materialized behavior and state failures; native Access comparisons
remain pending.
