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
and NULL policies. This does not widen the separate retained-input contracts of
date component or interval APIs.

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
