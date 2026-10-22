# Materialized arithmetic result promotion

`tablecraft-expressions::numeric::promoted_arithmetic` now implements an explicit
materialized-value profile for addition, subtraction and multiplication. SQL scalar
evaluation and bound read queries use it after any contextual Text conversion.
`numeric::arithmetic` remains the existing checked Int64/Double kernel for consumers
that need that contract, including current aggregate accumulation. Stored schemas,
values, parameters and core representational equality are unchanged.

| Materialized operands | Current result for +, - and * |
| --- | --- |
| Byte / Byte | Byte while in range; then Integer, Long or Double |
| Boolean / Integer with Byte, Boolean or Integer | Integer while in range; then Long or Double |
| Long with narrower integral types or Long | Long while in range; then Double |
| LongLong with any integral type | LongLong; overflow fails |
| Single with Byte, Boolean, Integer or Single | Single, with Double on Single overflow |
| Single with Long/LongLong, or Double with ordinary integers/floats | Double; existing exact integer-to-floating guards remain |
| Currency + or - ordinary numeric | Currency after checked four-place operand conversion |
| Currency * integer or Currency | Currency; checked Decimal product followed by four-place half-even conversion |
| Currency * Single/Double | Double |
| Decimal with numeric or legacy DATE | Checked Decimal operands and result |
| Legacy DATE + or - numeric, without Decimal | Existing serial/date-result profile |
| Legacy DATE * numeric, without Decimal | Double serial product |
| Either admitted scalar NULL | NULL |

Boolean arithmetic uses -1/0 and an Integer result, not Boolean. Widening never
silently narrows an explicitly wider integer operand. Integral overflow beyond
Long promotes directly to Double, not LongLong. The resulting Double may round
the exact integer product; the value remains Double. LongLong overflow fails.
Single operands are evaluated through the existing finite Double kernel and
rounded to Single when its range permits. Gradual underflow and signed zero stay
IEEE values; nonfinite input/result checks remain active.

Currency products reuse rust_decimal. Valid four-place Currency products fit the
intermediate Decimal envelope before the final rounding; products outside the
Currency range fail. Currency/Double operand conversion retains the existing
binary-value rounding policy: the binary64 value spelled 0.00005 lies just above
the exact decimal midpoint. This differs from exact lexical Text conversion and
is covered as a distinct input.

Decimal takes precedence over a legacy DATE operand. Conversion uses the stored
DATE serial, not its civil interpretation. Other DATE operations retain their
existing APIs. Extended DateTime and other unsupported carriers do not become
numeric. Error data, Empty/Missing and arrays do not acquire implicit scalar or
storage representations through this service.

## Boundaries and remaining work

This service uses materialized carrier identities. It does not infer a declared
SQL expression type or implement every static non-Variant overflow rule. Native
Access comparisons are still needed for its query-specific result typing. Integer
literals still materialize as Int64; this change does not reinterpret literal
spelling or suffixes. Unary signs, division, integer division, Mod and power retain
their previous profiles. Full declared/Variant promotion and those operator-specific
rules remain backend work. Materialized LongLong-to-Double precision guards remain
stricter than silent native rounding.

Text conversion remains [execution-local](ARITHMETIC_TEXT.md). For example, a Text
operand converted to Double participates as Double; Text + Text still concatenates.
Metadata-only computed crosstab headings use the same policy-independent arithmetic
without source scans. Aggregate kernels retain their separate result profile:
`Sum` of Byte expressions still accumulates to the existing Int64 result.

The evaluator charges node work and conversions under the same cumulative budgets;
the promotion kernels do not allocate or read sources. Source field policy routing,
filter preservation, typed snapshots and native storage contracts are unchanged.
No runtime host, dependency, crate, serializer or write behavior is added.

```sh
tablecraft query eval "SELECT VarType(CByte(255)+CByte(1)), VarType(CSng(1)+CInt(1))"
tablecraft query run "SELECT Octet+CByte(1), VarType(Octet+CByte(1)) FROM ScalarSamples" --database example.accdb
```

## References and evidence

[MS-VBAL arithmetic](https://learn.microsoft.com/en-us/openspecs/microsoft_general_purpose_programming_languages/ms-vbal/e070115f-8d40-40cf-ac6d-ab18b9c6c906)
defines effective operand types and separate Variant/static overflow branches.
The [addition reference](https://learn.microsoft.com/en-us/office/vba/language/reference/user-interface-help/plus-operator)
includes Byte widening; [multiplication](https://learn.microsoft.com/en-us/openspecs/microsoft_general_purpose_programming_languages/ms-vbal/917bed61-f19f-4e7d-9f93-2e44d4ec7432)
and [division](https://learn.microsoft.com/en-us/openspecs/microsoft_general_purpose_programming_languages/ms-vbal/8628f21c-3f02-42b1-908c-201bd7ffe1b5)
have operator-specific exceptions. These are implementation references, not native
Access SQL qualification or a claim that the unimplemented branches work.

Four kernel tests cover typed ranges/widening, Single overflow/underflow, fixed
point/DATE selection and a fixed-seed generated independent integer model. Query
and CLI tests verify carriers through derived/grouped/correlated/pivot reads,
inspection and buffered late overflow. A DATE regression now checks serial
multiplication instead of refusing it.

The original [range/type/Fraction runner](testing/compare_arithmetic_promotion.py)
passes 818 controls, including native Byte/Integer/Single field projections over
four unchanged original ScalarFixture files. The existing
[DATE runner](testing/compare_date_arithmetic.py) passes 62 model controls after
updating multiplication to its working profile. Getters only supply raw fields;
only TableCraft executes SQL. Native Access typing, rounding, optimizer and
error-order qualification remain pending.
