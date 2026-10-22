# Numeric Text arithmetic

SQL scalar evaluation and bound read queries now accept numeric Text operands under
an explicit NumberFormat. They reuse the existing checked conversion service,
including whole-input validation, regional separators, fallible normalization,
shared quotas and cancellation. No format is inferred from the operating system.
Prepared queries use each execution's policy; stored fields remain unchanged.

| Operation | Current Text route |
| --- | --- |
| Text + Text | Concatenation, without numeric parsing |
| Text + numeric, numeric + Text | Numeric addition; never implicit concatenation |
| Subtraction, multiplication, division | Checked numeric operation under the rules below |
| Unary + or - | Convert Text to Double before applying the sign |
| Power | Convert Text and its numeric counterpart to Double |
| Integer division and Mod | Convert Text directly to checked LongLong with half-even rounding, then use the existing Int64 quotient/remainder kernels |
| Decimal counterpart, except power/integer division/Mod | Convert Text directly to Decimal and retain existing checked rust_decimal arithmetic |
| Currency addition/subtraction | Convert Text to Currency at scale four before adding/subtracting |
| Currency multiplication/division with Text | Convert both operands to Double |
| Legacy DATE addition/subtraction with Text | Convert Text to Double and retain the existing raw serial/date-result policy |
| Other legacy DATE arithmetic with Text | Convert both operands to the selected numeric target; DATE is never parsed as calendar Text |

Other ordinary numeric counterparts select Double for Text conversion. Unsupported
native carriers are not made numeric by these rules. Existing no-Text arithmetic
keeps its current profile. General Variant/declared-type promotion, non-Text Currency
combinations, comparison coercion and extended native date semantics remain open.
The policy-free numeric kernels still refuse Text.

Integer Text conversion does not first round through Double or a bounded Decimal.
For example, a lexical value just above 2.5 beyond Decimal's retained precision
rounds to 3 for integer division. Decimal arithmetic separately retains its bounded
96-bit/scale-28 rounding profile. A mixed integer/Double operation still refuses an
inexact integer conversion with PrecisionLoss. This does not implement native
silent floating precision loss, all arithmetic subtype widening or native error
precedence. Existing zero-divisor/domain/overflow rules remain authoritative.

Both supported scalar operand expressions evaluate before operator conversion, in written order.
NULL arithmetic propagates without parsing the other already-evaluated scalar;
`Null + ('2'/0)` still fails. Text `+` with NULL propagates NULL. Ampersand retains
its existing Text/NULL-only contract; this change does not implicitly format numeric
values for concatenation. Error data, arrays and expression-only states do not
become implicit numbers or stored NULL.

```sh
tablecraft query eval "SELECT '2,5'+3, '2'+'3', '9,5' Mod '2,5'" --number-format de-de
tablecraft query run "SELECT ID, CStr(ID)+1 FROM ScalarSamples" --database example.accdb --number-format en-us
```

Numeric Text in fixed crosstab heading expressions remains an execution policy
dependency. Metadata-only preparation refuses it without scans; explicit staged
execution can resolve it. The result must still be an admitted Text/integral
heading, such as `IN (CLng('1'+1))`.

## References and evidence

Microsoft's [Access operator reference](https://support.microsoft.com/en-us/access/table-of-operators)
establishes expression/query operator scope. The [addition reference](https://learn.microsoft.com/en-us/office/vba/language/reference/user-interface-help/plus-operator)
and [MS-VBAL arithmetic rules](https://learn.microsoft.com/en-us/openspecs/microsoft_general_purpose_programming_languages/ms-vbal/e070115f-8d40-40cf-ac6d-ab18b9c6c906)
describe numeric/string dispatch and effective operand types. The
[multiplication](https://learn.microsoft.com/en-us/openspecs/microsoft_general_purpose_programming_languages/ms-vbal/917bed61-f19f-4e7d-9f93-2e44d4ec7432)
and [division](https://learn.microsoft.com/en-us/openspecs/microsoft_general_purpose_programming_languages/ms-vbal/8628f21c-3f02-42b1-908c-201bd7ffe1b5)
exceptions motivate Currency/Text conversion. These references do not establish
TableCraft's complete Access Variant or SQL type/error behavior.

Four scalar tests include a fixed-seed generated integer model, regional/exact
Decimal/Currency/date goldens, lexical midpoint cases, NULL/concatenation boundaries,
shared byte limits and every observed conversion cancellation checkpoint. Query
controls cover prepared policies, derived/grouped/pivot/domain/Eval consumers and
eager errors; a named-heading test preserves metadata-only preparation. Both CLI
query commands retain buffered failures and existing CSV/TSV quoting.

The original [independent runner](testing/compare_arithmetic_text.py) compares
Decimal arithmetic/half-even expectations and raw getters from unchanged original
ScalarFixture files. Only TableCraft executes SQL; native Access optimizer,
coercion, promotion and error-order qualification remain pending. No dependency,
core carrier, native reader/writer or serialization format changes.
