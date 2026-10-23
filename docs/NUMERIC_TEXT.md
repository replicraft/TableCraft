# Hex, Oct and Str numeric text

Unqualified Hex, Oct and Str now work in scalar SELECTs and bound retrieval,
including fields, reusable parameters, grouped expressions and aggregate results.
All return Text or propagate NULL. [CStr](CSTR.md) shares the bounded numeric renderer through a separate output-policy
adapter. It has distinct NULL/Boolean/Text/date behavior. These are expression
semantics, independent of
CLI display, character-code pages and database field decoding. SQL numeric Text
now uses an explicit NumberFormat as described below. Hex/Oct use raw legacy DATE
serials, while Str uses the explicit date output policy below.
General Variant coercion and broader regional parsing remain open.
Wrong arities, qualifiers and missing arguments fail at the normal call boundary;
written arguments remain eager. No VBA code executes.

## Hex and Oct

Hex emits uppercase hexadecimal digits; Oct emits octal digits. Both reuse the
existing checked half-even Long conversion for numeric/Boolean and legacy DATE
inputs, including Currency and Decimal. An explicit Int64 carrier bypasses Long conversion, preserving
all 64 bits; a Double outside Long range still fails. Zero produces `0`.
Positive values have no padding. The current negative-width intervals are:

| Value after conversion | Bit-pattern width |
| --- | --- |
| -32767 through -1 | 16 bits |
| -2147483648 through -32768 | 32 bits |
| Below -2147483648, explicit Int64 only | 64 bits |

For example, Hex(459) returns `1CB`, Oct(459) returns `713`, Hex(-1) returns
`FFFF` and Hex(-32768) returns `FFFF8000` in this profile. The intervals follow the
published MS-VBAL value-based rules, including the strict -32768 boundary. They
have **not** been qualified against native ACE expression execution. The formal
pages also contain inconsistent radix names and an overlapping upper endpoint in
the final negative range; TableCraft uses octal for Oct and the disjoint signed
intervals above. Native width/type behavior needs explicit comparison, including
Int16/Long/LongLong spellings, before claiming Access compatibility.

## Str

Str reserves a leading space for nonnegative values and uses a minus for negatives.
It uses a period, no grouping/currency symbol, and removes fractional trailing zeros.
Boolean TRUE is numeric -1 and FALSE is zero. Integer, Currency and Decimal inputs
retain all exact digits, including Int64 extrema and the complete Decimal magnitude;
fixed-point inputs never pass through floating point.

The current floating profile rounds Single to 7 and Double to 15 significant digits
using bounded standard-library scientific formatting. A rounded exponent at least
7/15 respectively uses uppercase scientific notation with an explicit exponent
sign and no exponent padding. Other values expand into decimal notation with a
leading zero for fractions. Both signed zeros become ` 0`; nonfinite inputs fail.
This is an explicit initial spelling profile, **not** a fully qualified native
formatter. Small-number scientific thresholds, leading-zero suppression, exponent
padding, Single precision and rounding/carry transitions require native comparison.
A rounded spelling near the largest finite Double can exceed the finite range on
reparsing; Str is not a lossless floating serialization or guaranteed Val inverse.

```sh
tablecraft query eval "SELECT Hex(459), Oct(459), Str(-459.65), Str(459.001)"
tablecraft query run "SELECT Hex(ORDR_NBR), Str(ORDR_NBR) FROM ROULETTE_TB" --database example.accdb
```

## Bounds, evidence and references

A borrowed NumericFormat recipe writes through the existing TextOutput contract.
Scratch formatting uses a fixed stack buffer sized from Decimal's envelope; expanded
floating fractions stream to the host. Every append checks cancellation, charges
work and enforces output bytes before fallible allocation. Partial results are
private and discarded on error. Ordinary aggregate, input, depth and total work
limits remain in force. Large batches of expanded tiny fractions may exhaust the
scalar command's default work limit; no partial stdout is published.

Focused kernel tests cover examples, ranges, exact fixed values, IEEE extremes,
generated integer/Decimal models and resource failures. Query tests cover fields,
NULLs, grouped calls, aggregates, nested queries, parameters and eager refusals.
CLI tests use csv to decode TSV cells and check failure buffering. The
[independent recipe](testing/compare_numeric_text.py) uses Python integers and
exact Decimal rounding of binary floating values. Optional Jackcess-decoded rows
supply independent inputs for sequential/indexed projections and a source hash
check; they do not validate native expression semantics. Native Windows Access
execution remains unrun. No dependency or fixture is added; all three functions
remain Partial in the [built-in tracker](ACCESS_BUILTINS.md).

Primary references:

- [Microsoft Hex](https://learn.microsoft.com/en-us/office/vba/language/reference/user-interface-help/hex-function), [Oct](https://learn.microsoft.com/en-us/office/vba/language/reference/user-interface-help/oct-function) and [Str](https://learn.microsoft.com/en-us/office/vba/language/reference/user-interface-help/str-function): numeric spelling, rounding, sign spaces and decimal separator.
- [MS-VBAL Hex](https://learn.microsoft.com/en-us/openspecs/microsoft_general_purpose_programming_languages/ms-vbal/b5f1801d-2e68-418b-b9db-1803368f012f) and [Oct](https://learn.microsoft.com/en-us/openspecs/microsoft_general_purpose_programming_languages/ms-vbal/f8c3018b-7005-4e1d-9c3b-6caea2a2625e): NULL, Long/LongLong conversion and negative-width intervals, with the inconsistencies noted above.
- [MS-VBAL Str](https://learn.microsoft.com/en-us/openspecs/microsoft_general_purpose_programming_languages/ms-vbal/c4ecaf21-da57-4f88-b5db-0276dd0693de) and [String coercion](https://learn.microsoft.com/en-us/openspecs/microsoft_general_purpose_programming_languages/ms-vbal/00113388-401b-41c2-8107-dc3fc0485554): numeric sign handling, period override and precision/notation guidance. The expression evaluator now supplies the deliberate Date/Error/Empty Str paths
  below; complete native spelling remains unqualified.


## Explicit numeric Text inputs (23 October 2026)

The SQL evaluator now accepts numeric Text for Hex/Oct/Str under a supplied
NumberFormat. Hex/Oct use checked Long conversion, including half-even rounding
and overflow. Only an actual Int64 carrier bypasses Long conversion; a string
outside Long range does not acquire that exception. Str parses Text to Double
before applying its existing period/sign/precision profile; the input separator
comes from NumberFormat, while output retains a period. Existing numeric Currency
and Decimal inputs retain exact spelling rather than passing through Double.

Conversion and output share the evaluator's cancellation, Text and work limits.
Binding does not parse values or invoke a format provider; parameter plans can be
reused under different explicit formats. Missing policy and conversion failures
publish no CLI output. The pure numeric renderer still rejects Text. NULL and
formal Variant return categories remain unchanged. The deliberate Str Date/Error/Empty paths are described below; broader coercion,
complete native spelling and negative-width qualification remain open.

The [MS-VBAL Hex](https://learn.microsoft.com/en-us/openspecs/microsoft_general_purpose_programming_languages/ms-vbal/b5f1801d-2e68-418b-b9db-1803368f012f)
and [Oct](https://learn.microsoft.com/en-us/openspecs/microsoft_general_purpose_programming_languages/ms-vbal/f8c3018b-7005-4e1d-9c3b-6caea2a2625e)
specifications select Long for non-LongLong values. The
[Str specification](https://learn.microsoft.com/en-us/openspecs/microsoft_general_purpose_programming_languages/ms-vbal/c4ecaf21-da57-4f88-b5db-0276dd0693de)
selects Double for the remaining nonnumeric input branch. These guide the current
Text paths without supplying native Access execution evidence.

The original [input runner](testing/compare_numeric_text_inputs.py) uses Python
integer/Decimal half-even models, explicit binary64 spelling and original
registered native inputs. It checks output type, missing-policy refusals, invalid
and overflowing Text, negative-width boundaries and unchanged hashes. It does not
execute Microsoft Access or infer a host locale.


## Str expression states and legacy dates (23 October 2026)

Str now deliberately consumes standard expression Error data through the same
bounded invariant `Error N` writer as CStr. No message catalog, host error state
or VBA execution is consulted. Operational failures remain failures; an eager
IIf branch that divides by zero cannot turn into an Error string. Opaque host
codes outside the existing standard-code envelope still require a mapping.
Expression Empty supplied through the host API converts to numeric zero, producing
` 0`; Missing remains non-scalar. These conversions produce ordinary Text without
making Error/Empty storable or admitting implicit arithmetic/concatenation.

Legacy Date inputs to the evaluator's Str use the existing CStr date formatter
under an explicit DateTextFormat, independently of NumberFormat. A missing date
policy fails DateFormatRequired. This follows the distinct Date branch of the
[Str specification](https://learn.microsoft.com/en-us/openspecs/microsoft_general_purpose_programming_languages/ms-vbal/c4ecaf21-da57-4f88-b5db-0276dd0693de);
the reviewed MS-VBAL section 5.5.1.2.11 supplies Empty-to-numeric zero. Extended
DateTime remains unsupported by Str. The pure numeric renderer retains its own
numeric-only admission, and NULL still propagates.

Query Nz(NULL) already returns empty Text in this profile; it does not construct
expression Empty. Str(Nz(NULL)) therefore retains Text conversion requirements,
whereas Str(Nz(NULL,0)) receives ordinary numeric zero. Formal Variant return
categories, exact fixed-point rendering and explicit numeric Text parsing remain
unchanged. Native locale/spelling, HRESULT mapping and full Variant semantics
remain unqualified.


## Hex/Oct legacy dates and expression states (23 October 2026)

Hex/Oct now admit materialized legacy DATE through their shared checked Long
conversion. Rounding uses the raw serial, not calendar components, and requires
no NumberFormat or date formatter. For example, Hex(CDate(-1.5)) is `FFFE` after
half-even conversion to -2. Extended DateTime remains unsupported. Negative-width
intervals, the actual-Int64-only bypass and numeric-source behavior are unchanged.
The host-free renderer admits these validated Date carriers without parsing Text.

The explicit expression-state entry point converts host Empty to Text `0`,
propagates scalar NULL, rejects Missing as non-scalar and rejects all Error data
with TypeMismatch. It never implicitly formats an Error code: Hex(CVErr(1)) fails,
while Hex(CLng(CVErr(1))) uses the explicit standard-code conversion and succeeds.
Operational evaluation errors and eager selector failures remain failures.
Query Nz(NULL) retains its empty-Text result and is not an Empty constructor.

The existing MS-VBAL Hex/Oct references specify Long coercion, the LongLong
exception and Empty result. The reviewed specification's section 5.5.1.2.9
requires TypeMismatch for Error Let-coercion outside Variant; this remains distinct
from explicit numeric conversion functions. No new `$` spelling, stored state,
error-code mapping or VBA execution is introduced. Native width/error/locale
qualification and full Variant semantics remain open.
