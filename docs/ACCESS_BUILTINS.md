# Access built-in implementation tracker

Reviewed: 23 October 2026. Baseline: the committed implementations recorded in
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

Microsoft's [explicit sandbox query list](https://support.microsoft.com/en-us/access/functions-and-properties-in-access-blocked-by-sandbox-mode)
also establishes general query availability for Array, IRR/MIRR, IsEmpty/IsError,
TypeName/VarType, IsObject and RGB/QBColor. That is stronger evidence than the
broad function catalog, but does not qualify TableCraft's argument carriers,
coercion, results or native invocation behavior. Query availability is necessary
but not sufficient for inclusion: IsObject remains `Not planned` because its
object-reference semantics belong to the deferred VBA runtime.

The explicit [arithmetic declaration profile](DECLARED_ARITHMETIC.md#financial-date-and-inspection-signatures-23-october-2026)
now reviews financial/cashflow, date and inspection return signatures separately
from result carriers. CVErr also retains its formal Variant return through
qualified Error/Error comparisons. This does not complete the function-specific
gaps below.

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
| IsNull | Full | Boolean test for stored NULL across materialized carriers; zero and empty text/binary are not NULL. [Declared result](DECLARED_ARITHMETIC.md#like-and-null-check-declarations-23-october-2026) stays Scalar Boolean independently of the argument category. [CVErr Error data](VARIANT_ERRORS.md) is distinct from NULL; Empty/Missing remain outside this SQL scope. |
| Nz | Partial | Query-context replacement; omitted fallback yields empty Text. Selected carrier is retained. VBA contextual Empty behavior and native result-type promotion are not implemented. |
| IIf | Partial | [Eager selectors](CONDITIONAL_FUNCTIONS.md) retain selected carriers/Error data and accept Boolean/numeric/NULL, legacy DATE and explicit-policy Text conditions. Typed Empty is false; Missing/Error conditions fail. Complete native coercion/promotion remains open. |
| Switch | Partial | Eager condition/value pairs, first true selection and NULL when none match. Shares IIf's [Date/Text/state coercion](CONDITIONAL_FUNCTIONS.md#date-text-and-explicit-expression-states); operational failures stop immediately. Native promotion/error precedence remains open. |
| Choose | Partial | Eager choices, rounded one-based numeric/legacy DATE/explicit-policy Text index and NULL outside the supported choice range. Typed Empty is out of range; Missing/Error indices fail. Full Variant/native promotion remains open. |
| IsArray | Partial | [Explicit arrays and materialized Binary](VALUE_INSPECTION.md) are recognized without payload coercion. General Variant arrays, host bindings and native binary/SQL qualification remain open. |
| IsDate | Partial | [Error/Empty recognition](ISDATE.md#expression-state-recognition-23-october-2026). [Explicit Gregorian date/time recognition](ISDATE.md), checked numeric/date carriers and FALSE for NULL. Missing policies and operational failures propagate; full regional/Variant behavior and native qualification remain open. |
| IsEmpty | Partial | [Explicit host-issued Empty](VALUE_INSPECTION.md) is distinct from NULL/empty Text; materialized SQL values return FALSE. Language variables and complete Variant/native behavior remain open. |
| IsError | Partial | [Host-issued Error and CVErr data](VARIANT_ERRORS.md) recognition works; operational errors still fail. Expression-valued result/derived cells work; complete Variant/native behavior remains open. |
| IsMissing | Not planned | `vba-rs` owns [optional Variant procedure-argument inspection](https://learn.microsoft.com/en-us/office/vba/language/reference/user-interface-help/ismissing-function). SQL omitted argument slots are separate syntax and do not require a VBA Missing runtime. |
| IsNumeric | Partial | [Error/Empty recognition](ISNUMERIC.md#expression-state-recognition-23-october-2026). [Numeric/Boolean carrier and whole-Text recognition](ISNUMERIC.md) under explicit number formats. Operational failures propagate; complete locale/Variant grammar and native qualification remain open. |
| IsObject | Not planned | Live VBA object-reference inspection belongs to `vba-rs`. Although Microsoft includes it in the query sandbox list, query callability does not put object-runtime inspection in TableCraft's scalar query scope. Stored OLE payloads are binary data, not live VBA object references. |
| TypeName | Partial | [Materialized carrier/state/array names](VALUE_INSPECTION.md), with metered Text output. GUID/extended DateTime reject guessed VBA names; objects, native promotion and wider arrays remain open. |
| VarType | Partial | [Named carrier/state subtype codes](VALUE_INSPECTION.md) and Variant/Byte array flags return Integer. Native-only carriers, objects, promotion and wider arrays remain open. |

## Text and formatting

See [text contracts](EXPRESSIONS.md#bounded-text-built-ins). Current strings are
valid Unicode; lossless unpaired-surrogate results are not representable.

| Built-in | Status | Notes |
| --- | --- | --- |
| Len | Partial | [Expression-state admission](CSTR.md#unary-string-expression-states). [Explicit-policy scalar string expressions](BYTE_STRING_FUNCTIONS.md#scalar-length-expressions); length counts UTF-16 units and NULL propagates. Variable/UDT storage sizes, expression-state coercion and native query overload qualification remain open. |
| LTrim | Partial | [Expression-state admission](CSTR.md#unary-string-expression-states). [Explicit-policy scalar string sources](TEXT_CASING.md#scalar-sources); removes leading U+0020 spaces. Full Variant/native conversion remains unqualified. |
| RTrim | Partial | [Expression-state admission](CSTR.md#unary-string-expression-states). [Explicit-policy scalar string sources](TEXT_CASING.md#scalar-sources); removes trailing U+0020 spaces. Full Variant/native conversion remains unqualified. |
| Trim | Partial | [Expression-state admission](CSTR.md#unary-string-expression-states). [Explicit-policy scalar string sources](TEXT_CASING.md#scalar-sources); removes U+0020 spaces at both ends. Full Variant/native conversion remains unqualified. |
| Left | Partial | [Ordered Error/Empty/Missing admission](SLICE_COUNTS.md#ordered-expression-state-operands-23-october-2026). [Explicit-policy scalar string sources and rounded Long counts](SLICE_COUNTS.md#scalar-string-sources), including numeric/legacy DATE and SQL Text positions. Surrogate-splitting, full Variant coercion and native qualification remain open. |
| Right | Partial | [Ordered Error/Empty/Missing admission](SLICE_COUNTS.md#ordered-expression-state-operands-23-october-2026). Same count/carrier limits as Left; counts UTF-16 units. |
| Mid | Partial | [Ordered Error/Empty/Missing admission](SLICE_COUNTS.md#ordered-expression-state-operands-23-october-2026). One-based [rounded Long start/length and scalar string sources](SLICE_COUNTS.md#scalar-string-sources), with explicit-policy SQL Text positions. Full Variant coercion, surrogate-splitting and native qualification remain open; assignment form is outside SQL scope. |
| StrComp | Partial | [Explicit UTF-16 binary and host-supplied text/database modes](STRING_COMPARISON.md#contextual-comparison-mode-conversion), with DATE/explicit-policy SQL Text mode conversion and [bounded scalar string operands](STRING_COMPARISON.md#contextual-scalar-string-operands) and [ordered state admission](STRING_COMPARISON.md#ordered-expression-state-admission-23-october-2026); no inferred locale/default, full Variant coercion or qualified native code-page/database collation. |
| InStr | Partial | [Ordered Error/Empty/Missing admission](STRING_SEARCH.md#ordered-expression-state-admission-23-october-2026). [Bounded UTF-16 search](STRING_SEARCH.md#scalar-search-operands), explicit-policy scalar string operands, numeric/DATE/SQL Text starts and explicit host modes; General database search now supports full-weight expansion spans. Explicit LCID host dispatch now works; concrete locale providers, full coercion and native locale/code-page behavior remain open. |
| InStrB | Partial | [Metered binary byte search](BYTE_STRING_FUNCTIONS.md#binary-byte-search) accepts [explicit-policy scalar sources](BYTE_STRING_FUNCTIONS.md#scalar-byte-string-sources), Unicode and lossless raw string operands with one-based byte starts/results. Text/database/LCID search providers and broader coercion/native behavior remain open. |
| InStrRev | Partial | [Ordered Error/Empty/Missing admission](STRING_SEARCH.md#ordered-expression-state-admission-23-october-2026). [Reverse UTF-16 search](STRING_SEARCH.md#scalar-search-operands) with explicit-policy scalar string operands and numeric/DATE/SQL Text starts, full-match end bounds, binary default and explicit General database spans. Full coercion and native locale/code-page behavior remain open. |
| LCase | Partial | [Expression-state admission](CSTR.md#unary-string-expression-states). Explicit-policy scalar string sources with explicit ICU language; bounded contextual Unicode output. Variant coercion, aliases and Windows/ACE mappings unqualified. [Contract](TEXT_CASING.md). |
| UCase | Partial | [Expression-state admission](CSTR.md#unary-string-expression-states). Explicit-policy scalar string sources with explicit ICU language; bounded contextual Unicode output. Variant coercion, aliases and Windows/ACE mappings unqualified. [Contract](TEXT_CASING.md). |
| Replace | Partial | [Ordered Error/Empty/Missing admission](TEXT_REPLACEMENT.md#ordered-expression-state-admission-23-october-2026). [Bounded suffix replacement](TEXT_REPLACEMENT.md#scalar-string-operands), explicit-policy scalar string operands and numeric/DATE/SQL Text start/count slots and separate comparison modes, metered output and actual unequal-length General database match spans. Full coercion, unpaired-surrogate results and native locale/default/error qualification remain open. |
| Space | Partial | [Ordered expression-state admission](TEXT_CONSTRUCTION.md#ordered-repetition-states). [Bounded repeated spaces](TEXT_CONSTRUCTION.md) with checked numeric/legacy DATE and explicit-policy SQL Text counts, and NULL propagation. Full Variant coercion and native NULL/conversion/error qualification remain open. |
| String | Partial | [Ordered expression-state admission](TEXT_CONSTRUCTION.md#ordered-repetition-states). [Bounded repetition](TEXT_CONSTRUCTION.md#contextual-repetition-counts), numeric/DATE/explicit-policy SQL Text counts, literal first-unit character selection, numeric modulo 256 and [explicit single-byte code pages](CHARACTER_CODE_PAGES.md). Native/DBCS mappings, unpaired surrogates and full coercion remain open. |
| StrReverse | Partial | [Expression-state admission](CSTR.md#unary-string-expression-states). [Explicit-policy scalar input and bounded BMP reversal](TEXT_CONSTRUCTION.md#scalar-reversal-sources); non-BMP results are refused. Full Variant coercion and native supplementary-character behavior remain unqualified. |
| StrConv | Partial | [Explicit casing and single-byte conversion](STRCONV.md): 0/1/2/3/64/128 and case/encoding combinations, fourteen exact LCIDs or host language/page settings, with [explicit-policy scalar sources](STRCONV.md#scalar-string-sources), lossless ANSI string intermediates and [DATE/explicit-policy numeric Text Integer/Long option conversion](STRCONV.md#contextual-flag-and-lcid-conversion). Width/kana, wider LCID/DBCS/Variant/best-fit and native NLS behavior remain open. |
| LenB | Partial | [Explicit-policy scalar expressions and raw byte strings](BYTE_STRING_FUNCTIONS.md#scalar-length-expressions) return Long byte counts; NULL propagates and overflow/work are checked. Variable/UDT sizes, wider raw-string consumers and complete Variant/native behavior remain open. |
| LeftB | Partial | [Lossless UTF-16LE byte slices with explicit-policy scalar sources](BYTE_STRING_FUNCTIONS.md#scalar-byte-string-sources) work in nested byte calls, CVar and inspection. Ordinary scalar publication requires valid Unicode; broader carriers/coercion and native behavior remain open. |
| RightB | Partial | [Lossless trailing byte slices and explicit-policy scalar sources](BYTE_STRING_FUNCTIONS.md#scalar-byte-string-sources), including odd counts and surrogate halves. Raw result/relational carriers, broader coercion and native behavior remain open. |
| MidB | Partial | [One-based raw byte slices and explicit-policy scalar sources](BYTE_STRING_FUNCTIONS.md#scalar-byte-string-sources) with omitted length and checked positions. Explicit missing lengths, wider raw-string consumers and native behavior remain open. |
| Asc | Partial | [Expression-state admission](CSTR.md#unary-string-expression-states). [Explicit-policy scalar sources and first-character single-byte encoding](CHARACTER_FUNCTIONS.md#scalar-string-sources) using explicit pages; full Variant/native behavior and best-fit/DBCS remain open. |
| AscB | Partial | [Explicit-policy scalar sources and first UTF-16LE byte](BYTE_STRING_FUNCTIONS.md#scalar-byte-string-sources) returns Integer without ANSI lookup. Empty/NULL errors are explicit; wider raw-string consumers and broader Variant/native behavior remain outside the profile. |
| AscW | Partial | [Expression-state admission](CSTR.md#unary-string-expression-states). [Explicit-policy scalar sources and signed first UTF-16 unit](CHARACTER_FUNCTIONS.md#scalar-string-sources), including leading surrogates; full Variant/native behavior unqualified. |
| Chr | Partial | [Expression-state code admission](CHARACTER_FUNCTIONS.md#character-construction-expression-states). [Checked byte-code construction](CHARACTER_FUNCTIONS.md#contextual-character-code-conversion), including DATE/explicit-policy SQL Text codes, explicit pages or ASCII-only default. DBCS/native mappings and full coercion remain open. |
| ChrB | Partial | [Single-byte string construction](BYTE_STRING_FUNCTIONS.md) accepts checked numeric/DATE/explicit-policy SQL Text Long codes 0..255; out-of-range codes overflow. LenB/AscB/CVar/type inspection retain raw bytes; ordinary publication, broader consumers/coercion and native behavior remain open. |
| ChrW | Partial | [Expression-state code admission](CHARACTER_FUNCTIONS.md#character-construction-expression-states). [Checked signed/unsigned UTF-16 code units](CHARACTER_FUNCTIONS.md#contextual-character-code-conversion), including DATE/explicit-policy SQL Text codes. Surrogate output is refused; full coercion and native qualification remain open. |
| Format | Partial | [Numeric masks/sections](FORMAT.md) and [Gregorian date masks/styles](DATE_FORMAT_MASKS.md), explicit names/separators/week rules, named numeric/Boolean styles, default CStr and [Text layouts/combined casing](TEXT_FORMAT_MASKS.md). [Scientific/scaling patterns](SCIENTIFIC_FORMATTING.md) and numeric display of stored legacy DATE serials and [interleaved numeric literals](FORMAT.md#interleaved-numeric-literals) now render under explicit policies; [Contextual week options](WEEK_SETTINGS.md#contextual-week-option-conversion) now accept DATE and numeric Text. Broader date sections, Extended numeric conversion and full regional/Variant/native behavior remain unfinished. |
| FormatCurrency | Partial | [Ordered expression-state admission](STYLED_NUMBER_FORMATTING.md#ordered-numeric-formatting-states). [Fixed-place currency output](STYLED_NUMBER_FORMATTING.md), exact fixed inputs, bounded normalized IEEE display and explicit profile/options. [Explicit-policy numeric Text options](STYLED_NUMBER_FORMATTING.md#contextual-numeric-options) and [raw DATE source values](STYLED_NUMBER_FORMATTING.md#legacy-date-source-values) now work. Full NLS/Variant, rounding and native qualification remain open. |
| FormatDateTime | Partial | [Ordered expression-state admission](DATE_FORMATTING.md#ordered-expression-state-admission). [Five named formats](DATE_FORMATTING.md) under explicit Gregorian profiles, checked date input and bounded output. [Explicit-policy numeric Text selectors](DATE_FORMATTING.md#contextual-named-selectors) now work. Full regional/Variant behavior and native precision/error qualification remain open. |
| FormatNumber | Partial | [Ordered expression-state admission](STYLED_NUMBER_FORMATTING.md#ordered-numeric-formatting-states). [Fixed-place numeric output](STYLED_NUMBER_FORMATTING.md), grouping/leading/sign options and 0–255 output places. [Explicit-policy numeric Text options](STYLED_NUMBER_FORMATTING.md#contextual-numeric-options) and [raw DATE source values](STYLED_NUMBER_FORMATTING.md#legacy-date-source-values) now work. Full NLS/Variant, rounding and native qualification remain open. |
| FormatPercent | Partial | [Ordered expression-state admission](STYLED_NUMBER_FORMATTING.md#ordered-numeric-formatting-states). [Percent display](STYLED_NUMBER_FORMATTING.md) with display-point scaling beyond stored carrier range, shared profiles/options and bounded output. [Explicit-policy numeric Text options](STYLED_NUMBER_FORMATTING.md#contextual-numeric-options) and [raw DATE source values](STYLED_NUMBER_FORMATTING.md#legacy-date-source-values) now work. Full NLS/Variant, rounding and native qualification remain open. |

`Chr$` shares the checked Chr kernel, including explicit NULL/code-page policies,
while retaining its distinct Scalar String return identity. The explicit query
profile now tracks [reviewed return categories for all admitted text built-ins](DECLARED_ARITHMETIC.md#text-built-in-return-categories-23-october-2026);
this does not complete their coercion or native qualification. Other typed `$` string-returning variants are not implemented aliases;
supporting an unsuffixed function does not establish support for its `$` variant.

## Numeric functions

See [numeric contracts](MATH_FUNCTIONS.md). Implemented functions accept explicit
numeric/Boolean carriers and [explicit-policy numeric Text](MATH_FUNCTIONS.md#explicit-numeric-text-operands-23-october-2026).
Complete Access date/Variant coercion remains unfinished. Math calls now retain
[Error data through eager argument evaluation](MATH_FUNCTIONS.md#expression-errorempty-math-arguments-23-october-2026)
and reject it at the kernel's validation position. Reviewed host Empty zero targets
are supported; Round's Variant Empty value and native precedence remain open.

| Built-in | Status | Notes |
| --- | --- | --- |
| Abs | Partial | Checked magnitude retaining numeric carrier; [legacy DATE returns Double](MATH_FUNCTIONS.md#legacy-date-unary-functions-23-october-2026). Minimum signed values overflow instead of inferring native widening. |
| Sgn | Partial | Exact sign returns Int16 -1/0/1; numeric/Boolean/NULL, legacy DATE and explicit-policy numeric Text profile. |
| Int | Partial | Floor retaining supported numeric carrier or [checked legacy DATE](MATH_FUNCTIONS.md#legacy-date-unary-functions-23-october-2026); Text converts to Double. Full Variant/native promotion remains open. |
| Fix | Partial | Truncation toward zero retaining supported numeric/legacy DATE carrier; same broader coercion limits as Int. |
| Round | Partial | Half-even rounding; supported place count 0–28. Floating rounding/subtype behavior is an explicit profile awaiting native comparison. |
| Sqr | Partial | Checked numeric-to-Double square root; negative inputs fail. Full argument coercion is absent. |
| Exp | Partial | Double exponential with finite-result checks; native precision/error details remain unqualified. |
| Log | Partial | Double natural logarithm for positive inputs; full argument coercion is absent. |
| Sin | Partial | Double sine in radians; full argument coercion and native last-bit fidelity remain open. |
| Cos | Partial | Double cosine in radians; same coercion/precision limits as Sin. |
| Tan | Partial | Double tangent in radians; same coercion/precision limits as Sin. |
| Atn | Partial | Double principal arctangent; same coercion/precision limits as Sin. |
| Rnd | Partial | [Explicit sequential state](RANDOM_FUNCTIONS.md) now supports omitted/positive advance, zero repeat, negative Single reseeding and CLI raw seeds. Native sequence/coercion/optimizer qualification, volatile group keys and broader host integration remain open. |
| Partition | Partial | [Bounded numeric buckets and padded labels](PARTITION.md), Long/half-even conversion, [explicit-policy numeric Text controls](PARTITION.md#explicit-numeric-text-controls-23-october-2026), NULL propagation and metered scalar/bound retrieval work. Native padding, interval-one outside behavior, full coercion and errors remain unqualified. |
| EuroConvert | Partial | [Fixed euro conversions](EURO_CONVERSION.md) now support the 12-code Access table, Double results, target rounding and explicit significant-digit triangulation through shared scalar/query calls. Later participants, wider coercion and native precision/error/default qualification remain open. |

`Randomize` is a VBA statement, not an implemented scalar function.

## Conversions

See [conversion contracts](EXPRESSIONS.md#checked-numeric-and-boolean-conversion-functions).
These are executable conversions, distinct from implicit arithmetic coercion or
native storage decoding. All implemented conversions reject explicit NULL.
CByte/CInt/CLng/CLngLng/CBool/CSng/CDbl/CCur/CDec also accept
[legacy DATE serials](DATE_NUMERIC.md); Extended numeric conversion and native
precision/coercion qualification remain open.

| Built-in | Status | Notes |
| --- | --- | --- |
| CBool | Partial | Numeric zero/nonzero plus [explicit numeric Text/Boolean spellings](NUMBER_FORMAT.md); full regional/Extended-date/Variant behavior remains open. |
| CByte | Partial | Checked half-even conversion, including TRUE to 255, with [explicit numeric Text](NUMBER_FORMAT.md). Extended-date/general regional coercion remains open. |
| CInt | Partial | Checked half-even Int16 conversion with [explicit numeric Text](NUMBER_FORMAT.md). Native regional/Extended-date behavior remains open. |
| CLng | Partial | Checked half-even Int32 conversion with [explicit numeric Text](NUMBER_FORMAT.md). Native regional/Extended-date behavior remains open. |
| CLngLng | Partial | Explicit Int64 conversion with [exact numeric Text](NUMBER_FORMAT.md), independent of host pointer width. Native host availability, regional/Extended-date behavior remain open. |
| CSng | Partial | Finite binary32 conversion and [direct numeric Text parsing](NUMBER_FORMAT.md). Full regional/Extended-date coercion and native qualification remain open. |
| CDbl | Partial | Finite binary64 conversion and [numeric Text parsing](NUMBER_FORMAT.md). Full regional/Extended-date coercion and native qualification remain open. |
| CCur | Partial | Scale-four half-even conversion and [exact numeric Text](NUMBER_FORMAT.md). Full regional/Extended-date behavior and native rounding/order qualification remain open. |
| CDec | Partial | 96-bit/scale-28 conversion and [exact numeric Text](NUMBER_FORMAT.md). Full regional/Extended-date behavior and native precision/rounding qualification remain open. |
| CDate | Partial | [Explicit Error/Empty/Missing admission](DATE_CONVERSIONS.md#single-date-expression-states-23-october-2026). [Checked serial/date and explicit Text conversion](DATE_CONVERSIONS.md), preserving existing DATE bits. Full regional/Variant and native precision/error behavior remain open. |
| CStr | Partial | [Text/Boolean, standard expression Error/Empty and explicit numeric/date output policies](CSTR.md). Exact fixed values are retained; full regional/Variant behavior, float/date precision and native qualification remain open. |
| CVar | Partial | [Materialized value identity](CVAR.md) retains carriers, NULL, payload ownership and field text policies in scalar/bound queries. Expression-only arrays/states and VBA variable/object behavior are outside this path; native mapping remains unqualified. |
| CVDate | Partial | [Same materialized DATE conversion as CDate](DATE_CONVERSIONS.md#cvdate-query-alias), with identical explicit policies/NULL failure and distinct Variant provenance in the explicit declared-arithmetic profile. No new stored carrier; full regional/Variant/native conversion remains unqualified. |
| CLngPtr | Not planned | Pointer-width-dependent VBA conversion belongs to `vba-rs`, not the query value model. |
| CVErr | Partial | [Expression Error construction](VARIANT_ERRORS.md), inspection, eager selection and explicit standard-code numeric conversion now work in scalar/bound queries. Expression-valued result/derived cells, Count nullness and standard Error/Error predicates now work; arithmetic/logical rejection remains distinct from conversion. Broader Variant/coercion and native qualification remain open; operational failures stay typed errors. |
| Str | Partial | [Explicit-policy numeric Text](NUMERIC_TEXT.md#explicit-numeric-text-inputs-23-october-2026) now converts to Double before rendering. [Numeric spelling and sign spaces](NUMERIC_TEXT.md), preserving exact fixed carriers. [Standard Error/host Empty and explicit-policy legacy dates](NUMERIC_TEXT.md#str-expression-states-and-legacy-dates-23-october-2026) now work. Native float notation, full coercion and regional behavior remain open. |
| Val | Partial | [Expression-state admission](CSTR.md#unary-string-expression-states). [Explicit-policy scalar inputs, numeric prefixes and checked radix/type suffixes](VAL.md#scalar-string-inputs). Finite Double result profile; full Variant coercion, native subtypes and error qualification remain open. |
| Hex | Partial | [Checked numeric-to-hex spelling](NUMERIC_TEXT.md), including explicit Int64 inputs. [Explicit-policy numeric Text](NUMERIC_TEXT.md#explicit-numeric-text-inputs-23-october-2026) now works. [Legacy DATE and host Empty inputs](NUMERIC_TEXT.md#hexoct-legacy-dates-and-expression-states-23-october-2026) now work; Error data fails TypeMismatch. Native negative widths, full Variant coercion and error qualification remain open. |
| Oct | Partial | [Checked numeric-to-octal spelling](NUMERIC_TEXT.md), including explicit Int64 inputs. [Explicit-policy numeric Text](NUMERIC_TEXT.md#explicit-numeric-text-inputs-23-october-2026) now works. [Legacy DATE and host Empty inputs](NUMERIC_TEXT.md#hexoct-legacy-dates-and-expression-states-23-october-2026) now work; Error data fails TypeMismatch. Native negative widths, full Variant coercion and error qualification remain open. |
| GUIDFromString | Partial | [Bounded Text-to-Binary GUID conversion](GUID_CONVERSIONS.md) returns sixteen GUID-structure bytes under explicit spelling/NULL/error policies. Native Byte-array representation and GUID/Binary comparison remain unqualified. |
| StringFromGUID | Partial | [Bounded inverse conversion](GUID_CONVERSIONS.md) accepts sixteen GUID-structure bytes and stored Guid fields, returning provisional uppercase `{guid {…}}` Text. Byte order, shared budgets and query dependency walks are tested; exact native output/coercion and direct-query qualification remain pending. |

## Date and time

See [component extraction](EXPRESSIONS.md), [DateAdd/DateDiff/DatePart](EXPRESSIONS.md)
and [constructors](DATE_CONSTRUCTORS.md). Current calendar operations are
Gregorian. DATE serial operators are tracked separately in
[DATE_ARITHMETIC.md](DATE_ARITHMETIC.md).

| Built-in | Status | Notes |
| --- | --- | --- |
| Year | Partial | [Gregorian extraction with bounded numeric/Text source coercion](EXPRESSIONS.md#date-component-source-coercion); existing date/NULL precision rules remain. Full Variant/native/alternate-calendar behavior remains open. |
| Month | Partial | Same bounded source conversion and carrier/calendar limits as Year. |
| Day | Partial | Same bounded source conversion and carrier/calendar limits as Year. |
| Hour | Partial | [Bounded numeric/Text/date source extraction](EXPRESSIONS.md#date-component-source-coercion); legacy fraction rounding and full Variant/native behavior need qualification. |
| Minute | Partial | Same bounded source conversion and carrier/rounding limits as Hour. |
| Second | Partial | Same bounded source conversion and carrier/rounding limits as Hour. |
| DateAdd | Partial | [Bounded numeric/Text sources and checked DATE/Text counts](DATE_INTERVALS.md), with [ordered Error/Empty state admission](DATE_INTERVALS.md#ordered-expression-state-operands-23-october-2026). Bounded Gregorian interval arithmetic over supported date carriers; full coercion/alternate calendar support absent. |
| DateDiff | Partial | [Bounded numeric/Text sources](DATE_INTERVALS.md), with [ordered state admission](DATE_INTERVALS.md#ordered-expression-state-operands-23-october-2026) and both validated before NULL propagation. Signed Gregorian interval-boundary counts with [explicit/system settings and checked DATE/SQL Text week options](WEEK_SETTINGS.md). Non-Sunday w anchors, full coercion and native/alternate-calendar behavior remain open. |
| DatePart | Partial | [Bounded numeric/Text sources](DATE_INTERVALS.md) after [ordered interval/week/state validation](DATE_INTERVALS.md#ordered-expression-state-operands-23-october-2026). Gregorian parts and [caller-issued settings/checked DATE and SQL Text week options](WEEK_SETTINGS.md), preserving omitted defaults and the documented final-Monday anomaly. Native/alternate-calendar behavior remains unqualified. |
| DateSerial | Partial | [Checked numeric/raw DATE/SQL Text components](DATE_CONSTRUCTORS.md#argument-coercion) and [ordered Error/Empty component admission](DATE_CONSTRUCTORS.md#ordered-expression-state-components-23-october-2026), normalized Gregorian construction and explicit two-digit-year window; Hijri and complete native coercion/edge behavior remain open. |
| TimeSerial | Partial | [Checked numeric/raw DATE/SQL Text Int16 components](DATE_CONSTRUCTORS.md#argument-coercion) and [ordered Error/Empty component admission](DATE_CONSTRUCTORS.md#ordered-expression-state-components-23-october-2026), with signed second/day carries into legacy DATE; native negative-carry details remain unqualified. |
| DateValue | Partial | [Date extraction and explicit Gregorian Text parsing](DATE_CONVERSIONS.md), with caller-issued omitted year. Full locale/calendar/Variant and native NULL/error behavior remain open. |
| TimeValue | Partial | [Time extraction and explicit 12/24-hour Text parsing](DATE_CONVERSIONS.md), validating discarded dates. Full regional/Variant and native precision/error behavior remain open. |
| MonthName | Partial | [Ordered expression-state admission](CALENDAR_NAMES.md#ordered-expression-state-admission). [Explicit English/German Gregorian full/short names](CALENDAR_NAMES.md), checked numeric ordinal/abbreviation operands and bounded output. Broader locales/calendars, Variant/Text coercion and native qualification remain open. |
| Weekday | Partial | [Ordered expression-state admission](WEEKDAY.md#ordered-expression-state-admission-23-october-2026). [Standalone Gregorian numbering](WEEKDAY.md), checked numeric/Text/date inputs, [checked DATE/Text first-day options](WEEK_SETTINGS.md#contextual-week-option-conversion) and caller-issued system context; NULL dates propagate. Full native NLS/Variant/calendar, precision and error behavior remain unqualified. |
| WeekdayName | Partial | [Ordered expression-state admission](CALENDAR_NAMES.md#ordered-expression-state-admission). [Explicit English/German full/short names](CALENDAR_NAMES.md), all first-day offsets, host-issued system/default context and SQL overrides. Broader locales/calendars, Variant/Text coercion and native NLS qualification remain open. |
| Date | Partial | [Host-issued clock function](CLOCK_FUNCTIONS.md), zero-argument calls, whole-second local Gregorian readings and request-local providers. Native timing/precision/calendar/platform qualification remains open. |
| Now | Partial | [Host-issued clock function](CLOCK_FUNCTIONS.md), zero-argument calls, whole-second local Gregorian readings and request-local providers. Native timing/precision/calendar/platform qualification remains open. |
| Time | Partial | [Host-issued clock function](CLOCK_FUNCTIONS.md), zero-argument calls, whole-second local Gregorian readings and request-local providers. Native timing/precision/calendar/platform qualification remains open. |
| Timer | Partial | [Explicit clock-backed Single seconds since midnight](CLOCK_FUNCTIONS.md), fractional fixed/system readings and bounded scalar/bound retrieval work. Microsoft supplies a [query example](https://support.microsoft.com/en-gb/access/functions/timer-function). Native resolution/rounding, invocation timing and platform differences remain unqualified; separate from form Timer events and stored VBA execution. |

The [explicit declaration profile](DECLARED_ARITHMETIC.md#math-signatures-and-lenlenb-correction)
now distinguishes Variant Abs/Int/Fix/Round returns from Scalar transcendental/Rnd
returns. Len/LenB also retain Variant returns containing Long. This concerns
return provenance; it does not complete their coercion or native qualification.

## SQL aggregates

See [aggregation rules](QUERY_EXECUTION.md). These operate on bound scalar inputs,
not implicit domain lookups or arbitrary complex fields. Native result-type
promotion is not complete.

| Built-in | Status | Notes |
| --- | --- | --- |
| Count | Partial | Count(*) and Count(expr), grouping and empty/all-NULL behavior. Explicit Error-valued expressions count as non-NULL without code conversion; native cell/error qualification and resource/result limits remain. |
| Sum | Partial | [Exact floating totals](testing/FLOATING_SUMS.md) and checked integral/Currency/Decimal folds; Currency/Single/Double mixing reuses CDbl conversion. Full mixed date/Variant and encounter-order promotion and native accumulation/rounding qualification remain open. |
| Avg | Partial | [Wider numeric means](AVERAGES.md), including Currency, exact binary floating totals and NULL elimination; fixed-point Double means round once from exact rational totals; wider exact Decimal totals; mixed conversion and native accumulation/promotion gaps remain. |
| Min | Partial | Supported comparable scalar carriers and provisional standard Error-only extrema retaining Error data; mixed Error/scalar kinds fail. Text requires explicit host collation; native aggregate/locale qualification remains. |
| Max | Partial | Same comparison/collation limits as Min. |
| First | Partial | First encountered scalar/NULL or [expression Error](VARIANT_ERRORS.md#encounter-order-aggregates-and-domains). All operands evaluate. Native scan/index encounter order is not guaranteed to match Access. |
| Last | Partial | Last encountered scalar/NULL or expression Error; all operands evaluate. Same encounter-order limits as First. |
| StDev | Partial | Sample deviation over finite numeric values; two known values required. [Bounded Double profile](STATISTICAL_AGGREGATES.md); native accumulation/promotion/precision qualification remains open. |
| StDevP | Partial | Population deviation over finite numeric values; singleton returns Double zero. [Bounded Double profile](STATISTICAL_AGGREGATES.md); native accumulation/promotion/precision qualification remains open. |
| Var | Partial | Sample variance over finite numeric values; two known values required. [Bounded Double profile](STATISTICAL_AGGREGATES.md); native accumulation/promotion/precision qualification remains open. |
| VarP | Partial | Population variance over finite numeric values; the documented two-value minimum is retained. [Bounded Double profile](STATISTICAL_AGGREGATES.md); native accumulation/promotion/precision qualification remains open. |

## Domain functions

All require explicit domain resolution and criteria execution contracts. Ordinary
SELECT aggregates and subqueries do not automatically implement these functions.

The [explicit declared-arithmetic profile](DECLARED_ARITHMETIC.md#domain-function-return-declarations)
now retains the reviewed Variant return declaration for all twelve functions.
Their materialized carrier/coercion/precision profiles and native qualification
remain separate; this does not complete any Partial entry.

| Built-in | Status | Notes |
| --- | --- | --- |
| DAvg | Partial | Domain average uses [wider means](AVERAGES.md). See [domain profile](DOMAIN_FUNCTIONS.md); broader host/criteria/coercion and native qualification remain. |
| DCount | Partial | Domain count, including non-NULL expression Error cells without code conversion. See [domain profile](DOMAIN_FUNCTIONS.md); broader host/criteria/coercion and native qualification remain. |
| DFirst | Partial | First domain value, including explicit Error data. See [domain profile](DOMAIN_FUNCTIONS.md); broader host/criteria/coercion and native qualification remain. |
| DLast | Partial | Last domain value, including explicit Error data. See [domain profile](DOMAIN_FUNCTIONS.md); broader host/criteria/coercion and native qualification remain. |
| DLookup | Partial | Domain value lookup, including explicit Error endpoint data. See [domain profile](DOMAIN_FUNCTIONS.md); broader host/criteria/coercion and native qualification remain. |
| DMin | Partial | Domain minimum, including provisional standard Error-only extrema. See [domain profile](DOMAIN_FUNCTIONS.md); broader host/criteria/coercion and native qualification remain. |
| DMax | Partial | Domain maximum, including provisional standard Error-only extrema. See [domain profile](DOMAIN_FUNCTIONS.md); broader host/criteria/coercion and native qualification remain. |
| DStDev | Partial | Domain sample standard deviation. See [domain profile](DOMAIN_FUNCTIONS.md); broader host/criteria/coercion and native qualification remain. |
| DStDevP | Partial | Domain population standard deviation. See [domain profile](DOMAIN_FUNCTIONS.md); broader host/criteria/coercion and native qualification remain. |
| DSum | Partial | Shares [exact floating Sum](testing/FLOATING_SUMS.md) and checked fixed-point folds. See [domain profile](DOMAIN_FUNCTIONS.md); broader host/criteria/coercion and native qualification remain. |
| DVar | Partial | Domain sample variance. See [domain profile](DOMAIN_FUNCTIONS.md); broader host/criteria/coercion and native qualification remain. |
| DVarP | Partial | Domain population variance. See [domain profile](DOMAIN_FUNCTIONS.md); broader host/criteria/coercion and native qualification remain. |

## Financial functions

NPV/IRR/MIRR now retain [explicit expression states through eager operand evaluation](CASHFLOW_FUNCTIONS.md#ordered-expression-state-admission-23-october-2026).
Error/Empty array elements remain refused rather than converted or skipped; host
Empty scalar controls use the current zero profile. Native admissibility,
Variant-array coercion and numerical qualification remain open.

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

All eleven scalar financial calls above now accept explicit-policy numeric Text
in numeric slots through the [shared financial conversion path](FINANCIAL_FUNCTIONS.md#contextual-numeric-text-inputs-23-october-2026).
EuroConvert keeps its currency-code strings separate. NPV/IRR/MIRR now also
accept explicit-policy Text for their scalar rates/guess, while cashflow elements
retain strict numeric admission. Full Variant/native qualification retains its
owning limits; these entries remain Partial.

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
| Eval | Partial | [Closed dynamic expressions](DYNAMIC_EVALUATION.md) now share evaluator context, budgets and depth; pure group keys work and hidden clock/Rnd keys fail explicitly. Dynamic field/parameter/form/UDF/domain/subquery lookup is not supplied. Wider host/Variant/regional/native behavior remains open; CLI query eval is a separate command. |
| HyperlinkPart | Partial | [Six stored-text components](HYPERLINK_PARTS.md), required SQL selector, display fallback and metered borrowed extraction work. Original native Hyperlink fields compare independently; malformed/extra-delimiter, coercion and native NULL/error behavior remain unqualified. |
| CurrentUser | Partial | [Explicit immutable Access session context](SESSION_FUNCTIONS.md) returns the supplied account name; the CLI declares unsecured Admin. Missing library context fails. This is separate from OS identity, authentication, MDW/ACL enforcement and native qualification, which remain open. |
| RGB | Partial | [Packed numeric channels](COLOR_FUNCTIONS.md) return Long, with checked conversion and upper saturation. Query availability is documented; full coercion, input widths and native errors remain unqualified. |
| QBColor | Partial | [Sixteen-entry Basic palette](COLOR_FUNCTIONS.md) returns Long with checked numeric index conversion. Query availability is documented; full coercion and native boundaries/errors remain unqualified. |
| Error | Partial | [Deliberate Error/Empty/Missing admission](ERROR_MESSAGES.md#deliberate-expression-state-admission-23-october-2026). [Explicit-number message lookup](ERROR_MESSAGES.md) returns budgeted Text through a caller-selected catalog; the CLI selects candidate English reference messages. Zero is empty Text and valid undefined codes use the selected fallback. Signed Long message numbers now remain intact; values above 65535 report Overflow. Error$ retains Scalar String identity in the explicit declaration profile. Native code coverage, wording, localization and sandbox behavior remain unqualified; omitted-number latest-runtime-error state remains deferred with `vba-rs`. |
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
