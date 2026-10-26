# Format date and time masks

Format now renders Gregorian date/time masks and named date styles in scalar and
bound read-only queries. Its [query contract](https://support.microsoft.com/en-us/access/functions/format-function)
is separate from a field's display property. The
[date/time formatting reference](https://support.microsoft.com/en-us/access/format-a-date-and-time-field)
describes related field tokens and regional settings; it does not establish that
every field property behaves identically in the expression function.

## Supported masks

| Tokens | Current output |
| --- | --- |
| d / dd | Numeric day, optionally padded |
| ddd / dddd | Abbreviated/full weekday name |
| ddddd / dddddd | Explicit short/long date profile |
| m / mm | Month, or minute after an hour field |
| mmm / mmmm | Abbreviated/full month name |
| y / yy / yyyy | Day of year, two-digit year, four-digit year |
| q | Quarter |
| h / hh, n / nn, s / ss | Hour, minute, second with optional padding |
| w / ww | Weekday and week number under supplied/default week rules |
| c / ttttt | General date/time and long-time profile |
| AM/PM, am/pm, A/P, a/p | Explicit English meridiem; select 12-hour hours |
| AMPM | Regional meridiem under the current en-US profile |
| / and : | Explicit profile's date/time separators |

The [Format function token reference](https://learn.microsoft.com/en-us/office/vba/language/reference/user-interface-help/format-function-visual-basic-for-applications)
distinguishes minute `m` following an hour from month `m`. The scanner retains the
preceding hour across literal punctuation/text, then clears that context after
another temporal field. Longer month-name tokens remain months. Field letters
are case-insensitive; quoted/escaped data is never reinterpreted as fields.
Meridiem fields select 12-hour output; quoted AM/PM text does not. Marker casing
follows the first letter; broader mixed-case/native behavior remains unqualified.

Intrinsic numeric fields work without a regional profile for existing date/numeric
carriers. Localized names, unescaped `/` and `:`, regional fragments and ordinary
named styles need an explicit DateTextFormat. Escaping a separator requests that
literal punctuation instead. The existing iso/en-US/de-DE profiles supply English
or German names; all three currently use `:` for time. Date separators are `-`,
`/`, and `.` respectively. No database LCID, OS locale or clock is inferred.

```sh
tablecraft query eval "SELECT Format(#1993-01-27#,'yyyy-mm-dd')"
tablecraft query run "SELECT Format(#1993-01-27 17:04:23#,'hh:mm:ss am/pm')" --date-format en-US
tablecraft query eval "SELECT Format(#2018-12-31#,'w ww',2,2)"
```

Results are `1993-01-27`, `05:04:23 pm` and `1 53` under the current profiles.

## Named styles and calendar conversion

General Date, Long Date, Short Date, Long Time and Short Time reuse the existing
[FormatDateTime profiles](DATE_FORMATTING.md). Short Time retains its intrinsic
24-hour path without a regional profile. Medium Date currently uses `dd-mmm-yy`
with the selected names; Medium Time uses `hh:nn AM/PM` under en-US only. Regional
AMPM and Medium Time reject iso/de-DE rather than inventing designators. These
medium-style choices and regional defaults still require native qualification.

Stored legacy/Extended dates use their shared calendar conversion. Numeric and
Text inputs use CDate's current explicit Gregorian profile, including two-digit
year and omitted-year controls. Text date input requires a date profile; numeric
serial input does not. Negative DATE serials retain their existing calendar
semantics. Extended components remain Extended until formatting's whole-second
half-expand rounding; range/day carry uses Jiff and checked overflow. No fractional
output mask is added, and exact stored fractions remain unchanged in ordinary
row browsing. The Extended year-one boundary is supported by the kernel; this
is not evidence that legacy DATE admits year one.

Week fields consume the same internal WeekSettings as DatePart, retaining its
[explicit system policies and documented final-Monday anomaly](../dates/WEEK_SETTINGS.md).
Concrete arguments override host settings; omitted options stay Sunday/Jan-1.
Explicit zero options require supplied settings. DatePart/DateDiff algorithms
were not replaced. Native week/rounding/coercion/error-order qualification remains
open.

## Boundaries and evidence

The shared 257-unit mask/quote rules and eager operand evaluation still apply.
Date-typed and Text sources still require a single date section. Numeric serial
sources additionally support the narrow positive-date/literal-alternatives profile
below. Negative calendar rendering, mixed numeric/date sections, numeric formatting
of Extended dates, broader Variant/raw-string coercion, other calendars/locales
and native precision/default/error behavior remain unfinished. NULL handling retains Format's existing empty/fourth-literal
path. Unknown fields/repetition counts fail without a fallback format.

The date scanner, patterns and renderer are feature slices under text/format/date.
Public Format recipes keep parser/upstream types out of query plans, retaining
only explicit policies and borrowed masks. Conversion scratch and output remain
separate, charged and fallible. Every scanned/consumed token and output fragment
checks work/cancellation. An error discards buffered output; preparation does not
execute a stored query or read rows to invent a schema.

Six kernel controls cover token/context/literal goldens, all named styles,
regional refusals, negative serials, Extended range/day carry, shared week behavior
and a fixed-seed 256-case component model. Two scalar and three query controls
cover conversion, aggregate/correlated/grouping dependencies, reusable parameters
and week/year policies, eager errors and every observed cancellation checkpoint. Two
CLI controls exercise both commands and empty stdout on refusal.

The original [independent runner](../../testing/compare_format_dates.py) passes 13020
calendar/week/refusal/native-field controls. Python calendar/components and the
existing enumerated-week model supply expectations. Four original ScalarFixture
files supply ordinary stored DATE and NULL values through reviewed Jackcess
getters, with unchanged hashes; generated files and the JAR remain untracked.
The initial harness exceeded the CLI's cumulative work ceiling with 22 fields in
one SELECT. The passing run batches six probes at a time, retaining every case
and the original limits. Actual Access evaluation, native Extended field-format
execution and Windows NLS comparisons remain unrun. Format stays **Partial**.

Numeric masks now also render the stored legacy DATE serial through the shared
[numeric Format profile](FORMAT.md#numeric-display-of-legacy-datetime-serials).
That path needs a number profile and preserves raw negative fractions; it does
not use this calendar renderer or infer a date/OS locale.

General Format's week options now also accept raw DATE and explicit-policy SQL
numeric Text through [the shared week conversion](../dates/WEEK_SETTINGS.md#contextual-week-option-conversion).
NULL preflight, progressive option validation, regional/date policies, borrowed
source/mask and existing calendar rendering are preserved.

## Numeric serial sections with literal alternatives

A numeric serial can use a positive date mask followed by literal-only negative,
zero and NULL sections. For example, `Format(N,'yyyy;"negative";"epoch";"missing"')`
renders numeric 2 as `1900`, -2 as `negative`, zero as `epoch`, and NULL as `missing`.
Boolean true follows its existing numeric -1 interpretation; false and signed
floating zero select the zero section. Positive serials use the existing checked
CDate/calendar renderer and explicit policies. Literal alternatives need no date
or numeric display profile.

The [Microsoft Format reference](https://learn.microsoft.com/en-us/office/vba/language/reference/user-interface-help/format-function-visual-basic-for-applications)
permits numeric date/time serials to use date masks and defines numeric section
selection by sign, zero and NULL. [Access expression documentation](https://support.microsoft.com/en-us/access/introduction-to-expressions)
establishes query expression use of Format with a date mask. This implementation
combines those rules only where no negative calendar interpretation is required.
It does not establish native Access multisection qualification.

Date rendering in this profile requires a strictly positive actual numeric
carrier. A nonpositive selected section containing date or numeric placeholders
remains refused, including empty-section fallback to a date mask. Existing
DateTime, Extended DateTime and Text multisection sources remain refused. NULL
retains the preexisting fourth-literal path. Quoted and escaped literals,
semicolon parsing, eager week controls, shared work/output limits and cancellation
use the existing checked services; unselected alternatives do not activate a
calendar or display provider.

Six new expression/query tests and twenty-nine selected regressions pass, along
with focused strict lint and standalone minimal expression checks. See the
[verification record](../../testing/results/format-serial-sections-2026-10-25.json).
