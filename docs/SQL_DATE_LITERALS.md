# SQL date and time literals

Queries now evaluate U.S./English date tokens, date/time
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
- Numeric date components are interpreted in documented priority: month/day/year,
  then year/month/day, then day/month/year, selecting the first valid date. Slashes,
  hyphens, commas and whitespace separate components; repeated punctuation fails.
- Token years 0..29 mean 2000..2029; 30..99 mean 1930..1999. Larger values are
  complete years within the legacy DATE envelope. The mapping uses numeric values,
  including leading zeros, and is independent of execution-local year windows.
- English full/abbreviated months can occur in any component position. The
  recognizer tries the written numeric day/year order, then reverses those roles
  if necessary. Jiff supplies month-name recognition and calendar validation.
- Known month/year forms such as Feb 2024 and 2024/2 select day one. Forms whose
  preferred interpretation needs a default year, such as Jan 2, remain refused.
- Dates may have a space-separated 12/24-hour time. Time-only literals use the
  Automation epoch date, 1899-12-30. Missing time means midnight.
- Colon times accept hours/minutes, optional seconds and up to nine fractional
  second digits, or an hour with AM/PM. Meridiem spelling is ASCII-insensitive;
  12 AM/PM resolves midnight/noon. Final values retain legacy floating DATE
  precision rather than a new lossless fractional carrier.

The additional profile has a 64-byte ASCII input ceiling. Borrowed parsing and
Jiff calendar validation retain invalid-date/time rejection, including leap seconds,
24:00, offsets and timezone spellings. Dotted times, other languages/regional date
orders beyond these token rules and dates needing a default year remain
unsupported or unqualified. The time grammar retains its existing bounded profile;
additional MS-VBAL dotted/short-meridiem forms are not supplied by this date change. No clock/current-year or host regional-setting lookup occurs during metadata
binding. Existing CDate/DateValue/TimeValue profiles
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

Expression tests include independent civil goldens and a fixed-seed
calendar property. Query tests cover policy-independent plan reuse, empty-result
binding, derived/UNION values, subqueries, membership and closed dynamic evaluation.
CLI tests cover both query commands, source policies, invalid literals and strict
ISO typed parameters. An older refusal golden now uses an invalid month because
its previous U.S. date is supported.

The original [independent runner](testing/compare_sql_date_literals.py) passes
442 controls: 418 scalar and 24 native sequential/indexed cases. Python calendars
supply expected components; reviewed read-only Jackcess getters supply 20,000
native DATE rows per registered Jet 4/ACE 2007/2016/2019 input. Existing original
hashes remain unchanged. Only TableCraft evaluates SQL, and native literal spelling,
regional/precision/error behavior and wider literal forms remain unqualified.


## Token year and interpretation rules

The [MS-VBAL date-token contract](https://learn.microsoft.com/en-us/openspecs/microsoft_general_purpose_programming_languages/ms-vbal/65097495-6498-414f-8af8-8e6f481f831f)
and its [year formula](https://learn.microsoft.com/en-us/openspecs/microsoft_general_purpose_programming_languages/ms-vbal/ms-vbal_files/image004.png)
establish the fixed token mapping and component priorities. TableCraft uses these
as the explicit date-component implementation profile; native Access SQL
qualification remains unrun. The configurable DateSerial/Text year window is a
separate runtime policy. Thus Year(#1/2/49#) remains 1949 while DateSerial(49,1,2)
can use 2049 under an explicit window. Preparation can validate these constants
without capturing execution settings or requiring scans.

Priority matters for short-year alternatives: 31/12/26 resolves as 1931-12-26
under the token model because year/month/day succeeds before day/month/year.
31/12/2026 resolves as 2026-12-31 because the earlier alternatives are invalid.
These examples are specification-model results, not recorded native Access output.
An implementation-defined default year is still needed for genuine month/day
forms; this service never substitutes the machine's current year.
