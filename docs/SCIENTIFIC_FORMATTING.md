# Format scientific notation and scaling

Format now supports custom scientific suffixes and whole-part scaling commas,
alongside its fixed numeric [mask/section profile](FORMAT.md). Both scalar and
bound queries use the same borrowed parser, explicit NumberFormat and charged
renderer. Format remains Partial; this is not a change to stored numeric types.

The [Microsoft Format reference](https://learn.microsoft.com/en-us/office/vba/language/reference/user-interface-help/format-function-visual-basic-for-applications)
defines the scientific sign/case/width operators and comma scaling. Its comma
examples distinguish internal grouping from terminal scaling. Access query use
is established in the [built-in tracker](ACCESS_BUILTINS.md); no VBA is executed.

| Mask | Value | Current en-US output |
| --- | --- | --- |
| `0.00E+00` | 12345 | `1.23E+04` |
| `0.00e-000` | 12345 | `1.23e004` |
| `0.00E+00` | 9.995 exact Decimal | `1.00E+01` |
| `000.0E+00` | 12 | `120.0E-01` |
| `##0,,` | 100000000 | `100` |
| `0,.00` | 123456 | `123.46` |
| `0,.00%` | 123456 | `12345.60%` |
| `#,##0,.00` | 1234567 | `1,234.57` |

`Scientific` selects `0.00E+00` under the explicit profile. Scientific masks
require at least one whole digit slot and a suffix `E+`, `E-`, `e+` or `e-`
followed immediately by a `0`/`#` slot. Further exponent slots may be separated
by supported literal text. Those slots determine the minimum exponent
width; larger exponents retain all digits. Upper/lower marker case is preserved.
Plus variants print both signs, while minus variants omit a nonnegative sign.
The exponent for zero is zero. Whole mantissa width uses the total whole digit
slots; normalization is decimal-width based, without engineering multiples of
three. Rounding carry adjusts the exponent. Optional mantissa fraction slots,
sections, literals and percentage scaling use the existing policies.

Each comma immediately before a decimal point, or at the end of a whole-digit
body, shifts the display point left by three places. Commas between whole digit
slots request grouping instead, including repeated internal commas. An escaped
or quoted comma is literal. Scaling and percentage shifts compose before
rounding/normalization, including scientific masks. Commas after fractional slots
remain unsupported. [Interleaved numeric literals](FORMAT.md#interleaved-numeric-literals)
now work in the mantissa and exponent.

## Values and bounds

Exact integers, Currency and Decimal keep their original coefficient and a signed
display-point offset. `rust_decimal` performs midpoint-away rounding where its
scale supports the required operation. The one extra 29-digit drop position uses
a bounded zero/one half-threshold decision: raw coefficients are at most 96 bits,
and the library cannot construct scale 29. Rebuilding a display coefficient in
u128 allows rounding carry beyond Decimal::MAX without making that result a
stored or arithmetic Decimal. This is display bookkeeping, not a replacement
Decimal arithmetic implementation.

IEEE input retains the existing seven/15-significant-digit normalization. Its
point/exponent remains separate, so finite maxima and subnormals need no Decimal
conversion. Scientific normalization does not first round to 255 fixed places;
that would incorrectly erase very small finite inputs. NaN/infinity remain
errors. Text conversion, sign/section selection, regional separators and NULL
routing retain the existing Format policies.

Rendering retains the 257-unit mask bound, input/output ceilings, fallible charged
allocation, cumulative work and cancellation. Exponent padding streams through
the charged output; there is no exponent-sized String or heap token array.

```sh
tablecraft query eval "SELECT Format(12345,'Scientific'),Format(123456,'0,.00')" --number-format en-US
```

## Evidence and remaining work

Four kernel controls cover sign/case/width, carry, scaling/grouping/literals,
subnormals/maxima, exact carrier boundaries and a fixed-seed 256-case independent
integer/exponent model. Scalar checks exercise every observed cancellation point,
work/byte/output refusal and eager errors. Query controls cover grouping,
aggregates, correlation and plan reuse with changed masks/profiles. CLI tests run
both commands and verify buffered errors.

The original [independent runner](testing/compare_format_scientific.py) passes
1456 Decimal/model/refusal/native-field controls. Reviewed getter values from the
four original ScalarFixture files supply ID, Currency and NULL inputs; all source
hashes remain unchanged. Its initial native query omitted brackets around the
reserved Money field name and failed parsing; the corrected `[Money]` query
retains all probes. This harness correction does not establish native behavior.
The existing numeric and styled-number runners replay 2570 and 1334 controls.

Native Access precision, rounding, optional-slot/exponent normalization and
section/coercion/NLS qualification remain open. Broader date sections, complete
Variant/raw-string behavior and regional formats remain separate Format work. No dependencies, stored carriers or native I/O changed.
