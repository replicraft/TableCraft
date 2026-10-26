# Format masks and sections

Format now executes bounded numeric, Gregorian date-mask and [text-layout](TEXT_FORMAT_MASKS.md) profiles in scalar and bound queries.
The [Access function reference](https://support.microsoft.com/en-us/access/functions/format-function)
establishes query use, optional mask and week arguments. This is an expression
function, separate from CLI table/TSV presentation. It does not execute VBA.

## Working profile

Custom masks support required `0` digits, optional `#` digits, a decimal point,
grouping commas and one percentage operator, with literals between numeric
positions. [Scientific notation and whole-part scaling commas](SCIENTIFIC_FORMATTING.md)
share this renderer and its exact/normalized display magnitude. Whole digits
align from the right; a required slot keeps the numeric positions to its right
visible. Required fractional slots can force preceding optional fractional
positions to appear. Extra whole digits are retained immediately before the first
numeric slot. Optional trailing fractional zeros disappear; an explicit decimal
placeholder is emitted even when no fractional digit remains. Literal text stays
in mask order and consumes no digit positions. Grouping uses the existing
three-digit explicit profiles. Commas after fractional slots remain unsupported.
Fractional masks have a 255-digit ceiling shared with the numeric display profile. The
[numeric formatting reference](https://support.microsoft.com/en-us/access/format-a-number-or-currency-field)
describes placeholder/grouping semantics; native edge behavior remains unqualified.

Quoted fragments and backslash-escaped characters are literal. Quoted/escaped
percent signs do not scale values, and their semicolons do not split sections.
Prefix/suffix punctuation is preserved. Unrecognized formatting instructions
fail rather than becoming plausible literal output.

The [function's section rules](https://learn.microsoft.com/en-us/office/vba/language/reference/user-interface-help/format-function-visual-basic-for-applications)
select positive/negative/zero/NULL sections, up to four. A missing non-NULL section
falls back to the positive mask. A selected negative section formats the magnitude;
the single/positive fallback path supplies an automatic minus for nonzero rounded
negative values. The fourth section currently supports literal NULL output;
without it NULL produces empty Text. Selection uses the original value before
rounding. Native omitted-section and rounded-negative-zero behavior still needs
qualification.

Masks truncate at 257 UTF-16 units. The borrowed lexer checks quoting and section
count across that prefix; selected formatting instructions are then interpreted.
A truncation boundary splitting a supplementary character fails explicitly.
Trailing escapes, unterminated quotes and excessive sections fail. Malformed or
unsupported instructions in unselected sections have no broader native
qualification; they are never executed as expressions.

Named Fixed, Standard, Percent and Currency route through the numeric mask path.
Currency supports the existing en-US/de-DE profiles. Yes/No, True/False and On/Off
use explicit English spellings for numeric zero/nonzero values. General Number
uses the existing numeric conversion spelling. Omitted/empty masks share CStr's
current profile, except NULL produces empty Text. Standalone `<`/`>` Text masks
share the explicit Unicode casing profile; other Text layouts use the
[bounded placeholder profile](TEXT_FORMAT_MASKS.md).

```sh
tablecraft query eval "SELECT Format(1234.5,'#,##0.00')" --number-format en-US
tablecraft query run "SELECT Format(-12,'0.00;(0.00)')" --number-format en-US
```

Results are `1,234.50` and `(12.00)`.

[Date/time masks and named styles](DATE_FORMAT_MASKS.md) now share Jiff calendar
conversion, explicit regional names/separators and the existing week rules.
Intrinsic numeric date fields work without regional discovery; Text date input
and localized fragments still require explicit profiles. Non-NULL date masks
currently require one section, and broader calendar/native behavior remains open.

## Policies and integration

Numeric output and numeric Text input require an explicit NumberFormat when
needed. The existing invariant/en-US/de-DE policies determine separators; an
invariant grouping request fails instead of inventing a separator. Literal NULL
output and Boolean named styles need no numeric formatting policy. No system
locale, date policy or database collation is inferred from a mask.

Numeric resolution reuses the same Rust Decimal and finite Single/Double display
magnitude as FormatNumber/Percent/Currency. That helper now lives in the numeric
format slice, with no arithmetic change. Exact fixed inputs use the established
half-away rounding profile; IEEE output retains its existing bounded significant
digits. These are TableCraft policies, not native precision/rounding evidence.

Written operands evaluate eagerly. Normalization preserves a missing mask as
empty Text, distinct from a written NULL; interior missing first-day slots use
the documented Sunday default. Optional week settings are checked against the
existing typed ranges, and explicit system-zero requests need supplied host
settings. Numeric masks do not otherwise use those settings. Broader native
NULL/coercion/option-validation precedence remains unfinished.

Binding retains ordinary TextCall operands and does not execute masks or scan
rows to prepare a schema. Execution-local policies survive fields, parameters,
aggregate arguments, correlations and grouping checks. Resolution uses bounded
private conversion scratch; rendering charges work and fallible output allocation
before publication. Text/mask ceilings, cumulative budgets and cancellation remain
active. Errors produce no partial CLI stdout.

## Remaining work and evidence

Format remains **Partial**. Broader cross-type conversion and native
scientific/scaling behavior remain implementation or qualification work. Text
`@`/`&`/`!` layouts and combined casing now have an explicit bounded profile;
conflicting directives and native casing/layout behavior remain unqualified.
Broader Variant/raw-string carriers, calendars, regional formats and native
Access precision/error/section qualification also remain open. Signed/mixed date
sections and numeric formatting of Extended dates remain unfinished. Date output through an
omitted mask inherits CStr's existing explicit date profile.

Six kernel controls cover masks, named styles, literals, section routing,
truncation/refusals and a fixed-seed 256-case integer-padding model. Three scalar
and three query controls cover optional slots, error order, parameters/policies,
group/aggregate/correlated dependencies, output limits and every observed
cancellation checkpoint. CLI controls exercise both commands and buffered errors.
The [independent runner](../../testing/compare_format.py) passes 2570 Decimal/model,
section/refusal and native Int32/NULL-field controls across the four original
Hyperlink fixtures, with unchanged hashes. Jackcess supplies values without
executing Format; actual Microsoft Access evaluation remains unrun.

## Numeric display of legacy Date/Time serials

Numeric masks and General Number now accept stored legacy Date/Time values. The
[Format function reference](https://learn.microsoft.com/en-us/office/vba/language/reference/user-interface-help/format-function-visual-basic-for-applications)
explicitly allows numeric formatting of date/time serials. The renderer uses the
stored OLE Double serial, without rounding through a calendar or canonicalizing
negative fractions. For example, a stored `-0.25` renders as `-0.25`, even though
its calendar interpretation coincides with positive `0.25`.

Fixed, scientific, percent, currency and sign/zero sections reuse the current
numeric display machinery and explicit NumberFormat. A DateTextFormat is not
required for a numeric mask. General Number uses the existing 15-significant-digit
Double spelling. Raw stored bits and ordinary date browsing remain unchanged;
this adds no implicit date-to-number arithmetic/storage conversion. Str still
rejects Date/Time, and CStr/omitted-mask date output still needs a date profile.
Extended dates retain their separate calendar path and do not receive an
implicit lossy numeric conversion. Signed date-token/mixed numeric-date sections,
other coercions and native precision/formatting qualification remain open.

Forty-four SQL-enabled Format-related kernel/evaluation tests, 32 separately run
parser-free Format-related tests and six focused query/CLI tests pass. The
original `testing/compare_format_serials.py` supplies 252 independent Decimal and
calendar/refusal/native-field controls, including exact quarter-day fractions,
pre-epoch dates and explicit en-US/de-DE policies. Independent Jackcess getters
supply the existing V2000/V2007/V2016/V2019 ScalarSamples timestamps; original
file hashes remain unchanged. These are independent engineering checks, not
native Access Format results. Clippy and formatting checks pass.

## Interleaved numeric literals

The renderer now streams whole, fractional and scientific-exponent placeholders
and literal spans in their original mask order. Quoted/escaped text and the
supported bare punctuation consume no digit slots. Extra leading digits precede
the first numeric placeholder; optional-slot suppression never removes adjacent
literal text. The percentage operator prints at its written position while
retaining the existing one-operator scaling rule. Quoted/escaped percent signs
remain literal. Required whole positions may be followed by optional positions;
for example, the current positional rule renders `1` with `0#` as `01`.

| Value | Mask | en-US output |
| ---: | --- | --- |
| 12 | `00-00` | `00-12` |
| 12345 | `00-00` | `123-45` |
| 12.345 exact Decimal | `0.0"x"0` | `12.3x5` |
| 12 | `0%0` | `120%0` |
| 12345 | `0.00E+0"x"0` | `1.23E+0x4` |

The scanner retains borrowed mask data and bounded scalar metadata, with no heap
token list or new arithmetic implementation. The renderer uses the existing
Decimal/IEEE rounding result and fixed stack buffers for coefficient/exponent
spelling. Mask scanning, digit/literal emission, output bytes, work ceilings and
cancellation remain metered. Exponent digits wider than their mask retain all
leading digits. An exponent marker/sign still requires an immediately following
digit placeholder; subsequent slots may be separated by literals. Repeated
percent operators, fractional scaling commas and unknown instructions fail.
These are executable engineering rules; native literal-placement/rounding,
locale/error-order and broader section/coercion qualification remain open.

Forty-eight SQL-enabled Format-related tests, 35 separately run parser-free
Format-related tests and 17 query/CLI mask tests pass. A generated whole-number
layout property checks independent decimal padding/slicing. Focused controls
check exact literal UTF-8 charges and every observed cancellation checkpoint,
including literals among exponent digits. Clippy and formatting checks pass.
The original `testing/compare_format_literals.py` uses independent Decimal
rounding and digit layouts with inert Jackcess field getters on the existing
ScalarSamples V2000/V2007/V2016/V2019 files; all original hashes stay unchanged.
The new runner passes 816 controls; the existing fixed numeric and scientific
comparison recipes pass 2,470 and 1,432 controls respectively.
These controls do not execute native Access SQL or qualify Windows NLS.

## Ordered expression-state admission

Format now retains eager expression states through its existing validation stages.
A NULL supplied week option is refused before week conversion; then first-day
and first-week controls validate progressively, followed by mask admission and
source admission. For example, `Format(CVErr(11),CVErr(12),8,NULL)` reports
InvalidNullConversion, while the same call without the final NULL reports Domain
for first-day 8 before admitting mask/source Error data. Mask grammar still
resolves at its existing rendering stage. These are preserved TableCraft rules,
not native Access error-precedence qualification.

Error data gives TypeMismatch and Missing gives NonScalarExpression at its
admission point. Explicit `Format(CLng(CVErr(11)),'0')` and
`Format(CStr(CVErr(11)),'@')` retain their separate numeric/string meanings. All
written argument expressions remain eager and once-only: a late division error
still stops before any retained Error value reaches these gates.

Host Empty mask values select empty Text. Host Empty week controls select numeric
zero and require the existing caller-issued system week settings; they do not
choose Sunday/Jan-1 silently. An Empty source with an omitted/empty mask uses
the existing blank string target. **Masked host Empty sources remain unqualified
and explicitly unsupported.** The reviewed Format contract does not establish
which numeric, text, date or section interpretation should be used for every
mask. This implementation does not replace that state with zero or stored NULL.
Native comparisons or a more specific contract are still needed. SQL Nz(NULL)
retains its existing empty Text identity; no Empty constructor is added.

Materialized and state-aware callers share the same borrowed preparation
service. It requests week controls, mask and source at their owning stages;
source/mask buffers are not copied into a replacement argument list. The existing
numeric/date/text recipes and charged materializer retain source normalization,
per-Text and cumulative byte/work/cancellation limits. No mask grammar, locale
fallback, return category, dependency or runtime is added.

The [MS-VBAL Format contract](https://learn.microsoft.com/en-us/openspecs/microsoft_general_purpose_programming_languages/ms-vbal/c2022d53-37dc-4597-8321-728e385afc92)
defines argument roles, defaults and format families. The shared
[conversion basis](../text/STRCONV.md#ordered-expression-state-admission) separates
explicit Error conversion and Empty targets. Neither establishes full native
Variant/mask-specific behavior; Format remains Partial.

Four expression tests cover NULL/week/mask/source order, supported versus
unqualified Empty targets, exact six-byte Text normalization/output, every
observed successful cancellation checkpoint and once-only Rnd controls. New
query/CLI tests add derived states, explicit conversion and atomic failures.
The original [state runner](../../testing/compare_format_states.py) passes 186 checks
(130 scalar and 56 four-family sequential/indexed native cases), retaining all
source hashes. The existing numeric Format runner also passes 2,470 scalar
checks; its optional native branch was not rerun. Only TableCraft executes SQL.
Full Variant, masked Empty, wider masks/NLS and Windows/native Access remain open.
