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
precision; a bare Text number follows the Double profile. Legacy DATE now uses
its raw serial through the same Double profile. Extended dates and binary/GUID
remain unsupported. Expression states follow the separate ordered profile below. There is no implicit
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

## Legacy DATE source values

FormatNumber, FormatPercent and FormatCurrency now format legacy DATE sources
numerically through the existing checked Double conversion. The result reflects
the raw OLE serial, including negative epoch aliases; it does not use Jiff civil
conversion, round to whole seconds or infer a date-output policy. For example,
FormatNumber(CDate(-0.25)) returns -0.25 under en-us, and
FormatPercent(#1899-12-29 06:00:00#) returns -125.00%. All calls still require an
explicit number profile. The stored DATE value and ordinary date rendering are
unchanged.

This reuses the current fifteen-significant-digit finite Double display profile,
then the same rust_decimal magnitude rounding, grouping/sign/currency styles and
streamed output. It introduces no separate decimal arithmetic or date algorithm.
Legacy range checks remain at the value constructor. Extended dates have no
implicitly chosen numeric serial and remain refused rather than narrowed. NULL
sources/options, eager expressions and function-specific validation order retain
their existing behavior.

Microsoft's [FormatNumber declaration](https://learn.microsoft.com/en-us/openspecs/microsoft_general_purpose_programming_languages/ms-vbal/357ce1a8-bacd-4a69-942e-921a6bc76c6d)
accepts a Variant expression. This slice applies the existing explicit
[DATE numeric conversion contract](DATE_NUMERIC.md) to the formatter; neither that
declaration nor independent calendar getters establishes native Access's complete
Variant/precision/rounding/error behavior. All three functions retain Partial
status and require native Access qualification.

Kernel goldens cover raw negative aliases, signed zero, quarter days and both
DATE range ends, plus explicit Extended refusal. Evaluator controls cover exact
output-byte ceilings and every observed cancellation checkpoint without a date
policy. Prepared query and CLI controls cover execution-local number policies,
raw DATE rebinding, nested calls, closed Eval, NULLs and buffered eager failures.
The original [independent runner](testing/compare_styled_date_sources.py) passes
1,636 Decimal/calendar/native-field controls over unchanged priority-family inputs.
The calendar cases deliberately use exactly representable quarter-day fractions;
direct raw-alias goldens cover representations that calendar getters normalize.
No native Access SQL execution or general full Variant qualification is claimed.

## Ordered numeric formatting states

FormatNumber, FormatPercent and FormatCurrency now retain eager expression
operands through the original admission order: source NULL refusal, required
NumberFormat, ordered places/leading/parentheses/grouping checks, then source
conversion and rendering. A source Error does not hide an invalid earlier option;
`FormatNumber(CVErr(11),256,CVErr(12))` reports Domain. A NULL source still reports
InvalidNullConversion before admitting Error options. These are preserved
TableCraft validation rules; native Access error precedence remains unqualified.

Host Empty selects numeric zero in the source and every supplied numeric option,
separately from omitted default/sentinel slots. Error data gives TypeMismatch at
its admission point and Missing gives NonScalarExpression. An explicit
`FormatNumber(CLng(CVErr(11)),0)` produces `11`; no implicit Error code or English
Error spelling is substituted. Host Empty is not a new SQL constructor. All
written operands remain eager and once-only: a late division failure still stops
before the format recipe is prepared, even with an earlier Error or NULL source.

The shared preparation service requests each option by index only when its owning
parser reaches it. It borrows the source after options pass, reuses the original
private scratch/number recipe and emits through the existing bounded materializer.
Materialized-value callers use the same service; no copied source/replacement
argument list, policy default, return category, locale lookup or dependency is
added. Per-Text, cumulative byte/work, cancellation and output guards remain.

Microsoft's [FormatNumber controls](https://learn.microsoft.com/en-us/office/vba/language/reference/user-interface-help/formatnumber-function),
[numeric Empty conversion](https://learn.microsoft.com/en-us/openspecs/microsoft_general_purpose_programming_languages/ms-vbal/e2637723-3010-449d-bead-9f17533dc105)
and [explicit Error conversion distinction](https://learn.microsoft.com/en-us/office/vba/language/reference/user-interface-help/cverr-function)
guide this extension of existing admitted query functions. Full native Variant,
NLS, precision and error-order qualification remains open.

Three expression tests cover source/option Empty targets, NULL/policy/domain/type
order, borrowed Text normalization under exact byte controls, every observed
successful cancellation checkpoint and once-only Rnd consumption after Error
data. Query/CLI tests add derived states and explicit conversions. The original
[independent runner](testing/compare_character_format_states.py) compares character
construction plus all three formatters with Python code-page/integer/Decimal
models and read-only native identity getters; only TableCraft executes SQL.
Its 528 controls pass (360 scalar and 168 four-family sequential/indexed cases)
with every source hash unchanged. Native Access/Windows, broader formatting
functions and complete Variant semantics remain open before further writes.
