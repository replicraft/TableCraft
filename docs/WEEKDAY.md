# Standalone Weekday

Unqualified `Weekday(date, [firstdayofweek])` now returns Int16 1–7, or NULL for
a NULL date, in scalar SELECTs, bound/nested retrieval and qualified saved SELECTs.
The first optional argument is independent of date language and calendar-week
labels. It supports dynamic expressions and an explicitly missing final slot.

| First-day argument | Current behavior |
| --- | --- |
| Omitted or missing | Sunday, regardless of host context. |
| 0 | Resolve caller-issued FirstDayOfWeek; absent context raises CalendarPolicyRequired. |
| 1–7 | Concrete Sunday-through-Saturday setting, overriding host context. |
| NULL, unsupported type or out-of-range | Existing checked option type/domain error; NULL is not omission. |

The result is relative to the chosen first day: Wednesday is 4 with Sunday first
and 3 with Monday first. Optional first-day numbers reuse checked Long half-even
coercion. The [host week setting](WEEK_SETTINGS.md) reaches the CLI through
`--first-day-of-week DAY`. First-week-of-year rules do not affect Weekday.
WeekdayName retains its distinct omitted system default, so callers composing the
two functions should select consistent numbering explicitly.

## Date carriers and conversion

Legacy and extended dates use the shared calendar kernel. Legacy values retain the
existing nanosecond-to-whole-second half-expand profile, including date carry.
Extended dates retain exact stored date components, including supported year-one
values; their fractional time is not rounded into another date. Numeric/Boolean
and Text inputs use the [checked CDate profile](DATE_CONVERSIONS.md). Text requires
an explicit date format, and omitted years require a caller-issued reference year.
Numeric Text fallback additionally requires NumberFormat. Binary/GUID inputs fail.

These are explicit current rules, not full native Variant or locale coercion.
Invalid Text, nonfinite values, range and missing-policy errors propagate. Optional
settings are validated before NULL date propagation, as in the current DatePart
profile; native error precedence remains unqualified. Both written arguments are
eager, so NULL or a failing IIf branch cannot hide a later expression error.

```sh
tablecraft query eval "SELECT Weekday(#1969-02-12#),Weekday(#1969-02-12#,2)"
tablecraft query eval "SELECT Weekday('Feb 29',0)" --date-format en-us --reference-year 2024 --first-day-of-week 2
tablecraft query run "SELECT Weekday(-1.25),Weekday(NULL)"
```

The results are 4/3, 4, and 6/NULL, respectively. Weekday uses no locale or clock
lookup and does not execute VBA. Inert database inspection never evaluates it.

## Boundaries and evidence

The parser-independent date slice owns weekday conversion and numbering; the
evaluator supplies input limits, work/cancellation and private charged numeric
Text scratch. Scratch never enters the result. The owned Weekday plan node keeps
both date and optional first-day expressions through pruning/remapping, grouping
identity, aggregate detection, correlations and qualified multivalue parent
criteria. It remains distinct from Nz and DatePart during structural comparisons.
Plans capture no date/year/host-week policy, and worker settings are request-local.
No predicate pushdown, UPDATE support, dependency or fixture is added.

Microsoft's [Weekday reference](https://learn.microsoft.com/en-us/office/vba/language/reference/user-interface-help/weekday-function)
defines the Integer result, Sunday omitted default, system sentinel, numeric/Text
date input and NULL propagation. The kernel test includes its Wednesday example.
Full NLS/calendar/Variant coercion, native precision/error ordering and actual
extended-date behavior remain unqualified; the [built-in tracker](ACCESS_BUILTINS.md)
therefore marks Weekday **Partial**.

Four kernel tests cover defaults/overrides/system context, NULL and invalid options,
shared numeric/Text/extended input, date-boundary precision and host failures,
including generated whole-serial-day controls with an independent seven-day model.
An evaluator test checks input/work/cancellation limits. Query tests cover dynamic
first-day dependencies, groups, aggregate arguments, correlations and policy reuse;
a multivalue regression covers hidden member dependencies. CLI and worker tests
cover optional slots, failure buffering, request isolation and recovery after
missing context. The [independent recipe](testing/compare_weekday.py) uses Python
Gregorian/Automation-day models and optional sequential/indexed projections over
Jackcess-decoded row inputs, checking unchanged source bytes. Native Windows Access
remains unrun; these model/reader checks do not establish native compatibility.
