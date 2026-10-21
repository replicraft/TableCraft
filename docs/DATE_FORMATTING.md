# FormatDateTime named output

Unqualified `FormatDateTime(date, [named_format])` now runs in scalar SELECT,
bound/nested retrieval and qualified saved SELECT execution. The optional mode
defaults to zero; an explicitly omitted final slot uses the same default. Written
arguments remain eager. Invalid arities, qualifiers, wildcards and missing required
slots fail before operand execution. No VBA content executes.

| Mode | Meaning | Explicit current output |
| --- | --- | --- |
| 0 | General date | Short date and/or long time, sharing CStr's component selection. The Automation epoch date emits time only; other midnight values emit date only. |
| 1 | Long date | en-us: `Tuesday, October 20, 2026`; de-de: `Dienstag, 20. Oktober 2026`; iso retains `2026-10-20`. |
| 2 | Short date | en-us `10/20/2026`; de-de `20.10.2026`; iso `2026-10-20`. The epoch date is included. |
| 3 | Long time | en-us `4:28:14 PM`; de-de/iso `16:28:14`. |
| 4 | Short time | Intrinsic 24-hour `16:28`, regardless of the selected profile. |

These are explicit Gregorian profiles, not a claim about every Windows regional
setting. ISO has no separate verbose date pattern in this profile. English long
names use Jiff; German month/weekday spellings belong to the fixed German profile.
No locale, clock or timezone is read. Modes 0–3 require `--date-format`; mode 4
needs no output profile. Text date input still requires an explicit input profile,
even for mode 4.

The first input accepts legacy/extended dates or the
[checked CDate input profile](DATE_CONVERSIONS.md), including numeric/Boolean
carriers and explicit-policy Text. Existing extended dates retain their components
without legacy narrowing. Text uses the reference year and two-digit window;
optional numeric Text fallback uses the separate NumberFormat. NULL raises
InvalidNullConversion. Binary/GUID inputs fail. The mode uses checked numeric-to-Long
half-even conversion, then must be 0–4. NULL, Text mode coercion and other unsupported
carriers are not hidden defaults.

Output shares CStr's current nearest-second half-expand rounding, including checked
day carry; mode 4 removes seconds after that rounding and does not round to a minute.
The current profile rounds before all mode projections. Native formatting precision,
carry, coercion, NULL and error precedence remain unqualified. Formatting the last
extended calendar instant can overflow if rounding would enter year 10000.
Formatting is separate from temporal SQL comparison and CLI transport rendering.

```sh
tablecraft query eval "SELECT FormatDateTime(#2026-10-20 16:28:14#,1)" --date-format en-us
tablecraft query run "SELECT FormatDateTime('Feb 29',2)" --date-format de-de --reference-year 2024
tablecraft query eval "SELECT FormatDateTime(0.5,4)"
```

The expression results are `Tuesday, October 20, 2026`, `29.02.2024` and `12:00`.
CLI TSV retains its existing typed Text quoting inside CSV-escaped transport;
this function does not change that format.

## Ownership and failure boundaries

The parser-free text/date slice owns the typed mode, borrowed input recipe,
validated civil display and shared renderer. CStr retains its existing public
DateText export and General behavior. Numeric Text normalization has a separate
charged scratch buffer; its bytes cannot leak into output. Output is bounded,
fallible and cancellation-aware. Input-size/work/allocation failures propagate;
failed query/CLI execution publishes no partial stdout. No function authorizes
inert inspection, UPDATE calls, implicit native execution or predicate pushdown.

The existing typed Text node carries all arguments through binding, pruning,
grouping identity, aggregation and correlations. Input/output settings remain
request-local, including the optional worker. No concrete backend dependency,
new runtime, fixture or registry dependency is added.

## References and evidence

Microsoft's [FormatDateTime reference](https://learn.microsoft.com/en-us/office/vba/language/reference/user-interface-help/formatdatetime-function)
defines all five selectors, the default and intrinsic 24-hour short-time format;
its other named formats depend on regional settings. Full regional/calendar
formats, Variant/runtime states, Text selector coercion and native precision/error
qualification remain open. The [built-in tracker](ACCESS_BUILTINS.md) marks the
function **Partial**.

Kernel tests cover all modes/profiles, epoch rules, scratch isolation, numeric/Text
conversion, rounding carry, selectors, fixed-buffer long-date bounds and host errors.
Evaluator tests check byte/work/input-size and cancellation limits. Query, CLI and
worker regressions cover fields, groups, correlated calls, optional slots, execution
settings and recovery after missing context. The [independent recipe](testing/compare_date_format.py)
uses Python Gregorian/weekday models and optional sequential/indexed projections
over Jackcess-decoded input rows, checking that the source hash is unchanged.
Independent model/reader checks do not qualify native Access; Windows comparisons
remain unrun.
