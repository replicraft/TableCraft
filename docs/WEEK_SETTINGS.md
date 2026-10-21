# Explicit host week settings

DatePart and DateDiff now accept caller-issued calendar settings for explicit
system (zero) options. They share the first-day setting already used by
[WeekdayName](CALENDAR_NAMES.md) and add a separate first-week rule. The kernel
never queries a clock, OS preference, language identifier or database provider.

| Setting | Valid concrete values | CLI flag |
| --- | --- | --- |
| FirstDayOfWeek | 1 Sunday through 7 Saturday | `--first-day-of-week DAY` |
| FirstWeekOfYear | 1 week containing January 1; 2 first week with at least four days; 3 first complete week | `--first-week-of-year RULE` |

Both constructors exclude zero. Zero is the SQL request to consult host context,
not a concrete host setting. Invalid CLI values fail during argument parsing.
Execution flags apply to query eval/run, including saved SELECT execution;
parse/inspect remain inert. Preferred language discovery via sys-locale cannot
supply these calendar settings.

| Call option | DatePart/DateDiff behavior |
| --- | --- |
| Omitted or explicit missing slot | Sunday and Jan-1-week defaults, regardless of supplied host settings. |
| Explicit zero | Resolve the corresponding host setting; absent context raises CalendarPolicyRequired. |
| Explicit concrete SQL value | Use that value, overriding host context. |
| Explicit NULL | Retain the existing option type error; NULL is not omission. |

WeekdayName retains its separate documented default: its omitted first-day option
requests the system setting. DatePart/DateDiff default to Sunday instead. The same
host first-day value can serve both without collapsing their distinct defaults.
The first-day and first-week controls are independent. Supplying only one cannot
satisfy a zero request for the other. Existing checked Long half-even coercion and
option validation remain unchanged, including evaluation before NULL date
propagation. All written argument expressions remain eager.

```sh
tablecraft query eval "SELECT DatePart('w',#2026-10-20#,0),DatePart('w',#2026-10-20#)" --first-day-of-week 2
tablecraft query run "SELECT DatePart('ww',#2021-01-01#,0,0)" --first-day-of-week 2 --first-week-of-year 2
tablecraft query eval "SELECT DateDiff('ww',#2026-10-18#,#2026-10-19#,0,0)" --first-day-of-week 2 --first-week-of-year 2
```

The results are 2/3, 53, and 1, respectively. Omitting DateDiff's SQL week settings
in the final example returns 0 because its default first day remains Sunday.

## Library and execution boundaries

The date slice owns validated FirstDayOfWeek/FirstWeekOfYear and the copyable
WeekContext. `date_part_with_context` and `date_diff_with_context` extend the existing
parser-independent entry points. Original date_part/date_diff remain available
with an empty context, preserving prior behavior. Hosts can supply policies through
Evaluator builders, ExecutionOptions and optional worker AsyncOptions. Query plans
capture no policy; nested/correlated/saved reads share the active execution controls.
Worker requests cannot retain a prior request's calendar settings.

The existing week algorithm and documented final-Monday DatePart anomaly remain
unchanged. DateDiff ww counts selected first-day boundaries; valid first-week
labels do not change that sequence. DateDiff w with a non-Sunday first-day setting
remains explicitly unsupported pending native anchor qualification, including when
that setting arrives through host context. This does not introduce implicit date
coercion, alternate calendars, UPDATE calls or native predicate pushdown.

CLI eval now groups execution controls in a typed struct rather than extending a
positional argument list. Native engines and backend-neutral contracts gain no
concrete adapter, runtime, locale or application dependency. No dependency or
fixture is added. Failed query/CLI execution still publishes no partial stdout.

## References and evidence

Microsoft's [DatePart guide](https://learn.microsoft.com/en-us/office/vba/language/reference/user-interface-help/datepart-function)
and [DateDiff guide](https://learn.microsoft.com/en-us/office/vba/language/reference/user-interface-help/datediff-function)
define Sunday/Jan-1 omitted defaults, system sentinels and concrete rule numbers.
These differ from [WeekdayName's system default](https://learn.microsoft.com/en-us/office/vba/language/reference/user-interface-help/weekdayname-function).
The published DatePart final-Monday warning remains part of the current profile.
Full native NLS, error precedence, broader coercion, alternate calendars and
DateDiff w anchors still require native Access evidence; the built-in tracker
retains Partial status.

Kernel tests cover context resolution, overrides, omitted defaults, incomplete
context, NULL option/date behavior, signed boundaries and all first-day/first-week
combinations around year boundaries. Query and CLI tests cover pruning, grouping,
correlations, execution-local settings, optional slots, flag validation and eager
failures. A worker regression checks request isolation and recovery after failure.
The [independent recipe](testing/compare_week_context.py) enumerates qualifying
Gregorian weeks/boundaries in Python and compares optional sequential/indexed
projections over Jackcess-decoded row inputs with unchanged source hashes.
Native Windows Access remains unrun; independent model/reader evidence is separate.
