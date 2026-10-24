# Automation DATE arithmetic operators

Legacy DateTime `+` and `-` now have an explicit serial-arithmetic kernel in
`tablecraft-expressions::functions::dates::date_arithmetic`. The scalar evaluator
and query executor share it for the selected non-Decimal addition/subtraction
profile. [Materialized promotion](ARITHMETIC_PROMOTION.md) now gives Decimal
precedence and supplies raw serial multiplication/division/power through Double,
plus checked rounded integer division/Mod. Legacy Date [Avg/DAvg](AVERAGES.md#legacy-datetime-means) separately admit stored
serial means; Date Sum retains its numeric-only boundary. These profiles do not
change DateAdd/DateDiff or establish native aggregate qualification.

| Operands / operator | Result |
| --- | --- |
| Date + numeric, numeric + Date, Date + Date | Date when representable |
| Date - numeric, numeric - Date | Date when representable |
| Date - Date | Double serial difference |
| Unary + Date | Same stored Date, preserving bits |
| Unary - Date | Negated serial, Date when representable |
| Finite result outside the Date envelope | Double |
| Either operand NULL | NULL after eager operand evaluation |

Numeric day operands accept the existing CDbl carrier/conversion policy, including
Boolean TRUE=-1, Single/Double, integers, Currency and Decimal. Their conversion
may round to binary64 at this explicit Date kernel boundary. SQL Text operands
now have [explicit contextual conversion](ARITHMETIC_TEXT.md); Decimal operands
use the promotion service instead. Extended DateTime, Empty/Error states and
unimplemented Date operator families remain unsupported.
NaN/infinite numeric operands fail explicitly. A non-finite calculated result is
Overflow. No universal Variant promotion or locale parsing is introduced.

The kernel uses exact stored Automation serials, preserving their binary64
fractions. It does not convert to a rounded civil calendar before calculation.
For example, the serial for `1899-12-29 06:00:00` is -1.25; adding 0.5 produces
-0.75 under this policy. Its calendar interpretation follows Automation DATE's
negative-fraction rules. This is **not an elapsed civil-time duration policy**.
Even -0.25 and +0.25 represent the same calendar time while their serial difference
is -0.5. Calendar interval operations remain in DateAdd/DateDiff with Jiff.
Native Access pre-epoch arithmetic, promotion boundaries and rounding still need
Windows qualification; the explicit current policy must not be mistaken for it.

Date construction retains years 100–9999 and validates the whole day rather than
rounding a serial into the envelope. A finite out-of-envelope result becomes a
Double according to the documented Date-variant promotion rule; it is not clamped,
wrapped or manufactured as an invalid Date. Subsequent operations see that actual
Double carrier. The core carrier and its representational equality are unchanged.

Microsoft's [plus operator](https://learn.microsoft.com/en-us/office/vba/language/reference/user-interface-help/plus-operator)
and [minus operator](https://learn.microsoft.com/en-us/office/vba/language/reference/user-interface-help/minus-operator)
describe Date participation, Date-minus-Date and overflow promotion. The
[Automation DATE overview](https://learn.microsoft.com/en-us/cpp/atl-mfc-shared/date-type)
describes the separate whole-day and time-fraction interpretation. These sources
guide implementation; they are not native Access execution evidence.

## Parsing and verification

Hash-delimited dates work adjacent to subtraction and comparison operators,
including `#1900-01-01#-#1899-12-30#` and `#1900-01-01#>=#1899-12-30#`.
The Access lexical adapter separates upstream PostgreSQL #-/#>/#>> tokens while
retaining exact spans and adjacent >=. It does not reinterpret quoted strings or
bracketed names containing those bytes. Unsupported shift operators remain outside
the query grammar; separate > and = tokens are not silently joined across spaces.

Three authored/generated value tests cover numeric carriers, operand order,
result classes, pre-epoch representations, NULL, non-finite inputs, boundaries
and an independent integer quarter-day model. A scalar dispatch test, two query
tests and a parser regression cover nested expressions, eager errors, pruning,
correlations, grouping, parameters, promotion and rejected aggregate/operator
expansion. The [CLI reference recipe](testing/compare_date_arithmetic.py) compares
explicit float/DATE presentation models and inert Jackcess raw date bits from
existing original Jet 4, ACE 2007, ACE 2016 and ACE 2019 scalar fixtures. It verifies
unchanged hashes. No new databases, dependencies or native Access oracle are added.
