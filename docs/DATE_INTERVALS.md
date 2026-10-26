# Date interval source conversion

DateAdd, DatePart and DateDiff now share the evaluator's bounded CDate source
conversion across scalar SELECTs, retrieval, nested calls, closed Eval and qualified
saved SELECT execution. Existing Gregorian calendar kernels remain authoritative;
no second date parser, arithmetic implementation or dependency is added.

| Function | Controls validated before source conversion | Result |
| --- | --- | --- |
| DateAdd | Strict interval spelling, then checked Long count | Legacy DATE for converted/legacy sources; Extended for an existing Extended source; NULL source propagates after controls |
| DatePart | Strict interval, then first-day/first-week options progressively | Int16 component/week label; NULL source propagates after controls |
| DateDiff | Strict interval, progressive week options and supported weekday anchor | Int32 boundary count; both sources validate before either NULL propagates |

All written operand expressions still evaluate eagerly before these materialized
checks. A division error in a later argument therefore is not hidden by an invalid
interval or NULL source. Validation order is an explicit current policy; native
Access error precedence remains unqualified.

## DateAdd legacy lower-bound errors

For legacy DATE sources, a calculated DateAdd result before year 100 fails with
`Domain` (error 5), including negative admitted Long counts whose calendar spans
exceed the arithmetic library's range. This follows the explicit lower-bound rule
in [MS-VBAL DateAdd, section 6.1.2.4.1.1](https://learn.microsoft.com/en-us/openspecs/microsoft_general_purpose_programming_languages/ms-vbal/4fd0361f-289b-4395-9623-beae29e39ffc).
It corrects the previous `Overflow` classification for that result boundary.

Count admission remains checked Long conversion. Count conversion failures,
nonfinite values, invalid intervals, source conversion and eager argument errors
retain their existing order. Upper-bound legacy failures still return `Overflow`.
Extended sources retain their separate years 1–9999 envelope and existing error
classification. This rule does not establish native Access SQL error precedence
or expand the supported count width.

## Source and control policies

Metadata-only binding now reports DateAdd's successful carrier for each supported
source family: converted numeric/Boolean/Text and legacy sources produce legacy
DateTime, while an existing Extended source retains Extended. Source nullability
survives; a NULL-only source stays NULL-only after successful control validation.
Unsupported or dynamically typed sources stay Dynamic. No interval/count
validation, Text parsing, provider lookup or row scan runs to establish metadata.
Execution retains every existing conversion/policy/range failure. These are
current evaluator guarantees, separate from native SQL typing; see
[result metadata](QUERY_RESULT_TYPES.md).

Existing DateTime, DateTimeExtended and NULL sources remain borrowed and bypass
CDate. Legacy dates retain their existing whole-second calendar profile; Extended
dates retain exact components and 100 ns fractions, including years before 100.
The operation does not narrow an existing Extended source into legacy DATE.
DateAdd may canonicalize/round a legacy result through its existing civil arithmetic;
source bits are never mutated.

Other supported numeric/Boolean sources use CDate's checked legacy serial
conversion. Negative fractions follow DATE's calendar interpretation, unlike the
raw numeric interpretation used by formatting and numeric conversions. Numeric
sources require no date text policy. Text requires an explicit DateTextFormat;
whole-string numeric Text fallback additionally uses the supplied NumberFormat.
Existing year-window/reference-year rules, Gregorian range checks and malformed
input refusals remain in force. No clock, timezone, OS locale or current year is
consulted implicitly. [Date conversion scope](DATE_CONVERSIONS.md) describes those
accepted inputs and policies.

DateAdd counts now reuse checked Long conversion for raw DATE as well as existing
numeric/Boolean inputs; SQL numeric Text counts use the selected number syntax.
Half-even rounding and range/nonfinite checks remain distinct from date calendar
conversion. Materialized NULL counts retain TypeMismatch. Optional week slots
retain [their existing defaults, system sentinels and NULL policy](WEEK_SETTINGS.md).

The evaluator converts DateDiff's first source and then its second. A malformed
or out-of-range source fails even when the other is NULL. A valid converted source
and NULL return NULL. Existing date carriers are not calendar-decoded merely to
validate a NULL-producing pair, preserving their prior behavior. These rules do
not establish complete native Variant dispatch or error precedence.

```sh
tablecraft query eval "SELECT DateAdd('m','1','2024-01-31'),DatePart('h','-1.25'),DateDiff('h','-1.25',0)" --date-format iso --number-format en-us
```

Results represent February 29 at midnight, 6 and 18. Number and date syntax policies
are independent; a missing policy is an explicit error, not an implicit fallback.

## Ownership and limits

Each date kernel owns a private prepared operation containing only validated
interval/count/week controls. The evaluator prepares it, converts non-date sources
under shared budgets, then invokes that same kernel. Public parser-independent
entry points preserve their explicit date-source profile and policy-free Text
refusal. Prepared query plans retain neither execution-local policies nor converted
source values. Backend-neutral query execution uses public evaluator methods;
it imports no native engine or concrete database adapter.

Original date/Text payloads are not cloned to normalize controls. Count/week/source
normalization shares cumulative byte/work limits; Text input ceilings and every
observed conversion checkpoint obey cancellation. Source-free and table-query
results remain buffered before CLI stdout. UPDATE calls, native pushdown and
storage/write profiles are unchanged.

Jiff still performs checked civil additions and month-end clamping. DateDiff still
counts the existing signed Gregorian boundaries; valid first-week labels do not
change ww boundary counts. DatePart retains the documented final-Monday anomaly.
Non-Sunday DateDiff w anchors, alternate calendars, full Variant/enum/subtype
inference and native precision/locale/error behavior remain open. These functions
remain Partial in the built-in tracker.

## References and evidence

Microsoft's [DateAdd declaration and month-end/count rules](https://learn.microsoft.com/en-us/office/vba/language/reference/user-interface-help/dateadd-function),
[DatePart declaration](https://learn.microsoft.com/en-us/office/vba/language/reference/user-interface-help/datepart-function)
and [DateDiff declaration and boundary rules](https://learn.microsoft.com/en-us/office/vba/language/reference/user-interface-help/datediff-function)
define the interval operations. This implementation composes the existing bounded
CDate/Long policies with those kernels; the declarations do not qualify complete
native coercion/optimizer behavior.

Kernel/evaluator/query/CLI tests cover raw DATE counts, count rounding, month ends,
Extended preservation, progressive controls, source validation before NULL,
prepared rebinding, filters/grouping/correlations and eager errors. Exact cumulative
normalization ceilings and transient cancellation at every observed checkpoint
are exercised. The original [independent recipe](testing/compare_date_interval_sources.py)
checks all ten intervals against Python Gregorian additions/components/enumerated
week boundaries and Decimal count rounding. Its 13,662 controls include both CLI
query modes and numeric/Text projections over unchanged Jet 4, ACE 2007 and
ACE 16-family date fields. Hash-checked Jackcess getters only supply field values;
these independent results are separate from native Access SQL qualification,
which remains pending.


## Ordered expression-state operands (23 October 2026)

DateAdd/DateDiff/DatePart now retain explicit expression values through eager
written-order evaluation. Error data fails TypeMismatch only when its control,
option or source is admitted; it never becomes a numeric code or stored NULL.
DateAdd(CVErr(1),1/0,0) and DatePart('yyyy',CVErr(1),1/0) therefore propagate
DivisionByZero. Later volatile operands execute once even when earlier Error
data eventually fails; failed calls do not roll back earlier draws.

Borrowed accessors preserve the existing validation sequence. Interval parsing
comes first; DateAdd then converts count before date. DateDiff/DatePart validate
first-day and first-week options progressively before source conversion. An invalid
first-day setting therefore wins over a later Error first-week value. DateDiff
converts its first source before admitting/converting its second, and validates
both before NULL propagation. Existing Value-only APIs share these preparations.

Host Empty uses numeric zero for count/date positions. Zero dates pass through the
existing legacy CDate service, not a current-date lookup. Empty week options are
explicit system-zero requests and require the supplied WeekContext; they do not
become omitted Sunday/Jan-1 defaults. Empty interval values remain refused under
the strict string-interval profile, and Missing remains non-scalar. Query Nz(NULL)
retains Text and introduces no SQL Empty constructor.

The earlier DateAdd/DateDiff/DatePart references define interval/count/week rules;
the reviewed MS-VBAL Error/Empty coercion rules guide state admission under the
current bounded profile. Native parameter/evaluation precedence, complete interval
string coercion and Variant behavior remain unqualified. Gregorian/Jiff arithmetic,
month-end clamping, source precision, week numbering/final-Monday behavior,
non-Sunday DateDiff w refusal and formal return categories remain unchanged.
Entry/conversion work, Text limits and cancellation stay shared. Accessors borrow
already evaluated payloads rather than adding normalization copies or caches.
