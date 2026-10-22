# Numeric conversion of legacy Date/Time

CByte, CInt, CLng, CLngLng, CBool, CSng, CDbl, CCur and CDec now accept materialized
legacy Date/Time values. The conversion layer uses the stored OLE Double serial
and the existing checked destination kernel. It requires neither a number-format
profile nor a calendar/timezone provider. Stored fields and parameter carriers
retain their Date identity; IsNumeric(Date) remains FALSE and IsDate(Date) TRUE.

Microsoft's [Date coercion specification](https://learn.microsoft.com/en-us/openspecs/microsoft_general_purpose_programming_languages/ms-vbal/8a4a9201-4e7f-4856-b9fc-5927d2879723)
defines Date-to-numeric/Boolean conversion through the Double representation.
[Numeric coercion](https://learn.microsoft.com/en-us/openspecs/microsoft_general_purpose_programming_languages/ms-vbal/56fb78c3-a0ea-4bf0-8ee4-eb5f89bda4f7)
and [Boolean coercion](https://learn.microsoft.com/en-us/openspecs/microsoft_general_purpose_programming_languages/ms-vbal/3a9f5227-5fd5-4240-949a-51ffc32e71a9)
define destination rounding and zero/nonzero truth. Microsoft's
[Date-to-Double](https://learn.microsoft.com/en-us/windows/win32/api/oleauto/nf-oleauto-varr8fromdate)
and [Date-to-Decimal](https://learn.microsoft.com/en-us/windows/win32/api/oleauto/nf-oleauto-vardecfromdate)
Automation APIs provide related representation references. TableCraft calls no
Windows API; native Access/OleAut comparison remains pending.

## Working behavior

| Conversion | Current DATE result |
| --- | --- |
| CDbl | Exact stored serial bits, including signed zero and negative fractional representations |
| CSng | Explicit finite binary32 rounding of the serial |
| CByte/CInt/CLng/CLngLng | Existing half-even integer conversion and destination range checks |
| CBool | FALSE for either signed zero, TRUE for other serials; no integer rounding |
| CCur | Existing retained-IEEE-to-Decimal path and scale-four half-even conversion |
| CDec | Existing rust_decimal normalized Double conversion |

For example, `CDbl(#2025-01-01 12:00:00#)` returns `45658.5` and
`CLng(#1900-01-01 12:00:00#)` returns `2`. The latter serial is `2.5`, so integer
conversion uses the existing midpoint-to-even rule. CInt on a modern date fails
its Int16 range check. CByte on a negative whole serial fails rather than using
the Boolean TRUE-to-255 exception. A raw stored `-0.25` retains its sign through
CDbl even though its calendar interpretation also has a positive representation.
No calendar rounding/canonicalization precedes conversion.

NULL retains InvalidNullConversion. Binary/GUID/Extended Date/Time operands
retain their unsupported-type failures. Extended dates need a separately defined
precision/range contract before numerical conversion; they are not silently
narrowed into legacy DATE. Numeric Text still needs its explicit
[number policy](NUMBER_FORMAT.md). CStr/omitted-mask date output retains its
separate [date text policy](CSTR.md), and Str keeps its numeric-only input gate.
The core stored Value/schema rules and representational equality do not change.

## Shared typed arguments and query integration

Numeric arguments already using this conversion layer inherit its legacy DATE
coercion. Examples include Double-valued mathematical/financial arguments, Rnd's
Single operand and checked Long/Boolean option slots such as MonthName. Domain
and argument checks still follow conversion: Sqr of a negative serial fails
Domain, and a date ordinal outside MonthName's 1..12 range fails. These are numeric
serial arguments; MonthName(CDate(2)) selects February rather than inspecting the
calendar month of that date. Name rendering still needs an explicit language.
Type-specific operators/functions that use other kernels retain their separately
documented carriers; this change is not complete Variant promotion.

Scalar, bound, nested, domain, grouped and parameterized queries use the same
conversion function. Plans capture no date values or locale. Native snapshots
retain their declared Date fields and validated original values before expression
conversion. Converted query results have their numeric/Boolean destination type.
Date formatting's numeric-mask path now reuses the explicit Double conversion.
NULL filtering needs a genuine evaluation boundary: eager AND evaluates both
operands, so `Stamp IS NOT NULL AND CDbl(Stamp)<0` still fails on NULL. Filtering
an inner derived source before converting in an outer query is explicit and works.
All errors retain buffered CLI publication and shared work/cancellation limits.

## Verification and limits

Kernel controls cover all nine destination carriers, raw signed-zero/negative
fraction bits, half-even rounding, overflow, NULL and Extended refusals. A
fixed-seed generated whole-serial model checks integer/Double results. Additional
controls retain numerical consumer domain checks, the Rnd negative-seed sequence
and MonthName's numeric ordinal. Query controls cover plan/parameter reuse,
closed Eval, filters, groups, aggregates, domains, exact Date round-trip through
CDbl and eager NULL failures. CLI controls use both commands without ambient
format providers and verify destination type names and buffered failures.

The original `testing/compare_date_numeric.py` uses Python calendar/Decimal and
IEEE binary32 models. Its quarter-day corpus avoids making an independent claim
about rust_decimal normalization of nonexact DATE fractions. A one-second
floating-only probe covers nonexact Single narrowing. Single transport is decoded
at binary32 width because its shortest printed spelling can differ from the
binary64 spelling of the same number. Inert Jackcess getters supply original
ScalarSamples timestamps on V2000/V2007/V2016/V2019; all source hashes remain
unchanged. These checks do not execute native Access SQL. Native precision,
coercion/error ordering, broader Variant states, Extended numerical conversion
and full regional behavior remain open acceptance work.

The expression/query batch passes 766 tests including doctests; 293 parser-free
expression tests and ten selected CLI tests pass in separate invocations. The
independent runner passes 848 controls. The updated MonthName ordinal control
passes, alongside all-feature/minimal Clippy, warnings-denied API documentation,
formatting and diff checks. The full native engine/workspace suite is not rerun
for this expression-layer change.

[IIf/Switch conditions and Choose indices](CONDITIONAL_FUNCTIONS.md#date-text-and-explicit-expression-states)
now also consume legacy DATE serials. Conditions test unrounded zero/nonzero;
Choose retains its checked rounded index policy. Selected dates stay dates.

[Logical DATE operands](INTEGER_LOGIC.md#rounded-numeric-and-text-operands) now
reuse the same numeric conversion: unary Not uses Long; binary logical operands
use Long or an Int64-selected LongLong target before operating on bits. Final
read predicates retain unrounded DATE zero/nonzero. Extended dates and complete
native Variant/subtype inference remain outside this profile.

Space/String counts and numeric character operands, plus InStr/InStrRev starts,
now also reuse legacy DATE Long conversion. DATE serial -1.25 rounds to reverse
search sentinel -1; no calendar interpretation changes that numeric input.
[Repetition](TEXT_CONSTRUCTION.md) and [search](STRING_SEARCH.md) retain their
own domains, NULL precedence and native qualification gaps.

[Replace start/count slots](TEXT_REPLACEMENT.md#contextual-start-and-count-conversion)
now consume raw DATE serials through checked Long conversion as well. Negative
fraction rounding can select unlimited -1 or zero replacements; required positive
starts retain their separate domain checks. Native subtype/NULL/error precedence
remains unqualified.
