# Scalar expression evaluation

`crates/query/tablecraft-expressions` owns evaluated scalar rules over core
`Value` carriers. Its direct scalar APIs are independent of SQL parsing, database
connections, GUI hosts and async runtimes. The syntax parser remains in
`tablecraft-sql`; binding, query plans and row execution are separate work.

This is an initial, explicit scalar profile, not complete Jet/ACE or VBA Variant
semantics. Core values describe materialized carriers, not the declared/Variant
type metadata required to infer every native promotion. Documentation-derived
tests are not observed native Access results.

## Implemented rules

| Area | Implemented behavior |
| --- | --- |
| Truth | TRUE/FALSE/unknown, Boolean/NULL conversion, NOT/AND/OR/XOR/EQV/IMP; predicate selection accepts only TRUE |
| Comparison | Either NULL returns unknown, including NULL = NULL; all six comparison operators use numeric ordering, not `Value::Eq` |
| Exact numerics | Boolean TRUE becomes -1 and FALSE becomes 0 in the numeric domain; Byte/Int16/Int32/Int64, Currency and Decimal compare without floating conversion or scale-alignment overflow |
| Floating comparison | Finite numeric comparison, signed zeros equal; Double rounds to Single precision when those two carriers are compared; overflow/non-finite values rejected |
| Integral coercion | Explicit Byte/Int16/Int32/Int64 targets; fractions round halves to even; bounds checked after rounding; NULL remains NULL |
| Integral arithmetic | Checked +, -, *, unary minus; results use Int64; overflow errors instead of wrapping or guessing Variant widening |
| Floating arithmetic | +, -, *, / and ^ over supported integers/floats; finite Double results; division by zero and overflow are errors |
| Integer division/remainder | `\` and Mod round numeric operands halves to even, then use a quotient truncated toward zero and a dividend-signed remainder; Int64 result |
| Currency arithmetic | Exact Currency +/- Currency/integer and Currency * integer, retaining scale four and rejecting overflow |
| Concatenation | Text/NULL `&` treats one NULL as empty and retains NULL for two NULL operands; text `+` propagates either NULL; UTF-8 output byte ceiling checked before allocation |

Decimal scale and core carrier equality remain unchanged. Exact numeric comparison
compares signs, whole parts and fractional digits without rescaling a coefficient
past its range. Large integer digits are never silently rounded into a Double:
unsupported inexact integer-to-float conversion returns `PrecisionLoss`.

Text comparison requires a caller-supplied `Collation`; no locale, Unicode
normalization, case-folding or Rust string order is silently selected. A host can
supply a deterministic test collation without claiming it matches Access.

`EvalError` uses thiserror and carries no value, SQL or field-name payloads.
Scalar routines accept `Value`, so Empty/Missing/Error expression-only states
cannot silently enter them as NULL. Omitted function arguments retain their
separate syntax representation.

## Explicit limits of this profile

Arithmetic result carriers are TableCraft's initial checked Int64/Double profile,
not VBA's automatic Byte/Integer/Long/Single/Variant promotion ladder. Explicit
coercion can select a narrower integral result. Decimal binary arithmetic and
Currency division, Currency * Currency, and Currency/Decimal mixed with floats
are rejected until rounding and the larger Variant Decimal envelope have their
own implementation and evidence. Decimal/Currency can still participate in exact
comparison and explicit integral coercion (including rounded `\`/Mod operands).

Logical operators accept Boolean/NULL only; numeric bitwise operators and implicit
numeric-to-truth conversion are not implemented. Numeric text parsing, mixed
text/numeric `+`, numeric/date string rendering, dates/GUID/binary comparisons,
locale-dependent conversions, built-in functions and complete VBA expression
states are not implemented. Unsupported combinations fail explicitly.

The direct text APIs require a byte ceiling. They reserve fallibly after checking
the combined length. Other scalar functions perform bounded work on fixed-size
carriers; a caller supplying a collation owns that callback's work and semantics.

## References and verification

The authored tests follow Microsoft primary references; they do not copy fixture
SQL or execute content from the supplied ACCDB.

| Rule | Reference |
| --- | --- |
| Access expression/operator scope | [Access operators](https://support.microsoft.com/en-us/access/table-of-operators) |
| Boolean/NULL tables | [And](https://learn.microsoft.com/en-us/office/vba/language/reference/user-interface-help/and-operator), [Or](https://learn.microsoft.com/en-us/office/vba/language/reference/user-interface-help/or-operator), [Not](https://learn.microsoft.com/en-us/office/vba/language/reference/user-interface-help/not-operator), [Xor](https://learn.microsoft.com/en-us/office/vba/language/reference/user-interface-help/xor-operator), [Eqv](https://learn.microsoft.com/en-us/office/vba/language/reference/user-interface-help/eqv-operator), [Imp](https://learn.microsoft.com/en-us/office/vba/language/reference/user-interface-help/imp-operator) |
| Numeric comparison and mixed precision | [Comparison operators](https://learn.microsoft.com/en-us/office/vba/language/reference/user-interface-help/comparison-operators) |
| Boolean numeric conversion | [Boolean data type](https://learn.microsoft.com/en-us/office/vba/language/reference/user-interface-help/boolean-data-type) |
| Coercion, rounding, locale and Variant Decimal boundaries | [Conversion functions](https://learn.microsoft.com/en-us/office/vba/language/concepts/getting-started/type-conversion-functions) |
| Arithmetic and native promotion distinctions | [Addition](https://learn.microsoft.com/en-us/office/vba/language/reference/user-interface-help/plus-operator), [subtraction](https://learn.microsoft.com/en-us/office/vba/language/reference/user-interface-help/minus-operator), [division](https://learn.microsoft.com/en-us/office/vba/language/reference/user-interface-help/forwardslash-operator), [integer division](https://learn.microsoft.com/en-us/office/vba/language/reference/user-interface-help/backslash-operator), [Mod](https://learn.microsoft.com/en-us/office/vba/language/reference/user-interface-help/mod-operator) |
| Distinct concatenation behavior | [Ampersand](https://learn.microsoft.com/en-us/office/vba/language/reference/user-interface-help/ampersand-operator), [Access text concatenation](https://support.microsoft.com/en-us/access/combine-text-values-by-using-an-expression) |

Initial local scalar checks on aarch64-apple-darwin with Rust 1.99.0 on 17 October
2026 pass formatting, Clippy with warnings denied and 12 tests, including four
fixed-seed properties with 256 cases each. Tests cover complete Boolean/NULL
tables, exact comparisons beyond f64 precision, half-even rounding, integer
extremes, signed quotient/remainder identities, Currency boundaries and Unicode
byte limits. Native Access comparison and other platforms remain unverified.
