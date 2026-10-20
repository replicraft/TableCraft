# Scalar expression evaluation

`crates/query/tablecraft-expressions` owns evaluated scalar rules over core
`Value` carriers. Its default build and direct scalar APIs are independent of SQL
parsing, database connections, GUI hosts and async runtimes. The optional `sql`
feature adds an evaluator of TableCraft-owned expression syntax. The syntax parser remains in
`tablecraft-sql`; [tablecraft-query](QUERY_EXECUTION.md) now supplies separate
binding, query plans and bounded row execution.

This is an initial, explicit scalar profile, not complete Jet/ACE or VBA Variant
semantics. Core values describe materialized carriers, not the declared/Variant
type metadata required to infer every native promotion. Documentation-derived
tests are not observed native Access results.

## Implemented rules

| Area | Implemented behavior |
| --- | --- |
| Truth | TRUE/FALSE/unknown, Boolean/NULL conversion, NOT/AND/OR/XOR/EQV/IMP; predicate selection accepts only TRUE |
| Comparison | Either NULL returns unknown, including NULL = NULL; all six comparison operators use numeric ordering, not `Value::Eq` |
| Exact numerics | Boolean TRUE becomes -1 and FALSE becomes 0 in the numeric domain; Byte/Int16/Int32/Int64, Currency and Decimal compare without floating conversion or scale-alignment overflow |
| Floating comparison | Finite numeric comparison, signed zeros equal; Double rounds to Single precision when those two carriers are compared; overflow/non-finite values rejected |
| Integral coercion | Explicit Byte/Int16/Int32/Int64 targets; fractions round halves to even; bounds checked after rounding; NULL remains NULL |
| Integral arithmetic | Checked +, -, *, unary minus; results use Int64; overflow errors instead of wrapping or guessing Variant widening |
| Floating arithmetic | +, -, *, / and ^ over supported integers/floats; finite Double results; division by zero and overflow are errors |
| Integer division/remainder | `\` and Mod round numeric operands halves to even, then use a quotient truncated toward zero and a dividend-signed remainder; Int64 result |
| Decimal arithmetic | Decimal operands select checked 96-bit +, -, *, / with exact fixed inputs and normalized floating imports; ^ returns Double; bounded library rounding is explicit below |
| Currency arithmetic | Exact Currency +/- Currency/integer and Currency * integer, retaining scale four and rejecting overflow |
| LIKE | Explicit ANSI-89/ANSI-92 mode, host scalar collation, lists/ranges and three-valued NOT; bounded iterative matching over Unicode scalar units ([contract](PATTERN_MATCHING.md)) |
| Concatenation | Text/NULL `&` treats one NULL as empty and retains NULL for two NULL operands; text `+` propagates either NULL; UTF-8 output byte ceiling checked before allocation |

Decimal scale and core carrier equality remain unchanged. Exact numeric comparison
compares signs, whole parts and fractional digits without rescaling a coefficient
past its range. Large integer digits are never silently rounded into a Double:
unsupported inexact integer-to-float conversion returns `PrecisionLoss`.

Text comparison requires a caller-supplied `Collation`; no locale, Unicode
normalization, case-folding or Rust string order is silently selected. A host can
supply a deterministic test collation without claiming it matches Access.

`EvalError` uses thiserror and carries no value, SQL or field-name payloads.
Scalar routines accept `Value`, so Empty/Missing/Error expression-only states
cannot silently enter them as NULL. Omitted function arguments retain their
separate syntax representation.

`Evaluator::apply_unary` and `apply_binary` also accept already evaluated operands
for the bound query executor. The caller charges node/row work; evaluator Text
intermediates continue consuming the supplied byte budget.

## Explicit limits of this profile

Arithmetic result carriers follow explicit checked Int64/Double/Currency/Decimal
profiles, without VBA's complete Byte/Integer/Long/Single/Variant promotion ladder. The
nullable `coerce_integer` kernel propagates SQL NULL and checks signed -1 numeric
Boolean representation against the chosen target range. The explicit conversion
functions below wrap it with distinct NULL and Boolean-to-Byte rules. Explicit
coercion can select a narrower integral result. Decimal binary arithmetic follows
the bounded profile below. Currency division,
Currency * Currency and Currency/float mixing without a Decimal operand remain
unsupported; the general Variant promotion ladder remains absent. Decimal/Currency
can still participate in exact comparison and explicit integral coercion (including rounded `\`/Mod operands).

Logical operators accept Boolean/NULL only; numeric bitwise operators and implicit
numeric-to-truth conversion are not implemented. Numeric text parsing, mixed
text/numeric `+`, numeric/date string rendering, GUID ordering/binary comparisons,
locale-dependent conversions, built-ins beyond the explicit NULL/conditional/text/date/conversion
profile below and complete VBA expression states are not implemented. Unsupported combinations fail explicitly.

The concatenation APIs require a byte ceiling and reserve fallibly after checking
the combined length. Text built-in kernels return borrowed slices and meter scans
through a host callback; evaluator hosts bound and charge materialized outputs.
A caller supplying a collation owns that callback's work and semantics.

## Bounded expression trees (`sql` feature)

Enable `tablecraft-expressions`' `sql` feature to use `evaluation::Evaluator`.
This consumes `tablecraft_sql::select::Expression`, with no upstream AST types.
It evaluates scalar nodes, not SELECT statements or rows from a database.
The CLI's `query eval` command now invokes it for one source-free SELECT's scalar
projections, with `NoBindings` and no implicit collation. It buffers a header and
one result row, rejecting database sources, parameters and query clauses that
would need execution. `query parse` remains syntax-only. See
[CLI behavior](DEVELOPMENT.md#cli-behavior) for output policy, limits and errors.

Supported nodes are NULL, Boolean, signed i64 literals, Text, finite fractional
or exponent literals, host-bound field/parameter references, unary numeric signs
and NOT, supported binary operators, IS NULL/IS NOT NULL, BETWEEN/NOT BETWEEN
and IN/NOT IN lists. Integer literals
materialize as Int64; fractional/exponent literals materialize as Double, not
exact Decimal. Numeric literal type inference for native Jet/ACE is not claimed.
Unmaterializable bare integer/hex/octal spelling outside the i64 syntax carrier,
non-finite float literals, underflow of a nonzero literal to zero and literals
longer than 128 bytes fail explicitly. Ordinary floating literal rounding to
binary64 remains distinct from exact Decimal value construction.

The host implements `Bindings` to provide borrowed core values for written field
references, including qualifiers and parameters. `NoBindings` supports constants
and rejects unresolved names. The evaluator has no filesystem, connection,
catalog, clock, external function or identity-state access. Binary materialization,
unrecognized/qualified calls,
@@IDENTITY, members and subqueries are explicit
unsupported cases in this first tree evaluator, even though the parser owns those
syntax forms. Omitted function arguments are never evaluated as stored NULL.

Both binary operands are evaluated left-to-right. Boolean truth and NULL do not
hide evaluation errors in another operand. This is an explicit scalar evaluation
order, not a promise about native query optimizer order or VBA short-circuiting.

Each evaluator borrows a caller-owned `ResourceBudget`. It charges one step per
visited node and cumulative Text bytes before allocating literal copies, binding
copies or concatenated results. Failed evaluations keep work already charged;
reusing the evaluator shares the remaining budget. Rows are not charged. String
capacity is reserved fallibly. This is an allocation/work allowance, not exact
resident memory accounting or a deadline.

`EvalLimits` clamps maximum depth to 32 nodes and each Text value to 65,536 UTF-8
bytes, even for manually constructed trees; callers can lower either ceiling.
An optional host cancellation callback runs before every node, before bindings.
Hosts own the boundedness and side effects of resolver/collation/cancellation
callbacks; the evaluator cannot interrupt a callback while it runs.

## References and verification

The authored tests follow Microsoft primary references; they do not copy fixture
SQL or execute content from the supplied ACCDB.

| Rule | Reference |
| --- | --- |
| Access expression/operator scope | [Access operators](https://support.microsoft.com/en-us/access/table-of-operators) |
| Boolean/NULL tables | [And](https://learn.microsoft.com/en-us/office/vba/language/reference/user-interface-help/and-operator), [Or](https://learn.microsoft.com/en-us/office/vba/language/reference/user-interface-help/or-operator), [Not](https://learn.microsoft.com/en-us/office/vba/language/reference/user-interface-help/not-operator), [Xor](https://learn.microsoft.com/en-us/office/vba/language/reference/user-interface-help/xor-operator), [Eqv](https://learn.microsoft.com/en-us/office/vba/language/reference/user-interface-help/eqv-operator), [Imp](https://learn.microsoft.com/en-us/office/vba/language/reference/user-interface-help/imp-operator) |
| Numeric comparison and mixed precision | [Comparison operators](https://learn.microsoft.com/en-us/office/vba/language/reference/user-interface-help/comparison-operators) |
| Boolean numeric conversion | [Boolean data type](https://learn.microsoft.com/en-us/office/vba/language/reference/user-interface-help/boolean-data-type) |
| Coercion, rounding, locale and Variant Decimal boundaries | [Conversion functions](https://learn.microsoft.com/en-us/office/vba/language/concepts/getting-started/type-conversion-functions) |
| Arithmetic and native promotion distinctions | [Addition](https://learn.microsoft.com/en-us/office/vba/language/reference/user-interface-help/plus-operator), [subtraction](https://learn.microsoft.com/en-us/office/vba/language/reference/user-interface-help/minus-operator), [division](https://learn.microsoft.com/en-us/office/vba/language/reference/user-interface-help/forwardslash-operator), [integer division](https://learn.microsoft.com/en-us/office/vba/language/reference/user-interface-help/backslash-operator), [Mod](https://learn.microsoft.com/en-us/office/vba/language/reference/user-interface-help/mod-operator) |
| Distinct concatenation behavior | [Ampersand](https://learn.microsoft.com/en-us/office/vba/language/reference/user-interface-help/ampersand-operator), [Access text concatenation](https://support.microsoft.com/en-us/access/combine-text-values-by-using-an-expression) |

Local checks on aarch64-apple-darwin with Rust 1.99.0 on 17 October 2026 pass
formatting and Clippy with warnings denied in both feature configurations.
The default profile has 12 tests and four fixed-seed properties; enabling `sql`
adds nine tests (including a fifth property) and an API doctest. Each property
uses 256 cases. Tests cover complete Boolean/NULL
tables, exact comparisons beyond f64 precision, half-even rounding, integer
extremes, signed quotient/remainder identities, Currency boundaries and Unicode
byte limits. Native Access comparison and other platforms remain unverified.
Parsed-expression tests also cover precedence, host binding, eager errors,
unsupported syntax, callback collation, cancellation, cumulative byte/step
charges and lowered/clamped depth/output limits.

Legacy Date/Time now supports temporal comparison and bounded ISO literals under
the SQL feature. [Native date contracts](NATIVE_TABLES.md#legacy-datetime) describe
negative-day semantics, rounding boundaries and rejected locale/timezone coercions.

GUID literals and equality/inequality are supported via canonical bytes; text/
numeric coercion and GUID ordering remain unsupported when evaluated.

## Date/Time Extended comparison

Two extended-date carriers compare by civil date and time at exact 100ns
precision. NULL still yields unknown. Mixed legacy/extended dates return
`TypeMismatch`; this profile does not invent implicit conversion or timezone
semantics. Query filtering, ordering, grouping and distinct reuse this comparison.
[Native read evidence](NATIVE_ACE16.md#modern-scalar-reads) remains independent-reader
evidence, not Microsoft Access query conformance.

## Shared range and membership predicates

The SQL-independent `predicates` module supplies eager inclusive BETWEEN with
either written bound order and a constant-space IN-list truth accumulator. Hosts
supply comparison/collation, typed error and work/cancellation policy. Query
execution and the source-free scalar tree evaluator now use these same kernels;
no list of materialized Values is needed. IN retains exact numeric comparison,
NULL/unknown and NOT semantics. A prior match does not suppress later errors.
An empty syntactic IN list remains invalid; an empty subquery is a separate case.

The tree evaluator charges candidates and comparisons to its shared budget and
checks cancellation; arbitrary calls/subqueries/host access remain unsupported.
LIKE uses the separate [bounded pattern kernel](PATTERN_MATCHING.md) with explicit
mode and collation. Native validation-rule preflight still rejects LIKE.
Text comparison still needs explicit collation. Native numeric field/table audits
admit these predicates only after their existing pure-expression preflight; text
rules and unresolved NULL validation results stay unqualified.

Fixed-seed interval checks cover full i64 values without float conversion, along
with reversed bounds, unknown truth, eager errors, cancellation and budget limits.
Original [native rule fixtures](testing/PredicateRuleFixture.java) separately check
independent writer acceptance/rejection on both qualified newer families. This
is not Microsoft Access conformance or a new writable ACE profile.


## Explicit IsNull and IIf built-ins

The parser-independent `functions` module owns a closed, allocation-free built-in
lookup and value kernels. `IsNull(value)` returns Boolean true only for stored
NULL; zero, empty text/binary and other materialized values are distinct. Core
Empty/Missing/Error expression states are not coerced into stored values.
`IIf(condition, truepart, falsepart)` selects and moves one already evaluated branch
without result-carrier promotion, cloning or allocation. Both scalar syntax and
bound retrieval evaluate condition, truepart and falsepart in written order.
An error or resource failure in the unselected branch still fails the expression.
No optimizer path drops that branch or its required source fields.

Supported conditions are Boolean, exact integer/Currency/Decimal and finite Single/
Double: zero is false and nonzero is true. Exact coefficients are tested without
floating conversion, so Int64 and full Decimal coefficient precision remain intact. NULL
selects falsepart. Text/date/GUID/binary truth conversion remains unsupported;
there is no implicit locale-sensitive text parsing. Other logical operators retain
their existing Boolean/NULL profile. Branches retain exact materialized values;
result-column type inference and complete native Variant promotion remain future
work. The standalone syntax evaluator retains its existing Binary-binding limit;
query retrieval can carry bounded Binary values through these functions.

Unqualified ASCII-insensitive IsNull(one expression), IIf(three expressions)
and Nz(one or two expressions) resolve alongside the text built-ins below. Unknown functions, module qualifiers, wildcards, omitted positions and
wrong arity fail explicitly. These are pure Rust built-ins, not VBA execution or
an external function registry. UPDATE binding retains its prior no-call contract.

```sh
tablecraft query eval "SELECT IsNull(Null), IsNull(''), IIf(Null,1,2)"
tablecraft query run "SELECT ID, IIf(IsNull(Label),'missing',Label) AS DisplayLabel FROM Items" --database copy.accdb
```

The contracts draw on Microsoft's [IsNull documentation](https://learn.microsoft.com/en-us/office/vba/language/reference/user-interface-help/isnull-function),
[IIf argument/eager-evaluation documentation](https://learn.microsoft.com/en-us/office/vba/language/reference/user-interface-help/iif-function),
[numeric Boolean conversion](https://learn.microsoft.com/en-us/office/vba/language/concepts/getting-started/type-conversion-functions)
and [Access expression examples](https://support.microsoft.com/en-us/access/examples-of-expressions).
These are documentation-derived semantics, not observed native Access optimizer,
coercion or result-type behavior. Windows comparison remains unrun.

Focused tests cover full-width numeric truth, nullable/empty distinctions, argument
order, unselected failures/allocations, cancellation, grouped/correlated retrieval,
projection pruning, saved-query execution and buffered CLI errors. Independent
original Jet 4/ACE 2007/2016/2019 getters compare individual null tests and conditional
wrappers across all fourteen stored scalar carriers, plus Currency/Decimal/Single
conditions; exact source bytes remain unchanged. No native SQL is executed by that
oracle and no registry dependency is added.


## Query-context Nz

`Nz(value[, fallback])` is a pure shared kernel used by scalar SELECT evaluation,
retrieval and qualified saved queries. Non-NULL input passes through with its exact
carrier. NULL selects an explicit fallback unchanged, including explicit NULL.
With no fallback, NULL becomes zero-length Text, following Microsoft's
[query-expression documentation](https://support.microsoft.com/en-au/access/functions/nz-function).
This query behavior does not introduce VBA's contextual Empty/zero semantics.

Supplied operands evaluate in written order under the existing eager scalar-call
profile; errors, cancellation and allocations in an unused fallback remain visible.
Selection moves a value without allocation, coercion or carrier promotion. Empty
text for an omitted fallback needs no heap allocation. All provided fields remain
in projection and correlation mapping, and aggregate/group rules apply to both
operands. `Nz(Null,)`, qualified calls, wildcards and wrong arities fail explicitly.
UPDATE retains its existing no-call boundary. Native Access optimizer, coercion
and evaluation-order comparisons remain unrun; these are explicit implemented
contracts rather than a claim of full Access expression equivalence.


## Bounded text built-ins

`Len`, `LTrim`, `RTrim`, `Trim`, `Left`, `Right` and `Mid` are closed,
ASCII-insensitive unqualified calls in scalar SELECT evaluation and retrieval,
including supported native saved SELECTs. Their parser-independent kernels borrow
input text; evaluator hosts charge result allocation before fallible copying.
Every scanned Unicode scalar checks cancellation and consumes a work step.
All supplied operands evaluate in written order before function selection.

- Text input returns the selected Text result; NULL input returns NULL. Len returns
  an Int32 length for Text and NULL for NULL. Numbers, binary and dates are not
  implicitly formatted; VBA's storage-size overload of Len is absent.
- Trim variants remove only U+0020 spaces, preserving tabs, newlines, nonbreaking
  spaces and embedded NUL. They do not normalize Unicode or perform collation.
- Lengths and positions count UTF-16 code units, not UTF-8 bytes, grapheme clusters
  or Unicode scalar values. Left/Right accept a nonnegative length and clamp it
  to the input length. Mid accepts a positive one-based start and optional
  nonnegative length; omitted length means the remainder. Zero-length selections
  and starts beyond the input return empty Text. Positional arguments currently
  accept exact Byte/Int16/Int32/Int64 in signed 32-bit Long range. Fractional,
  Boolean, Decimal/Currency, text and NULL positions have no coercion contract.
- Text is currently valid Unicode in Rust String. A nonempty UTF-16 slice that
  splits a surrogate pair returns `UnrepresentableText`, without replacement
  characters or rounding to a different boundary. Lossless unpaired-surrogate
  expression results remain a compatibility gap.

Wrong arity, wildcard/missing arguments and qualifiers fail during binding,
including for empty sources. Built-in operands participate in pruning, correlation,
aggregate detection and grouping validation. UPDATE still rejects function calls.
LenB/LeftB/RightB/MidB, `$` variants, locale-sensitive case conversion and complete
Variant coercion remain unsupported.

Microsoft documents [Access string functions](https://support.microsoft.com/en-us/access/string-functions-and-how-to-use-them),
[Len](https://learn.microsoft.com/en-us/office/vba/language/reference/user-interface-help/len-function),
[space trimming](https://learn.microsoft.com/en-us/office/vba/language/reference/user-interface-help/ltrim-rtrim-and-trim-functions),
[Left](https://learn.microsoft.com/en-us/office/vba/language/reference/user-interface-help/left-function),
[Right](https://learn.microsoft.com/en-us/office/vba/language/reference/user-interface-help/right-function)
and [Mid](https://learn.microsoft.com/en-us/office/vba/language/reference/user-interface-help/mid-function).
The UTF-16 positioning model follows the [Windows BSTR representation](https://learn.microsoft.com/en-us/openspecs/windows_protocols/ms-dtyp/692a42a9-06ce-4394-b9bc-5d2a50440168)
and [surrogate representation](https://learn.microsoft.com/en-us/windows/win32/intl/surrogates-and-supplementary-characters);
these sources and independent buffer models do not establish native Access SQL
Unicode, coercion, optimizer or error-precedence equivalence. Windows comparison
remains pending.


## Gregorian date component built-ins

Year, Month, Day, Hour, Minute and Second accept one DateTime,
DateTimeExtended or NULL expression and return an Int16 component or NULL.
They share parser-independent kernels across scalar evaluation, table retrieval
and qualified saved SELECTs. No clock, timezone or external runtime is consulted.
Only Gregorian behavior is implemented; numeric/text date coercion and Hijri
calendar selection remain unsupported. Wrong shapes and qualified calls fail
before scans. Arguments participate in ordinary pruning, correlation and grouping;
UPDATE retains its no-call contract.

Legacy DATE uses the existing Jiff-backed, nearest-nanosecond calendar conversion,
then Jiff nearest-second rounding with HalfExpand ties. This avoids truncating an
encoded whole second to the preceding second. Rounding can carry through minute,
day, month and year boundaries; a result outside the civil envelope fails with
Overflow. Stored DATE bits are never changed. This explicit rounding policy and
endpoint behavior still require native Access SQL comparison.

Extended Date/Time uses its already validated exact Gregorian components, without
legacy conversion or rounding its 100 ns fraction. This preserves years before
100 and the final second of year 9999. It does not infer that every Access/VBA
entry point accepts Extended Date/Time with the same behavior.

Microsoft's [Year](https://learn.microsoft.com/en-us/office/vba/language/reference/user-interface-help/year-function),
[Month](https://learn.microsoft.com/en-us/office/vba/language/reference/user-interface-help/month-function),
[Day](https://learn.microsoft.com/en-us/office/vba/language/reference/user-interface-help/day-function),
[Hour](https://learn.microsoft.com/en-us/office/vba/language/reference/user-interface-help/hour-function),
[Minute](https://learn.microsoft.com/en-us/office/vba/language/reference/user-interface-help/minute-function)
and [Second](https://learn.microsoft.com/en-us/office/vba/language/reference/user-interface-help/second-function)
document Integer results, NULL propagation and date/calendar input semantics.
[VarUdateFromDate](https://learn.microsoft.com/en-us/windows/win32/api/oleauto/nf-oleauto-varudatefromdate)
describes unpacked Automation dates. Wine's [independent conversion reference](https://github.com/wine-mirror/wine/blob/master/dlls/oleaut32/variant.c)
illustrates whole-second conversion and carry; this informs the implemented
policy, not a native qualification claim. No Wine code, constants, fixtures or
runtime is incorporated. Full coercion, tie/error precedence and Extended Date/Time
behavior need actual Access comparisons.


## Gregorian DateAdd

`DateAdd(interval, number, date)` is an eager three-argument scalar/retrieval
built-in, also available to qualified saved SELECTs. Interval is Text, matched
ASCII-insensitively against yyyy, q, m, y, d, w, ww, h, n and s. The y/d/w
spellings all add calendar days; w does not skip weekends. Quarters add three
months and ww adds calendar weeks. Unknown spellings fail with InvalidDateInterval;
no whitespace trimming or locale interpretation occurs.

Counts reuse the existing signed Long numeric conversion: numeric/Boolean values
round to the nearest integer with ties even, preserving exact Decimal/Currency
rounding. Count overflow, non-finite floating values and unsupported carriers fail
explicitly. NULL interval/count have no implicit conversion; valid interval/count
with NULL date returns NULL. All supplied expressions evaluate in written order,
including those whose result will not be used after NULL propagation. This explicit
NULL/error precedence and tie policy still require native Access comparison.

Jiff applies checked civil arithmetic without per-interval iteration and clamps
month/year changes to a valid month end. Legacy input uses the shared whole-second
function profile before arithmetic and returns a legacy DATE within years 100–9999;
even a zero count can canonicalize/round its representation. Stored inputs are
never mutated. Extended input returns Extended Date/Time, preserving the exact
100 ns fraction and years 1–9999. It is not implicitly reduced to Automation DATE.
Overflow outside the respective envelopes fails instead of wrapping or clamping
the year. Only Gregorian explicit date carriers are accepted; text/numeric date
conversion and Hijri calendars remain separate work.

The implementation follows Microsoft's [DateAdd interval, count and month-end
contracts](https://support.microsoft.com/en-us/access/functions/dateadd-function)
and [VBA DateAdd reference](https://learn.microsoft.com/en-us/office/vba/language/reference/user-interface-help/dateadd-function).
Half-even count conversion reuses the existing explicit numeric profile. Extended
carrier preservation and legacy subsecond/endpoint behavior remain implemented
contracts awaiting actual Access SQL qualification. Source projection, aggregate/
group validation and correlation mapping retain all three arguments; UPDATE
continues to reject function calls.


## Gregorian DateDiff

`DateDiff(interval, date1, date2[, firstdayofweek][, firstweekofyear])` now shares
an eager parser-independent kernel across scalar SELECTs, retrieval and qualified
saved SELECTs. It returns signed Int32 counts and fails on Long overflow, rather
than widening or wrapping. Interval lookup uses the same ten closed spellings as
DateAdd; the operations give w their distinct documented meanings.

Year, quarter, month, day, hour, minute and second intervals count civil boundaries,
not completed elapsed spans. A one-second crossing of New Year can count one
year. The y/d spellings count calendar days; w counts complete seven-day periods
anchored to the starting date's weekday. Calendar ww counts first-day boundaries,
excluding the first date and including a matching second date. Reversed operands
produce negative counts. Subsecond Extended Date/Time is never converted to
legacy DATE; legacy operands retain the explicit whole-second function profile.
Both explicit date carriers may be compared through their Gregorian civil values.

Omitted optional settings default to Sunday (1) and Jan-1-week (1), including
explicit missing optional positions such as `DateDiff('ww',a,b,,2)`. This does not
turn Missing into stored NULL: explicit NULL settings fail type checking. Supplied
settings use the shared Long coercion and validate day 1–7/week rule 1–3. System
setting 0 fails with CalendarPolicyRequired, since no host calendar policy exists.
The first-week rule labels weeks within years; valid choices do not change the
sequence of calendar-week boundaries counted by ww. All supplied arguments still
participate in evaluation, projection/correlation and aggregate/group checks.

The primary reference describes both a starting-weekday anchor for w and an
influence of firstdayofweek on w. Nondefault first-day settings for w are therefore
explicitly unsupported pending native comparison, rather than assigned guessed
semantics. Nondefault ww first-day settings have the explicit boundary model above.
NULL date operands propagate NULL after interval/settings and date-carrier validation; all supplied
expressions evaluate first, so NULL does not hide errors in later arguments.
Unsupported types, wrong arities, required missing arguments and qualified calls
fail. UPDATE retains its no-call policy.

Microsoft's [Access DateDiff reference](https://support.microsoft.com/en-us/access/functions/datediff-function)
and [VBA DateDiff reference](https://learn.microsoft.com/en-us/office/vba/language/reference/user-interface-help/datediff-function)
describe interval spellings, Long results, signed counts, optional defaults and
weekday/calendar-week distinctions. Native Access comparison remains required
for week options, NULL/error precedence, fractional legacy rounding and mixed/
Extended carrier behavior. Independent fixtures and calendar models do not supply
that qualification. The kernel performs fixed calendar arithmetic, without
iterating through the date range, clocks, locale discovery or timezone access.


## Gregorian DatePart

`DatePart(interval, date[, firstdayofweek][, firstweekofyear])` now shares a
parser-independent kernel across scalar SELECTs, retrieval and qualified saved
SELECTs. All ten closed interval spellings return Int16 or NULL. Shared interval
identities now distinguish y (day of year, including leap day) from d (day of
month), while DateAdd/DateDiff preserve their existing y/d day arithmetic.
Quarter is 1–4; weekday is 1–7 relative to the selected first day. Existing
legacy whole-second and exact Extended component policies remain in force.

Week options share DateDiff's validation/coercion: Sunday and Jan-1-week defaults,
explicit missing optional positions kept separate from NULL, valid first days
1–7 and first-week rules 1–3. System setting 0 requires a missing host policy and
fails explicitly. Week numbering uses checked Jiff Gregorian dates with small
integer offsets: Jan-1 weeks, first four-day weeks and first full weeks. Dates
before the first qualifying week use the preceding year's week label. The
Extended year-one boundary uses Jiff's proleptic year zero internally for this
calculation; no stored year-zero value or out-of-range date is created. The upper
year boundary needs no year-10000 civil construction. This is an explicit week
model, not a claim of qualified native behavior across all settings.

The Monday/four-day model preserves Microsoft's documented year-end anomaly:
DatePart('ww', #2003-12-29#, 2, 2) returns 53 while December 30/31 return 1.
It must not be advertised as ISO week numbering. The [Access DatePart reference](https://support.microsoft.com/en-us/access/functions/datepart-function),
[VBA reference](https://learn.microsoft.com/en-us/office/vba/language/reference/user-interface-help/datepart-function)
and [Microsoft's documented problem and output](https://learn.microsoft.com/en-us/previous-versions/troubleshoot/microsoft-365/microsoft-365-apps/access/functions-return-wrong-week-number)
provide the semantic basis and specific anomaly golden. No sample code from the
article is incorporated. Other first-day/week-rule year boundaries, legacy
fraction rounding, NULL/error precedence and Extended behavior still need actual
Access comparison. Independent calendar tests are separate from that qualification.

All provided arguments evaluate eagerly in written order and participate in
projection/correlation and aggregate/group validation. Wrong shapes, required
missing arguments, qualified calls, unsupported date conversion and explicit NULL
settings fail. NULL dates propagate only after interval/settings validation.
UPDATE continues to reject calls. Function/aggregate binding now lives in
`binding/calls.rs`; the expression walker retains composition and tree traversal.


## Checked numeric and Boolean conversion functions

CByte, CInt, CLng, CLngLng and CBool now use a closed parser-independent
conversion layer across scalar SELECTs, retrieval and qualified saved SELECTs.
They currently accept numeric and Boolean carriers; locale-dependent Text parsing,
date-to-number conversion, binary/GUID and expression-only Empty/Missing/Error
states remain unsupported. This is a bounded value profile, not the full VBA
conversion module. CStr/CDate/CLngPtr are still separate work; CCur/CDec are described below. CSng/CDbl extend this
profile as described below.

Integer conversions reuse existing exact half-even rounding with target checks
after rounding: Byte, Int16, Int32 and Int64 respectively. Currency/Decimal never
pass through float; Int64 boundaries and coefficients beyond 2^53 remain exact.
Non-finite floats and overflow fail explicitly. The Boolean-to-Byte exception
maps true to 255 and false to 0, while numeric -1 still overflows Byte. Other
integer targets retain Boolean true=-1/false=0. The internal nullable coercion
helper keeps its prior signed numeric behavior and is not changed by this wrapper.

CBool tests exact numeric zero/nonzero without rounding/narrowing and checks
floating finiteness; signed zero is false, tiny nonzero Decimal/IEEE values are
true. NULL conversion fails with InvalidNullConversion, rather than producing
stored NULL, zero or false. Use a deliberate replacement such as CInt(Nz(Score,0))
when that is the intended query behavior. Eager IIf cannot suppress an error in
an unselected conversion operand. All calls require one actual expression,
participate in projection/correlation/group rules and remain rejected in UPDATE.
CLngLng is TableCraft's explicit Int64 conversion; this does not enable VBA or
infer a pointer-width CLngPtr/platform runtime contract.

Microsoft's [Access conversion reference](https://support.microsoft.com/en-gb/access/functions/type-conversion-functions),
[VBA conversion rules](https://learn.microsoft.com/en-us/office/vba/language/concepts/getting-started/type-conversion-functions),
[Boolean representation](https://learn.microsoft.com/en-us/office/vba/language/reference/user-interface-help/boolean-data-type)
and [invalid NULL use](https://learn.microsoft.com/en-us/office/vba/language/reference/user-interface-help/invalid-use-of-null-error-94)
provide the target/rounding/error basis. The Boolean-to-Byte exception is also
recorded in Wine's [independent VBScript API tests](https://github.com/wine-mirror/wine/blob/master/dlls/vbscript/tests/api.vbs)
and Microsoft's [Visual Basic conversion description](https://learn.microsoft.com/en-us/dotnet/api/system.boolean).
The latter is a related-language reference, not a substitute for Access evidence.
No Wine implementation/tests or runtime is copied into TableCraft. These are
explicit documented contracts with independent integration checks; native Access
SQL NULL/Boolean/platform/coercion/error behavior remains unqualified.


## Explicit Single and Double conversions

CSng/CDbl now extend the shared conversion layer with Single/Double results for
numeric and Boolean carriers. NULL still fails with InvalidNullConversion;
Text/date/binary/GUID conversion remains unsupported. Arity, qualifier, eager-error,
grouping and UPDATE restrictions are unchanged. Integer inputs convert directly
to the target IEEE width, rounding to the nearest representable value with ties
to even. Boolean true remains numeric -1. Single-to-Double widening is exact;
finite same-width input retains its bits, including signed zero and subnormals.

This explicit request may lose precision: CSng(16777217) returns 16777216 and
CDbl(9007199254740993) returns 9007199254740992. Implicit arithmetic conversion
retains its existing PrecisionLoss guard; these functions do not relax it.
Finite Double input outside Single's finite magnitude range fails with Overflow,
even if a raw cast might round back to the maximum Single. In-range narrowing
uses IEEE rounding, including gradual underflow to signed subnormal/zero. Non-finite
inputs fail with NonFinite; no NaN/Infinity is introduced as a successful result.
Native Access edge-range/underflow/rounding behavior still requires comparison.

Currency/Decimal use their existing exact canonical decimal spelling in a bounded
stack buffer derived from the field precision envelope. Rust's standard parser
converts that exact spelling directly to the requested width. No coefficient-as-
float division, intermediate Double for CSng, heap-backed formatting, text-value
allocation or general decimal arithmetic engine is added. The fixed formatting/
parse work is bounded by the 96-bit coefficient and scale-28 limits. Midpoint and Currency
goldens catch double rounding that a coefficient/divisor approach would introduce.

[Microsoft's conversion reference](https://learn.microsoft.com/en-us/office/vba/language/concepts/getting-started/type-conversion-functions)
provides the function targets/range basis. [Rust's numeric cast specification](https://doc.rust-lang.org/reference/expressions/operator-expr.html#numeric-cast)
and [standard Single parser](https://doc.rust-lang.org/std/primitive.f32.html#impl-FromStr-for-f32)
provide the implementation's IEEE conversion mechanisms. Independent exact-rational
models compare result bits across Jet/ACE getters; they establish this explicit
numeric profile, not native Access SQL or OleAut rounding compatibility.

## Explicit Currency conversion

CCur uses the shared conversion node for scalar evaluation, retrieval and qualified
saved SELECTs. Currency inputs retain their complete scaled-i64 identity. Boolean
true becomes -1.0000; integer and field Decimal inputs convert exactly before
rounding to four fractional places with midpoint ties to even. Bounds are checked
after rounding against signed i64 scaled by 10,000. NULL raises
InvalidNullConversion; unsupported Text/date/GUID/binary inputs raise TypeMismatch;
non-finite IEEE inputs and out-of-range results fail explicitly.

Single/Double inputs use rust_decimal 1.43.0's retained conversion, followed by the
same explicit rounding. This avoids normalization to a short decimal spelling;
conversion remains bounded by the library's 96-bit coefficient and scale 28.
For example, Double 0.00005 is slightly above the mathematical decimal midpoint
and produces 0.0001, while exact Decimal 0.00005 rounds to zero. Exactly
representable Double 0.03125 rounds to 0.0312; its next representable neighbor
rounds to 0.0313. Tiny finite inputs round to zero. These are tested engineering
rules, not observed native Access/OleAut edge behavior. Native rounding and
range-check ordering still require Windows comparison.

The dependency stays behind TableCraft values; core representation, precision,
scale, equality, Serde, field/index codecs and implicit promotion are unchanged.
Remaining Currency operations and broader promotion rules are separate work. Calls retain existing arity/eager-error/budget and UPDATE restrictions.
[Benchmarks](testing/CURRENCY_CONVERSIONS.md) record absolute conversion costs.

Primary references: [Microsoft conversion functions](https://learn.microsoft.com/en-us/office/vba/language/concepts/getting-started/type-conversion-functions),
[rust_decimal retained conversion](https://docs.rs/rust_decimal/1.43.0/rust_decimal/struct.Decimal.html#method.from_f64_retain)
and [explicit rounding](https://docs.rs/rust_decimal/1.43.0/rust_decimal/struct.Decimal.html#method.round_dp_with_strategy).

## Explicit Decimal conversion and expression range

CDec extends the shared conversion node across scalar SELECTs, retrieval and
qualified saved SELECTs. Results use the full 96-bit signed-magnitude Decimal
carrier (maximum coefficient 2^96-1, scale 0..=28). Exact integer/Boolean/Currency
inputs avoid float intermediates; true becomes -1, integers retain scale zero,
and Currency retains scale four. Decimal inputs preserve coefficient and scale.
This includes the bounded 29-digit top band beyond any Access field declaration.
For example SELECT CDec(2^95) produces 39614081257132168796771975168 exactly.
Unary negation, comparison and existing explicit conversions accept this envelope.

Floating inputs use rust_decimal 1.43.0 FromPrimitive's normalized conversion;
Double/Single 0.1 produce Decimal 0.1. This intentionally differs from CCur's
retained-IEEE path. Very small values can round to zero; non-finite values fail
with NonFinite and out-of-range values fail with Overflow. The library's binary
conversion/normalization is a documented engineering profile, not a native
Access/OleAut qualification or a promise of a fixed number of significant digits.
Boundary/range-check ordering and native floating conversions still need Windows
comparison. Locale-sensitive Text/date/GUID/binary conversions remain unsupported;
NULL raises InvalidNullConversion. Arity, eager-error and UPDATE gates remain.

The private library boundary is shared with CCur; dependency versions/features
are unchanged. Native Decimal schema/codec checks continue to enforce precision
1..=28 and exact declared scale. Native index sign bands derive from field bounds,
not the expanded scalar maximum. Wider values cannot enter native write previews.
Core Serde validates the larger scalar constructor without bypassing field checks.
Decimal arithmetic and aggregates now use the checked kernel below; full implicit
Variant promotion remains separate work.

References: [Microsoft Decimal](https://learn.microsoft.com/en-us/office/vba/language/reference/user-interface-help/decimal-data-type),
[Microsoft conversion functions](https://learn.microsoft.com/en-us/office/vba/language/concepts/getting-started/type-conversion-functions)
and [rust_decimal FromPrimitive](https://docs.rs/rust_decimal/1.43.0/rust_decimal/struct.Decimal.html#impl-FromPrimitive-for-Decimal).

## Checked Decimal arithmetic

If either numeric operand is Decimal, +, -, * and / return Decimal through
rust_decimal 1.43.0 checked operations. Integer/Boolean/Currency/Decimal operands
import exactly, including values beyond Double precision; Boolean true is -1.
Single/Double import through CDec's normalized conversion policy before the
operation. No intermediate Double is used for fixed inputs. Text, dates, GUIDs
and binary remain explicit type errors. NULL propagates under the existing rules;
expression evaluation remains eager before the kernel is called.

The result is bounded to 96-bit magnitude and scale 0..=28. The library can reduce
scale and round to nearest with midpoint ties to even when intermediates do not
fit. Nonterminating quotients are bounded approximations, not exact rational
results: 1/3 ends in 3333 and 2/3 ends in 6667 at scale 28. Multiplying 1e-28 by
0.5 rounds to zero; 3e-28 by 0.5 rounds to 2e-28. At the magnitude ceiling,
MAX+0.4 rounds to MAX while MAX-0.5 rounds to MAX-1. Unrepresentable results fail
with Overflow; converted zero divisors fail with DivisionByZero, and non-finite
floating inputs fail with NonFinite. In particular, float-to-Decimal underflow
can turn a nonzero float divisor into zero before division.

Result scale follows the arithmetic library; arithmetic does not promise a
particular retained scale or normalize every result. The observed 1/2 result is
0.50, while 1/8 is 0.125. Core representation equality remains scale-sensitive;
query grouping, duplicate removal and comparisons retain numeric equality. Stored
field declarations/codecs remain separate and impose their own checked scale.

Exponentiation converts numeric operands to finite Double and uses the existing
Double power kernel; it does not return Decimal. Invalid real powers and zero to
a negative power retain Domain/DivisionByZero errors. Integer division and Mod
retain the existing half-even-to-Int64 coercion and checked range contracts.
Decimal/float comparisons are not enabled by arithmetic support. Currency-only
operations and the non-Decimal promotion ladder are unchanged.

These are a bounded library-backed implementation and independent rational/golden
checks, not native Access/OleAut rounding or SQL aggregate qualification. General
Variant/declared-type promotion, locale coercion and native comparison remain
required. Library versions/features and core/native storage representations are
unchanged. [Arithmetic benchmarks](testing/DECIMAL_ARITHMETIC.md) measure the kernel.

Primary references: [Microsoft addition](https://learn.microsoft.com/en-us/office/vba/language/reference/user-interface-help/plus-operator),
[subtraction](https://learn.microsoft.com/en-us/office/vba/language/reference/user-interface-help/minus-operator),
[multiplication](https://learn.microsoft.com/en-us/office/vba/language/reference/user-interface-help/operator),
[division](https://learn.microsoft.com/en-us/office/vba/language/reference/user-interface-help/forwardslash-operator),
[exponentiation](https://learn.microsoft.com/en-us/office/vba/language/reference/user-interface-help/caret-operator)
and [MS-VBAL arithmetic](https://learn.microsoft.com/en-us/openspecs/microsoft_general_purpose_programming_languages/ms-vbal/e070115f-8d40-40cf-ac6d-ab18b9c6c906).
These define result-type/overflow direction; [rust_decimal checked operations](https://docs.rs/rust_decimal/1.43.0/rust_decimal/struct.Decimal.html#method.checked_div)
and numerical tests define the current bounded rounding profile.

## Numeric built-ins

The closed [Abs/Sgn/Int/Fix/Round profile](MATH_FUNCTIONS.md) now works in the
materialized-value API, scalar evaluator and retrieval. Decimal/Currency retain
exact paths; Int floors, Fix truncates and Round uses half-even rounding. Carrier,
NULL, argument/range and floating-point policies are explicit. No VBA execution,
locale parsing or full native Variant promotion is inferred.
