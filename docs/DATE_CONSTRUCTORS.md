# Gregorian date/time constructors

`DateSerial(year,month,day)` and `TimeSerial(hour,minute,second)` now execute through
`tablecraft-expressions::functions::dates::DateConstructor`. Scalar evaluation and
retrieval share the closed three-argument API, including grouping, nested calls,
parameters, source pruning and correlated inputs. Qualified VBA/UDF calls, missing
slots and other arities remain unsupported. UPDATE keeps its existing call gates.

All supplied expressions evaluate eagerly in written order before construction.
Each materialized argument then undergoes the existing half-even checked Int16
conversion; numeric/Boolean carriers are accepted without text parsing. Values
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
The query/CLI currently uses the named default; SQL host-policy overrides and
Hijri calendar semantics remain future work. Four-digit years avoid that ambiguity.

Normalization computes a constant-time Gregorian year offset and uses a
cycle-equivalent Jiff date for the month ordinal. Jiff performs checked day
addition and supplies the final civil date, which passes through the core's
validated legacy DATE constructor. Relative inputs beyond Jiff's span/calendar
bounds return typed errors through fallible APIs; no per-month/day loop or
infallible untrusted span constructor is used.

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
