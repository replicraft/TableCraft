# MonthName and WeekdayName

These unqualified calls now return Text in scalar SELECTs, bound/nested retrieval
and qualified saved SELECT execution:

- `MonthName(month, [abbreviate])`
- `WeekdayName(weekday, [abbreviate], [firstdayofweek])`

The numeric month range is 1–12 in the current Gregorian profile. Weekday values
are 1–7 relative to the selected first day, with wraparound. Numeric operands use
checked Long half-even conversion; abbreviate uses the current numeric/Boolean
conversion profile and defaults to FALSE. NULL raises InvalidNullConversion in
any supplied slot. Text/date/binary/GUID ordinal coercion and full Variant promotion
remain outside this profile. Month 13 and alternate calendars are unsupported.

Optional missing slots work without treating SQL NULL as omission. For example,
`MonthName(3,)` returns the full name, while `WeekdayName(1,,2)` requests the full
name of Monday. Missing required slots, qualifiers, wildcards and wrong arities
fail before operand evaluation. Written expressions remain eager; errors inside
arguments or unselected IIf branches are not hidden.

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

This first-day host setting currently serves WeekdayName. DatePart/DateDiff's
existing system-option behavior remains unchanged; sharing host calendar settings
with those calls is further work, not implied support from this flag.

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
The [library policy](LIBRARY_POLICY.md#host-locale-discovery) keeps discovery outside
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
comparison evidence. Broader locales/calendars, full Variant/Text coercion, native
NLS spelling, default behavior and error precedence remain unqualified. Both
functions are **Partial** in the [built-in tracker](ACCESS_BUILTINS.md).

Kernel tests exhaust full/short month names and all seven first-day/weekday
combinations, overrides, missing policy, coercion, domain/NULL errors and host
failures. Separate tests cover first-day validation and evaluator resource limits.
Query, CLI and worker regressions cover fields, grouping identity, correlations,
optional slots, flag validation, settings isolation and recovery after missing
context. The [independent recipe](testing/compare_calendar_names.py) uses Python
Gregorian weekday mapping and optional sequential/indexed projections over
Jackcess-decoded row inputs, verifying unchanged source bytes. Native Windows
Access remains unrun; these checks do not establish native compatibility.
