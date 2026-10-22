# Format masks and sections

Format now executes bounded numeric, Gregorian date-mask and [text-layout](TEXT_FORMAT_MASKS.md) profiles in scalar and bound queries.
The [Access function reference](https://support.microsoft.com/en-us/access/functions/format-function)
establishes query use, optional mask and week arguments. This is an expression
function, separate from CLI table/TSV presentation. It does not execute VBA.

## Working profile

Custom masks support a contiguous fixed numeric body with required `0` digits,
optional `#` digits, a decimal point, grouping commas and one percentage operator.
Required whole digits follow optional whole digits; fractional required digits
can force preceding optional fractional positions to appear. Extra whole digits
are retained. Optional trailing fractional zeros are removed; an explicit decimal
placeholder is emitted even when no fractional digit remains. Grouping uses the
existing three-digit explicit profiles. Interleaved numeric/literal digit patterns
and scaling commas remain unsupported. Fractional masks have a 255-digit ceiling
shared with the current numeric display profile. The
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
share the explicit Unicode casing profile; other Text placeholders are unfinished.

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

Format remains **Partial**. Scientific notation, scaling commas, interspersed
numeric literals and broader cross-type conversion remain implementation work. Text
`@`/`&`/`!` layouts and combined casing now have an explicit bounded profile;
conflicting directives and native casing/layout behavior remain unqualified.
Broader Variant/raw-string carriers, calendars, regional formats and native
Access precision/error/section qualification also remain open. Signed/mixed date
sections and date-to-numeric formatting remain unfinished. Date output through an
omitted mask inherits CStr's existing explicit date profile.

Six kernel controls cover masks, named styles, literals, section routing,
truncation/refusals and a fixed-seed 256-case integer-padding model. Three scalar
and three query controls cover optional slots, error order, parameters/policies,
group/aggregate/correlated dependencies, output limits and every observed
cancellation checkpoint. CLI controls exercise both commands and buffered errors.
The [independent runner](testing/compare_format.py) passes 2570 Decimal/model,
section/refusal and native Int32/NULL-field controls across the four original
Hyperlink fixtures, with unchanged hashes. Jackcess supplies values without
executing Format; actual Microsoft Access evaluation remains unrun.
