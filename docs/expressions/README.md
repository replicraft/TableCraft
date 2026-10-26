# Expressions

Query-callable built-ins, scalar semantics and explicit comparison policies.

[Documentation index](../README.md)

- [Access built-in implementation tracker](ACCESS_BUILTINS.md)
- [Packed Access color expressions](COLOR_FUNCTIONS.md)
- [Eager conditional expressions](CONDITIONAL_FUNCTIONS.md)
- [Materialized CVar values](CVAR.md)
- [Closed query Eval expressions](DYNAMIC_EVALUATION.md)
- [Numbered error-message expressions](ERROR_MESSAGES.md)
- [Scalar expression evaluation](EXPRESSIONS.md)
- [Access GUID byte-array conversion](GUID_CONVERSIONS.md)
- [HyperlinkPart query components](HYPERLINK_PARTS.md)
- [Retained String conversion API parity](RETAINED_CONVERSIONS.md)
- [CurrentUser and explicit Access session data](SESSION_FUNCTIONS.md)
- [Value inspection and expression-only arrays](VALUE_INSPECTION.md)
- [CVErr and expression Error data](VARIANT_ERRORS.md)

## Dates

- [MonthName and WeekdayName](dates/CALENDAR_NAMES.md)
- [Host-issued Date, Time, Now and Timer](dates/CLOCK_FUNCTIONS.md)
- [Automation DATE arithmetic operators](dates/DATE_ARITHMETIC.md)
- [Gregorian date/time constructors](dates/DATE_CONSTRUCTORS.md)
- [CDate, CVDate, DateValue and TimeValue](dates/DATE_CONVERSIONS.md)
- [Date interval source conversion](dates/DATE_INTERVALS.md)
- [Numeric conversion of legacy Date/Time](dates/DATE_NUMERIC.md)
- [IsDate scalar inspection](dates/ISDATE.md)
- [Standalone Weekday](dates/WEEKDAY.md)
- [Explicit host week settings](dates/WEEK_SETTINGS.md)

## Financial

- [Bounded periodic cashflow functions](financial/CASHFLOW_FUNCTIONS.md)
- [Scalar depreciation functions](financial/DEPRECIATION.md)
- [EuroConvert query profile](financial/EURO_CONVERSION.md)
- [Fixed-rate financial functions](financial/FINANCIAL_FUNCTIONS.md)
- [Period counts and payment components](financial/PERIOD_PAYMENTS.md)
- [Bounded annuity rate solving](financial/RATE.md)

## Formatting

- [FormatDateTime named output](formatting/DATE_FORMATTING.md)
- [Format date and time masks](formatting/DATE_FORMAT_MASKS.md)
- [Format masks and sections](formatting/FORMAT.md)
- [Explicit numeric Text conversion](formatting/NUMBER_FORMAT.md)
- [Format scientific notation and scaling](formatting/SCIENTIFIC_FORMATTING.md)
- [FormatNumber, FormatPercent and FormatCurrency](formatting/STYLED_NUMBER_FORMATTING.md)
- [Format text layouts](formatting/TEXT_FORMAT_MASKS.md)

## Numeric

- [Materialized arithmetic result promotion](numeric/ARITHMETIC_PROMOTION.md)
- [Numeric Text arithmetic](numeric/ARITHMETIC_TEXT.md)
- [Explicit declarations for arithmetic and comparisons](numeric/DECLARED_ARITHMETIC.md)
- [Declared numeric/String predicates](numeric/DECLARED_NUMERIC_TEXT.md)
- [Host Empty arithmetic admission](numeric/EMPTY_ARITHMETIC.md)
- [Integer logical operators and numeric predicates](numeric/INTEGER_LOGIC.md)
- [IsNumeric scalar inspection](numeric/ISNUMERIC.md)
- [Numeric built-ins](numeric/MATH_FUNCTIONS.md)
- [Contextual numeric and date comparisons](numeric/NUMERIC_PREDICATES.md)
- [Hex, Oct and Str numeric text](numeric/NUMERIC_TEXT.md)
- [Partition range labels](numeric/PARTITION.md)
- [Hexadecimal and octal query literals](numeric/RADIX_LITERALS.md)
- [Rnd query state](numeric/RANDOM_FUNCTIONS.md)
- [Val numeric-prefix parsing](numeric/VAL.md)

## Text

- [UTF-16 byte strings and observations](text/BYTE_STRING_FUNCTIONS.md)
- [Explicit numeric character code pages](text/CHARACTER_CODE_PAGES.md)
- [Chr, Asc, ChrW and AscW](text/CHARACTER_FUNCTIONS.md)
- [Contextual string concatenation](text/CONCATENATION.md)
- [CStr with explicit output policies](text/CSTR.md)
- [Explicit ICU host text profile](text/ICU_TEXT_PROFILE.md)
- [Explicit Japanese string encoding](text/JAPANESE_STRING_ENCODING.md)
- [LIKE matching](text/PATTERN_MATCHING.md)
- [Predicates over retained String data](text/RAW_PREDICATES.md)
- [Retained String comparison](text/RAW_STRING_COMPARISON.md)
- [Lossless Strings in query cells](text/RETAINED_STRINGS.md)
- [Left, Right and Mid scalar sources and count coercion](text/SLICE_COUNTS.md)
- [StrConv casing, kana and string encoding](text/STRCONV.md)
- [StrComp and explicit comparison policies](text/STRING_COMPARISON.md)
- [InStr and InStrRev substring search](text/STRING_SEARCH.md)
- [Explicit Unicode text casing](text/TEXT_CASING.md)
- [Bounded Space, String and StrReverse](text/TEXT_CONSTRUCTION.md)
- [Bounded Replace expressions](text/TEXT_REPLACEMENT.md)
