# SQL date and time literals

Queries now evaluate complete-year U.S. and English date literals, date/time
combinations and time-only literals through the shared Jiff-backed parser.
For example, #1/2/2026# means January 2 regardless of --date-format. The SQL parser
still retains trimmed literal contents without evaluating them. Scalar evaluation
and query binding/execution use the new parse_query_literal service.

```sh
tablecraft query eval 'SELECT Month(#1/2/2026#),Day(#1/2/2026#),Hour(#2:24PM#)'
tablecraft query run 'SELECT * FROM Orders WHERE OrderedAt BETWEEN #1/1/2026# AND #1/31/2026 11:59:59PM#' --database example.accdb
```

Microsoft's [SQL date-order contract](https://learn.microsoft.com/en-us/office/vba/access/concepts/date-time/format-dates-as-u-s-dates-regardless-of-regional-settings)
requires U.S. dates in SQL strings regardless of regional settings. Its
[expression guidance](https://support.microsoft.com/en-us/access/introduction-to-expressions)
establishes hash-delimited date/time values, named-month spellings and query
criteria. These establish the literal family and U.S. ordering. They do not
qualify every accepted spelling, conversion boundary or native error below.

## Implemented profile

- Existing strict YYYY-MM-DD and YYYY-MM-DD HH:MM:SS[.fraction] literals remain.
- Slash dates use month/day/complete-year, with one or two month/day digits.
  Complete years have three/four digits and validate within the legacy DATE
  envelope, 0100 through 9999. Two-digit years do not enter this service.
- English full/abbreviated month dates reuse the existing date recognizer, including
  Mar-7-2017, 7-Mar-2017 and September 30, 2024.
- Dates may have a space-separated 12/24-hour time. Time-only literals use the
  Automation epoch date, 1899-12-30. Missing time means midnight.
- Colon times accept hours/minutes, optional seconds and up to nine fractional
  second digits, or an hour with AM/PM. Meridiem spelling is ASCII-insensitive;
  12 AM/PM resolves midnight/noon. Final values retain legacy floating DATE
  precision rather than a new lossless fractional carrier.

The additional profile has a 64-byte ASCII input ceiling. Borrowed parsing and
Jiff calendar validation retain invalid-date/time rejection, including leap seconds,
24:00, offsets and timezone spellings. Dotted times, other languages/regional date
orders, two-digit/omitted years and Access's possible invalid-month reinterpretation
remain unsupported or unqualified. No clock/current-year lookup or regional
fallback runs during metadata binding. Existing CDate/DateValue/TimeValue profiles
and their execution-local year controls retain their separate behavior.

## Boundaries and verification

The existing parse_literal API remains strict ISO and continues to serve CLI
TYPE=date parameters. Extending SQL hash literals therefore does not silently
change typed parameter syntax. Reusable plans do not capture a locale or year
window. Closed invalid literals still fail preparation on empty/filtered results;
new date spellings do not defer malformed constants until a row happens to use
one. Binding reads no records or formatting providers.

The parser shares the existing date/time split and component recognizers with
text-date functions. Their dotted-time option remains enabled only at the original
text boundary. There is no new dependency, AST carrier, stored type or timezone
conversion. Input size is bounded before extended recognition; normal evaluator,
query/output budgets, cancellation and buffered failure behavior remain.

Three new expression tests include independent civil goldens and a fixed-seed
calendar property. Query tests cover policy-independent plan reuse, empty-result
binding, derived/UNION values, subqueries, membership and closed dynamic evaluation.
CLI tests cover both query commands, source policies, invalid literals and strict
ISO typed parameters. An older refusal golden now uses an invalid month because
its previous U.S. date is supported.

The original [independent runner](testing/compare_sql_date_literals.py) passes
230 controls: 206 scalar and 24 native sequential/indexed cases. Python calendars
supply expected components; reviewed read-only Jackcess getters supply 20,000
native DATE rows per registered Jet 4/ACE 2007/2016/2019 input. Existing original
hashes remain unchanged. Only TableCraft evaluates SQL, and native literal spelling,
regional/precision/error behavior and wider literal forms remain unqualified.
