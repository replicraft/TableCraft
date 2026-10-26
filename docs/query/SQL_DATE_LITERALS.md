# SQL date and time literals

Queries now evaluate U.S./English date tokens, date/time
combinations and time-only literals through the shared Jiff-backed parser.
For example, #1/2/2026# means January 2 regardless of --date-format. The SQL parser
still retains trimmed literal contents without evaluating them. Scalar evaluation
and query binding/execution use the query-literal service.

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
- Known month/year forms such as Feb 2024 and 2024/2 select day one. Month/day
  forms such as Jan 2 require an explicit reference year. Numeric two-component
  dates first try month/day, then day/month, using that year's calendar validity;
  named-month forms first try month/day, then month/year with day one.
- Dates may have a space-separated 12/24-hour time. Time-only literals use the
  Automation epoch date, 1899-12-30. Missing time means midnight.
- Colon times accept hours/minutes, optional seconds and up to nine fractional
  second digits, or an hour with AM/PM. Meridiem spelling is ASCII-insensitive;
  12 AM/PM resolves midnight/noon. Final values retain legacy floating DATE
  precision rather than a new lossless fractional carrier.

The additional profile has a 64-byte ASCII input ceiling. Borrowed parsing and
Jiff calendar validation retain invalid-date/time rejection, including leap seconds,
24:00, offsets and timezone spellings. Dotted times, other languages/regional date
orders beyond these token rules remain unsupported or unqualified. Omitted-year
forms require caller-issued context; no ambient year is selected. The time grammar retains its existing bounded profile;
additional MS-VBAL dotted/short-meridiem forms are not supplied by this date change. No clock/current-year or host regional-setting lookup occurs during metadata
binding. Existing CDate/DateValue/TimeValue profiles
and their execution-local year controls retain their separate behavior.

## Boundaries and verification

The existing parse_literal API remains strict ISO and continues to serve CLI
TYPE=date parameters. Extending SQL hash literals therefore does not silently
change typed parameter syntax. Reusable plans do not capture a locale or year
window. Explicit reference years used by SQL tokens are captured at preparation.
Closed invalid literals still fail preparation on empty/filtered results;
new date spellings do not defer malformed constants until a row happens to use
one. Binding reads no records or formatting providers.

The parser shares the existing date/time split and component recognizers with
text-date functions. Their dotted-time option remains enabled only at the original
text boundary. There is no new dependency, public SQL AST carrier, stored type or timezone
conversion. Input size is bounded before extended recognition; normal evaluator,
query/output budgets, cancellation and buffered failure behavior remain.

Expression tests include independent civil goldens and a fixed-seed
calendar property. Query tests cover policy-independent plan reuse, empty-result
binding, derived/UNION values, subqueries, membership and closed dynamic evaluation.
CLI tests cover both query commands, source policies, invalid literals and strict
ISO typed parameters. An older refusal golden now uses an invalid month because
its previous U.S. date is supported.

The original [independent runner](../testing/compare_sql_date_literals.py) passes
534 controls: 502 scalar and 32 native sequential/indexed cases. Python calendars
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
without capturing runtime date-text settings or requiring scans. An explicit
lexical reference year is captured when an omitted-year token needs it.

Priority matters for short-year alternatives: 31/12/26 resolves as 1931-12-26
under the token model because year/month/day succeeds before day/month/year.
31/12/2026 resolves as 2026-12-31 because the earlier alternatives are invalid.
These examples are specification-model results, not recorded native Access output.
An implementation-defined default year is needed for genuine month/day forms;
this service uses a caller-issued year and never substitutes the machine's current
year.


## Explicit default-year context

Use the existing --reference-year option for omitted-year literals:

```sh
tablecraft query eval 'SELECT Year(#Mar 7#),Day(#29/2#)' --reference-year 2024
tablecraft query run 'SELECT Count(*) FROM Orders WHERE OrderedAt >= #Mar 7#' --database example.accdb --reference-year 2024
```

The library prepare_with_options API accepts PreparationOptions.reference_year.
Its metadata-only binder resolves every SQL date constant into a checked legacy
DATE, retaining its written spelling for structural expression identity. The
owned plan captures the date directly, without formatting and reparsing floating
DATE values. No source rows, clock or provider are consulted. Plain prepare
retains its context-free admission and refuses dates requiring a default year.
An invalid constant fails even when a filter would produce no rows.

execute_with_schema_discovery uses ExecutionOptions.reference_year as the lexical
default for each plan prepared during that operation. Named/derived queries and
subqueries share that context. Date-derived integral crosstab headings such as
CLng(#Mar 7#) can establish static schemas without scanning; their captured dates remain stable
if a reusable plan executes under a different reference year.

Execution-time DateValue/CDate text, explicitly requested Eval text and domain
expression/criteria text retain the execution reference year. They are parsed at
execution and do not replace a previously captured SQL date. Scalar query eval
also uses its explicit reference year when evaluating a date token.

Calendar validity participates in the documented interpretation priorities.
Under the specification model, Feb 29 means February 29 in reference year 2024;
with reference year 2023 the named-month fallback instead means February 1, 2029.
The numeric 2/29 form fails with reference year 2023 because the fallback year 29
is below the legacy DATE envelope. These are explicit model results; native Access
comparison remains unrun. Choosing a year does not expand the qualified time
syntax or supply a regional date-text policy.
