# MonthName and WeekdayName

These unqualified calls now return Text in scalar SELECTs, bound/nested retrieval
and qualified saved SELECT execution:

- `MonthName(month, [abbreviate])`
- `WeekdayName(weekday, [abbreviate], [firstdayofweek])`

The numeric month range is 1–12 in the current Gregorian profile. Weekday values
are 1–7 relative to the selected first day, with wraparound. Numeric operands use
checked Long half-even conversion; abbreviate uses the current numeric/Boolean
conversion profile and defaults to FALSE. NULL raises InvalidNullConversion in
any supplied slot. [Legacy DATE serials](DATE_NUMERIC.md) now use the same numeric
ordinal/Boolean conversion; calendar month/day extraction is separate. Extended-date,
binary/GUID ordinal coercion and full Variant promotion remain
outside this profile. Month 13 and alternate calendars are unsupported.

Optional missing slots work without treating SQL NULL as omission. For example,
`MonthName(3,)` returns the full name, while `WeekdayName(1,,2)` requests the full
name of Monday. Missing required slots, qualifiers, wildcards and wrong arities
fail before operand evaluation. Written expressions remain eager; errors inside
arguments or unselected IIf branches are not hidden.

## Numeric Text controls

Evaluator calls now admit Text ordinal, abbreviation and first-day controls through
[the shared numeric converter](../numeric/NUMERIC_TEXT.md). A reached Text conversion requires
an explicit NumberFormat (`--number-format`); this setting is independent of the
DateTextFormat used for labels. Ordinals and first-day controls use direct checked
Long half-even conversion without a floating intermediate. Abbreviation controls
use the existing Boolean Text profile, including numeric nonzero and True/False.
For example, `2.5` selects month 2, while
`2.5000000000000000000000000000000001` selects month 3.

Conversion follows existing admission order: ordinal, abbreviation, ordinal range,
then WeekdayName first-day and label language. An invalid weekday range therefore
fails before a later Text first-day needs conversion or number policy. NULL,
Empty/Error, explicit system-first-day context and eager written expressions keep
their existing rules. The parser-free recipe API continues to refuse Text; only
the evaluator owns metered contextual conversion. Input/output limits, cumulative
allocation/work and cancellation remain enforced.

```sh
tablecraft query eval "SELECT MonthName('10'),WeekdayName('1','False','2')" --number-format en-us --date-format en-us
```

Microsoft's [MonthName query examples](https://support.microsoft.com/en-us/access/functions/monthname-function)
and [WeekdayName query examples](https://support.microsoft.com/en-us/access/functions/weekdayname-function)
explicitly use Text ordinals. MS-VBAL revision 3.0 sections 6.1.2.11.1.28 and
6.1.2.11.1.43 provide the Long/Boolean argument declarations; the
[reviewed specification record](../VARIANT_ERRORS.md#explicit-numeric-empty-and-cdec-error-rules)
identifies the cached revision. These references and independent controls do not
establish native Access locale/coercion/error equivalence.

## Language and first-day policies

Names require an explicit DateTextFormat through `--date-format` or library
execution settings. en-us and iso use Jiff's English full/abbreviated names.
de-de uses fixed German standalone full and abbreviated labels; for example,
March is `März` or `Mär`, and Monday is `Montag` or `Mo`. Abbreviations are not
locale-independent prefixes. German full names are shared with FormatDateTime.
These are explicit supported profiles, not proof of every native NLS spelling.

WeekdayName's omitted first-day argument and explicit zero request the host's
system setting. Hosts supply a validated FirstDayOfWeek, numbering Sunday 1 through
Saturday 7. The CLI provides `--first-day-of-week DAY`; no process setting or
calendar preference is inferred from date syntax/language. Without host context,
the system/default case raises CalendarPolicyRequired. An explicit SQL value 1–7
overrides the host setting. Values outside 0–7 fail. The flag is available only
on query eval/run, including saved SELECT execution; parse/inspect remain inert.

The same first-day host setting now serves explicit zero options in
[DatePart/DateDiff](WEEK_SETTINGS.md). Their omitted options retain Sunday and
Jan-1 defaults. A separate --first-week-of-year flag supplies their system
first-week requests.

```sh
tablecraft query eval "SELECT MonthName(3),MonthName(3,True)" --date-format de-de
tablecraft query eval "SELECT WeekdayName(1),WeekdayName(1,,1)" --date-format en-us --first-day-of-week 2
tablecraft query run "SELECT WeekdayName(7,True,7)" --date-format en-us
```

Expression results are `März`/`Mär`, `Monday`/`Sunday`, and `Fri`, respectively.
CLI TSV preserves its existing typed Text quoting and CSV transport escaping.

`sys-locale` can discover preferred language identifiers for a future host
selection flow, but does not supply calendar labels, abbreviation patterns or
first-day settings. It is not a replacement for the explicit data/policies above.
The [library policy](../../development/LIBRARY_POLICY.md#host-locale-discovery) keeps discovery outside
the query engine, with user settings taking precedence.

## Ownership and evidence

The parser-free calendar-name slice owns validated name recipes and shared German
labels; Jiff owns English calendar spelling. The date slice owns FirstDayOfWeek
validation. Text nodes preserve all arguments through binding, pruning, grouping
identity, aggregates and correlations. Prepared plans retain no language or host
week setting; optional worker settings are request-local. The engine does not
activate VBA, load locale providers or perform I/O to evaluate these functions.

Name emission uses bounded stack formatting or static labels and a charged,
fallible output sink. Resource/input/output limits and cancellation remain errors.
Failed query/CLI execution publishes no partial stdout. No UPDATE support or
predicate pushdown is added. No dependency or fixture is added.

Microsoft's [MonthName reference](https://learn.microsoft.com/en-us/office/vba/language/reference/user-interface-help/monthname-function)
defines ordinal naming and the default full-name option. Its
[WeekdayName reference](https://learn.microsoft.com/en-us/office/vba/language/reference/user-interface-help/weekdayname-function)
defines first-day-relative numbering and the default system sentinel zero.
[Unicode CLDR's German Gregorian data](https://github.com/unicode-org/cldr-json/blob/main/cldr-json/cldr-dates-full/main/de/ca-gregorian.json)
provides a reference for the explicit standalone labels; CLDR is not native Access
comparison evidence. Broader locales/calendars, full Variant coercion, native
NLS spelling, default behavior and error precedence remain unqualified. Both
functions are **Partial** in the [built-in tracker](../ACCESS_BUILTINS.md).

Kernel tests exhaust full/short month names and all seven first-day/weekday
combinations, overrides, missing policy, coercion, domain/NULL errors and host
failures. Separate tests cover first-day validation and evaluator resource limits.
Query, CLI and worker regressions cover fields, grouping identity, correlations,
optional slots, flag validation, settings isolation and recovery after missing
context. The [independent recipe](../../testing/compare_calendar_names.py) uses Python
Gregorian weekday mapping and optional sequential/indexed projections over
Jackcess-decoded row inputs, verifying unchanged source bytes. Native Windows
Access remains unrun; these checks do not establish native compatibility.

## Ordered expression-state admission

MonthName and WeekdayName now retain Error/Empty/Missing operands until their
owning admission stages. Ordinal conversion precedes abbreviation conversion;
ordinal range validation precedes WeekdayName's first-day conversion. Therefore
`MonthName(0,CVErr(11))` gives TypeMismatch during abbreviation admission, while
`WeekdayName(0,False,CVErr(11))` gives Domain before first-day admission. Final
name resolution still requires explicit DateTextFormat. These are preserved
TableCraft validation stages, not native error-order qualification.

Host Empty selects numeric zero: an ordinal then fails its range, an abbreviation
becomes FALSE and a supplied first-day zero requests the caller's explicit system
setting. Missing host first-day settings remain CalendarPolicyRequired. Error
data gives TypeMismatch and Missing gives NonScalarExpression when reached.
NULL and all ordinary numeric/DATE/Text/binary/extended-date boundaries remain.
Written operands stay eager and once-only, including controls following Error
data; later division failures cannot be masked by ordinal validation.

The shared kernel now requests each borrowed argument at its original conversion
point; materialized callers use the same path. Existing Jiff/explicit German
labels, output limits, policies and return categories remain. Two expression
tests exercise Empty/domain/type/first-day order, output budgets and every
observed successful cancellation checkpoint. A third checks once-only Rnd
controls across these calls and FormatDateTime. Query/CLI tests retain derived
Error values through validation and keep failed buffered output empty.

Microsoft's [Access MonthName](https://support.microsoft.com/en-us/access/functions/monthname-function)
and [WeekdayName](https://support.microsoft.com/en-us/access/functions/weekdayname-function)
contracts guide the existing argument roles; the [shared conversion basis](../formatting/STYLED_NUMBER_FORMATTING.md#ordered-numeric-formatting-states)
separates Empty targets and explicit Error conversion. Broader Variant/Text
coercion, NLS/calendar names and native Access precedence remain unqualified.

The original [runner](../../testing/compare_calendar_format_states.py) passes 302
combined controls: 238 scalar eval/run and 64 native sequential/indexed cases over
four unchanged registered large-index inputs. Explicit English/German full/short
labels and epoch-date arithmetic supply independent models. Read-only getters
supply native identities; no reference SQL/VBA executes. Host Empty is tested
separately in Rust; SQL Nz(NULL) retains its existing empty Text identity.
