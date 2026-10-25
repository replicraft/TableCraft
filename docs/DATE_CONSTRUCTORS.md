# Gregorian date/time constructors

`DateSerial(year,month,day)` and `TimeSerial(hour,minute,second)` now execute through
`tablecraft-expressions::functions::dates::DateConstructor`. Scalar evaluation and
retrieval share the closed three-argument API, including grouping, nested calls,
parameters, source pruning and correlated inputs. Qualified VBA/UDF calls, missing
slots and other arities remain unsupported. UPDATE keeps its existing call gates.

All supplied expressions evaluate eagerly in written order before construction.
Each materialized argument then undergoes the existing half-even checked Int16
conversion; numeric/Boolean and raw legacy DATE carriers are accepted. SQL numeric
Text uses an explicitly selected number format. Values
outside -32768..32767 fail with Overflow. NULL is InvalidNullConversion, rather
than an implicit zero or silently propagated Date. For example,
`DateSerial(NULL,1,1/0)` reports the eagerly evaluated division error. Host Variant,
locale and native NULL/error-order fidelity still require Access qualification.

## DateSerial

The constructor normalizes months across year boundaries and days across months,
using the Gregorian calendar. `DateSerial(2024,3,0)` is 29 February 2024;
`DateSerial(2023,3,0)` is 28 February. Negative/zero month/day values are relative
components, not immediate invalid-field defaults. Arguments may temporarily place
the normalized year outside the final Date envelope when day adjustment brings it
back. Final dates must fit years 100–9999; they never clamp or promote to Double.

SQL uses `TwoDigitYearWindow::ACCESS_DEFAULT`, the explicit 1930–2029 window:
00–29 map to 2000–2029 and 30–99 map to 1930–1999. This deterministic profile does
not read machine settings or infer a database's user preferences. Library callers
can construct another validated 100-year window and call `apply_with_window`.
The query/CLI uses the named default when no execution override is supplied;
Hijri calendar semantics remain future work. Four-digit years avoid that ambiguity.

Normalization computes a constant-time Gregorian year offset and uses a
cycle-equivalent Jiff date for the month ordinal. Jiff performs checked day
addition and supplies the final civil date, which passes through the core's
validated legacy DATE constructor. Relative inputs beyond Jiff's span/calendar
bounds return typed errors through fallible APIs; no per-month/day loop or
infallible untrusted span constructor is used.

## Explicit execution windows

```sh
tablecraft query eval 'SELECT Year(DateSerial(49,1,1)) AS Y' --two-digit-year-max 2049
tablecraft query run 'SELECT DateSerial(YearPart,MonthPart,DayPart) AS D FROM Parts' --database copy.accdb --two-digit-year-max 2049
```

`--two-digit-year-max YEAR` is available on `query eval` and `query run`, including
inline, native/saved and SQLite retrieval. YEAR is the inclusive last year of a
100-year Gregorian window within the Date envelope; accepted values are 199–9999.
Omission retains 2029 (1930–2029). The same shared constructor validates command
input before database I/O. Invalid flags publish no successful stdout. This does
not affect inert parse/inspection commands or infer preferences from a file.

Library hosts use `Evaluator::with_year_window` or
`ExecutionOptions::year_window` with a validated `TwoDigitYearWindow`. Optional
SQLite worker hosts forward `AsyncOptions::year_window` through the same contract.
The option
belongs to one execution and applies to nested/correlated and named queries.
Preparation does not capture it, and reused plans can execute under different
windows without cache or parameter leakage. Four-digit years and TimeSerial are
unaffected; omitted execution options remain backward compatible. Policies do
not change source metadata, read/index caches, budgets or validation boundaries.

## TimeSerial

Signed hour/minute/second components combine as a bounded integer second count,
then divide by seconds per day into an Automation DATE serial. Carries are
retained: 75 minutes becomes 1 hour 15 minutes and 25 hours includes one day.
The current signed-serial policy retains a negative result rather than wrapping
it to a clock-only time. Negative fractions therefore follow Automation DATE's
stored representation, not an elapsed civil-time projection. Native negative
carry and rounding behavior remain unqualified. No current date, clock, timezone
or ambient execution context participates.

## References and evidence

Microsoft's [DateSerial reference](https://learn.microsoft.com/en-us/office/vba/language/reference/user-interface-help/dateserial-function),
[MS-VBAL DateSerial](https://learn.microsoft.com/en-us/openspecs/microsoft_general_purpose_programming_languages/ms-vbal/051aa28f-0ea7-40a5-95ab-8428ac682826)
and [TimeSerial reference](https://learn.microsoft.com/en-us/office/vba/language/reference/user-interface-help/timeserial-function)
supply argument widths, relative-unit behavior, return type and documented default
year-window guidance. These guide implementation and do not establish Access SQL
or Windows comparison evidence.

Five value tests cover rollover, leap years, year-window boundaries, half-even
conversion, errors, signed times and generated independent date/second models.
A scalar dispatch test and two retrieval tests exercise grouping, pruning,
correlations, constructor identity, reusable parameters, nesting and eager errors.
The [CLI comparison](testing/compare_date_constructors.py) uses Python calendar
models and exact rational seconds, plus inert native Int32 input in sequential
and indexed reads. It verifies original source hashes and adds no database
fixtures, dependencies or runtime host. Native Access comparison remains pending.

## Argument coercion

DateSerial/TimeSerial now use the same checked Integer conversion as CInt for each
component. Raw legacy DATE arguments contribute their stored numeric serials,
including fractional/negative aliases; calendar decoding would give the wrong
component. SQL numeric Text components use the evaluator's NumberFormat and share
its metered normalization buffers. A date syntax profile is unnecessary because
these are numeric fields, not date strings. Extended dates and binary/GUID
remain unsupported. Expression-state admission uses the explicit path below;
Error codes are never interpreted as numeric components implicitly.

The owning constructor validates arity, then each component from left to right,
before relative Gregorian normalization. It retains InvalidNullConversion for a
materialized NULL. Out-of-range first components fail before a later malformed
Text component is converted; already written SQL operand expressions remain eager.
The Int16 range and ties-to-even policy remain unchanged. No complete native
Variant/enum/subtype or error precedence claim follows from this composition.

A crate-private converter hook keeps the existing parser-independent constructor
and SQL evaluator on one calendar implementation. The public apply/apply_with_window
entry points retain Text refusal because they have no normalization budget or
number-format provider. Public Evaluator::apply_date_constructor supplies that
context for Value-only hosts; scalar/query dispatch now uses the explicit
expression-state entry point below. Only three validated i16
components are retained on the stack; source values are not cloned to normalize
arguments. Normalization shares cumulative byte/work budgets and cancellation.
Prepared plans capture neither number policies nor year windows.

Kernel/evaluator/query/CLI controls cover raw DATE components, numeric Text,
Int16 ties/ranges, progressive error order, exact cumulative allocation ceilings,
input limits, every observed cancellation checkpoint, groups/filters/correlations,
closed Eval, prepared rebinding and execution-local year windows. The original
[independent coercion recipe](testing/compare_constructor_coercion.py) passes 388
Decimal/Gregorian/Fraction/native-field controls. It includes fractional Text
components projected over unchanged priority-family native IDs under selected
number profiles and distinct year windows. Hash-checked Jackcess getters supply
IDs, not Access SQL execution. No fixture, dependency or runtime is added; both
functions remain Partial pending native coercion/calendar qualification.


## Ordered expression-state components (23 October 2026)

DateSerial/TimeSerial now evaluate all three supplied SQL expressions as explicit
expression values before Integer conversion. Error data fails TypeMismatch at
its component position without suppressing later operational failures. For
example, DateSerial(CVErr(1),1/0,1) reports DivisionByZero. Later Rnd components
execute once before an earlier Error component is rejected; failed calls do not
roll back earlier draws.

One borrowed argument accessor retains written conversion order before calendar
normalization. DateSerial(NULL,CVErr(1),1) retains InvalidNullConversion, while
DateSerial(9999,13,CVErr(1)) reports TypeMismatch before attempting an out-of-range
calendar result. Explicit CLng(CVErr(...)) remains a separate numeric-code
conversion. Missing remains non-scalar; no stored value carrier changes.

Host Empty becomes Integer zero, then follows the existing relative-component and
year-window rules. Empty year with month/day one yields 2000-01-01 under the
default window and 1900-01-01 under a window ending in 1999. Empty month/day are
relative zero components, not a blanket epoch substitute. TimeSerial with three
Empty components yields the existing zero serial. Query Nz(NULL) retains Text
and introduces no SQL Empty constructor.

The existing DateSerial/TimeSerial references define Integer components and
relative normalization; reviewed [MS-VBAL](https://learn.microsoft.com/en-us/openspecs/microsoft_general_purpose_programming_languages/ms-vbal/d5418146-0bd2-45eb-9c7a-fd9502722c74)
Error/Empty coercion rules guide the
explicit state path. Native parameter/evaluation precedence and Hijri behavior
remain unqualified. Jiff calendar arithmetic, legacy range checks, formal Variant
Date returns, numeric Text policies and execution-local windows are unchanged.
Value-only public APIs share the same kernel and retain their host-free Text
boundary. Entry/conversion checks share cumulative work and cancellation; no
payload copies beyond ordinary expression materialization are added.


## Retained String constructor components

The public DateSerial and TimeSerial expression APIs now accept valid retained
numeric Strings wherever materialized numeric Text already worked. Only the
selected active range is strictly published, then converted through the existing
metered Integer conversion. A private component callback combines admission and
conversion at each left-to-right position, before the unchanged Gregorian/time
kernel. It borrows scalar carriers and does not build an eager input vector.

An earlier component conversion failure still precedes a later malformed String;
NULL, Error, Empty and Missing retain their previous behavior. Arity precedes
component admission. Malformed reached ranges fail before missing numeric-format
policy; valid ranges require the explicit NumberFormat. Two-digit-year windows,
half-even component rounding and relative normalization are unchanged. Ordinary
supported bound queries already admitted these valid Strings; this closes a
public API consistency gap without introducing new native semantics.

Three focused retained tests check independent date/time expectations, both
number formats, an explicit nondefault year window, active-range pruning,
progressive error precedence, every observed cancellation point and byte/work/
per-Text limits. Query controls cover retained compositions, malformed data and
eager unselected-branch errors. Native failure order and coercion remain
unqualified.

The constructor batch passes 18 focused evaluator/kernel tests and two query
tests. Strict expression-library and query-target Clippy, minimal-feature
compilation and scoped formatting pass. These checks preserve the existing
calendar model; no Windows comparison was run.
