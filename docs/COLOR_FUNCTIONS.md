# Packed Access color expressions

Unqualified, parenthesized `RGB(red, green, blue)` and `QBColor(index)` now work
in scalar evaluation and bound read-only queries. They produce Access Long
(`Value::Int32`) numbers, without creating UI colors or invoking application code.
Microsoft explicitly lists both among functions usable in Access database-engine
queries in its [sandbox query reference](https://support.microsoft.com/en-us/access/functions-and-properties-in-access-blocked-by-sandbox-mode).
This establishes query scope; it is not native execution qualification.

| Function | Implemented numeric profile |
| --- | --- |
| RGB | Three required channels; red occupies the least significant byte, then green and blue. Nonnegative channels above 255 clamp to 255 after checked conversion. |
| QBColor | One required index in 0..15; selects the Basic palette and returns its packed RGB number. |

The [RGB reference](https://support.microsoft.com/en-us/access/functions/rgb-function)
specifies a Long result and upper channel saturation. The
[QBColor reference](https://support.microsoft.com/en-us/access/functions/qbcolor-function)
defines the numbered palette and red/green/blue byte order. Index 7 uses
192/192/192, index 8 uses 128/128/128, and index 15 uses 255/255/255. These are
packed Access numbers, independent of host-native color objects or themes.

```sh
tablecraft query eval 'SELECT RGB(1,2,3) AS Packed,QBColor(4) AS DarkRed'
tablecraft query run 'SELECT RGB(999,0,0) AS Saturated'
```

Results are 197121/128 and 255, respectively.

## Conversion and failure boundaries

RGB and QBColor reuse checked numeric/Boolean CInt conversion for their Integer
arguments. Both use half-even integer rounding. The
profile accepts numeric stored carriers, rejects NULL
with InvalidNullConversion, negative converted arguments and out-of-range palette
indices with Domain, and conversion overflow/non-finite inputs with existing
typed failures. Boolean TRUE converts to -1 and therefore fails the domain check.
SQL numeric Text now uses an explicit NumberFormat through the shared checked
target conversion. Binary and object/default-property coercions remain unsupported.
RGB inputs beyond the signed Integer conversion envelope fail before saturation.

Every supplied expression evaluates eagerly in written order before numeric
validation. Thus a later division by zero is not hidden by an earlier NULL or
invalid channel. Wrong arities, missing slots, wildcard arguments and qualified
names fail before operand execution. Expanding the math call buffer to three
arguments does not expand Round's two-argument limit or other unary functions.

The color implementation lives in the expression crate's math feature slice;
shared math binding, dependency walks, grouping identity and execution supply
query integration. Binding requires only source metadata. Cancellation and work
budgets retain ordinary scalar/query checkpoints; output remains buffered until
successful completion. No dependency, application host or UI package was added.

## Evidence and open compatibility work

Focused tests cover all palette numbers, independent channel-order goldens,
numeric carriers, half-even conversion, saturation, refusals and a fixed-seed
256-case arithmetic property. Query tests cover inert binding, all observed
cancellation checkpoints, projections, filtering, correlations, grouping,
aggregates/domains, parameter-plan reuse and eager/late errors. CLI tests cover
both execution paths and empty output on failures.

The original [independent runner](testing/compare_colors.py) uses decimal rounding
and byte construction for RGB, and palette bit/intensity arithmetic for QBColor.
Optional original SavedForest-derived AData fixtures exercise local native-file
projections with hashes checked before/after. It does not execute formulas in
Microsoft Access or add saved color-query fixtures. Full Variant coercion, input
widths, fractional/NULL boundaries and native error precedence remain unqualified;
both functions stay **Partial** in the [tracker](ACCESS_BUILTINS.md).

The shared checked numeric argument conversions also accept
[legacy DATE serials](DATE_NUMERIC.md); existing channel/index domain checks apply.
This supplies numbers only and adds no UI or Windows object dependency.


Numeric Text controls follow the existing per-channel validation order and
conversion envelope; a failed red channel does not parse green or blue Text.
See [math Text evidence](MATH_FUNCTIONS.md#explicit-numeric-text-operands-23-october-2026).
RGB and QBColor now model their formal Integer argument contracts;
native Access query-wrapper qualification remains unrun.


Color calls now use [explicit expression-value admission](MATH_FUNCTIONS.md#expression-errorempty-math-arguments-23-october-2026)
after all supplied SQL expressions evaluate. Error data fails TypeMismatch at its
channel/index position; earlier numeric domain errors retain precedence. Host
Empty is zero under the RGB/QBColor Integer control profiles. This does not qualify
native argument widths or evaluation/error precedence.


## RGB Integer channel boundary

[MS-VBAL revision 3.0](https://learn.microsoft.com/en-us/openspecs/microsoft_general_purpose_programming_languages/ms-vbal/d5418146-0bd2-45eb-9c7a-fd9502722c74),
section 6.1.2.7.1.11, declares three Integer channel arguments and a Long result.
RGB now uses checked Integer conversion before each channel's existing negative
validation and upper saturation. Thus 32767 saturates to 255, 32768/-32769 fail
Overflow, and -32768 converts successfully before failing Domain. Half-even
32767.5 also overflows. Numeric Text uses exact lexical Integer conversion under
the selected NumberFormat; it does not round through Double first.

Packing and saturation follow the Access RGB reference above. The published
formal runtime equation uses `max`, conflicting with that documented upper clamp;
the implementation retains the Access reference's saturation behavior. The formal
declaration supplies the argument widths, not proof of native query-wrapper
coercion or error precedence. Written arguments still evaluate eagerly, and
channel conversion/validation proceeds red, green, blue under the current profile.

Typed kernel checks cover all channel positions and fixed/floating operands.
Query and CLI controls cover distinct range/domain failures, plan reuse, late
errors under TOP and empty stdout on failure. The independent Decimal runner
models rounding, signed Integer limits, channel packing and saturation; native
Access comparisons remain pending. No UI, VBA execution or dependency is added.

## QBColor Integer argument boundary

The [formal QBColor contract](https://learn.microsoft.com/en-us/openspecs/microsoft_general_purpose_programming_languages/ms-vbal/4b7087c6-20fc-4d90-8d83-730a0c6a4aad)
declares an Integer argument and Long result. Existing query availability remains
established by the Access sandbox query reference above. The implementation now
uses the shared Integer conversion before validating the 0..15 palette range:
32768/-32769 report Overflow, while 32767/-32768 convert successfully and then
report Domain. Fractional/Decimal/Currency and explicit-policy numeric Text
arguments use the same checked conversion, without changing stored values.
This corrects the earlier Long control profile, which reported Domain for values
that overflow Integer. RGB's separately documented channel saturation remains.

Focused checks include exact typed errors, plan reuse after failed parameters,
scalar/query CLI refusal with no stdout, and independent Decimal half-even/range
models. Host Empty/Error/Missing and eager evaluation retain their existing
admission path. The correction follows the formal argument model; actual native
Access coercion, query-wrapper behavior and error wording remain unqualified.
