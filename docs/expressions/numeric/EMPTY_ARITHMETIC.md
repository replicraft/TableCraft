# Host Empty arithmetic admission

The expression API now consumes host-supplied `ExpressionValue::Empty` at arithmetic
boundaries. Empty stays distinct from NULL and empty Text; it is not a stored
Value, query parameter or new SQL constructor. Query `Nz(NULL)` still returns
empty Text. This implements shared expression semantics without introducing a
VBA interpreter or reopening deferred runtime functions.

## Current effective-type profile

Operands evaluate before admission. Standard Error data remains TypeMismatch;
Missing remains refused. Unsupported Binary/GUID/extended-date and non-finite
counterparts retain their arithmetic failures. Once admitted, Empty selects an
operator-specific zero before existing checked conversion/promotion runs:

| Case | Current admission |
| --- | --- |
| Empty with Byte under +, - or * | Byte zero, retaining the Byte-selected result/overflow rules |
| Empty with Byte under integer division or Mod | Integer zero, retaining the explicit rounded-operator exception |
| Empty with floating/fixed-point, Date or numeric Text under integer division or Mod | Long zero, retaining Long-selected rounding/conversion |
| Empty with other numeric carriers | Integer zero; the counterpart still selects the existing effective result type |
| Empty with Text under numeric operators other than + | Double zero, with explicit numeric Text parsing policies |
| Empty with Empty | Integer zero operands; division errors and power rules remain in their owning kernels |
| Unary numeric signs on Empty | Integer zero |
| Plus with Empty and String | Return the existing String, including a raw active byte range, without parsing or another payload allocation |

Empty is not converted into Date(0): doing so would wrongly select the
Date-minus-Date Double exception. For example, Empty minus Date(1.25) retains a
Date result in this profile. Currency/Decimal operations reuse the existing exact
carriers and arithmetic. NULL results still require operand-family admission and
cannot suppress eager failures. Zero divisors are not bypassed merely because
the numerator came from Empty.

Division also retains original Empty provenance before lowering. The formal
[division contract](https://learn.microsoft.com/en-us/openspecs/microsoft_general_purpose_programming_languages/ms-vbal/8628f21c-3f02-42b1-908c-201bd7ffe1b5)
distinguishes Single/Double/String/Date zero divided by Empty (DivisionByZero)
from Integer zero divided by Empty (Overflow). The owning evaluator validates
the original numerator under its conversion/finite/Text policy and resource
controls before raising the Empty-divisor error. It does not reinterpret an
arbitrary conversion overflow as a division error.

The bounded pure `evaluation::arithmetic_operands` and
`arithmetic_unary_operand` functions retain ordinary scalar carriers and do no
Text parsing or declaration inference. Scalar evaluation and bound declared
dispatch use the same admission. Original caller-issued Scalar/Variant categories
survive it; the selected zero is not evidence of a declaration. Logical operators,
comparisons, aggregates and storage retain their separate Empty boundaries.
For the original-type division exception, `empty_division_operand` selects the
numerator and `Evaluator::apply_empty_division` validates it. The pure pair helper
refuses that case so callers cannot accidentally erase the error distinction.

## References and qualification

The [MS-VBAL arithmetic contract](https://learn.microsoft.com/en-us/openspecs/microsoft_general_purpose_programming_languages/ms-vbal/e070115f-8d40-40cf-ac6d-ab18b9c6c906)
provides the general Empty/effective-type basis. The
[integer division and Mod contract](https://learn.microsoft.com/en-us/openspecs/microsoft_general_purpose_programming_languages/ms-vbal/09fdf266-a9d5-4ecb-b7d8-ffd1361cb349)
provides the Byte/Empty exception and rounded targets. Existing multiplication,
division, exponentiation and numeric promotion kernels retain their qualified
profiles; admitting Empty does not establish complete Variant compatibility.

The general effective-type table's Double/String presentation leaves an ambiguous
cell in the flattened reference text. TableCraft follows the dedicated
[Office plus reference](https://learn.microsoft.com/en-us/office/vba/language/reference/user-interface-help/plus-operator)
for Empty/String identity, rather than inferring numeric parsing from that cell.
Numeric Empty cases retain the general effective-type profile. Broader identity,
subtype, signed-zero and native query behavior still need native qualification;
these sources are not observed Windows Access execution.

## Independent controls

The original `empty_arithmetic` example is an explicit host API probe, enabled only
with the existing SQL feature. It uses clap's Parser, typed integral inputs and
caller-issued declarations, without constructing SQL or opening files:

```sh
CARGO_INCREMENTAL=0 cargo build -p tablecraft-expressions --example empty_arithmetic --features sql --locked --offline
python3 docs/testing/compare_empty_arithmetic.py --probe target/debug/examples/empty_arithmetic
```

The [independent Python model](../../testing/compare_empty_arithmetic.py) checks 3,230
integral value/carrier/refusal outcomes, including both operand orders, boundaries,
seeded values, rounded Byte exceptions, NULL and all four issued declaration
combinations. Its Python integer model is independent of production zero selection.
Scalar declarations for Empty in this engineering probe test the API contract;
they are not evidence of a native static Empty declaration.

Focused evaluator goldens additionally cover Single, Currency, Decimal, Date,
numeric Text policies, Error/Missing, unary signs and raw String identity under a
zero-byte budget. Native Access, stored Empty values and SQL Empty construction
are not supplied by this evidence.
