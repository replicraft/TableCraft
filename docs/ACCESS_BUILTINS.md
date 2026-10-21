# Access built-in implementation tracker

Reviewed: 21 October 2026. Baseline: the committed implementations recorded in
[implementation status](IMPLEMENTATION.md).
Update this tracker with each built-in implementation or material scope change.

This tracks Access expression and SQL functions, including missing functions,
against executable TableCraft behavior. It also records application/VBA functions
separately so their deferral is visible. It is a coverage checklist, not an
exhaustive inventory of Access object-model methods, SQL syntax or VBA statements.

Access query expressions include functions supplied by the VBA expression service;
this tracker is not limited to standalone SQL grammar or engine-native functions.
Microsoft documents that [expression-service boundary](https://learn.microsoft.com/en-us/office/client-developer/access/desktop-database-reference/sql-expressions).
Implementing a closed expression function in Rust does not activate or execute
stored VBA modules, and does not change the VBA runtime deferral.

**Current implementation priority is direct Access query availability.** Add new
functions only with a specific Microsoft query example/contract or native execution
evidence. A function's presence in the broad Access/VBA catalog does not prove
that a query can call it. Split, Join and Filter are `Not planned`: their VBA
array behavior belongs to the standalone `vba-rs` project. Queries calling a VBA
wrapper depend on the deferred VBA/UDF integration, not direct built-in support. A
[first-hand Split query report](https://learn.microsoft.com/en-us/answers/questions/5014470/how-can-i-use-split-function-in-a-microsoft-access)
describes an undefined-function error and a VBA-wrapper workaround; this is a
specific reported outcome, not qualification of every Access version.

Recent [cashflow](CASHFLOW_FUNCTIONS.md) and [array/inspection](VALUE_INSPECTION.md)
implementations retain their documented unqualified SQL-admission limits. Their
Rust kernels and synthetic saved-query tests establish TableCraft execution,
not native query availability. Do not count those tests as Access query admission
evidence or expand those profiles until that boundary is established. Absence of
a query example alone does not prove that a function is unavailable.

## Status meanings

| Status | Meaning |
| --- | --- |
| Not yet | Intended TableCraft query functionality without a committed executable implementation. Parsing a call or documenting a plan does not count. |
| Not planned | Outside TableCraft's query implementation scope. VBA language/runtime functionality belongs to the standalone `vba-rs` project and its host integrations; this is not a TableCraft backlog item or a claim that Access rejects every such call. |
| Unconfirmed | Direct Access query availability has not been established. Keep outside the active implementation queue pending evidence; this is neither a planned implementation nor a finding that the function is unavailable. |
| Partial | An executable subset exists, but known argument, carrier, calendar, locale, context or semantic gaps remain. Notes describe the subset and gaps. |
| Full | The function's SQL expression behavior is implemented for the materialized values TableCraft can supply, with no known function-specific omission in that scope. This does not include VBA runtime states or imply native Access qualification. |

**Implementation and native compatibility are separate.** These statuses do not
establish Microsoft Access conformance. Windows Access comparisons remain pending
for the functions below; independent models and readers are separate evidence.
`Full` does not mean that every SQL expression around a function, every database
profile, or every VBA overload is supported. Known gaps stay `Partial` even when
the common case works. There is deliberately no overall completion percentage.

Scalar implementations live in
[`tablecraft-expressions`](../crates/query/tablecraft-expressions/src/functions/mod.rs);
SQL binding/execution lives in
[`tablecraft-query`](../crates/query/tablecraft-query/src/binding/calls.rs).
Unless noted, implemented scalar functions work in `query eval` and `query run`;
aggregates require `query run`. Qualified saved SELECTs use the same runtime,
subject to [saved-query limits](NATIVE_SAVED_QUERIES.md). Native inspection never
executes functions. General UPDATE function calls remain outside the current
[editing profile](QUERY_EDITING.md).

## NULL, selection and inspection

See [scalar contracts](EXPRESSIONS.md#explicit-isnull-and-iif-built-ins) and
[Switch/Choose](CONDITIONAL_FUNCTIONS.md).

| Built-in | Status | Notes |
| --- | --- | --- |
| IsNull | Full | Boolean test for stored NULL across materialized carriers; zero and empty text/binary are not NULL. Expression-only Empty/Missing/Error states are outside this SQL scope. |
| Nz | Partial | Query-context replacement; omitted fallback yields empty Text. Selected carrier is retained. VBA contextual Empty behavior and native result-type promotion are not implemented. |
| IIf | Partial | Eager operands, Boolean/numeric/NULL conditions and unchanged selected carrier. Text/date truth coercion and complete native result promotion are absent. |
| Switch | Partial | Eager condition/value pairs, first true selection and NULL when none match. Shares IIf's restricted truth conversion; native promotion/error precedence remains open. |
| Choose | Partial | Eager choices, rounded one-based numeric index and NULL outside the supported choice range. Full Variant coercion and native promotion remain open. |
| IsArray | Partial | [Explicit arrays and materialized Binary](VALUE_INSPECTION.md) are recognized without payload coercion. General Variant arrays, host bindings and native binary/SQL qualification remain open. |
| IsDate | Partial | [Explicit Gregorian date/time recognition](ISDATE.md), checked numeric/date carriers and FALSE for NULL. Missing policies and operational failures propagate; full regional/Variant behavior and native qualification remain open. |
| IsEmpty | Partial | [Explicit host-issued Empty](VALUE_INSPECTION.md) is distinct from NULL/empty Text; materialized SQL values return FALSE. Language variables and complete Variant/native behavior remain open. |
| IsError | Partial | [Explicit host-issued Error](VALUE_INSPECTION.md) recognition works; operational errors still fail. SQL CVErr and complete Variant/native behavior remain open. |
| IsMissing | Not planned | `vba-rs` owns [optional Variant procedure-argument inspection](https://learn.microsoft.com/en-us/office/vba/language/reference/user-interface-help/ismissing-function). SQL omitted argument slots are separate syntax and do not require a VBA Missing runtime. |
| IsNumeric | Partial | [Numeric/Boolean carrier and whole-Text recognition](ISNUMERIC.md) under explicit number formats. Operational failures propagate; complete locale/Variant grammar and native qualification remain open. |
| IsObject | Not planned | Object/Variant runtime inspection belongs to `vba-rs`; TableCraft does not implement the VBA object runtime. |
| TypeName | Partial | [Materialized carrier/state/array names](VALUE_INSPECTION.md), with metered Text output. GUID/extended DateTime reject guessed VBA names; objects, native promotion and wider arrays remain open. |
| VarType | Partial | [Named carrier/state subtype codes](VALUE_INSPECTION.md) and Variant/Byte array flags return Integer. Native-only carriers, objects, promotion and wider arrays remain open. |

## Text and formatting

See [text contracts](EXPRESSIONS.md#bounded-text-built-ins). Current strings are
valid Unicode; lossless unpaired-surrogate results are not representable.

| Built-in | Status | Notes |
| --- | --- | --- |
| Len | Partial | Text/NULL; length counts UTF-16 units. Numeric/storage-size overloads and full Variant coercion are absent. |
| LTrim | Partial | Text/NULL; removes leading U+0020 spaces. Implicit non-Text/Variant conversion is absent. |
| RTrim | Partial | Text/NULL; removes trailing U+0020 spaces. Implicit non-Text/Variant conversion is absent. |
| Trim | Partial | Text/NULL; removes U+0020 spaces at both ends. Implicit non-Text/Variant conversion is absent. |
| Left | Partial | Text/NULL and nonnegative integral Long-range count. Fractional/Variant count coercion and surrogate-splitting results are absent. |
| Right | Partial | Same count/carrier limits as Left; counts UTF-16 units. |
| Mid | Partial | One-based integral start and optional integral length. Full position coercion and surrogate-splitting results are absent; assignment form is outside SQL scope. |
| StrComp | Partial | [Explicit UTF-16 binary and host-supplied text/database modes](STRING_COMPARISON.md); no inferred locale/default, full Variant coercion or qualified native code-page/database collation. |
| InStr | Partial | [Bounded UTF-16 search](STRING_SEARCH.md), checked start and explicit host modes; General database search now supports full-weight expansion spans. Explicit LCID host dispatch now works; concrete locale providers, full coercion and native locale/code-page behavior remain open. |
| InStrB | Not yet | Byte-oriented substring search needs an explicit encoding contract. |
| InStrRev | Partial | [Reverse UTF-16 search](STRING_SEARCH.md) with full-match end bounds, binary default and explicit General database spans. Full coercion and native locale/code-page behavior remain open. |
| LCase | Partial | Text/NULL with explicit ICU language; bounded contextual Unicode output. Variant coercion, aliases and Windows/ACE mappings unqualified. [Contract](TEXT_CASING.md). |
| UCase | Partial | Text/NULL with explicit ICU language; bounded contextual Unicode output. Variant coercion, aliases and Windows/ACE mappings unqualified. [Contract](TEXT_CASING.md). |
| Replace | Partial | [Bounded suffix replacement](TEXT_REPLACEMENT.md), optional start/count/modes, metered output and actual unequal-length General database match spans. Full coercion, unpaired-surrogate results and native locale/default/error qualification remain open. |
| Space | Partial | [Bounded repeated spaces](TEXT_CONSTRUCTION.md) with checked numeric counts and NULL propagation. Full Variant coercion and native NULL/conversion/error qualification remain open. |
| String | Partial | [Bounded repetition](TEXT_CONSTRUCTION.md), first-unit selection, numeric modulo 256 and [explicit single-byte code pages](CHARACTER_CODE_PAGES.md). Native/DBCS mappings, unpaired surrogates and full coercion remain open. |
| StrReverse | Partial | [Bounded BMP code-unit reversal](TEXT_CONSTRUCTION.md); non-BMP results are refused. Full string coercion and native supplementary-character behavior remain unqualified. |
| StrConv | Not yet | Locale, case and encoding conversion modes. |
| LenB | Not yet | Byte-length overload; not an alias for UTF-8 length. |
| LeftB | Not yet | Byte-based slicing needs an encoding policy. |
| RightB | Not yet | Byte-based slicing needs an encoding policy. |
| MidB | Not yet | Byte-based slicing needs an encoding policy. |
| Asc | Partial | [Exact first-character single-byte encoding](CHARACTER_FUNCTIONS.md) using explicit pages; no best-fit/DBCS or full string coercion. |
| AscB | Not yet | Byte conversion needs an encoding policy. |
| AscW | Partial | [Signed first UTF-16 unit](CHARACTER_FUNCTIONS.md), including leading surrogates; Text only, full coercion/native behavior unqualified. |
| Chr | Partial | [Checked byte-code construction](CHARACTER_FUNCTIONS.md), explicit pages or ASCII-only default. DBCS/native mappings and full coercion remain open. |
| ChrB | Not yet | Byte-string construction needs a representation policy. |
| ChrW | Partial | [Checked signed/unsigned UTF-16 code units](CHARACTER_FUNCTIONS.md). Surrogate output is refused; full coercion and native qualification remain open. |
| Format | Not yet | Access formatting masks, sections, locale and calendar rules. CLI display formatting is not this function. |
| FormatCurrency | Partial | [Fixed-place currency output](STYLED_NUMBER_FORMATTING.md), exact fixed inputs, bounded normalized IEEE display and explicit profile/options. Full NLS/Variant, rounding and native qualification remain open. |
| FormatDateTime | Partial | [Five named formats](DATE_FORMATTING.md) under explicit Gregorian profiles, checked date input and bounded output. Full regional/Variant behavior, Text mode coercion and native precision/error qualification remain open. |
| FormatNumber | Partial | [Fixed-place numeric output](STYLED_NUMBER_FORMATTING.md), grouping/leading/sign options and 0–255 output places. Full NLS/Variant, rounding and native qualification remain open. |
| FormatPercent | Partial | [Percent display](STYLED_NUMBER_FORMATTING.md) with display-point scaling beyond stored carrier range, shared profiles/options and bounded output. Full NLS/Variant, rounding and native qualification remain open. |

Typed `$` string-returning variants are not implemented aliases. Supporting an
unsuffixed function does not establish support for its `$` variant.

## Numeric functions

See [numeric contracts](MATH_FUNCTIONS.md). Implemented functions accept explicit
numeric/Boolean carriers rather than complete Access text/date/Variant coercion.

| Built-in | Status | Notes |
| --- | --- | --- |
| Abs | Partial | Checked magnitude retaining numeric carrier; minimum signed values overflow instead of inferring native widening. |
| Sgn | Partial | Exact sign returns Int16 -1/0/1; numeric/Boolean/NULL profile only. |
| Int | Partial | Floor retaining supported numeric carrier; no text/date coercion or complete native promotion. |
| Fix | Partial | Truncation toward zero retaining supported numeric carrier; same coercion limits as Int. |
| Round | Partial | Half-even rounding; supported place count 0–28. Floating rounding/subtype behavior is an explicit profile awaiting native comparison. |
| Sqr | Partial | Checked numeric-to-Double square root; negative inputs fail. Full argument coercion is absent. |
| Exp | Partial | Double exponential with finite-result checks; native precision/error details remain unqualified. |
| Log | Partial | Double natural logarithm for positive inputs; full argument coercion is absent. |
| Sin | Partial | Double sine in radians; full argument coercion and native last-bit fidelity remain open. |
| Cos | Partial | Double cosine in radians; same coercion/precision limits as Sin. |
| Tan | Partial | Double tangent in radians; same coercion/precision limits as Sin. |
| Atn | Partial | Double principal arctangent; same coercion/precision limits as Sin. |
| Rnd | Not yet | Requires explicit random-state/seed semantics; no ambient generator is selected. |
| Partition | Not yet | Numeric range-bucket string construction. |
| EuroConvert | Not yet | No expression-call implementation of Access currency-conversion rules. |

`Randomize` is a VBA statement, not an implemented scalar function.

## Conversions

See [conversion contracts](EXPRESSIONS.md#checked-numeric-and-boolean-conversion-functions).
These are executable conversions, distinct from implicit arithmetic coercion or
native storage decoding. All implemented conversions reject explicit NULL.

| Built-in | Status | Notes |
| --- | --- | --- |
| CBool | Partial | Numeric zero/nonzero plus [explicit numeric Text/Boolean spellings](NUMBER_FORMAT.md); full regional/date/Variant behavior remains open. |
| CByte | Partial | Checked half-even conversion, including TRUE to 255, with [explicit numeric Text](NUMBER_FORMAT.md). Date/general regional coercion remains open. |
| CInt | Partial | Checked half-even Int16 conversion with [explicit numeric Text](NUMBER_FORMAT.md). Native regional/date behavior remains open. |
| CLng | Partial | Checked half-even Int32 conversion with [explicit numeric Text](NUMBER_FORMAT.md). Native regional/date behavior remains open. |
| CLngLng | Partial | Explicit Int64 conversion with [exact numeric Text](NUMBER_FORMAT.md), independent of host pointer width. Native host availability, regional/date behavior remain open. |
| CSng | Partial | Finite binary32 conversion and [direct numeric Text parsing](NUMBER_FORMAT.md). Full regional/date coercion and native qualification remain open. |
| CDbl | Partial | Finite binary64 conversion and [numeric Text parsing](NUMBER_FORMAT.md). Full regional/date coercion and native qualification remain open. |
| CCur | Partial | Scale-four half-even conversion and [exact numeric Text](NUMBER_FORMAT.md). Full regional/date behavior and native rounding/order qualification remain open. |
| CDec | Partial | 96-bit/scale-28 conversion and [exact numeric Text](NUMBER_FORMAT.md). Full regional/date behavior and native precision/rounding qualification remain open. |
| CDate | Partial | [Checked serial/date and explicit Text conversion](DATE_CONVERSIONS.md), preserving existing DATE bits. Full regional/Variant and native precision/error behavior remain open. |
| CStr | Partial | [Text/Boolean and explicit numeric/date output policies](CSTR.md). Exact fixed values are retained; full regional/Variant behavior, float/date precision and native qualification remain open. |
| CVar | Not yet | Variant subtype conversion is outside the materialized-value model. |
| CVDate | Not yet | No Variant Date conversion alias. |
| CLngPtr | Not planned | Pointer-width-dependent VBA conversion belongs to `vba-rs`, not the query value model. |
| CVErr | Not yet | No Variant Error construction; evaluation failures remain typed errors. |
| Str | Partial | [Numeric spelling and sign spaces](NUMERIC_TEXT.md), preserving exact fixed carriers. Native float notation, general coercion and regional/date behavior remain open. |
| Val | Partial | [Bounded numeric prefixes and checked radix/type suffixes](VAL.md). Finite Double result profile; broader coercion, native subtypes and error qualification remain open. |
| Hex | Partial | [Checked numeric-to-hex spelling](NUMERIC_TEXT.md), including explicit Int64 inputs. Native negative widths, Text/date coercion and error qualification remain open. |
| Oct | Partial | [Checked numeric-to-octal spelling](NUMERIC_TEXT.md), including explicit Int64 inputs. Native negative widths, Text/date coercion and error qualification remain open. |
| GUIDFromString | Not yet | Core GUID parsing is not an expression implementation of this Access method. |
| StringFromGUID | Not yet | Core GUID display is not an expression implementation of this Access method. |

## Date and time

See [component extraction](EXPRESSIONS.md), [DateAdd/DateDiff/DatePart](EXPRESSIONS.md)
and [constructors](DATE_CONSTRUCTORS.md). Current calendar operations are
Gregorian. DATE serial operators are tracked separately in
[DATE_ARITHMETIC.md](DATE_ARITHMETIC.md).

| Built-in | Status | Notes |
| --- | --- | --- |
| Year | Partial | DateTime/ExtendedDateTime/NULL extraction; no text/numeric coercion or alternate calendar. |
| Month | Partial | Same carrier/calendar limits as Year. |
| Day | Partial | Same carrier/calendar limits as Year. |
| Hour | Partial | Explicit date carriers/NULL; legacy fraction-rounding policy needs native qualification. |
| Minute | Partial | Same carrier/rounding limits as Hour. |
| Second | Partial | Same carrier/rounding limits as Hour. |
| DateAdd | Partial | Bounded Gregorian interval arithmetic over supported date carriers; full coercion/alternate calendar support absent. |
| DateDiff | Partial | Signed Gregorian interval-boundary counts with [explicit/system host week settings](WEEK_SETTINGS.md). Non-Sunday w anchors, full coercion and native/alternate-calendar behavior remain open. |
| DatePart | Partial | Gregorian parts and [caller-issued system week settings](WEEK_SETTINGS.md), preserving omitted defaults and the documented final-Monday anomaly. Native/alternate-calendar behavior remains unqualified. |
| DateSerial | Partial | Normalized numeric Gregorian constructor with explicit two-digit-year window; Hijri and complete native coercion/edge behavior remain open. |
| TimeSerial | Partial | Checked Int16 arguments and signed second/day carries into legacy DATE; native negative-carry details remain unqualified. |
| DateValue | Partial | [Date extraction and explicit Gregorian Text parsing](DATE_CONVERSIONS.md), with caller-issued omitted year. Full locale/calendar/Variant and native NULL/error behavior remain open. |
| TimeValue | Partial | [Time extraction and explicit 12/24-hour Text parsing](DATE_CONVERSIONS.md), validating discarded dates. Full regional/Variant and native precision/error behavior remain open. |
| MonthName | Partial | [Explicit English/German Gregorian full/short names](CALENDAR_NAMES.md), checked numeric ordinal/abbreviation operands and bounded output. Broader locales/calendars, Variant/Text coercion and native qualification remain open. |
| Weekday | Partial | [Standalone Gregorian numbering](WEEKDAY.md), checked numeric/Text/date inputs, optional first day and caller-issued system context; NULL dates propagate. Full native NLS/Variant/calendar, precision and error behavior remain unqualified. |
| WeekdayName | Partial | [Explicit English/German full/short names](CALENDAR_NAMES.md), all first-day offsets, host-issued system/default context and SQL overrides. Broader locales/calendars, Variant/Text coercion and native NLS qualification remain open. |
| Date | Partial | [Host-issued clock function](CLOCK_FUNCTIONS.md), zero-argument calls, whole-second local Gregorian readings and request-local providers. Native timing/precision/calendar/platform qualification remains open. |
| Now | Partial | [Host-issued clock function](CLOCK_FUNCTIONS.md), zero-argument calls, whole-second local Gregorian readings and request-local providers. Native timing/precision/calendar/platform qualification remains open. |
| Time | Partial | [Host-issued clock function](CLOCK_FUNCTIONS.md), zero-argument calls, whole-second local Gregorian readings and request-local providers. Native timing/precision/calendar/platform qualification remains open. |
| Timer | Partial | [Explicit clock-backed Single seconds since midnight](CLOCK_FUNCTIONS.md), fractional fixed/system readings and bounded scalar/bound retrieval work. Microsoft supplies a [query example](https://support.microsoft.com/en-gb/access/functions/timer-function). Native resolution/rounding, invocation timing and platform differences remain unqualified; separate from form Timer events and stored VBA execution. |

## SQL aggregates

See [aggregation rules](QUERY_EXECUTION.md). These operate on bound scalar inputs,
not implicit domain lookups or arbitrary complex fields. Native result-type
promotion is not complete.

| Built-in | Status | Notes |
| --- | --- | --- |
| Count | Partial | Count(*) and Count(expr), grouping and empty/all-NULL behavior. Scalar execution profile and resource/result limits apply. |
| Sum | Partial | Bounded supported numeric folds, including Decimal; full Currency/date/Variant promotion is absent. |
| Avg | Partial | Supported numeric averaging and NULL elimination; full native accumulation/promotion is absent. |
| Min | Partial | Supported comparable scalar carriers; text requires explicit host collation, native locale ordering is not supplied. |
| Max | Partial | Same comparison/collation limits as Min. |
| First | Partial | First encountered scalar, including NULL. Native scan/index encounter order is not guaranteed to match Access. |
| Last | Partial | Last encountered scalar, including NULL. Same encounter-order limits as First. |
| StDev | Partial | Sample deviation over finite numeric values; two known values required. [Bounded Double profile](STATISTICAL_AGGREGATES.md); native accumulation/promotion/precision qualification remains open. |
| StDevP | Partial | Population deviation over finite numeric values; singleton returns Double zero. [Bounded Double profile](STATISTICAL_AGGREGATES.md); native accumulation/promotion/precision qualification remains open. |
| Var | Partial | Sample variance over finite numeric values; two known values required. [Bounded Double profile](STATISTICAL_AGGREGATES.md); native accumulation/promotion/precision qualification remains open. |
| VarP | Partial | Population variance over finite numeric values; the documented two-value minimum is retained. [Bounded Double profile](STATISTICAL_AGGREGATES.md); native accumulation/promotion/precision qualification remains open. |

## Domain functions

All require explicit domain resolution and criteria execution contracts. Ordinary
SELECT aggregates and subqueries do not automatically implement these functions.

| Built-in | Status | Notes |
| --- | --- | --- |
| DAvg | Partial | Domain average. See [domain profile](DOMAIN_FUNCTIONS.md); broader host/criteria/coercion and native qualification remain. |
| DCount | Partial | Domain count. See [domain profile](DOMAIN_FUNCTIONS.md); broader host/criteria/coercion and native qualification remain. |
| DFirst | Partial | First domain value. See [domain profile](DOMAIN_FUNCTIONS.md); broader host/criteria/coercion and native qualification remain. |
| DLast | Partial | Last domain value. See [domain profile](DOMAIN_FUNCTIONS.md); broader host/criteria/coercion and native qualification remain. |
| DLookup | Partial | Domain value lookup. See [domain profile](DOMAIN_FUNCTIONS.md); broader host/criteria/coercion and native qualification remain. |
| DMin | Partial | Domain minimum. See [domain profile](DOMAIN_FUNCTIONS.md); broader host/criteria/coercion and native qualification remain. |
| DMax | Partial | Domain maximum. See [domain profile](DOMAIN_FUNCTIONS.md); broader host/criteria/coercion and native qualification remain. |
| DStDev | Partial | Domain sample standard deviation. See [domain profile](DOMAIN_FUNCTIONS.md); broader host/criteria/coercion and native qualification remain. |
| DStDevP | Partial | Domain population standard deviation. See [domain profile](DOMAIN_FUNCTIONS.md); broader host/criteria/coercion and native qualification remain. |
| DSum | Partial | Domain sum. See [domain profile](DOMAIN_FUNCTIONS.md); broader host/criteria/coercion and native qualification remain. |
| DVar | Partial | Domain sample variance. See [domain profile](DOMAIN_FUNCTIONS.md); broader host/criteria/coercion and native qualification remain. |
| DVarP | Partial | Domain population variance. See [domain profile](DOMAIN_FUNCTIONS.md); broader host/criteria/coercion and native qualification remain. |

## Financial functions

| Built-in | Status | Notes |
| --- | --- | --- |
| DDB | Partial | Declining-balance depreciation. See [depreciation profile](DEPRECIATION.md); bounded Double kernels, factor defaults and saved retrieval work; native boundaries/precision remain unqualified. |
| FV | Partial | Future value. See [financial profile](FINANCIAL_FUNCTIONS.md); numeric Double kernel, optional slots and saved retrieval work; full coercion/precision/native qualification remain. |
| IPmt | Partial | Interest component of a payment. See [period/payment profile](PERIOD_PAYMENTS.md); bounded Double kernels and saved retrieval work; coercion/precision/native qualification remain. |
| IRR | Partial | [Explicit numeric cashflows](CASHFLOW_FUNCTIONS.md), optional guess and bounded residual-qualified iteration work; general Variant arrays, native SQL admissibility and solver/coercion equivalence remain open. |
| MIRR | Partial | [Explicit numeric cashflows](CASHFLOW_FUNCTIONS.md), separate finance/reinvestment rates and retained periods work; general arrays and native numeric/coercion qualification remain open. |
| NPer | Partial | Number of payment periods. See [period/payment profile](PERIOD_PAYMENTS.md); bounded Double kernels and saved retrieval work; coercion/precision/native qualification remain. |
| NPV | Partial | [Period-one discounting](CASHFLOW_FUNCTIONS.md) over explicit numeric cashflows works; general arrays, native sign/rate boundaries and coercion/rounding remain open. |
| Pmt | Partial | Periodic payment. See [financial profile](FINANCIAL_FUNCTIONS.md); numeric Double kernel, optional slots and saved retrieval work; full coercion/precision/native qualification remain. |
| PPmt | Partial | Principal component of a payment. See [period/payment profile](PERIOD_PAYMENTS.md); bounded Double kernels and saved retrieval work; coercion/precision/native qualification remain. |
| PV | Partial | Present value. See [financial profile](FINANCIAL_FUNCTIONS.md); numeric Double kernel, optional slots and saved retrieval work; full coercion/precision/native qualification remain. |
| Rate | Partial | [Bounded rate solver](RATE.md), optional guess, residual checks and shared iteration budgets work; native basins/coercion/precision remain unqualified. |
| SLN | Partial | Straight-line depreciation. See [depreciation profile](DEPRECIATION.md); bounded Double kernels, factor defaults and saved retrieval work; native boundaries/precision remain unqualified. |
| SYD | Partial | Sum-of-years depreciation. See [depreciation profile](DEPRECIATION.md); bounded Double kernels, factor defaults and saved retrieval work; native boundaries/precision remain unqualified. |

## Arrays and application/runtime functions

These remain visible for completeness without changing the
[VBA deferral](VBA_COMPATIBILITY.md). Host-facing functions must not gain implicit
filesystem, process, network, UI or automation access through query evaluation.
The following grouped rows share a status; split them when implementations differ.
`Not planned` runtime entries remain here as scope records, not unfinished query
work. Their classification does not defer Access-native macros, non-VBA actions
or future application UI services.

| Built-in | Status | Notes |
| --- | --- | --- |
| Array | Partial | Direct scalar-element Array/VBA.Array inputs feed [inspection/bounds](VALUE_INSPECTION.md) and [cashflows](CASHFLOW_FUNCTIONS.md). Bounded owned expression intermediates retain explicit Long bounds; general results/parameters, nested/typed/multidimensional arrays and full Variant behavior remain open. |
| Filter | Not planned | VBA array filtering belongs to `vba-rs`; SQL WHERE filtering is separate. VBA wrapper execution requires the deferred VBA/UDF integration. |
| Join | Not planned | VBA array-to-string joining belongs to `vba-rs`; VBA wrapper execution is deferred. SQL JOIN syntax is separate and already implemented under its own profile. |
| Split | Not planned | VBA string-to-array splitting belongs to `vba-rs`. Access query use requires a VBA wrapper; that execution remains deferred. |
| LBound | Partial | [Single-dimensional explicit Variant/Byte inputs](VALUE_INSPECTION.md), Long results and optional dimension work. Wider arrays/module defaults and native edge behavior remain open. |
| UBound | Partial | Same [bounds profile](VALUE_INSPECTION.md), including retained empty extents. Wider arrays and native empty/coercion/error behavior remain open. |
| Eval | Not yet | Microsoft documents [direct query examples](https://support.microsoft.com/en-au/access/functions/eval-function), including evaluating a numeric expression string. Not VBA-only. Dynamic expression evaluation is unimplemented; form/control and user-VBA resolution remain deferred host/runtime capabilities. CLI query eval is a separate command, not this function. |
| HyperlinkPart | Not yet | No hyperlink-component expression function. |
| CurrentUser | Not yet | No Access application/user-security context. |
| RGB | Not yet | No packed-color expression function. |
| QBColor | Not yet | No indexed-color expression function. |
| Error | Not yet | Error(number) returns message Text; it does not raise an error or construct CVErr. Microsoft's [query sandbox list](https://support.microsoft.com/en-us/access/functions-and-properties-in-access-blocked-by-sandbox-mode) includes Error/Error$, although the [function page](https://support.microsoft.com/en-gb/access/functions/error-function) says sandbox disables it; that conflict needs native qualification. Explicit-number lookup/localization is unimplemented. Omitted-number lookup depends on the latest VBA runtime error and remains deferred with `vba-rs`; TableCraft operational errors are not that state. |
| Command, Shell | Not planned | `vba-rs` and its process host integrations; no implicit command-line or process access from queries. |
| CreateObject, GetObject, CallByName | Not planned | `vba-rs` object runtime and optional COM providers; no automatic activation. |
| DDE, DDEInitiate, DDERequest, DDESend | Not planned | External application/runtime integration is outside the query backend; any future VBA-facing provider belongs at the `vba-rs` host boundary. |
| EOF, FreeFile, Input, Loc, LOF, Seek | Not planned | `vba-rs` file-channel runtime; ordinary TableCraft database I/O does not implement these calls. |
| CurDir, Dir, FileAttr, FileDateTime, FileLen, GetAttr | Not planned | `vba-rs` filesystem runtime and explicitly supplied host access. |
| Environ, GetAllSettings, GetSetting | Not planned | `vba-rs` environment/settings host access. |
| InputBox, MsgBox | Not planned | `vba-rs` dialog host integration; future TableCraft UI dialogs do not make these query built-ins. |
| DoEvents | Not planned | `vba-rs` runtime event-loop integration. |
| IMEStatus, MacID, MacScript | Not planned | `vba-rs` platform-specific runtime/host functionality. |
| Spc, Tab | Not planned | `vba-rs` output-context functions. |

## Sources and maintenance

The inventory uses Microsoft's
[Access functions by category](https://support.microsoft.com/en-au/access/functions/access-functions-by-category)
and [Access expression function categories](https://learn.microsoft.com/en-us/office/vba/access/concepts/criteria-expressions/functions-category-list),
with conversion/Unicode variants named explicitly. The supplied
[W3Schools Access reference](https://www.w3schools.com/sql/sql_ref_msaccess.asp)
is an additional coverage checklist, not the authority for edge semantics.
Function-specific Microsoft references and observed checks belong in the linked
feature contracts. The current `IsNull` scope follows Microsoft's
[NULL predicate contract](https://learn.microsoft.com/en-us/office/vba/language/reference/user-interface-help/isnull-function).

`Concat` on the W3Schools checklist describes the `&` operator, not a delivered
`Concat(...)` call. Operators, LIKE, SQL clauses and date literals have separate
[expression](EXPRESSIONS.md), [pattern](PATTERN_MATCHING.md) and
[parser](SQL_PARSER.md) contracts. Accepting a function spelling in SQL syntax,
reading a native cached result, decoding a field or rendering CLI output is not
an implementation of that callable function.

When changing a row, check the closed scalar dispatch and query aggregate binding,
record supported inputs and known gaps, link focused tests/feature evidence and
keep the baseline current. Promote to `Full` only after known function-specific
gaps in the stated SQL scope are addressed. Record actual native Access evidence
separately; documentation-derived goldens and independent-reader comparisons
must not be relabeled as Windows qualification.
