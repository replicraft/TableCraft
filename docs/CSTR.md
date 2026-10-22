# CStr with explicit output policies

Unqualified CStr(value) now works in scalar SELECTs, bound/nested queries and
qualified saved SELECT execution. It returns Text and does not invoke VBA, infer
machine settings or reuse CLI display cells as expression semantics.

| Input | Current behavior |
| --- | --- |
| Text | Retains the exact decoded string, including empty Text, punctuation and controls; no format policy is required. |
| Boolean | English `True` or `False`, without a format policy. |
| Numeric carriers | Requires NumberFormat; emits numeric text with the selected decimal separator, no grouping/currency symbol and no sign-reservation space. |
| Legacy/extended date | Requires DateTextFormat; emits the selected Gregorian date/time spelling. |
| NULL | InvalidNullConversion, unlike Str's NULL propagation. |
| Binary/GUID | TypeMismatch; no implicit binary/object formatting is inferred. |

Arguments remain eager. Wrong arities, qualifiers, wildcards and missing slots fail
at the existing call boundary. An error in an unselected IIf branch still fails.
Native Variant Empty/Missing/Error/array/object states are outside the materialized
model; an evaluation error does not become an `Error ...` string.

## Numeric output

[NumberFormat](NUMBER_FORMAT.md) controls CStr's decimal separator: invariant and
en-us use a period; de-de uses a comma. It does not change Text input, Boolean
keywords, Str, Hex/Oct, Val, SQL literal syntax or parameter transport. Omission is
NumberFormatRequired even for an integer; callers choose policy explicitly.

Numeric CStr adapts the existing [Str renderer](NUMERIC_TEXT.md) through a charged
stream. It removes only Str's reserved leading sign space and translates decimal
points; it adds no second numeric formatting engine or intermediate String.
Integer, Currency and Decimal values retain every exact digit, with fractional
trailing zeros removed. Numeric Text input is copied, not parsed/reformatted.
The current Single/Double profile uses 7/15 significant digits and the same explicit
notation rules as Str; nonfinite inputs fail. It is not a lossless floating serializer.
Native precision, small-number notation, exponent padding, Boolean localization and
complete regional/Variant behavior remain unqualified.

## Date/time output

DateTextFormat is independent of NumberFormat and the DateSerial year window.
Its CLI flag is `--date-format`; omission for date input is DateFormatRequired.
The same syntax policy now serves [explicit date conversion](DATE_CONVERSIONS.md);
its year-window/reference-year settings govern input parsing, not CStr output.
These are named output policies, not complete Windows regional-setting emulations.

| Profile | Date portion | Time portion |
| --- | --- | --- |
| iso | YYYY-MM-DD | HH:mm:ss, 24 hours |
| en-us | M/d/YYYY | h:mm:ss AM/PM, English suffix |
| de-de | dd.MM.YYYY | HH:mm:ss, 24 hours |

Legacy DATE reuses the current checked Jiff calendar conversion and nearest-second
half-expand policy. Extended dates use exact stored components, then the same
nearest-second half-expand rounding. Midnight carry and year bounds are checked;
overflow fails. The normalized Automation epoch date, 1899-12-30, emits time only.
Other midnight values emit date only; other date/time values emit both separated
by a space. For example, epoch zero in en-us emits `12:00:00 AM`; 2026-10-20 at
13:04:05 emits `10/20/2026 1:04:05 PM`.

Fractional output precision, rounding/error order, extended-type behavior, alternate
calendars and configurable regional patterns still require native comparison.
This is a second-precision string conversion, not lossless date serialization or
an implicit CDate/DateValue parser. The `iso` name identifies the date-part style;
the epoch/time-only and date-only rules still apply. Stored bits/100 ns values,
ordinary row display and database files are unchanged.

```sh
tablecraft query eval "SELECT CStr(True), CStr('1.23'), CStr(1.25)" --number-format de-de
tablecraft query eval "SELECT CStr(#2026-10-20 13:04:05#)" --date-format en-us
tablecraft query run "SELECT CStr(ORDR_NBR), CStr(NBR_VALUE) FROM ROULETTE_TB" \
  --database docs/examples/Access_Example_VBA.accdb --number-format invariant
```

## Contracts and verification

Library callers use TextContext, Evaluator::with_number_format/with_date_format,
or ExecutionOptions. Prepared plans do not capture policy; nested queries and the
optional SQLite worker use each execution/request's settings. The CLI flags are
available on eval/run, not parsing or inert inspection. No new runtime/dependency
or fixture is added. The selected Jiff and numeric kernels remain in use.

Text input is checked against existing byte limits. Borrowed Text/static Boolean
spelling and numeric/date recipes are materialized under shared budgets. Date and
numeric scratch use bounded stack storage; appends check cancellation/work and
charge only emitted payload bytes before fallible allocation. Failed commands
publish no partial output. The numeric adapter charges no removed sign-space byte.
All borrowed values remain immutable and output buffers are discarded on error.

Five kernel tests cover exact values, profiles, dates, epoch/midnight carry, bounds,
resources and generated Int64 models. An evaluator test checks emitted-byte charging
and failures; query tests check grouped parameters, nested dates and policy reuse.
CLI and worker regressions verify separate policies, buffering and failure reuse.
The [independent recipe](testing/compare_cstr.py) uses Python integer/Decimal and
calendar models plus optional sequential/indexed native-row controls, checking
source hashes. Explicit exact Currency inputs keep its separate IEEE import policy
out of the numeric preservation comparison. Native Windows Access is unrun; CStr
is Partial in the [built-in tracker](ACCESS_BUILTINS.md).

Primary references: Microsoft's [conversion functions](https://learn.microsoft.com/en-us/office/vba/language/concepts/getting-started/type-conversion-functions)
cover CStr's Boolean/date/NULL/numeric cases; [MS-VBAL String coercion](https://learn.microsoft.com/en-us/openspecs/microsoft_general_purpose_programming_languages/ms-vbal/00113388-401b-41c2-8107-dc3fc0485554)
describes String identity, regional numeric output, sign/precision guidance and
epoch-date time-only behavior. The explicit rounding/pattern/precision policies
above are current implementation contracts, not observed native Access behavior.

The General date renderer is shared with [FormatDateTime](DATE_FORMATTING.md);
its additional named modes do not change CStr component selection or policy.

The bounded seven/fifteen-digit float normalization is also shared with
[named numeric formatting](STYLED_NUMBER_FORMATTING.md); fixed-place adornments
and rounding do not alter CStr or Str's separate spelling contracts.


The contextual [`&` operator](CONCATENATION.md) now reuses these numeric/date
renderers for scalar operands, with the same explicit policies and formatting
limits. It handles NULL at the operator boundary rather than calling CStr(NULL).
This does not make implicit Binary/GUID or expression Error rendering available.
