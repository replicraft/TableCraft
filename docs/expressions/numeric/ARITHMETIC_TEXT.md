# Numeric Text arithmetic

SQL scalar evaluation and bound read queries now accept numeric Text operands under
an explicit NumberFormat. They reuse the existing checked conversion service,
including whole-input validation, regional separators, fallible normalization,
shared quotas and cancellation. No format is inferred from the operating system.
Prepared queries use each execution's policy; stored fields remain unchanged.

The String/String branch of plus now also accepts [lossless raw string inputs](../text/CONCATENATION.md#lossless-string-addition).
Mixed raw String/numeric addition strictly publishes Text before reusing the
numeric rules below, retaining original declaration categories. NULL propagation
requires no decoding or numeric parsing after operand-family/state admission.

| Operation | Current Text route |
| --- | --- |
| Text + Text | Concatenation, without numeric parsing |
| Text + numeric, numeric + Text | Numeric addition; never implicit concatenation |
| Subtraction, multiplication, division | Checked numeric operation under the rules below |
| Unary + or - | Convert Text to Double before applying the sign |
| Power | Convert Text and its numeric counterpart to Double |
| Integer division and Mod | Select Integer/Long/LongLong from original operands, convert Text directly with checked half-even rounding, then retain the selected result/widening profile |
| Decimal counterpart, except power/integer division/Mod | Convert Text directly to Decimal and retain existing checked rust_decimal arithmetic |
| Currency addition/subtraction | Convert Text to Currency at scale four before adding/subtracting |
| Currency multiplication/division with Text | Convert both operands to Double |
| Legacy DATE addition/subtraction with Text | Convert Text to Double and retain the existing raw serial/date-result policy |
| Other legacy DATE arithmetic with Text | Convert both operands to the selected numeric target; DATE is never parsed as calendar Text |

Other ordinary numeric counterparts select Double for Text conversion. Unsupported
native carriers are not made numeric by these rules. Existing no-Text arithmetic
uses its owning [materialized promotion profile](ARITHMETIC_PROMOTION.md) for +, -
and *. General Variant/declared-type promotion, wider compatibility cases,
comparison coercion and extended native date semantics remain open.
The policy-free numeric kernels still refuse Text.

Integer Text conversion does not first round through Double or a bounded Decimal.
For example, a lexical value just above 2.5 beyond Decimal's retained precision
rounds to 3 for integer division. Decimal arithmetic separately retains its bounded
96-bit/scale-28 rounding profile. A mixed integer/Double arithmetic operation uses the existing checked Double
converter, including nearest-even rounding of large integers before the operation.
This does not implement all arithmetic subtype widening or native error precedence. Existing zero-divisor/domain/overflow rules remain authoritative.

Both supported scalar operand expressions evaluate before operator conversion, in written order.
NULL arithmetic propagates after admitting both operand families, without parsing
the other already-evaluated supported scalar;
`Null + ('2'/0)` still fails. Text `+` with NULL propagates NULL. Ampersand follows its separate [contextual formatting contract](../text/CONCATENATION.md). Error/Missing and arrays do not
become implicit numbers or stored NULL. Host Empty now uses the separate
[operator-specific admission profile](EMPTY_ARITHMETIC.md), without becoming a stored value.

In particular, GUID, Binary and Extended-date arithmetic operands now fail
TypeMismatch even beside NULL. Non-finite floating operands fail NonFinite before
NULL propagation. The same shared admission rules apply to contextual numeric
arithmetic and the explicit declared rounded API. Valid finite values do not need
to fit an otherwise-unused conversion target when NULL already determines the
result. Invalid numeric Text still needs no parsing policy in that case. Eager
operand evaluation remains first; a later written division error is not suppressed
by a NULL or an unsupported carrier. Native error precedence remains unqualified.

```sh
tablecraft query eval "SELECT '2,5'+3, '2'+'3', '9,5' Mod '2,5'" --number-format de-de
tablecraft query run "SELECT ID, CStr(ID)+1 FROM ScalarSamples" --database example.accdb --number-format en-us
```

Numeric Text in fixed crosstab heading expressions remains an execution policy
dependency. Metadata-only preparation refuses it without scans; explicit staged
execution can resolve it. The result must still be an admitted Text/integral
heading, such as `IN (CLng('1'+1))`.

## Lossless String operands in numeric arithmetic

Subtraction, multiplication, division, integer division, Mod and power now admit
raw String intermediates before Unicode publication. Both expressions still
evaluate eagerly in written order. After operand-family/state admission, NULL
determines the result without decoding the other String or requiring NumberFormat.
For example, `ChrW(55296)-NULL` returns NULL even though that isolated surrogate
cannot be published as ordinary Text. An invalid constructor or a later operand's
division error still fails; unsupported GUID/Binary and Error/Missing states do
not become NULL.

Non-NULL raw Strings must publish valid Text before the existing checked numeric
conversion. NumberFormat, integral rounding, Decimal conversion and original-type
Empty-divisor rules remain authoritative. The bound query path retains each
operand's declaration category and text policy before existing declaration-aware
dispatch; decoding does not infer a new category. Parameter rebinding and grouped,
correlated expressions use the same service. NULL propagation allocates no decoded
payload; actual publication retains byte quotas and cancellation checks.

This closes an arithmetic admission gap, not general raw relational storage,
comparison, Variant promotion or native Access qualification. The independent
runner passes 1,350 source-free Decimal/arithmetic/Text controls, including raw
regional numeric spellings, all six NULL operators, unsupported families and eager
failures. Optional native getter inputs were not supplied for this run.

## Public retained-value operator dispatch

`Evaluator::apply_expression_binary` now normalizes retained active String ranges
through the same preparation services as SQL evaluation for addition,
concatenation and the six numeric operators. Valid numeric Strings therefore use
the existing checked conversion targets; malformed UTF-16 still fails when actual
publication is required. NULL propagation, Empty divisor behavior, unsupported
families, explicit formats and cumulative resource/cancellation checks retain their
owning contracts. The scalar return type still requires strict final publication.

This fixes a public API inconsistency. Ordinary SQL evaluation already used these
preparation services; no new native promotion or default query policy is inferred.

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

The original [independent runner](../../testing/compare_arithmetic_text.py) compares
Decimal arithmetic/half-even expectations and raw getters from unchanged original
ScalarFixture files. Only TableCraft executes SQL; native Access optimizer,
coercion, promotion and error-order qualification remain pending. No dependency,
core carrier, native reader/writer or serialization format changes.


Under explicit `query run --declared-integral`, integer division and Mod now
retain original Scalar/Variant categories through these existing rounded Text
conversions. The [declaration service](DECLARED_ARITHMETIC.md#rounded-integer-division-and-mod-operands)
checks effective-width overflow without changing lexical rounding, required number
formats, conversion ranges or default materialized arithmetic.
