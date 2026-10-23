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

[Standalone Weekday](WEEKDAY.md) shares the first-day policy and omitted Sunday
default, with no first-week argument. WeekdayName retains its separate documented default: its omitted first-day option
requests the system setting. DatePart/DateDiff default to Sunday instead. The same
host first-day value can serve both without collapsing their distinct defaults.
The first-day and first-week controls are independent. Supplying only one cannot
satisfy a zero request for the other. Checked Long half-even conversion now also accepts raw legacy DATE options
and, through the SQL evaluator, explicit-policy numeric Text. Option validation
still precedes NULL date propagation. All written argument expressions remain eager.

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

## Contextual week-option conversion

DatePart, DateDiff, Weekday and general Format now use checked Long conversion
for numeric and legacy DATE week options. DATE contributes its stored serial,
including fractional/negative aliases, rather than a calendar-rounded day. SQL
numeric Text requires an explicit NumberFormat; normalization shares the query's
cumulative byte/work limits and cancellation. Number syntax and the concrete
first-day/first-week settings remain independent policies. Boolean words are not
accepted as numeric option spellings. Extended dates remain unsupported as numeric
options; they retain their existing role as calendar source values.

The shared date parser resolves and validates first-day before asking for
first-week conversion. Thus an out-of-range first-day is not hidden by malformed
Text in a later option. DatePart/DateDiff validate the interval before week
options; Weekday validates its first-day before propagating a NULL source. General
Format retains its different preflight: any supplied NULL week option raises
InvalidNullConversion before other option conversion. The three date functions
retain their existing TypeMismatch for a materialized NULL option. All written
SQL operand expressions remain eager, so a later division error still occurs
before these materialized checks. These are explicit current error policies,
not native Access precedence qualification.

The parser-independent entry points still refuse Text options: they have no
numeric normalization buffer. Public evaluator apply_date_part/apply_date_diff
methods now own the metered Text boundary for both scalar SQL and query execution.
They borrow original interval/date operands and never copy a source to normalize
an option. WeekdayName keeps its separate default and naming parser; this change
does not broaden its Text argument profile. No calendar algorithm, DateDiff w
anchor, native pushdown, write path, runtime or dependency is added.

Microsoft's [Format declaration](https://learn.microsoft.com/en-us/office/vba/language/reference/user-interface-help/format-function-visual-basic-for-applications),
[DatePart declaration](https://learn.microsoft.com/en-us/office/vba/language/reference/user-interface-help/datepart-function),
[DateDiff declaration](https://learn.microsoft.com/en-us/office/vba/language/reference/user-interface-help/datediff-function)
and [Weekday declaration](https://learn.microsoft.com/en-us/office/vba/language/reference/user-interface-help/weekday-function)
define the option identities/defaults. Checked numeric conversion follows the
existing [explicit conversion service](DATE_NUMERIC.md), including ties-to-even.
Full Variant/enum subtype rules, native locale behavior and native Access error
precedence remain open acceptance work.

The original [independent runner](testing/compare_week_options.py) uses Python
Decimal rounding and enumerated Gregorian weeks/boundaries. It checks Text and
DATE options, half-even ties, system sentinels, invalid rounded ranges, both CLI
query modes and native date/ID projections over unchanged Jet 4, ACE 2007 and
ACE 16-family sources. Jackcess only supplies read-only field values, not SQL
execution. Kernel/evaluator/query/CLI controls cover defaults, progressive error
order, prepared rebinding, grouping/correlations, exact cumulative normalization
budgets, input ceilings, eager failures and every observed cancellation checkpoint.
Native Access comparisons remain unrun and Partial status remains unchanged.

DatePart/DateDiff now separately support [bounded numeric/Text date sources](DATE_INTERVALS.md)
after the progressive interval/week parser completes. Week algorithms, NULL option
errors and system/default settings remain unchanged. The private prepared operation
keeps validation order without retaining or cloning source values.


DateDiff/DatePart now request [explicit expression-state options](DATE_INTERVALS.md#ordered-expression-state-operands-23-october-2026)
progressively after interval validation. A failed first-day setting does not
admit/convert the later first-week state. Host Empty means explicit zero/system
under the existing context requirements; omission remains distinct. Other week
consumers retain their own admission profiles. No OS locale lookup is introduced.
