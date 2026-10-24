# Retained String conversion API parity

The evaluator's explicit retained-value conversion APIs now accept valid String
ranges for CByte, CInt, CLng, CLngLng, CBool, CSng, CDbl, CCur, CDec, CDate,
CVDate, DateValue and TimeValue. They use the existing strict Unicode publication
service, then the same semantic converter as materialized Text. Previously valid
retained numeric Strings failed TypeMismatch, and retained date Strings failed
NonScalarExpression, although equivalent materialized Text already converted.

This fixes public API representation consistency. Ordinary supported scalar and
bound query paths already published valid String arguments before invoking these
APIs, and their numerical/date behavior is unchanged. No new SQL function, parser
syntax, ambient locale, numeric promotion profile or native conversion rule is
introduced.

## Exact values and explicit policies

Numeric conversions keep the existing NumberFormat requirement and checked
conversion kernel. LongLong numeric Text remains exact across its full signed
range without an intermediate Double. Currency retains four-place half-even
conversion and its signed storage range. Decimal retains its validated 96-bit
coefficient and scale envelope. Single/Double conversion, numeric truth and
integral half-even rounding are unchanged.

Date conversions retain explicit date syntax, numeric fallback, reference-year
and calendar controls. CDate/CVDate still share their materialized DateTime
carrier while their query declaration categories remain separate. DateValue and
TimeValue retain their existing component extraction, discarded-part validation
and NULL policies. Date components and interval operations additionally use the staged retained-input
contract below; DateSerial and TimeSerial constructors remain unchanged.

Only active String bytes are published. Discarded backing bytes are not decoded.
A selected unpaired surrogate or odd-byte range remains UnrepresentableText, even
when a conversion format was omitted: strict publication precedes conversion.
Cancellation and resource failures from publication remain authoritative before
later syntax/policy failures. Publication uses the current per-Text ceiling and
shared decoded allocation/work pools; conversion uses those same existing pools.
No raw backing buffer is copied just to expose its selected range.

## Expression states remain distinct

Numeric Empty-to-zero behavior and reviewed standard Error-code conversions are
unchanged. CDec still refuses implicit Error-code conversion; opaque Error codes
retain their existing refusal. Numeric Missing remains TypeMismatch, while the
date APIs retain NonScalarExpression. CDate/CVDate Empty still means DATE zero;
DateValue/TimeValue still refuse host Empty. CDate/CVDate NULL fails conversion,
whereas DateValue/TimeValue keep their current NULL propagation. These remain
expression states rather than stored field values or new SQL constructors.

## References and controls

Microsoft's [Access type conversion reference](https://support.microsoft.com/en-us/access/functions/type-conversion-functions)
describes explicit conversions and supplies query examples. The
[DateValue](https://support.microsoft.com/en-us/access/functions/datevalue-function)
and [TimeValue](https://support.microsoft.com/en-us/access/functions/timevalue-function)
references describe date/time extraction and include query examples. CVDate's
query availability is recorded in the existing
[date-conversion contract](DATE_CONVERSIONS.md#cvdate-query-alias). These establish
the existing function family; TableCraft's retained byte ranges are an internal
representation, not a qualified native data type. Native Access comparisons
remain deferred.

Focused tests compare all thirteen APIs with independent expected numeric/date
carriers and materialized Text. They exercise exact LongLong/Currency/Decimal
endpoints, active ranges, malformed selected ranges, absent policies, existing
expression states, every observed cancellation checkpoint, byte/work exhaustion
and per-Text limits. A separate query matrix compares retained API results with
scalar evaluation and bound Mid/IIf/CVar compositions under two number formats,
while retaining raw-publication and eager-error refusals.

The focused conversion/state/date regression batch passes 28 tests, including
four retained API tests and two scalar/bound parity tests. Strict scoped Clippy
passes for the expression library/tests with SQL enabled and the new query test;
formatting and diff checks also pass. The captured pre-fix regression returned
TypeMismatch for retained numeric Text while its materialized control succeeded.

## Date components and interval operations

The retained-value APIs also accept valid active String ranges for Year, Month,
Day, Hour, Minute, Second, Weekday, DateAdd, DatePart and DateDiff. Their previous
NonScalarExpression refusal for valid retained date inputs was inconsistent with
materialized Text. Scalar and bound queries already admitted valid String
arguments, so this is API parity rather than a newly corrected ordinary query
result or a new native compatibility claim.

Publication happens at each existing admission position. DateAdd parses the
interval before converting its count, then admits the date source. DatePart and
DateDiff parse the interval, convert and validate first-day before first-week,
and only then admit date sources. DateDiff converts its first date before
publishing the second and still checks an invalid second date when the first is
NULL. Weekday validates its optional first-day before admitting the date source.
Invalid controls therefore do not accidentally inspect later malformed retained
ranges. Reached control callbacks execute once. Callers still evaluate supplied
operand expressions eagerly; this staging does not suppress an operational
failure in an unselected IIf branch.

Strict publication checks only selected bytes, rejecting odd-byte ranges and
unpaired surrogates. It uses the existing decoded allocation/work budget,
cancellation callback and per-Text ceiling before contextual date/numeric
conversion. Retained controls require the same explicit number format as Text;
date spelling and system week settings retain their explicit policies. Existing
DATE and ExtendedDateTime inputs are borrowed, retaining their carrier and exact
Extended precision. Empty, NULL, Error and Missing retain each operation's
existing admission behavior. No input vector or raw backing-buffer copy is added.

Operation entry checkpoints remain in place. Weekday also retains its earlier
size check for an already materialized Text source; retained source publication
occurs later, after week-control validation. Resource/error ordering therefore
follows each representation's established admission stage, rather than globally
publishing every operand in advance.

Microsoft's [Year](https://support.microsoft.com/en-us/access/functions/year-function),
[Weekday](https://support.microsoft.com/en-us/access/functions/weekday-function),
[DateAdd](https://support.microsoft.com/en-us/access/functions/dateadd-function),
[DatePart](https://support.microsoft.com/en-us/access/functions/datepart-function)
and [DateDiff](https://support.microsoft.com/en-us/access/functions/datediff-function)
references provide query examples and the existing component, interval and
week-control contracts. They do not qualify TableCraft's internal retained
representation or establish native failure precedence.

The added five API tests cover all ten calls, active ranges with invalid discarded
backing bytes, progressive independently expected failure precedence, exact
Extended carriers, callback-once controls and every observed cancellation point,
plus byte/work/per-Text exhaustion. Two query tests check 80 successful retained,
materialized, scalar and bound results across two number formats, and 30 raw
refusal/eager-error controls. The pre-fix Year regression failed with
NonScalarExpression for a valid retained date whose materialized control worked.

For this extension, 81 distinct focused tests passed: 19 evaluator tests (five new
retained controls and 14 existing component/interval/Weekday controls), 55 existing
date-kernel tests, the two new query tests and five existing conversion query
regressions. Strict Clippy passed for SQL-enabled expression library/tests and the
new query target; the expression library also passed a no-default-features check.
Formatting and diff checks passed. No native Windows comparison was run.
