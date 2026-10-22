# FormatNumber, FormatPercent and FormatCurrency

All three unqualified functions now return bounded Text in scalar SELECTs,
bound/nested retrieval and qualified saved SELECT execution:

`Function(expression, [digits], [leading_zero], [negative_parentheses], [grouping])`

They share a parser-independent formatter, typed option validation and metered
output. Missing optional slots preserve their function defaults; explicit NULL is
not omission. Written arguments remain eager. Wrong arities, qualifiers, wildcards
and missing required slots fail before operand execution. No VBA content executes.

| Option | Current behavior |
| --- | --- |
| digits | Checked Long half-even coercion, then 0–255. Omitted or -1 selects the profile default of 2. Values below -1 or above the explicit output ceiling fail. This ceiling is not a native Access limit claim. |
| leading_zero | -1 TRUE, 0 FALSE, -2/default uses TRUE. FALSE removes the zero before fractional output; zero-place output still includes a whole zero. |
| negative_parentheses | Same tristate values. Default TRUE for en-us currency, FALSE otherwise. Parentheses enclose the complete formatted value, including its suffix. |
| grouping | Same tristate values. Default FALSE for invariant, TRUE for en-us/de-de. Group size is three. |

Boolean TRUE/FALSE operands supply -1/0 through checked option conversion. Other
numeric values are not arbitrary truthiness: for example, tristate 1 is invalid.
SQL numeric Text option coercion now uses the explicitly selected NumberFormat;
pure text kernels retain their policy-free Text-option refusal. NULL expression/options raise
InvalidNullConversion; native NULL/error precedence remains unqualified.

## Explicit output profiles

Every call requires `--number-format invariant|en-us|de-de` or the corresponding
library setting, including calls supplying all optional arguments. No OS preference
or database locale is inferred. The defaults below are selected TableCraft profile
values, not automatic discovery of customizable Windows Number/Currency tabs.

| Profile | Decimal/group separator | Currency | Default negative currency |
| --- | --- | --- | --- |
| invariant | period / absent | Unsupported | Unsupported |
| en-us | period / comma | Dollar prefix: `$1,234.50` | `($1,234.50)` |
| de-de | comma / period | Euro suffix with a space: `1.234,50 €` | `-1.234,50 €` |

Explicit grouping TRUE with invariant fails because that profile has no group
separator. FormatCurrency with invariant fails because no currency symbol is
selected. FormatPercent shifts the decimal point two places and appends `%` in
all three profiles; its current de-de profile has no extra percent spacing.
Default number/percent negatives use a minus prefix. Rounded zero is canonical
positive zero; negative zero adornments are not retained.

```sh
tablecraft query eval "SELECT FormatNumber(1234.5),FormatPercent(0.125),FormatCurrency(-1234.5)" --number-format en-us
tablecraft query run "SELECT FormatNumber(0.5,2,False),FormatNumber(1234.5,,,,False)" --number-format en-us
tablecraft query eval "SELECT FormatCurrency('1.234,50 €')" --number-format de-de
```

Expression results are `1,234.50`/`12.50%`/`($1,234.50)`, `.50`/`1234.50`, and
`1.234,50 €`, respectively. CLI output retains its existing typed Text quoting and
CSV transport escaping; ordinary numeric CLI rendering is unchanged.

## Numeric precision and bounded formatting

Byte, signed integers, Currency and Decimal retain their exact input magnitude.
The existing private rust_decimal dependency supplies nearest rounding with ties
away from zero for this output profile. This is separate from the half-even
conversion/arithmetic contracts; native formatting rounding remains unqualified.
Percent scaling is a display-point shift after rounding at the corresponding
input precision. Even the maximum scalar Decimal can display a percentage beyond
its stored carrier range without constructing an invalid carrier.

Finite Single/Double values reuse Str/CStr's current seven/fifteen significant-digit
normalization, then use bounded Decimal coefficient rounding with a separate signed
decimal exponent. This covers every finite IEEE magnitude without narrowing large
floats to a stored Decimal. Extremely small values can round to zero. Precision
beyond the normalized digits pads zeros; it does not expose exact binary expansion.
Nonfinite values fail. Full native subtype/last-digit behavior remains unqualified.

Text uses the [whole-string numeric profile](NUMBER_FORMAT.md) and finite Double
conversion before formatting. Supplying typed CDec/CCur values preserves fixed
precision; a bare Text number follows the Double profile. Date/binary/GUID and
expression-only Variant/runtime states are unsupported. There is no implicit
Text-to-number coercion elsewhere or new general Format-mask implementation.

Private charged scratch handles Text normalization. The resolved display owns a
bounded coefficient/options/exponent; output digits and grouping stream directly
to a charged, cancellation-aware sink. The 255-place ceiling is independent of
stored Decimal's 28-place scale. Scratch bytes never enter results, and failed
query/CLI execution publishes no partial stdout. Source/output/work/allocation
limits remain errors. No dependency, serializer or fixture was added.

The existing Text plan node preserves all value/options through pruning, grouping,
aggregate detection and correlations. Profiles remain execution/request-local,
including optional worker reuse after failure. No UPDATE support, native predicate
pushdown, GUI or network runtime is introduced. The shared float normalization
also retains the existing Str/CStr output contracts.

## References and evidence

Microsoft's [FormatNumber](https://learn.microsoft.com/en-us/office/vba/language/reference/user-interface-help/formatnumber-function),
[FormatPercent](https://learn.microsoft.com/en-us/office/vba/language/reference/user-interface-help/formatpercent-function)
and [FormatCurrency](https://learn.microsoft.com/en-us/office/vba/language/reference/user-interface-help/formatcurrency-function)
references define argument order, default sentinels, tristate options, percent
scaling and regional Number/Currency settings. Complete NLS patterns/defaults,
broader Variant coercion, native precision/rounding/spacing and error ordering
remain open. All three are **Partial** in the [built-in tracker](ACCESS_BUILTINS.md).

Kernel tests cover profiles, overrides, exact values, midpoint/zero behavior,
maximum Decimal percentages, finite IEEE bounds, 255 places, scratch isolation,
refusals and host failures, with generated Currency cases checked against an
independent integer rounding model. Evaluator tests check input/scratch/output/work
limits and cancellation. Query, CLI and worker tests cover dynamic options,
fields, grouping, aggregates, correlations, optional slots, profile isolation and
buffered failures. The [independent recipe](testing/compare_styled_number.py) uses
Python Decimal half-up models plus explicit float normalization and optional
sequential/indexed projections over Jackcess-decoded row inputs, checking unchanged
source bytes. Existing CStr and numeric-text recipes check the shared float helper.
Native Windows Access remains unrun; independent model/reader checks do not
establish native compatibility.

## Contextual numeric options

FormatNumber/Percent/Currency now normalize numeric Text digits and tristates
through checked Long conversion under the execution-local number profile. Existing
numeric/legacy DATE conversions, -1 digit defaults and -2 tristate defaults remain.
A digits value of `2,5` under de-de rounds to 2. Tristates remain -1/0/-2 codes,
not arbitrary numeric truthiness or Boolean-word parsing. Display-value rounding
still follows the existing magnitude renderer; option rounding does not change it.

The owning option parser requests each conversion only when that option is due
for validation. Invalid digit ranges therefore fail before malformed later
tristates, and a NULL expression retains its prior early error. All written SQL
operands evaluate eagerly before option validation. No replacement argument list
or formatted-value clone is created. Normalization, value resolution and streamed
output share cumulative limits and every observed contextual formatting checkpoint
can cancel. Regional defaults, source carriers and unsupported profile choices
are unchanged; these named profiles do not discover machine preferences.

The combined [numeric/date option recipe](testing/compare_format_options.py)
passes 420 independent Decimal/Gregorian controls, including en-us/de-de Text
options, default/sentinel/error cases and unchanged priority native ID/DATE getter
integration. The expression/query batch passes 840 tests including doctests, with
309 separately run minimal-profile tests and three selected CLI tests. All-feature/
minimal Clippy, warnings-denied rustdoc and formatting/diff checks pass. No native
Access SQL/NLS qualification or broader Variant/source-coercion completion is
claimed. General Format week options retain their separate existing profile.
