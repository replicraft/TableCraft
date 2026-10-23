# Scalar expression evaluation

LCase/UCase now use an explicit execution-local language and streamed ICU output;
see [text casing](TEXT_CASING.md) for policies, limits and pending native qualification.

`crates/query/tablecraft-expressions` owns evaluated scalar rules over core
`Value` carriers. Its default build and direct scalar APIs are independent of SQL
parsing, database connections, GUI hosts and async runtimes. The optional `sql`
feature adds an evaluator of TableCraft-owned expression syntax. The syntax parser remains in
`tablecraft-sql`; [tablecraft-query](QUERY_EXECUTION.md) now supplies separate
binding, query plans and bounded row execution.

The [Access built-in tracker](ACCESS_BUILTINS.md) lists implemented subsets,
missing functions and their remaining gaps.

This is an initial, explicit scalar profile, not complete Jet/ACE or VBA Variant
semantics. Core values describe materialized carriers, not the declared/Variant
type metadata required to infer every native promotion. Documentation-derived
tests are not observed native Access results.

## Implemented rules

| Area | Implemented behavior |
| --- | --- |
| Truth | TRUE/FALSE/unknown, Boolean/NULL conversion, NOT/AND/OR/XOR/EQV/IMP; predicate selection accepts only TRUE |
| Comparison | Either NULL returns unknown; SQL predicates add [contextual numeric/date Text and fixed/floating conversion](NUMERIC_PREDICATES.md), separate from strict key equality and `Value::Eq` |
| Exact numerics | Boolean TRUE becomes -1 and FALSE becomes 0 in the numeric domain; Byte/Int16/Int32/Int64, Currency and Decimal compare without floating conversion or scale-alignment overflow |
| Floating comparison | Finite numeric comparison, signed zeros equal; Double rounds to Single precision when those two carriers are compared; overflow/non-finite values rejected |
| Integral coercion | Explicit Byte/Int16/Int32/Int64 targets; fractions round halves to even; bounds checked after rounding; NULL remains NULL |
| Integral arithmetic | SQL arithmetic and unary minus use [materialized carrier selection](ARITHMETIC_PROMOTION.md); direct checked Int64 kernels retain their separate contracts |
| Floating arithmetic | Qualified Single results are retained or widen to Double, including division; power returns finite Double |
| Integer division/remainder | `\` and Mod select a checked operand/result width before half-even conversion; quotient truncates toward zero and remainder retains its dividend sign |
| Decimal arithmetic | Decimal operands select checked 96-bit +, -, *, / with exact fixed inputs and normalized floating imports; ^ returns Double; bounded library rounding is explicit below |
| Date arithmetic | Explicit legacy DATE serial +/-, unary signs and result promotion; [policy and native limits](DATE_ARITHMETIC.md) |
| Currency arithmetic | SQL +, -, * use checked operand/result selection; exact kernel contracts and four-place rounding remain explicit in [promotion](ARITHMETIC_PROMOTION.md) |
| Numeric Text arithmetic | [Explicit contextual conversion](ARITHMETIC_TEXT.md) for mixed addition, other numeric operators and unary signs; Text + Text remains concatenation |
| LIKE | Explicit ANSI-89/ANSI-92 mode, host matching profile, lists/ranges and three-valued NOT; bounded scalar matching by default or explicit UTF-16 weighted literal/list spans through General policies ([contract](PATTERN_MATCHING.md)) |
| Concatenation | `&` accepts Text/Boolean and explicit-policy numeric/date formatting; one NULL becomes empty, two stay NULL. Text `+` propagates either NULL. [Scope and limits](CONCATENATION.md). |

Decimal scale and core carrier equality remain unchanged. Exact numeric comparison
compares signs, whole parts and fractional digits without rescaling a coefficient
past its range. Strict comparison refuses inexact integer-to-float conversion with `PrecisionLoss`.
Double-selected materialized arithmetic separately uses checked nearest-even
operand conversion under the [promotion profile](ARITHMETIC_PROMOTION.md#rounded-operands-for-double-selected-arithmetic).

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

SQL arithmetic and unary minus now follow the [materialized promotion
profile](ARITHMETIC_PROMOTION.md), with operator-specific division/rounding selection.
Direct checked kernels retain their explicit result contracts; complete declared/
Variant promotion remains open. The
nullable `coerce_integer` kernel propagates SQL NULL and checks signed -1 numeric
Boolean representation against the chosen target range. The explicit conversion
functions below wrap it with distinct NULL and Boolean-to-Byte rules. Explicit
coercion can select a narrower integral result. Decimal binary arithmetic follows
the bounded profile below. The direct checked kernel still refuses Currency
division, Currency * Currency and Currency/float combinations outside its profile.
SQL materialized promotion supplies the qualified arithmetic cases above; complete
declared-type promotion, literal typing and native operator/error behavior remain
unfinished. Decimal/Currency can still participate in exact comparison and explicit
integral coercion (including rounded `\`/Mod operands).

[Exact integer logical operators](INTEGER_LOGIC.md) now complement/combine integer
bits while retaining Boolean/NULL tables. Final read predicates additionally accept
numeric/legacy DATE zero/nonzero. [Rounded logical coercion](INTEGER_LOGIC.md#rounded-numeric-and-text-operands)
now uses checked Long/LongLong targets, and SQL Text operators/predicates require
an explicit number policy. Full Variant/native result-type inference remains open. Numeric text parsing, mixed
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
[Unsuffixed hexadecimal/octal literals](RADIX_LITERALS.md) now materialize as
signed Integer/Long using a bounded unsigned 32-bit magnitude. Bare decimal integers
outside the i64 syntax carrier, unsupported radix widths, non-finite floats,
underflow of a nonzero float literal to zero and numeric spellings longer than
128 bytes fail explicitly. Ordinary floating literal rounding to
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

`EvalLimits` clamps maximum depth to 32 nodes. Each Text value defaults to 65,536
UTF-8 bytes; explicit `max_text_bytes` can request up to 1 MiB, including manually
constructed trees. Callers can lower either limit; cumulative allocation/work
budgets remain independent. See [larger payloads](LARGE_PAYLOADS.md).
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
selects falsepart. [Selector coercion](CONDITIONAL_FUNCTIONS.md#date-text-and-explicit-expression-states)
now accepts legacy DATE and explicit-policy Text; GUID/binary/Extended truth
conversion remains unsupported. Text conversion uses the supplied number profile.
Logical operators additionally support the [exact integer profile](INTEGER_LOGIC.md);
selector Text/date coercion does not extend their operator inputs. Branches retain exact materialized values;
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
  an Int32 UTF-16 length and NULL for NULL. Len/LenB now accept ordinary scalar
  string-expression sources through [explicit CStr policies](BYTE_STRING_FUNCTIONS.md#scalar-length-expressions);
  variable/UDT storage-size overloads remain absent. Trimming, casing and Left/Right/Mid use
  explicit CStr policies for ordinary scalar sources; see
  [scalar trimming/casing](TEXT_CASING.md#scalar-sources) and
  [scalar slicing](SLICE_COUNTS.md#scalar-string-sources).
- Trim variants remove only U+0020 spaces, preserving tabs, newlines, nonbreaking
  spaces and embedded NUL. They do not normalize Unicode or perform collation.
- Lengths and positions count UTF-16 code units, not UTF-8 bytes, grapheme clusters
  or Unicode scalar values. Left/Right accept a nonnegative length and clamp it
  to the input length. Mid accepts a positive one-based start and optional
  nonnegative length; omitted length means the remainder. Zero-length selections
  and starts beyond the input return empty Text. [Count coercion](SLICE_COUNTS.md)
  now accepts checked half-even numeric/Boolean and legacy DATE Long conversion;
  SQL numeric Text positions require an explicit NumberFormat. Pure Text position
  conversion and NULL counts retain their explicit refusal profile.
- Text is currently valid Unicode in Rust String. A nonempty UTF-16 slice that
  splits a surrogate pair returns `UnrepresentableText`, without replacement
  characters or rounding to a different boundary. Lossless unpaired-surrogate
  expression results remain a compatibility gap.

Wrong arity, wildcard/missing arguments and qualifiers fail during binding,
including for empty sources. Built-in operands participate in pruning, correlation,
aggregate detection and grouping validation. UPDATE still rejects function calls.
Byte-string functions now have their [separate lossless carrier profile](BYTE_STRING_FUNCTIONS.md),
and casing uses [explicit ICU language data](TEXT_CASING.md). Missing string aliases,
complete Variant coercion and native Access/NLS qualification remain open.

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

Year, Month, Day, Hour, Minute and Second accept one date/NULL expression or
supported numeric/Text source and return an Int16 component or NULL. They share
parser-independent date kernels and the evaluator's bounded CDate source conversion
across scalar evaluation, table retrieval and qualified saved SELECTs. No clock,
timezone or external runtime is consulted. Only Gregorian behavior is implemented;
Hijri calendar selection remains unsupported. Wrong shapes and qualified calls fail
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

Counts reuse checked Long conversion: numeric/Boolean and raw DATE values round
to the nearest integer with ties even, preserving exact Decimal/Currency rounding.
SQL numeric Text counts require the selected NumberFormat. Count overflow, non-finite floating values and unsupported carriers fail
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
the year. Numeric/Text SQL sources now share the bounded CDate service under
[explicit source-conversion rules](DATE_INTERVALS.md); Hijri calendars remain unsupported.

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
setting 0 now resolves the corresponding caller-issued [host week setting](WEEK_SETTINGS.md);
missing context still raises CalendarPolicyRequired.
The first-week rule labels weeks within years; valid choices do not change the
sequence of calendar-week boundaries counted by ww. All supplied arguments still
participate in evaluation, projection/correlation and aggregate/group checks.

The primary reference describes both a starting-weekday anchor for w and an
influence of firstdayofweek on w. Nondefault first-day settings for w are therefore
explicitly unsupported pending native comparison, rather than assigned guessed
semantics. Nondefault ww first-day settings have the explicit boundary model above.
NULL date operands propagate NULL after interval/settings and bounded conversion
of both source operands; all supplied
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
1–7 and first-week rules 1–3. System setting 0 resolves explicit [host week
context](WEEK_SETTINGS.md) and fails if the corresponding setting is absent. Week numbering uses checked Jiff Gregorian dates with small
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
They accept numeric/Boolean carriers, [legacy DATE serials](DATE_NUMERIC.md) and
[explicit numeric Text profiles](NUMBER_FORMAT.md). Complete regional behavior,
Extended-date numerical conversion, binary/GUID and
expression-only Empty/Missing states remain unsupported. Standard
[CVErr Error codes](VARIANT_ERRORS.md) now support explicit numeric conversion;
opaque host mappings and broader propagation remain unfinished. This is a bounded value profile, not the full VBA
conversion module. [CStr](CSTR.md) uses separate explicit output policies;
[CDate](DATE_CONVERSIONS.md) now has explicit date-conversion policies; CLngPtr
remains separate work. CCur/CDec are described below. CSng/CDbl extend this
profile as described below.

Integer conversions reuse existing exact half-even rounding with target checks
after rounding: Byte, Int16, Int32 and Int64 respectively. Currency/Decimal never
pass through float; Int64 boundaries and coefficients beyond 2^53 remain exact.
Non-finite floats and overflow fail explicitly. The Boolean-to-Byte exception
maps true to 255 and false to 0, while numeric -1 still overflows Byte. Other
integer targets retain Boolean true=-1/false=0. The internal nullable coercion
helper keeps its prior signed numeric behavior and is not changed by this wrapper.

For materialized numeric inputs, CBool tests exact zero/nonzero without
rounding/narrowing and checks
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
Text uses the [explicit number-format profile](NUMBER_FORMAT.md); date/binary/GUID
conversion remains unsupported. Arity, qualifier, eager-error,
grouping and UPDATE restrictions are unchanged. Integer inputs convert directly
to the target IEEE width, rounding to the nearest representable value with ties
to even. Boolean true remains numeric -1. Single-to-Double widening is exact;
finite same-width input retains its bits, including signed zero and subnormals.

This explicit request may lose precision: CSng(16777217) returns 16777216 and
CDbl(9007199254740993) returns 9007199254740992. Double-selected materialized
arithmetic now reuses the Double converter; strict numeric kernels and key comparison retain their PrecisionLoss guard. Contextual
LongLong/Single, LongLong/Double and LongLong/Text predicates separately round
both operands to Double; integer-only predicates remain exact.
Finite Double input outside Single's finite magnitude range fails with Overflow,
even if a raw cast might round back to the maximum Single. In-range narrowing
uses IEEE rounding, including gradual underflow to signed subnormal/zero. Non-finite
inputs fail with NonFinite; no NaN/Infinity is introduced as a successful result.
Native Access edge-range/underflow/rounding behavior still requires comparison.

The numeric-carrier path for Currency/Decimal uses the existing exact canonical
decimal spelling in a bounded
stack buffer derived from the field precision envelope. Rust's standard parser
converts that exact spelling directly to the requested width. That numeric-carrier path needs no coefficient-as-float division, intermediate
Double for CSng, heap-backed formatting or text-value allocation. The fixed formatting/
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
InvalidNullConversion. Text uses the [explicit number-format profile](NUMBER_FORMAT.md);
legacy DATE inputs use the stored serial; Extended-date/GUID/binary inputs raise TypeMismatch;
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
comparison. Text uses the [explicit number-format profile](NUMBER_FORMAT.md);
legacy DATE serials now use the same normalized Double path; complete regional
and Extended-date/GUID/binary conversions remain unsupported;
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

The closed [numeric built-in profile](MATH_FUNCTIONS.md) now works in the
materialized-value API, scalar evaluator and retrieval. Decimal/Currency retain
exact paths; Int floors, Fix truncates and Round uses half-even rounding. Carrier,
NULL, argument/range and floating-point policies are explicit. No VBA execution,
locale parsing or full native Variant promotion is inferred.

[Partition](PARTITION.md) now produces padded numeric bucket labels through the
metered TextOutput contract, with explicit Long conversion, NULL/domain rules and
bounded stack formatting. Native padding and edge behavior remain unqualified.

## Eager variable-arity conditionals

[Switch and Choose](CONDITIONAL_FUNCTIONS.md) now share a move-based selector
across scalar evaluation and retrieval. Every expression evaluates in written
order; first-match/index selection retains the exact chosen carrier. Complete
arity, explicit truth/index, NULL and deferred-error contracts remain documented
separately from native Access qualification.

Sqr/Exp/Log/Sin/Cos/Tan/Atn now explicitly promote numeric inputs through the
existing CDbl policy and return finite Double results. Sqr/Log enforce their
mathematical domains; overflow fails rather than publishing infinity. The
[mathematical-function contract](MATH_FUNCTIONS.md#double-valued-mathematical-functions)
records NULL, underflow, precision and pending native Access qualifications.

## Gregorian DateSerial and TimeSerial

The closed [date/time constructor profile](DATE_CONSTRUCTORS.md) now accepts three
eager, checked Int16 numeric/raw DATE and explicit-policy SQL numeric Text
arguments and returns a validated legacy Date.
DateSerial supports relative Gregorian components and the explicit default
1930–2029 two-digit-year window; TimeSerial retains signed day carries. No ambient
clock, locale parser or VBA runtime is invoked. Explicit year-window overrides are available through evaluator/query options and
`--two-digit-year-max`; Hijri and native NULL/negative-time/rounding qualification
remain explicit limitations.

## Explicit StrComp comparison modes

`StrComp` shares checked mode selection and Int16/NULL results between scalar
evaluation and retrieval. Binary UTF-16 comparison needs no host; text/database
and omitted default modes require explicit comparison policies. The CLI ordinal
policy supplies database/default comparison, not locale text comparison.
[Comparison contracts and remaining native limits](STRING_COMPARISON.md) describe
argument conversion, eager errors, metered scans and independent evidence.

## Bounded substring search

`InStr` and `InStrRev` share UTF-16 position scans and explicit search providers.
Forward comparison defaults require a host policy; reverse comparison defaults
to binary. Optional slots retain their function-specific meaning, and provider
matching supports unequal-length locale spans without equating search with
ordering. [Search contracts and evidence](STRING_SEARCH.md) record current
arguments, budgets, SQL integration and native qualification gaps.

## Bounded text replacement

`Replace` now uses a shared borrowed recipe and bounded streaming output across
scalar SELECTs and retrieval. Optional start/count/comparison slots retain their
own defaults; results return only the requested suffix, and inserted text is not
searched again. [Replacement contracts](TEXT_REPLACEMENT.md) describe output-byte
charging before fallible growth, provider-span validation and native limits.

## Bounded repeat and reverse functions

`Space`, `String` and `StrReverse` share metered construction recipes with the
existing bounded text-output sink. Current counts, BMP/ASCII character selection,
NULL rules and surrogate/code-page limitations are explicit in
[construction contracts](TEXT_CONSTRUCTION.md). Eager arguments, grouped/correlated
execution, parameter reuse and buffered errors retain their ordinary query rules.

## Execution-local character codes

[Explicit character-code pages](CHARACTER_CODE_PAGES.md) now extend numeric
`String` byte codes beyond ASCII under a validated per-execution setting.
Unselected high bytes and unmapped selected-page bytes fail explicitly. Prepared
plans, nested calls and worker requests retain isolated settings; existing Text
and native storage decoding remain separate.

## Character-code functions

[Chr/Asc and ChrW/AscW](CHARACTER_FUNCTIONS.md) now share checked single-byte
code-page and signed UTF-16 profiles across scalar evaluation and retrieval.
Character output charges UTF-8 bytes before allocation; numeric codes retain
Int16. Unmappable selected-page input and unrepresentable surrogate output fail
without replacement. Full coercion/DBCS/native qualification remain incomplete.

[Val](VAL.md) now parses bounded Text numeric prefixes with explicit radix and
type-suffix checks. Results use the current finite Double profile; this does not
enable general implicit Text-to-number coercion or establish native compatibility.

[Hex, Oct and Str](NUMERIC_TEXT.md) now provide numeric-to-Text recipes with
checked conversion, NULL propagation and charged output. Exact fixed values are
preserved; native negative widths, float spelling and broader coercions remain
unqualified. This does not replace CLI rendering; [CStr](CSTR.md) has its own
explicit output policies and general Format remains unimplemented.

[Numeric Text conversion](NUMBER_FORMAT.md) is now execution-local across scalar,
bound/nested query and optional worker paths. Missing policy is explicit; input is
validated completely and normalized under shared byte/work/cancellation limits.
Val and implicit arithmetic retain their separate policies.

[IsNumeric](ISNUMERIC.md) now recognizes materialized numeric/Boolean types and
whole numeric Text under the selected NumberFormat. NULL/date/binary/GUID values
return FALSE. Malformed/overflowing numeric Text is separate from propagated host,
allocation, cancellation and limit failures. It does not change eager evaluation.

[CStr](CSTR.md) now copies Text, spells Boolean values and emits exact fixed/finite
numeric or second-precision date/time Text under separate explicit profiles.
NULL raises InvalidNullConversion. Materialization shares budgets/cancellation;
full regional, Variant and native precision/rounding behavior remain open.

[CDate, DateValue and TimeValue](DATE_CONVERSIONS.md) now convert/project current
scalar/date carriers and parse Gregorian Text under explicit profiles. Existing
DATE bits/fractions remain exact where retained; Text and extended components are
projected before legacy narrowing. Omitted years require caller-issued ReferenceYear;
full native regional/Variant and precision/error behavior remain unqualified.

[IsDate](ISDATE.md) now recognizes supported Gregorian dates/times with explicit
input/year policies and checked date/numeric carriers. NULL returns FALSE; missing
policies and operational failures remain errors. Full regional/Variant behavior
and native qualification remain open.

[FormatDateTime](DATE_FORMATTING.md) now supplies all five named modes under
explicit Gregorian profiles. It shares CStr rendering and CDate input policy,
with private charged normalization and request-local year settings. Full regional/
Variant behavior and native formatting precision/error qualification remain open.

[MonthName and WeekdayName](CALENDAR_NAMES.md) now provide explicit English/German
full/short names through typed Text nodes. WeekdayName system/default numbering
requires caller-issued FirstDayOfWeek; SQL first-day arguments override it. Settings
remain execution-local. Full regional/Variant and native NLS qualification remain open.

[Standalone Weekday](WEEKDAY.md) now supplies checked Gregorian relative numbering
with numeric/Text/date input, optional first-day expressions and caller-issued system
context. Its owned plan node retains date/option dependencies, grouping identity,
aggregates, correlations and qualified multivalue predicates. Native NLS/Variant/
calendar, precision and error qualification remain open.

[FormatNumber/FormatPercent/FormatCurrency](STYLED_NUMBER_FORMATTING.md) now provide
fixed-place output under explicit NumberFormat profiles, checked optional controls
and private normalization. Exact fixed values and bounded normalized IEEE magnitudes
use the shared formatter; output settings remain request-local. Full NLS/Variant
behavior and native precision/rounding/error qualification remain open.

[Date()/Time()/Now()](CLOCK_FUNCTIONS.md) now use caller-issued clock providers.
Each executed call reads once; binding/inspection never read clocks. Callback failures
remain eager, plans capture no readings, and volatile group keys do not imply
structural equivalence. CLI hosts own Jiff system time/zone discovery and --clock
fixed overrides; libraries/worker requests require explicit providers. Native
precision, platform timezone and invocation/grouping qualification remain open.

## Fixed-rate financial calls

[Financial annuities](FINANCIAL_FUNCTIONS.md) now implement Pmt/PV/FV over explicit
numeric/Boolean operands with Double results, shared optional-slot normalization,
eager argument evaluation and bounded constant work. No VBA, locale or financial
provider is invoked. Full native coercion/error-order/rounding remains pending.

## Explicit periodic cashflows

[NPV/IRR/MIRR](CASHFLOW_FUNCTIONS.md) now consume bounded explicit numeric
`Array(...)`/`VBA.Array(...)` inputs under shared allocation/work/cancellation
limits. Every zero and period is retained; supplied expressions evaluate in
written order before conversion. Arrays remain financial inputs rather than stored
scalars or general query results. General Variant arrays, native SQL admissibility,
coercion and numerical equivalence remain unqualified.

## Type/state inspection and array bounds

[IsArray/IsEmpty/IsError/TypeName/VarType/LBound/UBound](VALUE_INSPECTION.md)
now consume the explicit materialized profile, including direct Array/VBA.Array
intermediates. NULL, Empty and Error remain distinct; operational failures are
not caught. Type names use the normal bounded Text output path; binding remains
metadata-only. General arrays, native-only subtype mappings, native SQL admission
and broader Variant/COM behavior remain explicit gaps.

## Hyperlink query components

[HyperlinkPart](HYPERLINK_PARTS.md) extracts stored hyperlink components through
borrowed Text results. Queries require an explicit selector; no address is opened,
normalized or used to resolve a provider. Current edge/NULL/selector rules and
native qualification limits remain explicit in the owning contract.

`StringFromGUID` now supplies the [inverse GUID conversion](GUID_CONVERSIONS.md#stringfromguid-inverse-conversion)
for exactly sixteen Binary bytes and stored Guid values. The provisional output
is bounded uppercase literal Text, with no implicit GUID/Binary comparison or
identifier generation. Native spelling/coercion and query qualification remain open.

[Closed Eval expressions](DYNAMIC_EVALUATION.md) now parse one expression through
the existing owned SQL model, preflight dynamic names/effects, and reuse evaluator
context, depth and cumulative budgets. Nested CVErr data remains explicit; SQL
subqueries, application/member/UDF resolution and dynamic host names remain
outside this profile. Chr$ is a specific checked Chr spelling, not a generic
typed-function alias rule. Parser-free builds retain their existing dependency
boundary; the implementation lives under the optional SQL evaluator.

[CurrentUser](SESSION_FUNCTIONS.md) now reads explicit immutable
AccessSessionContext metadata through the evaluator. Missing context fails; the
function does not consult an OS/database provider or grant permissions. It uses
ordinary cancellation/work and charged Text allocation. Closed Eval inherits the
same context. The CLI explicitly composes an unsecured Admin expression session.

## Numbered diagnostic messages

[Error(number) and Error$(number)](ERROR_MESSAGES.md) now use the explicit
execution-local ErrorMessages provider and shared Text limits. Zero returns empty
Text without a catalog; valid undefined codes use the provider's selected
fallback. The CLI selects optional Fluent-backed candidate English data.
Missing providers, failed providers and operational errors remain distinct.
Omitted latest-runtime-error state remains deferred; native Access wording,
code coverage, locale and sandbox qualification remain open.

## Contextual conditional conversion

[IIf/Switch/Choose](CONDITIONAL_FUNCTIONS.md#date-text-and-explicit-expression-states)
now share legacy DATE truth/index conversion and explicit NumberFormat Text
conversion through Evaluator's contextual selection methods. Semantic conversion
errors preserve eager argument evaluation; operational failures stop and latch
immediately. The typed expression API handles Empty without storing it as NULL;
Error/Missing condition/index inputs remain invalid. Selected Error/payload data
retains its carrier. This does not change SQL logical/bitwise predicate profiles.

## Rounded slicing counts

[Left/Right/Mid positions and lengths](SLICE_COUNTS.md) now reuse checked Long
conversion for numeric and legacy DATE carriers. SQL numeric Text counts require
the explicit number policy and share normalization/output budgets. The source is
borrowed without an intermediate copy. Source Text/NULL restrictions, current
NULL/domain precedence and surrogate-splitting refusal remain explicit; this does
not broaden all text-function arguments or establish native Variant compatibility.

DatePart/DateDiff/Weekday and general Format now share [checked DATE and SQL numeric
Text week options](WEEK_SETTINGS.md#contextual-week-option-conversion), preserving
function-specific NULL/interval/progressive validation and existing week models.
Public evaluator methods supply the same bounded conversion in scalar and read
query execution; parser-independent date kernels continue to refuse Text options.

## Date component source coercion

The SQL evaluator now routes Year/Month/Day/Hour/Minute/Second numeric and Text
sources through the existing bounded CDate service. Numeric and Boolean values
use its checked DATE serial conversion without requiring a date-output policy.
Text requires an explicit date syntax profile; numeric Text fallback additionally
uses the supplied number profile. Existing two-digit-year and caller-issued
reference-year policies remain in force. Neither language discovery nor the
machine's current date/locale supplies a missing policy.

NULL propagates before conversion. Existing DateTime/DateTimeExtended sources
bypass CDate so the existing whole-second legacy component profile and exact
Extended components remain intact, including dates before year 100 and final
fractional seconds. Converted sources use the legacy DATE component profile;
this does not change stored bits or make source Text a new stored date type.
Malformed inputs, overflow, missing policies, limits and cancellation remain
explicit failures. Written operand expressions remain eager.

The public evaluator apply_date_function method owns source checks and conversion;
the parser-independent DateFunction::apply kernel retains its explicit date/NULL
contract. Original source Text stays borrowed, and numeric normalization uses
shared cumulative byte/work limits. Full-query output remains buffered. This does
not broaden native predicate pushdown,
UPDATE calls or alternate calendars. No dependency or standalone date parser is added.

Microsoft's [Year declaration](https://learn.microsoft.com/en-us/office/vba/language/reference/user-interface-help/year-function)
and [Second declaration](https://learn.microsoft.com/en-us/office/vba/language/reference/user-interface-help/second-function)
explicitly allow numeric/string expressions representing dates/times and NULL
propagation; the other component declarations are linked above. The existing
[CDate profile](DATE_CONVERSIONS.md) supplies the bounded implementation. Full Variant,
native precision/locale/error semantics remain qualification gaps and the built-in
tracker retains Partial status.

Original evaluator/query/CLI controls cover number/date-policy separation,
regional and numeric spellings, NULLs, raw negative serials, exact Extended bypass,
prepared rebinding, filters/grouping/correlations and closed Eval. Exact normalization
ceilings and every observed transient cancellation checkpoint are exercised.
The [independent runner](testing/compare_date_component_sources.py) passes 1,406
Python Gregorian/native-field controls over unchanged priority-family inputs.
Native getters supply raw fields, not Access SQL results; native Access qualification
remains pending.

[DateAdd/DatePart/DateDiff source conversion](DATE_INTERVALS.md) now also composes
bounded CDate with their existing kernels. Prepared controls preserve interval/
count/week validation before source conversion; DateDiff validates both sources
before NULL propagation. This remains separate from full Variant/native acceptance.


[Contextual numeric predicates](NUMERIC_PREDICATES.md) now share the execution-local
number profile for mixed numeric/Text comparisons, plus checked Currency/Decimal
floating conversions. Pure key comparison, Text/Text policy routing and complete
declared/Variant inference remain distinct contracts.

Legacy DATE/Text predicates now reuse [CDate with explicit date policies](NUMERIC_PREDICATES.md#legacy-date-and-text),
including current year controls and optional numeric fallback. Extended precision,
strict set/group keys and stored carriers retain their existing boundaries.


Math calls now retain explicit Error data until their owning kernel requests
operand admission, after every supplied SQL expression evaluates. Error data
fails TypeMismatch, while later operational argument failures still propagate.
Host Empty follows reviewed zero targets; Round's Variant value slot and Missing
retain explicit refusals. The existing Value-only API shares the same kernels.
See [math argument states](MATH_FUNCTIONS.md#expression-errorempty-math-arguments-23-october-2026).

Space/String now admit eager Error/Empty/Missing operands through their
[owning NULL/count/character gates](TEXT_CONSTRUCTION.md#ordered-repetition-states).
Error data is not implicitly converted to its code; host Empty has an explicit
numeric zero target. Text character sources remain borrowed and literal.
Operational errors remain eager and bounded text output stays shared.

Len, LTrim/RTrim/Trim, LCase/UCase, Asc/AscW, StrReverse and Val now share
[explicit unary string state admission](CSTR.md#unary-string-expression-states).
Host Empty selects empty Text; Error data requires an explicit conversion and
Missing remains non-scalar. Owning NULL, policy, byte-string and Unicode rules
remain unchanged. Operational failures remain eager.

Chr/Chr$/ChrW now use [explicit code-state admission](CHARACTER_FUNCTIONS.md#character-construction-expression-states);
FormatNumber/Percent/Currency retain states through [ordered numeric options and
source conversion](STYLED_NUMBER_FORMATTING.md#ordered-numeric-formatting-states).
Host Empty has numeric zero targets, implicit Error conversion stays refused and
written operational failures remain eager. Other formatting/byte-string state
paths and native qualification remain separate acceptance work.

FormatDateTime now preserves states through [selector/NULL/policy/source stages](DATE_FORMATTING.md#ordered-expression-state-admission).
MonthName/WeekdayName retain [ordinal/abbreviation/range/first-day stages](CALENDAR_NAMES.md#ordered-expression-state-admission).
Host Empty has explicit numeric targets; Error conversion remains explicit and
operational failures stay eager. Existing caller-issued date/week policies and
budgeted renderers remain; general Format/raw-byte/StrConv state work is separate.
