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

The current profile reuses checked numeric/Boolean CLng conversion, including
half-even integer rounding. It accepts numeric stored carriers, rejects NULL
with InvalidNullConversion, negative converted arguments and out-of-range palette
indices with Domain, and conversion overflow/non-finite inputs with existing
typed failures. Boolean TRUE converts to -1 and therefore fails the domain check.
Text, dates, binary and object/default-property coercions are not implemented.
RGB inputs beyond the signed Long conversion envelope fail before saturation.

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

The shared checked Long argument conversion now also accepts
[legacy DATE serials](DATE_NUMERIC.md); existing channel/index domain checks apply.
This supplies numbers only and adds no UI or Windows object dependency.
