# Host-issued Date, Time and Now

Parenthesized, unqualified `Date()`, `Time()` and `Now()` now run in scalar SELECTs,
bound/nested retrieval and qualified saved SELECT execution. Each call takes zero
arguments and returns legacy DateTime. Wrong arities, qualifiers, wildcards and
missing slots fail before operand/provider execution. Bare names remain field
references; `$` string aliases and Date/Time statements are not implemented.

| Function | Current result |
| --- | --- |
| Date() | The provider's local Gregorian date at midnight. |
| Time() | The provider's local time on the Automation epoch date, 1899-12-30. |
| Now() | The provider's local Gregorian date and time. |

ClockReading validates the legacy envelope (years 100–9999) and truncates fractions
to whole seconds before projection. Date/time components project before legacy
encoding, preserving epoch/sign rules and avoiding a distant date's precision
loss in Time(). Stored values remain timezone-free Access carriers. Native
subsecond precision and full calendar behavior remain unqualified.

## Provider and CLI ownership

The parser-independent expression library owns Clock, ClockReading and ClockFunction.
The host selects what local time means; generic query/native engines perform no
clock, timezone, environment or filesystem discovery. Libraries require an explicit
provider through Evaluator/ExecutionOptions. Missing context is ClockRequired.
Provider errors propagate without retry or fallback; cancellation and work limits
are checked before callbacks. Providers must bound their own I/O/work.

The CLI supplies a provider during query eval/run. It reads SystemTime for each
executed clock call and uses Jiff's fallible local timezone discovery/conversion.
The resolved timezone is cached for that command, while readings remain fresh.
Jiff honors the host's TZ setting and normal platform discovery. A missing/invalid
timezone produces ClockUnavailable instead of silently choosing UTC. Normal
queries without clock calls do not initialize the timezone or read system time.
Parsing, inspection and saved-query reconstruction never evaluate these calls.

`--clock LOCAL-DATETIME` supplies a fixed reading for deterministic execution:

```sh
tablecraft query eval "SELECT Year(Now()),Hour(Time()),Second(Time())" --clock 2024-02-29T23:59:59.999999999
tablecraft query run "SELECT DateDiff('s',Date(),Now())" --clock 2024-02-29T23:59:59
```

Results are 2024/23/59 and 86399, respectively. The input is canonical local
`YYYY-MM-DDTHH:MM:SS[.fraction]`, years 0100–9999, with up to nine fractional digits.
Offsets, zone annotations and invalid calendars/times fail during argument parsing;
fractions are truncated. This is a civil reading, so it does not resolve a DST
fold/gap as an instant. Fixed readings bypass system-clock/timezone lookup.
The flag is unavailable on parse/inspect. Date format and omitted-year parsing
policies remain separate: a fixed clock does not implicitly set ReferenceYear.

Jiff's system/zoneinfo/platform-bundle features belong to the CLI consumer only.
Unix builds can use system zoneinfo; the configured Windows dependency closure
includes the platform timezone-data bundle. This is configuration evidence, not a
Windows build or native Access qualification. Standalone expression/engine feature
profiles retain civil-only Jiff usage. [Dependency records](DEPENDENCIES.md) cover
the two newly resolved platform data packages and unchanged existing versions.

## Evaluation and volatility

Every executed clock node reads its provider once. No reading is captured during
binding, cached in a plan, folded into a constant or supplied to native predicate
pushdown. ClockReading itself implements Clock for deterministic fixed providers.
AsyncOptions carries an owned request-local Arc provider; the worker does not
create one or reuse a previous request's setting implicitly.

Normal argument eagerness remains: IIf's unselected clock branch can still fail,
and unused computed projections retain clock errors. Clock-based GROUP BY keys
can execute, but structural expression equivalence never equates separate clock
calls. Projections may read again; no group-key value replaces those calls.
Field-dependent projections cannot use volatile expression equivalence to prove
group membership. Existing pure-only DISTINCTROW, crosstab-key/heading and qualified
multivalue-parent eligibility remain restricted where they require that proof.
Full native volatile/grouping behavior needs further qualification.

Clock results and failures share the query's budgets and buffered output boundary.
There is no UPDATE support, stored-code execution, system-clock mutation or automatic
cross-backend time/transaction guarantee. Normal platform clock access is a host
service, not the deferred Windows API emulation/VBA subsystem.

## References and evidence

Microsoft's [Date](https://learn.microsoft.com/en-us/office/vba/language/reference/user-interface-help/date-function),
[Time](https://learn.microsoft.com/en-us/office/vba/language/reference/user-interface-help/time-function)
and [Now](https://learn.microsoft.com/en-us/office/vba/language/reference/user-interface-help/now-function)
references define current system date/time results. Jiff's [feature documentation](https://docs.rs/jiff/latest/jiff/#crate-features)
and checked upstream implementation guide CLI timezone integration. Full native
NLS/calendar, platform timezone rules, invocation timing/volatility, precision and
error ordering remain unqualified. All three functions are **Partial** in the
[built-in tracker](ACCESS_BUILTINS.md).

Kernel tests cover projections, truncation, epoch signs, calendar endpoints and
provider failure/no-retry behavior. Evaluator tests check limits/cancellation before
callbacks. Query tests cover inert preparation, repeated/correlated reads, missing
context, eager/unused failures, group keys/equivalence and field-name preservation.
CLI tests cover fixed readings, invalid inputs/calls, lazy discovery, UTC system-day
checks and invalid-timezone refusal. A worker test covers request isolation and
recovery after missing context. The [independent recipe](testing/compare_clock.py)
uses Python Gregorian models and optional sequential/indexed projections over
Jackcess-decoded row inputs, checking unchanged source bytes. Native Windows Access
remains unrun; model/reader checks do not establish native compatibility.
