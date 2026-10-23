# Implementation status

## Single-date expression-state admission

Weekday, CDate/CVDate, DateValue/TimeValue and the six Gregorian component calls
now retain expression states until deliberate admission in both SQL evaluators.
Weekday preserves first-day validation before date-state admission, early raw-Text
limits, eager operational failures and once-only volatile arguments. Error data
fails TypeMismatch and host Missing stays NonScalarExpression. Host Empty uses
DATE zero for CDate/CVDate, components and Weekday; Empty week options require
explicit system context. DateValue/TimeValue host Empty remains refused under
their distinct String-parameter boundary. Value-only callers share the existing
kernels. Numeric/Text policies, precision, NULL rules and formal return categories
are unchanged. See [conversion states](DATE_CONVERSIONS.md#single-date-expression-states-23-october-2026)
and [Weekday order](WEEKDAY.md#ordered-expression-state-admission-23-october-2026).
No payload clone, provider, dependency, stored state or write permission is added.

All-feature expression testing passes 595 tests including its doctest; standalone
minimal expression testing passes 345. Ten selected query tests and eight CLI
tests pass. Four evaluator and three query controls exercise host states, policy/
source precedence, eager derived failures, plan reuse, every observed successful
state-path cancellation checkpoint, cancelled refusals, work limits and once-only
Rnd operands. The independent Weekday runner passes 2,306 controls (2,242
source-free and sixty-four native integrations), verifying registered unchanged
hashes across four original Jet/ACE fixture families. The component-source runner
passes 1,214 source-free controls. Java mode was not used. Expression/query/CLI
all-target/all-feature Clippy, CLI build, formatting, Python syntax and diff checks
pass. Full query/workspace/backend suites, benchmarks and native Access comparisons
were not repeated. Full Variant/coercion, aggregate/UNION categories, native
calendar/error qualification and native text P0 acceptance remain unfinished.


## Ordered DateAdd/DateDiff/DatePart expression-state operands

Date interval calls now retain explicit expression operands through eager SQL
evaluation and request admission at their original validation position. Shared
preparations validate interval, count or progressive week settings before source
conversion. Error data fails TypeMismatch without suppressing later operational/
volatile operands; DateDiff still converts first then second source before NULL
propagation. Host Empty uses existing numeric/date zero and explicit system-week
semantics; strict intervals/Missing remain refused. Value-only APIs share the
same kernels. Calendar algorithms, source precision, week rules and formal return
categories are unchanged. See [scope](DATE_INTERVALS.md#ordered-expression-state-operands-23-october-2026).
No payload normalization copy, provider, dependency, stored state or write
permission is introduced.

All-feature expression testing passes 591 tests including its doctest; standalone
minimal expression testing passes 345. Thirteen selected query tests and seven
CLI tests pass. Four evaluator and three query controls cover Empty/policy and
Error/Missing order, progressive options, first-source conversion precedence,
eager derived failures, plan reuse, every observed state-path cancellation
checkpoint and once-only Rnd operands. The independent interval runner passes
13,614 controls (13,586 source-free and twenty-eight native integrations),
verifying registered unchanged hashes; Java mode was not used. Expression/query/
CLI all-target/all-feature Clippy, CLI build, formatting, Python syntax and diff
checks pass. Full query/workspace/backend suites, benchmarks and native Microsoft
Access comparisons were not repeated. Full Variant/coercion, native calendar/
precedence qualification, aggregate/UNION categories and native text P0 acceptance
remain open.


## Ordered DateSerial/TimeSerial expression-state components

Date/time constructors now retain explicit component values through eager SQL
evaluation and admit them at ordered Integer conversion. Error data fails
TypeMismatch without hiding later operational failures; volatile components run
once. Host Empty becomes zero before existing relative normalization and year
window resolution. Missing/NULL/Int16 overflow retain explicit failures. Value-only
APIs share the same calendar kernel; Jiff arithmetic, legacy envelopes, numeric
Text policy and formal Variant Date categories are unchanged. See [scope](DATE_CONSTRUCTORS.md#ordered-expression-state-components-23-october-2026).
No provider, dependency, stored state, clock or write permission is added.

All-feature expression testing passes 587 tests including its doctest; standalone
minimal expression testing passes 345. Twelve selected query tests and seven CLI
tests pass. Four evaluator and two query controls cover Empty relative components,
execution-local windows, Error/Missing/NULL/overflow order, eager derived failures,
plan reuse, every observed conversion cancellation checkpoint, work limits and
once-only Rnd components. The independent constructor runner passes 162 controls
(134 source-free and twenty-eight native integrations), verifying registered
unchanged hashes; its Java mode was not used in this run. Expression/query/CLI
all-target/all-feature Clippy, CLI build, formatting, Python syntax and diff
checks pass. Full query/workspace/backend suites, benchmarks and native Microsoft
Access comparisons were not repeated. Full Variant/coercion, native constructor/
Hijri qualification, aggregate/UNION categories and native text P0 acceptance remain
open.


## Ordered cashflow expression-state admission

NPV/IRR/MIRR now retain expression controls/elements through eager written-order
evaluation. Borrowed input accessors admit Error data with TypeMismatch at the
existing conversion position: NPV rate first, IRR/MIRR array first. Array Error/
Empty never becomes a code, zero or a skipped period; host Empty scalar controls
use current Double zero targets. Missing/NULL and omitted IRR defaults remain
explicit. Numeric algorithms, relative positions, solver limits and formal Double
categories are unchanged. Temporary SQL arrays charge actual ExpressionValue
widths; numeric f64 buffers retain separate charges. See [scope](CASHFLOW_FUNCTIONS.md#ordered-expression-state-admission-23-october-2026).
No payload clone beyond ordinary expression materialization, provider, dependency,
stored state or write permission is introduced.

All-feature expression testing passes 583 tests including its doctest; standalone
minimal expression testing passes 345. Thirteen selected query tests and six CLI
tests pass. Four evaluator and two query controls cover Empty scalar targets,
strict Error/Empty array admission, NULL/Missing order, eager derived failures,
parameter-plan reuse, numeric-buffer quota, every observed construction/solver
cancellation checkpoint and once-only Rnd array effects. The independent financial
runner passes 274 controls (202 source-free and seventy-two native integrations),
verifying registered unchanged hashes. Expression/query/CLI all-target/all-feature
Clippy, CLI build, formatting, Python syntax and diff checks pass. Full query/
workspace/backend suites, benchmarks and native Microsoft Access comparisons were
not repeated. Full Variant/coercion, native admissibility/precedence, aggregate/
UNION categories and native text P0 acceptance remain open.


## Ordered scalar financial expression-state admission

The eleven scalar financial calls now retain explicit expression operands through
eager evaluation and use one borrowed argument accessor at the kernel's conversion
position. Error data fails TypeMismatch without hiding later operational errors
or skipping/repeating volatile arguments. Written Double conversion order and
EuroConvert amount/currency/option validation remain shared with Value-only
callers. Host Empty uses zero in current numeric controls; Empty currency codes
and Missing remain refused. Omitted defaults, solver algorithms, fixed currency
facts, resource checks and formal Double return categories are unchanged. See
[scope](FINANCIAL_FUNCTIONS.md#ordered-expression-state-admission-23-october-2026).
No payload clone, provider, dependency, stored state or write permission is added;
cashflow-array handling remains separate.

All-feature expression testing passes 579 tests including its doctest; standalone
minimal expression testing passes 345. Twenty-one selected query tests and six
CLI tests pass. Five evaluator and three query controls cover Empty targets,
Error/Missing/NULL order, eager derived/parameterized failures, plan reuse,
every observed conversion/solver cancellation checkpoint and a once-only Rnd
operand consumed before data rejection. The independent financial runner passes
242 controls (182 source-free and sixty native integrations), verifying registered
unchanged hashes. Expression/query/CLI all-target/all-feature Clippy, CLI build,
formatting, Python syntax and diff checks pass. Full query/workspace/backend
suites, benchmarks and native Microsoft Access comparisons were not repeated.
Full Variant/coercion, native parameter widths/precedence, aggregate/UNION
categories and native text P0 acceptance remain open.


## Ordered math expression-state admission

Scalar/bound math calls now collect explicit expression values before kernel
admission. A shared borrowed argument accessor requests operands at the original
validation position: Round validates digits first, RGB validates channels in order
and Rnd requires state before control admission. Error data fails TypeMismatch
without implicit code conversion and cannot suppress later operational argument
failures. Reviewed host Empty zero targets work; Round's Variant value slot and
Missing retain explicit refusals. Existing Value-only callers share the same
kernels, Text policies, entry work/cancellation and Rnd repeatability rules. Math
evaluation now lives in its owning feature module with separate state/random
tests. See [scope](MATH_FUNCTIONS.md#expression-errorempty-math-arguments-23-october-2026).
No argument payload clone, dependency, stored state, VBA runtime or write
permission is introduced.

All-feature expression testing passes 574 tests and query testing passes 738,
each including its doctest. Standalone minimal expression testing passes 345;
eight selected CLI tests pass. Four evaluator and two query controls cover Empty
carriers, Error/Missing admission, count/channel precedence, eager nested/derived/
correlated/encounter failures, cancellation and unchanged random state. The math
runner passes 222 independent controls (178 source-free and forty-four native),
the DATE runner 1,384 (1,368 source-free and sixteen native), and the Rnd runner
1,644 source-free controls. Registered native hashes remain unchanged. Expression/
query/CLI all-target/all-feature Clippy, CLI build, formatting, Python syntax and
diff checks pass. Full workspace/backend suites, benchmarks and native Microsoft
Access comparisons were not repeated. Full Variant/coercion, native precedence,
aggregate/UNION categories and native text P0 acceptance remain open.


## Hex/Oct legacy DATE and explicit expression-state handling

Hex/Oct now admit materialized legacy DATE through their shared checked Long
conversion, retaining half-even rounding, negative-width intervals and the actual
Int64 bypass. Scalar/bound query dispatch now uses an explicit expression-state
entry point: host Empty becomes zero Text, NULL propagates, Missing stays
non-scalar and Error data fails TypeMismatch without implicit code conversion.
Explicit CLng(CVErr(...)) remains separate; eager operational errors still fail.
Query Nz(NULL), formal Variant categories and Extended DateTime refusals remain
unchanged. See [scope and references](NUMERIC_TEXT.md#hexoct-legacy-dates-and-expression-states-23-october-2026).
No provider, dependency, fixture bytes, VBA execution or write permission is added.

All-feature expression testing passes 570 tests including its doctest; standalone
minimal expression testing passes 345. Thirteen selected query tests and eight
CLI tests pass. Kernel/evaluator controls cover Date rounding, Extended refusal,
Empty/NULL/Missing/standard and opaque Error states, output limits and every
observed Empty-path cancellation checkpoint. Query/CLI controls cover explicit
code conversion, eager errors and buffered failure. The independent input runner
passes 378 controls (338 source-free and forty native integrations), verifying
registered unchanged hashes; 3,708 existing source-free numeric-output controls
also pass. Expression/query/CLI all-target/all-feature Clippy, CLI build,
formatting, Python syntax and diff checks pass. Full query/workspace/backend
suites, benchmarks and native Microsoft Access comparisons were not repeated.
Full Variant/coercion, native width/error qualification, aggregate/UNION categories
and native text P0 acceptance remain open.


## Legacy DATE unary math results

Abs/Sgn/Int/Fix now admit materialized legacy DATE. Abs returns Double serial
magnitude and Sgn Integer sign; Int/Fix floor/truncate raw serials and reconstruct
checked Date results, preserving signed zero. Lower-envelope Int underflow fails
Overflow. Binding exposes Date/Double output types without executing values;
formal Variant categories remain separate. Numeric carriers, Round Date and
Extended DateTime boundaries are unchanged. See [scope and references](MATH_FUNCTIONS.md#legacy-date-unary-functions-23-october-2026).
No provider, dependency, calendar normalization or write permission is introduced.

All-feature expression testing passes 567 tests including its doctest; standalone
minimal expression testing passes 344. Sixteen selected query tests and sixteen
CLI tests pass. Three kernel tests include a fixed-seed quarter-day integer model,
negative fractions, exact signed zeros, envelope limits and unsupported types.
Query tests cover derived results, inert metadata, empty subqueries, UNION
selection and eager failures. The independent DATE runner passes 1,384 controls
(1,368 source-free and sixteen native field controls), verifying registered
unchanged original hashes. Expression/query/CLI all-target/all-feature Clippy,
CLI build, formatting, Python syntax and diff checks pass. Full query/workspace/
backend suites, benchmarks and native Microsoft Access comparisons were not
repeated. Full Variant/coercion, native result/error qualification, aggregate/
UNION categories and native text P0 acceptance remain open.


## Str standard Error, host Empty and explicit legacy dates

Str now has an explicit expression-value entry point in scalar and bound queries.
Standard Error data shares CStr's bounded invariant code writer; operational
failures are never caught or formatted. Host Empty converts to numeric zero;
Missing and unmapped host codes retain explicit refusals. Legacy dates use the
existing CStr date formatter under an explicit DateTextFormat, independently of
NumberFormat. Extended dates remain unsupported by Str. Query Nz(NULL) retains
its existing empty-Text result and therefore its Text conversion requirements.
Formal Variant return categories, stored carriers and pure renderer admission
remain unchanged. See [scope and references](NUMERIC_TEXT.md#str-expression-states-and-legacy-dates-23-october-2026).
No dependency, runtime activation, fixture bytes or write permission is added.

All-feature expression testing passes 564 tests including its doctest. Fifteen
selected query tests and seven CLI tests pass, covering Error/NULL selection,
encounter aggregates, explicit date policy, unchanged query Nz identity, host
state refusals, eager failures, Text limits and every observed Error/Empty-path
cancellation checkpoint. The independent input runner passes 338 controls
(306 source-free and 32 native integrations), verifying registered unchanged
hashes; 3,708 existing source-free numeric-output controls also pass. Expression/
query/CLI all-target/all-feature Clippy, CLI build, formatting, Python syntax and
diff checks pass. Full query/minimal-feature/workspace/backend suites, benchmarks
and native Microsoft Access comparisons were not repeated. Full Variant/coercion,
native date/spelling/HRESULT qualification, aggregate/UNION categories and native
text P0 acceptance remain open.


## Explicit numeric Text Partition controls

Partition's four controls now accept SQL numeric Text through one shared internal
Long converter under an explicit NumberFormat. The kernel retains its NULL
precheck, written conversion order and range validation after all conversions;
numeric callers still use the public host-free entry point. The evaluator checks
entry work/cancellation and retains Text/input/output limits, eager operands and
formal Variant result categories. No label arithmetic, padding, native format,
dependency or write permission changes. See [scope](PARTITION.md#explicit-numeric-text-controls-23-october-2026).

All-feature expression testing passes 562 tests including its doctest; standalone
minimal expression testing passes 341. Twelve selected query tests and five CLI
tests pass. New controls cover rounding, conversion/domain precedence, NULL and
eager errors, parameter-plan reuse across formats, limits and every observed
Text-path cancellation checkpoint. The independent range runner passes 838
controls (830 source-free and eight native integrations), verifying registered
unchanged original hashes. Expression/query/CLI all-target/all-feature Clippy,
CLI build, formatting, Python syntax and diff checks pass. Full query/workspace/
backend suites, benchmarks and native Microsoft Access comparisons were not
repeated. Full Variant/coercion, native width/padding/error qualification,
aggregate/UNION categories and native text P0 acceptance remain open.


## Explicit numeric Text inputs to Hex/Oct/Str

Hex/Oct now convert SQL numeric Text under an explicit NumberFormat to Long;
Str converts it to Double before its existing numeric rendering. Long overflow,
half-even rounding and the actual-Int64-only bypass remain explicit. Text parsing
uses the supplied input separator while Str output retains a period. Numeric
Currency/Decimal sources keep exact rendering. Conversion/output share cancellation,
Text limits and work; binding remains inert and formal Variant result metadata
is unchanged. See [scope and primary references](NUMERIC_TEXT.md#explicit-numeric-text-inputs-23-october-2026).
No dependency, fixture bytes, Date coercion or write permission is introduced.

All-feature expression testing passes 560 tests and query testing passes 728,
each including its doctest; sixteen selected CLI tests pass. Focused evaluator
controls cover targets, Long boundaries, format requirements, Text limits and
every observed conversion/render cancellation checkpoint. Query/CLI controls
cover parameter-plan reuse across explicit policies, derived inputs and buffered
failure. The new independent input runner passes 308 controls (284 source-free
and 24 native integrations), checking registered unchanged hashes. The existing
numeric-output runner separately passes 3,708 source-free regression controls.
Expression/query/CLI all-target/all-feature Clippy, CLI build, formatting,
Python syntax and diff checks pass. Minimal-feature/workspace/backend suites,
benchmarks and native Microsoft Access comparisons were not repeated. Full
Variant/Date/Error coercion, native spelling/widths, aggregate/UNION categories
and native text P0 acceptance remain open.


## Explicit numeric Text math operands

All fifteen math identities now accept numeric Text through the evaluator's
supplied NumberFormat and existing checked conversion. Value inputs use Double,
Round digits and color controls use Long, and Rnd uses Single. Conversion stays
at the original validation position: Round checks digits first, RGB checks
channels in order and failed Rnd conversion leaves its sequence unchanged.
Numeric Decimal/Currency sources retain their carriers. Direct valid math calls
check entry work/cancellation; Text paths share conversion budgets. Successful
Text-source Abs/Int/Fix/Round outputs expose Double metadata without binding-time
parsing/provider calls. See [scope](MATH_FUNCTIONS.md#explicit-numeric-text-operands-23-october-2026).
No dependency, native format or write permission is added.

All-feature expression testing passes 558 tests and query testing passes 727,
each including its doctest. Standalone minimal expression testing passes 341;
sixteen selected CLI tests pass. Three evaluator and three query tests cover
conversion targets, exact fixed-point retention, metadata/UNION composition,
explicit formats, plan reuse, validation/eager-error order, Text limits and every
observed Text-path cancellation checkpoint. The original math Text runner passes
164 independent controls (132 source-free and 32 native integrations), verifying
registered original hashes before/after. Expression/query/CLI all-target/all-feature
Clippy, CLI build, formatting and diff checks pass. Full workspace/backend suites,
benchmarks and native Microsoft Access comparisons were not repeated. Full
Variant/Date coercion, native precision/error qualification, aggregate/UNION
categories and native text P0 acceptance remain open.


## Explicit numeric Text cashflow rates and guesses

NPV/IRR/MIRR scalar controls now use the existing checked Double conversion under
an explicit NumberFormat. One shared internal converter preserves the kernel's
rate-versus-array validation order; strict numeric cashflow elements, positions,
optional guess semantics and eager expressions remain unchanged. The converter
uses the same quota, Text limit and cancellation callback and consults no binding
or other provider. Direct evaluator calls check work/cancellation at entry.
See [cashflow scope](CASHFLOW_FUNCTIONS.md#explicit-numeric-text-rates-and-guesses-23-october-2026).
No array coercion, new dependency, native format or write permission is introduced.

All-feature expression testing passes 555 tests and query testing passes 724,
each including its doctest. Standalone minimal expression testing passes 341
tests; sixteen selected CLI tests pass. Two evaluator and two query tests cover
rates/guesses, explicit policy/plan reuse, derived results, array admission,
validation/eager-error order, shared Text limits and every observed Text-path
cancellation checkpoint. The financial Text runner passes 174 independent controls
(126 source-free and 48 native integrations), checking known discount/root/endpoint
examples and original registered hashes/unchanged bytes. Expression/query/CLI
all-target/all-feature Clippy, CLI build, formatting and diff checks pass.
Full workspace/backend suites, benchmarks and native Microsoft Access comparisons
were not repeated. Array/native query admissibility, full Variant/numerical
qualification, aggregate/UNION categories and native text P0 acceptance remain open.


## Explicit numeric Text for scalar financial calls

All eleven scalar financial functions now use one internal conversion/check
context. The SQL evaluator supplies existing explicit-policy numeric Text
conversion at the kernel's original conversion position. Numeric-only callers
retain a host-free context. EuroConvert preserves currency-code strings and its
Double/Boolean/Integer option targets and validation order. Direct evaluator
financial calls now check work/cancellation at entry, including noniterative
functions; solver and Euro checkpoints remain shared. See the
[financial contract](FINANCIAL_FUNCTIONS.md#contextual-numeric-text-inputs-23-october-2026).
No converted argument list, Text cloning, new library or native/write permission
is introduced. Full Variant/native financial semantics remain unqualified.

All-feature expression testing passes 553 tests and query testing passes 722,
each including its doctest. Standalone minimal expression testing passes 341
tests; fifteen selected CLI tests pass. Two evaluator and two query controls
cover every scalar financial identity, explicit formats, typed Euro options,
plan reuse, derived/correlated input, validation order and cancellation. The
original financial Text runner passes 118 controls (94 source-free and 24 native
integrations), using independent elementary results and registered unchanged
four-family inputs. Expression/query/CLI all-target/all-feature Clippy, CLI build,
formatting and diff checks pass. Full workspace/backend suites, benchmarks and
native Microsoft Access comparisons were not repeated. Native Text/LCID provider
mapping remains separate from General database weights; full coercion, aggregate/
UNION categories and native text P0 acceptance remain open.


## Signed numbered Error lookup and distinct Error$ identity

Numbered message lookup now preserves negative Long codes through the checked
ErrorMessageCode provider argument, separately from CVErr data. Positive codes
above 65535 report Overflow. Host ErrorMessages implementations now receive this
checked signed type and can read its exact i32 through number(). Flat Fluent
catalogs admit canonical error-negative-N keys, preserving Long MIN without
unsigned reinterpretation; unknown signed codes use the selected fallback.
The bundled English data/provenance remain unchanged. The existing Error$ spelling
now retains a distinct Scalar String identity through binding, traversal,
grouping identity and execution; Error retains Variant. Both use one lookup kernel.
See [scope and references](ERROR_MESSAGES.md#signed-numbers-and-typed-error-identity-23-october-2026).

All-feature expression testing passes 551 tests and query testing passes 720,
each including its doctest. Standalone minimal expression testing passes 341
tests; seventeen selected CLI tests and the focused async SQLite request-local
catalog test pass. Constructor/property, signed catalog, exact-provider-code,
Error/CVErr separation, Error$ metadata/runtime and buffered failure checks pass.
The message runner passes 552 source-free catalog/refusal controls; the declaration
runner passes 5,741 controls (4,769 source-free and 972 native integrations),
verifying original registered hashes and unchanged bytes. Expression/query/CLI/
SQLite all-target/all-feature Clippy, CLI build, formatting and diff checks pass.
Full workspace/backend/SQLite suites, benchmarks and native Microsoft Access
comparisons were not repeated. Native message/HRESULT/sandbox mapping, omitted
runtime state, aggregate/UNION categories and native text P0 acceptance remain open.


## Reviewed session, Error and GUID return categories

Existing CurrentUser now carries its documented Scalar String category; numbered
Error and both GUID conversions carry their reviewed Variant category through
shared static/runtime metadata. Empty subqueries activate no session/catalog
provider or converter. Actual Error results retain once-only lookup behavior,
missing-provider errors and late operational failures. Session assertions and GUID
byte/string carriers retain their existing permission/publication boundaries.
The [contract](DECLARED_ARITHMETIC.md#session-numbered-error-and-guid-return-categories-23-october-2026)
records primary references and why aggregate/UNION categories remain unknown.
No new function, host state, dependency, native format or write permission is added.

All-feature query testing passes 719 tests including its doctest. Two new query
tests cover empty metadata and direct/derived/scalar-subquery runtime results,
explicit session/catalog data, lookup counts, failures and GUID String ordering.
Sixteen CLI declaration/logical/session/catalog tests pass. The independent
runner passes 5,729 controls (4,765 source-free and 964 native integrations),
checking registered hashes and unchanged original bytes. Query/CLI all-target/
all-feature Clippy, standalone minimal query check, CLI build, formatting and diff
checks pass. Full expression/workspace/backend suites, benchmarks and native
Microsoft Access comparisons were not repeated. Aggregate/UNION categories,
full Variant/state semantics and native text P0 acceptance remain open.


## Reviewed text return categories and distinct Chr$ identity

All 43 currently admitted text function identities now have reviewed formal
return categories in a feature-local shared signature classifier. Scalar String
and Double returns remain distinct from Variant-contained Text/numbers even when
the returned carriers match. The existing Chr$ spelling now retains a distinct
ChrString identity while sharing Chr's checked character kernel, code-page/arity/
NULL behavior and output metadata. No new VBA-only function or `$` spelling is
admitted. See the [signature table](DECLARED_ARITHMETIC.md#text-built-in-return-categories-23-october-2026).
Defaults, stored values, dependencies and write permissions remain unchanged.

All-feature query testing passes 717 tests and expression testing passes 547,
each including its doctest. Standalone minimal expression testing passes 339
tests. Fifteen CLI declaration/logical/character/dynamic-evaluation tests pass.
Two new query tests cover metadata for every admitted text identity without
executing provider-requiring functions, plus actual direct/derived/scalar-
subquery propagation. A character kernel test covers Chr$ resolution and shared
construction boundaries. The independent runner passes 5,711 controls (4,755
source-free and 956 native integrations), verifying registered hashes and unchanged
original bytes. Expression/query/CLI all-target/all-feature Clippy, CLI build,
formatting and diff checks pass. Full workspace/backend suites, benchmarks and
native Microsoft Access comparisons were not repeated. Aggregate/UNION return
categories, full Variant/state semantics and native text P0 acceptance remain open.


## Effective legacy Date predicate coercion

The explicit effective comparison profile now selects Date for legacy
Date/numeric pairs and Decimal when Decimal participates. A feature-local adapter
reuses CDate, Decimal conversion and the existing temporal/raw-serial comparators.
It preserves Date/Text policy requirements and Date/Date ordering; Extended Date
is not silently truncated. Out-of-Date-range numeric operands fail rather than
bypassing conversion. The [contract](DECLARED_ARITHMETIC.md#effective-legacy-date-comparison-types-23-october-2026)
separates effective type selection from unqualified native temporal behavior.
Defaults, strict set/group/sort keys, stored values and writes are unchanged.

All-feature expression testing passes 546 tests and query testing passes 715,
each including its doctest. Twelve CLI declaration/logical tests pass. Three
new evaluator tests and a query test cover numeric families, operand order,
negative fractional dates, range ends, Decimal precedence, explicit policies,
NULL/non-finite/Extended Date refusals, eager failures and cancellation.
The independent runner passes 5,598 controls (4,658 source-free and 940 native
integrations), checking exact day/fraction or raw-serial models, registered hashes
and unchanged originals. Expression/query/CLI all-target/all-feature Clippy,
standalone minimal expression check, CLI build, formatting and diff checks pass.
Full workspace/backend suites, benchmarks and native Microsoft Access comparisons
were not repeated. Full Variant/state, Extended Date and native text P0 acceptance
remain open.


## Explicit effective numeric predicate coercion

The declaration profile now applies the reviewed numeric effective-type table
before predicate comparison. Integral/Text pairs convert directly with half-even
rounding and range checks; Single/Double selects Single, while Single/Long selects
Double. Currency/Decimal keep their checked conversions. Both-known-Variant
String/numeric category ordering still runs first. The public evaluator method
and shared query predicate path preserve explicit NumberFormat, NULL/errors,
eager evaluation, cancellation and budgets. See the
[scope and table](DECLARED_ARITHMETIC.md#effective-numeric-predicate-types-23-october-2026).
Default comparison, set/group/sort keys, nonnumeric owning profiles, native
formats and write permission remain unchanged. CLI help now describes the wider
explicit comparison profile.

All-feature expression testing passes 543 tests and query testing passes 714,
each including its doctest. Standalone minimal expression testing passes 338
tests; eleven CLI declaration/logical tests pass. Three evaluator and two query
tests cover selected-width rounding/range, exact Int64, floating/fixed pairs,
shared consumers, declaration exceptions, eager errors and policy/NULL/
cancellation. The independent runner passes 4,646 controls (3,722 source-free
and 924 native integrations), verifying registered hashes and unchanged original
bytes. Expression/query/CLI all-target/all-feature Clippy, CLI build, formatting
and diff checks pass. Full workspace/backend suites, benchmarks and native
Microsoft Access comparisons were not repeated. Date/Extended Date coercion,
complete effective/static type tables and native text P0 acceptance remain open.


## Numeric Text retains Date declaration overflow rules

Date/numeric-Text addition and subtraction now enter the explicit declaration
profile before contextual conversion. The public evaluator reuses one shared
operator-specific conversion path with default arithmetic, retaining original
Scalar/Variant categories; the standalone kernel still rejects Text. Explicit
NumberFormat remains required. Scalar envelope overflow, Variant widening,
NULL/cancellation and buffered CLI failures follow the
[contextual declaration contract](DECLARED_ARITHMETIC.md#contextual-numeric-text-and-date-declarations-23-october-2026).
Default arithmetic, dependency boundaries, stored values and writes are unchanged.

All-feature expression testing passes 540 tests including its doctest; standalone
minimal expression testing passes 338 tests. Thirty-two focused query tests and
ten CLI declaration/logical tests pass. The independent runner passes 4,294
controls (3,386 source-free and 908 native integrations), verifying registered
hashes and unchanged original bytes. Expression/query/CLI all-target/all-feature
Clippy, CLI build, formatting and diff checks pass. The full query/workspace/
backend suites, benchmarks and native Microsoft Access comparisons were not
repeated for this batch. General effective-type/coercion tables and native text
P0 acceptance remain open.


## Declared Date overflow preserves Scalar and Variant rules

The explicit numeric kernel now admits legacy Date values and enforces
Scalar envelope overflow for Date-selected addition/subtraction. A Variant
participant permits the existing finite Double fallback. Query unary minus uses
the same checked kernel; Date-minus-Date retains its direct Double result.
Defaults, stored values and calendar conversion remain unchanged. The
[contract](DECLARED_ARITHMETIC.md#declared-legacy-date-overflow-23-october-2026)
records the reviewed specification and remaining Text/Extended Date/native gaps.
No dependency, native format or write-permission change.

All-feature expression testing passes 539 tests and query testing passes 711,
each including its doctest. Nine CLI declaration/logical tests pass. New kernel
and query tests cover category pairs, range ends, operand order, unary minus,
NULL, in-range Date, scalar subqueries and unchanged defaults. The independent
runner passes 4,274 controls (3,366 source-free and 908 native integrations),
checking registered hashes and unchanged original bytes. Expression/query/CLI
all-target/all-feature Clippy, standalone minimal expression check, CLI build,
formatting and diff checks pass. Full workspace/backend suites, benchmarks and
native Microsoft Access comparisons were not repeated. Complete Variant
promotion/coercion and native text P0 acceptance remain open.


## Static declarations survive empty query results

Bound slots and read plans now carry optional declaration categories derived
from scalar metadata, explicit expressions and the shared reviewed function
signatures. Scalar subqueries keep this metadata even when empty; membership
and quantified predicates combine it with left provenance. Derived, named,
joined and correlated sources preserve categories without reading records during
preparation. Named crosstab group columns retain static categories, while pivot
aggregates, UNION and unreviewed function/aggregate results remain unknown.
Inference and metadata allocation use existing binding limits. See the
[contract](DECLARED_ARITHMETIC.md#static-query-output-categories-23-october-2026).
No numeric kernel, dependency, native format or write-permission change.

All-feature query testing passes 710 tests including its doctest. Four focused
query tests cover static propagation and preparation scan counters. Eight CLI
declaration/logical tests pass, including both flag names and buffered failures.
The independent runner passes 4,258 controls (3,350 source-free and 908 native
integrations), verifying registered hashes and unchanged original bytes.
Query/CLI all-target/all-feature Clippy, standalone minimal query check,
formatting and diff checks pass. Full workspace/backend suites, benchmarks and
native Microsoft Access comparisons were not repeated. Full Variant effective-
type tables and native text P0 acceptance remain open.


## List, range and subquery predicates preserve result categories

The explicit declaration profile now composes IN-list/BETWEEN categories from
all written operands, with a known Variant establishing Variant and unresolved
categories remaining conservative. EXISTS uses a fixed Scalar Boolean policy.
For IN/ANY/SOME/ALL subqueries, a known Variant left operand establishes Variant
without inspecting returned row declarations, including empty results. Other
cases await static plan-output inference. These
[composition-derived engineering policies](DECLARED_ARITHMETIC.md#list-range-and-subquery-result-policies-23-october-2026)
are not captured native signatures. Feature-local predicate adapters retain the
existing matcher, equality accumulator, range/quantified comparisons, runtime
text policies and eager materialization. No default value/coercion, dependency,
native format, source scan permission or write-permission changes.

All-feature query testing passes 706 tests including its doctest. Four new tests
cover operand categories, query scopes, casts, unknowns, NULL/Error behavior,
late failures, empty results, once-only random operands and cancellation. Seven
CLI declaration/logical tests pass. The independent runner passes 4,200 controls
(3,308 source-free and 892 native integrations), checking registered hashes and
unchanged original bytes. Query/CLI all-target/all-feature Clippy, CLI build,
minimal query check, formatting and diff checks pass. Full workspace/backend
suites, benchmarks and native Microsoft Access comparisons were not repeated.
Complete static output declarations, Variant effective-type tables and native
text P0 acceptance remain open before broader write expansion.

## LIKE and NULL checks retain reviewed declarations

LIKE/NOT LIKE now preserve reviewed Scalar/Variant result categories through
bound query composition. A feature-local adapter reuses the existing matcher,
explicit modes and field-policy routing; it evaluates each operand once without
a second match. IsNull and the existing shared IS NULL/IS NOT NULL node retain
Scalar Boolean independently of the argument category. Unknown aggregate
declarations remain unknown through LIKE unless a known Variant establishes its
result. [The contract](DECLARED_ARITHMETIC.md#like-and-null-check-declarations-23-october-2026)
records the primary signatures, normalized NULL-check policy and native
qualification limits. No pattern kernel, coercion, provider, dependency, default
result or write-permission changes.

All-feature query testing passes 702 tests including its doctest. Four focused
predicate tests cover both ANSI modes, negation, query scopes, explicit casts,
unknown First, NULL/Error distinctions, eager failures, mode/pattern refusals,
one literal-comparer call and cancellation. Six CLI declaration/logical tests
pass, including both flag names and buffered failures. The independent runner
passes 3,278 controls (2,474 source-free and 804 native integrations), checking
original registered hashes/bytes. Query/CLI all-target/all-feature Clippy, CLI
build, standalone minimal query check, formatting and diff checks pass. Full
workspace/backend suites, benchmarks and Microsoft Access comparisons were not
repeated. Membership/Between/EXISTS declaration policies, full Variant tables
and native text P0 acceptance remain open.

## Both-Variant String/numeric comparison ordering

The explicit declared-arithmetic profile now applies the reviewed rule that a
numeric value precedes a String when both operands have known Variant
declarations. The new parser-independent comparison helper handles all eight
numeric carriers and both directions without parsing text or invoking a
provider. Boolean/Date are separate types; non-finite inputs retain rejection.
Query integration is shared by comparison consumers, including filters, joins,
domain criteria and membership. Unknown/one-Variant operands and default
execution retain their existing conversion profiles. API comments and CLI help
now describe the broader flag, preserving its existing name and alias. The
[contract](DECLARED_ARITHMETIC.md#both-variant-stringnumeric-comparison-exception-23-october-2026)
keeps full Variant tables, key semantics and native Access qualification separate.

All-feature testing passes 538 expression tests and 698 query tests including
their doctests; standalone minimal expression testing passes 337 tests and the
minimal query check passes. Five CLI tests cover the flag/alias, unchanged
defaults and buffered failures. The existing async worker request-local test
also passes alternating comparison policies with unchanged read-only source
bytes. Expression/query/SQLite/CLI all-target/all-feature Clippy, CLI build,
formatting and diff checks pass. The independent runner passes 3,044 controls
(2,296 source-free and 748 native integrations), including seven native numeric
field carriers and full-table domain criteria after an outer selected snapshot.
JAR/input hashes and unchanged originals are checked. Full workspace/backend
suites, benchmarks and native Microsoft Access comparisons were not repeated.
No dependency, stored carrier, native format or write-permission changes.

## Relational results and CVErr retain declaration categories

The six ordinary comparison operators now preserve reviewed Scalar/Variant
result categories through bound query composition. Two known Scalar operands
yield Scalar Boolean; either Variant yields Variant; other unknown categories
remain unknown. CVErr also retains its formal Variant return, allowing qualified
Error/Error comparisons to carry that category. The existing comparator still
owns values, runtime text policies, Error/NULL and coercion; no operand or
comparer is evaluated twice. [The reviewed contract](DECLARED_ARITHMETIC.md#relational-result-categories-23-october-2026)
keeps category propagation separate from complete declared-type-specific
comparison semantics and native Access qualification. No dependency, native
format, numeric kernel or default comparison-policy change.

All-feature query testing passes 694 tests including its doctest. Three dedicated
relational tests cover every operator, both Variant operand positions, derived/
grouped/correlated outputs, explicit result casts, unknown Count, NULL, Error
pairs/mixed refusals, eager failures, once-only random operands and one comparer
call. Six CLI declaration/Error/numeric-text tests pass. The independent runner
passes 2,210 controls (1,806 source-free and 404 native integrations), verifying
registered hashes and unchanged original bytes. Query/CLI all-target/all-feature
Clippy, CLI build, minimal query check, formatting and diff checks pass. Full
workspace/backend suites, benchmarks and Microsoft Access comparisons were not
repeated. Mixed String/numeric declared comparison rules, SQL-only predicate
categories, full Variant promotion and native text P0 work remain open.

## Logical operators retain declaration categories

Not, And, Or, Xor, Eqv and Imp now preserve Scalar/Variant categories through
bound query composition. Unary logical results retain the operand category;
binary results use the existing conservative category merge, with a known
Variant participant establishing Variant. Subsequent arithmetic now distinguishes
Scalar overflow from Variant widening. Unknown aggregate categories remain
unknown. Existing bitwise widths, checked conversions, Boolean/NULL kernels,
eager failures and default materialized results retain their contracts. The
[reviewed logical tables](DECLARED_ARITHMETIC.md#logical-operator-declaration-propagation-23-october-2026)
keep declared categories separate from full subtype/runtime Variant qualification.

All-feature query testing passes 691 tests including its doctest; the final
four-test logical target also passes added missing-format and checked conversion
refusals. Five CLI declaration/logical tests pass. The independent declaration
runner passes 1,644 controls (1,384 source-free and 260 native integrations),
checking unchanged original hashes and bytes. Tests cover grouped/derived/
correlated outputs, both Variant operand positions, exact widths, unknown Count,
LongLong overflow, eager errors, once-only random operands and cancellation.
Query/CLI all-target/all-feature Clippy, CLI build, minimal query check, formatting
and diff checks pass. Full workspace/backend suites, benchmarks and native
Microsoft Access comparisons were not repeated. Relational/other declaration
categories, complete Variant promotion and native text P0 work remain open.

## Reviewed financial, date and inspection return declarations

The explicit declared-arithmetic profile now assigns reviewed signatures to all
implemented financial/cashflow returns, date constructors/conversions/arithmetic
and clocks, and recognition/type inspection calls. Financial/cashflow results
remain Scalar Double even with Variant arguments. Date-producing functions retain
Variant, CDate remains Scalar, and Timer retains Scalar Single. Inspection
predicates and TypeName retain Scalar Boolean/String. Feature-local exhaustive
policy matches preserve the distinction without scanning rows or adding provider
calls. [The reviewed signature table](DECLARED_ARITHMETIC.md#financial-date-and-inspection-signatures-23-october-2026)
records the dated MS-VBAL and Microsoft Access sources. No query function,
dependency, numeric kernel, default arithmetic or VBA execution is introduced.

Thirty-one focused query tests and seven CLI declaration/clock tests pass,
including financial and date returns through grouped/derived/correlated outputs,
explicit casts, eager failures, NULL extraction and one clock read per expression.
The independent runner passes 1,272 controls (1,036 source-free and 236 native
integrations), checking registered hashes and unchanged original inputs.
Query/CLI all-target/all-feature Clippy, CLI build, minimal query check, formatting
and diff checks pass. Full workspace/query suites, benchmarks and native
Microsoft Access comparisons were not repeated for this metadata-only change.
Ordinary aggregate declarations, unreviewed function/operator categories, complete
Variant promotion and native text acceptance remain unfinished.

## Fixed-point means avoid intermediate Decimal rounding

Non-Decimal Avg/DAvg now convert their exact total/count directly to Double using
the existing bounded rational finalizer, including totals that fit Decimal. A
16,913-row Currency regression failed before the fix because a scale-28 Decimal
quotient moved the final Double by one bit; positive and negative ordinary/domain
cases now pass. Explicit Decimal means keep their quotient/carrier/scale rules.
There is no new dependency or decimal implementation. The
[mean contract](AVERAGES.md#single-rounding-for-fixed-point-double-means-23-october-2026)
records rounding, resource ownership and separate native qualification.

All-feature query testing passes 682 tests including its doctest. The expanded
independent Fraction runner passes 125 controls, including raw native Money-field
regressions across the four registered Jet 4/ACE families and both index orders.
JAR/input hashes and unchanged originals are checked. Query/CLI all-target and
all-feature Clippy, CLI build and standalone minimal query check pass. The identical
expanded release harness versus `4f8e6a1` shows no clear overall regression;
Currency grouped medians are about 1.050/1.054 ms and 3.233/3.233 ms, with all
samples retained. Full workspace/backend suites and Microsoft Access were not
rerun. Full Variant/result declarations, mixed-type encounter order and native
text P0 work remain open before broader write expansion.

## Mixed Currency/floating aggregate inputs

Sum/DSum now admit Currency mixed with Single/Double using the existing explicit
Double converter for Currency operands and checked prefixes. Pure Currency folds,
exact integral admission, Decimal precedence, eager failures and shared bounded
scratch ownership retain their existing behavior. The
[contract](testing/FLOATING_SUMS.md#currencyfloating-integration) records prefix
overflow and mixed encounter-order limits; this is not complete native aggregate
promotion qualification. No dependency, physical format or writer change.

All-feature query testing passes 681 tests including its doctest. Three dedicated
Currency tests cover conditional encounter orders, typed fields and scaled i64
extrema, NULL, domain calls and preserved fixed-prefix overflow. The independent
runner passes 331 Fraction/native-field controls with unchanged registered inputs.
Query/CLI all-target/all-feature Clippy, CLI build, minimal query check and
formatting pass. The identical release harness versus `4429d23` shows no clear
overall regression: the largest median difference is about 30 microseconds for
2,048 floating rows across 128 groups; whole-process Hyperfine means are 108.7
versus 109.0 ms. All raw samples and scope limits are recorded in the owning
document. Full workspace/backend suites and Microsoft Access were not rerun.

Broader Variant/coercion and native text P0 work remain next; ordinary ACE writes
remain blocked and the backend roadmap remains open.

## Reviewed math returns and corrected length declarations

All fifteen implemented math functions now have explicit reviewed return
categories: Abs/Int/Fix/Round/Sgn are Variant, transcendental functions are Scalar
Double, Rnd is Scalar Single, and RGB/QBColor retain Scalar Long. The exhaustive
match requires review when another MathFunction is added. Argument declarations
do not replace the function's return category; explicit result casts still do.
The published MS-VBAL 3.0 review also corrects Len/LenB to Variant containing Long,
rather than the earlier Scalar classification. Default materialized arithmetic,
actual function carriers, numeric kernels and random sequence ownership are
unchanged. [The contract and dated reference hash](DECLARED_ARITHMETIC.md#math-signatures-and-lenlenb-correction)
record the distinction and separate native SQL qualification.

Twenty-six focused query math/declaration/random/length tests and four CLI
declaration tests pass. New checks cover Byte/Integer/Single widening, Scalar
result-cast overflow, grouped/derived/correlated returns, NULL/eager errors and
Rnd overflow without extra draws. The updated independent runner passes 1,136
controls (948 scalar and 188 native integrations), with exact IEEE result checks,
Fraction-modeled Rnd quotients, explicit seeds and unchanged registered Jet 4,
ACE 2007 and ACE 2016/2019 inputs. Only TableCraft executes SQL; reviewed Jackcess
getters remain inert. Query/CLI all-target/all-feature Clippy, CLI build,
standalone minimal query check, formatting and diff checks pass. No dependency or
performance-kernel changes. Full query/workspace suites and native Microsoft
Access comparisons were not repeated. Mixed aggregate/coercion, other function
declarations and native text acceptance remain unfinished.

## Reviewed domain-function return declarations

All twelve domain functions now retain their documented Variant return category
through the explicit declared-arithmetic profile. The shared reviewed-signature
policy supplies the category; the domain cell keeps its existing Value/Error
carrier and runtime text policy. Byte/Integer/Single selector results can widen,
explicit casts still enforce Scalar overflow, and ordinary SQL aggregates retain
their separate unknown declaration boundary. Default materialized arithmetic,
source schemas, actual aggregate carriers and native write gates are unchanged.
The [reviewed signatures](DECLARED_ARITHMETIC.md#domain-function-return-declarations)
record each Microsoft reference and the separate native SQL qualification limits.

Seventy-seven focused query declaration/domain/discovery/text-policy regressions
pass. New controls cover every domain function, derived/correlated outputs,
explicit cast refusal, NULL/Error distinction and discovered crosstab domains
whose preparation scans no rows and whose execution discovers once. Existing
text-policy, cancellation, resource and cycle checks remain green.
The extended declaration runner passes 1,042 independent controls (886 scalar and
156 native integrations). Reviewed Jackcess getters confirm Byte/Integer/Single
source values in unchanged registered Jet 4, ACE 2007 and ACE 2016/2019 inputs.
Outer ID-1 reads and domain ID-2 selections prove selected snapshots stay separate;
only TableCraft executes SQL. Query/CLI all-target/all-feature Clippy, CLI build,
formatting and diff checks pass. No dependencies or performance kernels change.
Full query/workspace suites and native Microsoft Access comparisons were not
repeated for this metadata change. Mixed aggregate promotion, broader coercion
and native text behavior remain P0 work.

## Exact floating Sum/DSum

Floating sums now reuse the bounded binary accumulator previously owned by the
mean slice. Pure floating totals retain cancellation residuals and avoid temporary
Double overflow before one final rounding/range check. NULL/empty behavior,
Double result metadata, checked fixed-point folds and explicit mixed-type
refusals remain. A shared execution-local numeric scratch pool serves ordinary/
domain sums and means without another row buffer or argument-evaluation pass.
The [contract and benchmark record](testing/FLOATING_SUMS.md) distinguish this
numerical profile from native Access precision/promotion qualification.

All 669 query tests plus one doctest, 199 Access adapter tests and 35 SQLite tests
plus one doctest pass. Focused sum tests additionally check nested active domain
sum/mean scratch, every observed cancellation checkpoint, volatile inputs once,
late operand errors after an otherwise overflowing prefix, NULL groups, allocation
ceilings and unchanged fixed/mixed admission. Synthetic native sums cover Jet 4,
ACE 2007 and ACE 2016/2019 with metadata-only preparation. The original Fraction/
raw-getter runner passes 155 controls, including sequential and both ID-index
orders over unchanged registered 20,000-row inputs; only TableCraft executes SQL.

Six final alternating release pairs measured about 15 microseconds extra for
4,096 floating rows/one group and 111 microseconds extra for 2,048 rows/128 groups
(about 1.6%/3.5%). The integral control stayed within spread. Hyperfine whole-process
means were 109.4/112.4 ms; all raw measurements, memory fields and exploratory
samples excluded for concurrent verification are retained. No speed gain is
claimed. Query/Access/CLI all-target/all-feature Clippy, standalone minimal query
check, formatting and diff checks pass. No dependency, stored carrier or write
boundary changes. Full workspace/native Access qualification remains unrun and
broader P0 promotion, coercion and native text work remains unfinished.

## Distinct CDate/CVDate return declarations

Date conversion resolution now preserves CDate and CVDate identities while
sharing the same materialized conversion. The explicit declared-arithmetic
profile marks CDate Scalar and CVDate Variant from the reviewed MS-VBAL signatures,
independently of the input category. Narrow quotient overflow now reaches the
correct Scalar refusal or Variant widening through direct, derived, grouped and
correlated expressions. Public DATE carriers, exact stored bits, conversion
policies, NULL/operand failures and default arithmetic remain unchanged.
The [declaration contract](DECLARED_ARITHMETIC.md#date-conversion-return-declarations)
and [conversion contract](DATE_CONVERSIONS.md#cvdate-query-alias) retain scope and
separate native Access qualification.

All 534 all-feature expression tests plus one doctest and 334 minimal-feature
expression tests pass. Seventeen focused query date/declaration/zero-divisor tests
and eight CLI date/declaration tests pass. The declaration runner passes 930
independent controls, including half-even DATE operands, exact quotient/remainder
bounds, both conversion identities, four Variant combinations and unchanged
registered native inputs. The separate CVDate replay passes 2,470 Gregorian/
Automation conversion/refusal controls. Only TableCraft executes SQL.
Expression/query/CLI all-target/all-feature Clippy, CLI build and formatting/diff
checks pass. Full query/workspace suites were not repeated for this extraction;
other function declarations and broader P0/native qualification remain unfinished.
No dependency, stored value or write boundary changes.

## Operator-specific zero-divisor errors

Floating-selected zero/zero now reports Overflow, including signed zeros;
nonzero/zero retains DivisionByZero. Decimal division and rounded integer
division/Mod retain DivisionByZero for zero divisors. Checked input conversion,
NULL propagation and written operand failures keep their ordering. Aggregate,
domain, correlated and TOP consumers propagate the operational failure; IsError
does not convert it into Error data. The [contract](ARITHMETIC_PROMOTION.md#zero-divisor-error-selection)
records primary language references, explicit profile boundaries and outstanding
native Access qualification.

All 533 all-feature expression tests plus one doctest, 333 minimal-feature
expression tests, 662 query tests plus one doctest and eleven focused CLI
eval/arithmetic/division tests pass. Three new kernel tests cover zero-carrier
pairs, four Scalar/Variant combinations, Decimal scales, rounded divisors,
subnormal numerators, NULL and nonfinite inputs. Query tests cover parameter reuse,
eager failures and late errors through eleven ordinary and eleven domain folds,
including count/encounter/extrema consumers and buffered TOP results.

The original runner passes 609 independent operator/type/error-class controls,
including exact CLI error class with empty stdout and unchanged registered Jet 4,
ACE 2007 and ACE 2016/2019 native getters through sequential and both explicit ID
index directions. Only TableCraft executes SQL. Expression/query/CLI all-target/
all-feature Clippy and formatting/diff checks pass. No dependency, stored carrier
or native write boundary changes. Full workspace suites, benchmarks and native
Microsoft Access comparisons were not run; broader P0 semantics remain open.

## Restored source-free evaluation allocation budget

Source-free query eval now derives its cumulative allocation allowance from the
actual default EvalLimits rather than the 1 MiB optional per-value ceiling.
This restores the existing 64 KiB shared allowance across columns/intermediates;
explicit larger query-run controls remain separate. The existing CLI eval limit
regression exposed the mismatch and now passes, alongside all eight eval tests,
arithmetic/division CLI regressions and all-target/all-feature CLI Clippy.
See [the limit contract](LARGE_PAYLOADS.md#separate-query-and-output-limits).

## Explicit scalar string-expression lengths

Len/LenB now accept ordinary scalar query string-expression sources through the
existing CStr policies. Numerics require NumberFormat and legacy DATE requires
DateTextFormat. Text/NULL and raw LenB byte intermediates retain their existing
routes; native-only carriers, Binary and variable/UDT storage-size overloads are
not inferred. Shared conversion/scanning limits and eager expression failures
remain in force. The [length contract](BYTE_STRING_FUNCTIONS.md#scalar-length-expressions)
records the Microsoft references and separate native query qualification limits.

All 530 all-feature expression tests plus one doctest and all 330 minimal-feature
expression tests pass. Twelve focused query length/byte/slice tests pass, including
prepared-policy rebinding, parameters, CVar and correlated subqueries. Tests cover
every observed scalar conversion/length-scan cancellation point, explicit policy
absence, byte/work/Text ceilings, Binary refusal and raw publication boundaries.
The extended independent runner passes 356 combined spelling/length/byte controls
in both CLI modes and sequential/ascending/descending reads of unchanged Jet 4,
ACE 2007 and ACE 2016/2019 fixtures, including one-to-five-digit native Long fields.
The independent expectations use explicit spellings/UTF-16 bytes and raw Jackcess
getters; only TableCraft executes SQL.

Expression/query/CLI all-target/all-feature Clippy, CLI build, formatting and diff
checks pass. No dependency or write boundary changes. Full query/workspace suites,
benchmarks and native Microsoft Access comparisons were not run. Complete Variant
coercion, native query overload selection and broader P0 semantics remain open.

## Known fixed-prefix reads in deleted-column histories

Native scalar readers now read surviving fixed values and the null mask when
metadata proves no surviving variable column existed at the historical row count.
They do not infer the removed columns' storage classes or parse an ambiguous
deleted tail. Absent visible fields remain NULL/FALSE. Required visible variable
tails, known prefix/mask bounds, counts and malformed metadata retain validation,
including for zero-width projections. This is metadata routing, not fallback
following a failed tail parse. The separate complete-tail parser used by metadata
patching still rejects the ambiguous case; all native edit gates remain unchanged.

The [history contract](testing/COLUMN_HISTORIES.md#known-fixed-prefix-qualification)
and [fixture provenance](FIXTURES.md#known-fixed-prefix-history-inputs) record the
qualification. Original fixed-first and variable-first recipes use reviewed
Jackcess only for inert creation/getters; removed declarations are separate
engineering copies. The new runner passes 128 controls across Jet 4, ACE 2007,
ACE 2016 and ACE 2019: 56 positive full/cursor/CLI/stream/query/count controls and
72 atomic corrupt-copy refusals. Existing immutable history inputs add 64 passing
controls, including the formerly refused AmbiguousGap visible values. All base
and derivative bytes remain unchanged; no fixture binary is tracked.

Native unit tests pass 139 tests with one intentional regeneration generator
ignored. Fifty-six distinct focused native row/schema/projection/check/encryption/
mutation regressions pass, including both deleted storage orders, every observed
prefix-read cancellation checkpoint and unchanged write blockers. All 198 Access
adapter tests pass. All-target/all-feature ACE/Access/CLI Clippy, the standalone
minimal ACE check, formatting and diff checks pass, as do 35 CLI browsing/native-query
regressions. The full ACE/workspace suites and native Windows comparisons
were not run; this does not qualify deleted tails, DDL, repair or native save/reopen.

The P0 text-policy review found no evidence justifying a native mixed-policy
precedence rule or substituting the ICU ordering probe for consumed-span search.
Existing conservative routing remains; native precedence, concrete locale
providers and broader backend acceptance work continue separately.

## Declared Single overflow and the shared numeric service

The numeric library now provides `declared_numeric_arithmetic` for nullable
ordinary numeric carriers plus `uses_declared_overflow` for the query dispatch
boundary. Integral arithmetic reuses the existing explicit kernel; checked
rounded numeric operands retain their original declaration categories. Qualified
Single-selected +, -, *, and / reject widening for two Scalars and retain finite
Double widening with Variant participation. Long/LongLong or Double mixing selects
Double independently. Currency/Decimal/LongLong/Double keep existing range checks.

The metered `Evaluator::apply_declared_numeric_arithmetic` consumes shared work and
checks cancellation before its allocation-free kernel. Query execution preserves
nested/correlated/parameter provenance and only requires unknown declarations when
a qualified width decision needs them. NULL, eager errors, signed zero, underflow
and strict unsupported-family admission keep explicit boundaries. The CLI now
exposes `--declared-arithmetic`; `--declared-integral` remains a visible alias.
Public execution/worker option fields retain their historical names and defaults.
The [contract](DECLARED_ARITHMETIC.md#single-selected-overflow-and-the-numeric-kernel)
records the exact engineering profile and separate native SQL qualification gaps.

All 527 expression tests plus one doctest, 330 minimal-feature expression tests,
657 query tests plus one doctest, 35 SQLite tests plus one doctest, and four focused
CLI tests pass. Worker tests prove Scalar Single refusal/default widening stays
request-local and leaves the reader reusable. All-target/all-feature expression/
query/CLI/SQLite Clippy and warnings-denied expression/query rustdoc pass.
The original runner passes 1,300 independent IEEE carrier/bit controls, including
signed zeros, subnormals, Single overflow rounding boundaries, four operators,
all declaration combinations and raw Jackcess Single fields from unchanged
registered Jet 4/ACE 2007/2016/2019 inputs. Only TableCraft executes SQL.

No dependency, core stored carrier, default arithmetic or ordinary write gate
changes. Full workspace suites, benchmarks and native Windows comparisons were
not run. General declared-expression inference, wider Variant/state semantics,
P0 text work and the full backend objective remain ongoing.

## Declaration-aware rounded quotient and remainder

The expression evaluator now exposes `apply_declared_rounded_arithmetic` for
integer division/Mod after checked integral operand coercion. Original declaration
categories survive conversion from Single/Double, Currency/Decimal, legacy DATE
and exact numeric Text; Scalar effective-width overflow fails while Variant
participation retains the materialized widening profile. Conversion ranges,
rounded zero divisors, explicit numeric formats and contextual NULL behavior stay
in force. The pure integral kernel's admission and default SQL behavior are unchanged.

`query run --declared-integral` routes qualified rounded operands through this
service. [The contract](DECLARED_ARITHMETIC.md#rounded-integer-division-and-mod-operands)
records effective-width ordering, exact lexical rounding and primary references.
No declaration is inferred from the converted value. Unsupported carriers retain
existing refusals and required unknown declaration categories remain explicit.

All 523 expression tests plus one doctest, thirteen focused query regressions,
three CLI declaration tests and all-target/all-feature expression/query/CLI Clippy
pass. The final focused API run additionally covers Single and Decimal source
cases. The runner passes 482 scalar SQL controls and 36 registered native-field
controls (518 total), covering operand order, all Scalar/Variant combinations,
exact Text midpoints, zero divisors and conversion limits. Native inputs retain
unchanged original hashes. No dependency, native
write gate or automatic declaration policy changes. Full query/workspace suites,
benchmarks and native Windows qualification were not run for this opt-in increment;
P0 value/text semantics and the full backend objective remain ongoing.

## Scalar sources for byte-string query functions

LeftB, RightB, MidB, AscB, InStrB and StrConv now accept ordinary scalar string
arguments through the existing execution-local CStr service. Numeric formatting
and legacy date spelling use explicit policies; raw string intermediates retain
exact bytes. Binary identity is preserved for StrConv's specific ANSI-array route
and remains rejected by ordinary byte-string consumers. Numeric LenB/storage-size
overloads remain outside this increment. [The contract](BYTE_STRING_FUNCTIONS.md#scalar-byte-string-sources)
records evaluation order, limits and remaining native qualification gaps.

Three new scalar tests cover all six calls, regional/date spellings, nested
encoding, raw publication, Binary rejection, NULL/eager failures, shared limits
and every observed cancellation checkpoint. Two prepared-query tests verify
request-local format reuse and nested failure boundaries. The CLI test parses
TSV with csv and checks both commands and buffered refusals. Existing missing-policy
expectations for numeric byte/StrConv sources now report NumberFormatRequired.

All 520 expression tests plus one doctest, 327 minimal expression tests, fifteen
focused query regressions and the new CLI test pass. All-target/all-feature
expression/query/CLI Clippy, formatting and diff checks pass. The original runner passes
348 independent spelling/UTF-16 controls, including raw Jackcess Long getters,
sequential reads and both index directions on the four unchanged registered
20,000-row priority-family native files. Original bytes remain unchanged.
No dependency, stored carrier or write gate changes. Full query/workspace suites,
benchmarks and native Windows comparisons were not run for this bounded conversion
increment; broader P0 value/text semantics remain ongoing.

## Date, reverse-search and color return declarations

The explicit integral query profile now covers eleven additional reviewed
function declarations: InStrRev/RGB/QBColor return Scalar Long; Month/Day/Hour/
Minute/Second/Weekday/DatePart return Variant containing Integer, and DateDiff
returns Variant containing Long. Return categories are independent of argument
categories; explicit result casts/CVar still replace them. The
[signature table](DECLARED_ARITHMETIC.md#reviewed-function-return-declarations)
records Microsoft's primary references and the separate native SQL qualification gap.

Twenty-six focused query regressions, twelve CLI regressions and all-target/
all-feature query/CLI Clippy pass. The independent runner passes 250 scalar SQL
controls plus 28 native stored-field controls across the original registered
20,000-row Jet 4, ACE 2007, ACE 2016 and ACE 2019 fixtures. It checks source carriers,
Scalar overflow, Variant widening, function/field composition and default behavior;
all original hashes remain unchanged. Date expectations use independent civil
components. NULL, eager errors, nested outputs and result-cast resets also pass.

No dependencies or write gates change. The full workspace/query suites and native
Windows comparisons were not rerun for this classification-only increment; the
previous full query suite remains recorded below. Broader coercion/declaration
rules and P0 text semantics remain ongoing.

## Reviewed function declaration categories

The explicit integral declaration profile now distinguishes reviewed function
signatures from successful result carriers. Len/LenB, Asc/AscB/AscW and VarType
issue Scalar categories; InStr/InStrB, StrComp, Sgn and Year issue Variant
categories. Their primary Microsoft signature references are recorded in
[the declaration contract](DECLARED_ARITHMETIC.md#reviewed-function-return-declarations).
Other functions remain unknown until reviewed. This affects only the explicit
engineering overflow policy, not default value results or native SQL claims.

Focused tests cover equal-width Scalar/Variant differences, signed/Long overflow,
derived output propagation, NULL and eager failures. The original SQL model now
passes 217 controls including 22 direct/derived function cases. Eighteen focused
CLI regressions, the full all-feature query suite and its doctest, and
all-target/all-feature query/CLI/SQLite Clippy pass. Broader
query declaration inference, carrier/coercion rules and native Access SQL
qualification remain ongoing; no new dependency or write permission is introduced.

## Explicit query integral declaration provenance

Query execution now offers `ExecutionOptions::declared_integral` and the CLI
`query run --declared-integral`. This explicit engineering profile checks declared
integral overflow while tracking Scalar/Variant categories separately from value
carriers and text policies. Typed source fields, literals, parameters and casts
supply Scalar; CVar and conditional selectors supply Variant. Derived/named,
correlated and scalar-subquery results retain provenance. Integral unary minus
uses checked declaration-aware arithmetic. Default arithmetic remains unchanged.

UNION and aggregate/domain/unclassified function outputs remain unknown rather
than inheriting a selected row's category. Required unknown declaration fails
explicitly; no binding-time scan or value-based declaration inference is added.
Wider numeric families, complete static inference and native Access SQL type
classification remain unqualified. [The contract](DECLARED_ARITHMETIC.md)
documents exact scope. The optional SQLite worker forwards both this option and
the previously introduced individual Text allowance per request; worker tests
prove requests restore defaults and remain reusable after refusals.

All 650 query tests plus one doctest, 35 SQLite tests plus one doctest, and eighteen
focused CLI regressions (including the two new tests) pass. The full query run exposed an older Asc scalar rejection
expectation; it now asserts the already-supported explicit-format conversion.
The original [SQL runner](testing/compare_query_declarations.py) passes 195
independent integer-bound/category controls, including nested selectors, carrier
results, eager errors and unchanged default behavior. All-target/all-feature
query/CLI/SQLite Clippy and the minimal query check pass. No new dependency,
source mutation or ordinary write permission is introduced. Native Windows
comparisons and the full workspace suite were not run; P0 semantics and the full
backend objective remain ongoing.

## Explicit declared-overflow integral API

The expression library now provides `OperandDeclaration`, `ArithmeticDeclarations`
and `declared_integral_arithmetic` for caller-established scalar/Variant categories.
Nullable Boolean/Byte/Integer/Long/LongLong inputs use the existing numeric profile,
but two declared scalars prohibit integral result widening. A Variant participant
retains current materialized widening; LongLong overflow still fails. Division and
power retain their current Double profile. Both input families validate before
NULL propagation. No declaration is inferred from a value or record sample.

The metered `Evaluator::apply_declared_integral_arithmetic` checks host cancellation
and consumes shared work before the bounded, allocation-free kernel. Failed
operations retain charged work. Existing default SQL dispatch remains unchanged:
query-plan provenance, CVar/derived result tracking, other carrier families and
native Access qualification remain to be integrated. [The contract](DECLARED_ARITHMETIC.md)
separates this working first layer from complete declared-type query support.
The original clap Parser probe exercises the public API without SQL/native I/O.
No new dependency, source mutation or write gate change is introduced.

Five new tests cover declared/Variant overflow differences, all 65,536 Byte pairs
for three operators against independent wide bounds, signed-width/LongLong and
quotient limits, NULL/type/zero boundaries, generated Long arithmetic and evaluator
work/cancellation/reuse. The original [runner](testing/compare_declared_integral.py)
passes 7,728 independent API controls using unbounded integer arithmetic and exact
Double bits. These are explicit declarations, not inferred Access SQL types.

All 517 expression tests plus one doctest, 327 minimal-feature expression tests
and fifteen focused query arithmetic/type/error regressions pass. All-target/
all-feature expression/query Clippy, minimal checks, warnings-denied expression
rustdoc, example build, Python compilation, formatting and diff checks pass.
Full query/workspace suites, benchmarks and native Windows comparisons were not
run. Query declaration tracking is the next integration step; the full backend
objective and its wider acceptance scope remain ongoing.

## Contextual Val scalar string inputs

Val now converts ordinary non-Text scalars through the existing CStr service,
then re-enters the unchanged period-only numeric-prefix parser with proven Text.
Numeric/date formats remain explicit, and Boolean True/False spellings parse as
zero. A decimal-comma numeric source can therefore yield an integral prefix while
literal period-decimal Text still parses its fraction. ISO date sources parse the
leading year, and large integral spellings pass through the existing Double parser.
The result remains Double rather than acquiring a new integer subtype.

Text/NULL dispatch, Binary/GUID and expression-state refusal, eager errors,
byte/work/Text/cancellation limits and buffered publication remain explicit.
Source formatting and prefix normalization share the same allowance; no new
formatter, parser, dependency or stored carrier exists. [The contract](VAL.md#scalar-string-inputs)
and builtin tracker retain complete Variant/native subtype and locale/error gaps.

Two expression and two query tests cover decimal-comma separation, literal Text,
Boolean/date inputs, nested/reused policies, NULL/eager failures, combined byte
quotas and every observed conversion/prefix cancellation checkpoint. The original
[scalar runner](testing/compare_val_scalars.py) passes 336 independent controls with
authored spellings, prefix parsing and exact Double-bit comparisons including
signed zero, plus sequential/both index orders over four unchanged 20,000-row
native fixtures. Jackcess supplies raw IDs only. Another 3,096 existing prefix/
radix/Currency/refusal controls pass in scalar CLI mode; their optional native
branch was not rerun.

All 512 expression tests plus one doctest, four focused query tests and three CLI
tests pass. All-target/all-feature expression/query/CLI Clippy, minimal expression/
query checks, debug CLI build, Python compilation, formatting and diff checks pass.
Full query/workspace suites, benchmarks and native Windows comparisons were not
run. The [arithmetic boundary](ARITHMETIC_PROMOTION.md#declared-type-boundary) now
records why materialized carriers do not establish declared-Variant promotion,
with the primary language reference separated from unqualified ACE/Jet SQL rules.
The full backend objective and remaining P0 semantics stay ongoing.

## Contextual scalar character codes and reversal

Asc/AscW and StrReverse now convert ordinary non-Text scalar operands through the
existing CStr service before using their established kernels. Numeric/date
formatting remains explicit; Boolean spelling retains the existing profile.
Conversion occurs once with proven Text re-entry. Text/NULL inputs keep their
existing dispatch path and no additional Text source copy is introduced.

Asc/AscW retain Int16 results, explicit code-page/ASCII rules and signed leading
UTF-16 units. For example Asc(65) with NumberFormat returns 54, the code for the
first character of "65". StrReverse retains bounded BMP code-unit reversal.
NULL remains an error, empty reversal remains empty, and surrogate/code-page
refusals stay explicit. Source formatting and output share cumulative bytes/work,
per-value Text and cancellation controls. No new formatter, provider, dependency
or stored carrier exists. [Character contracts](CHARACTER_FUNCTIONS.md#scalar-string-sources)
and [reversal limits](TEXT_CONSTRUCTION.md#scalar-reversal-sources) retain full
Variant, regional, DBCS/Unicode and native qualification gaps.

Three expression and two query tests cover ordinary scalar kinds, format rebinding,
nested calls, exact numeric result carriers, NULL/empty/provider failures,
surrogates, combined quotas and every observed conversion/output checkpoint.
The original [runner](testing/compare_character_reverse_scalars.py) passes 336
independent authored spelling/ASCII-code/type/reversal controls through scalar
CLI paths and sequential/both index directions over the four unchanged 20,000-row
native fixtures. Jackcess supplies raw IDs only; it does not evaluate SQL.

All 510 expression tests plus one doctest, six focused query tests and six CLI
tests pass. All-target/all-feature expression/query/CLI Clippy, minimal expression/
query checks, debug CLI build, Python compilation, formatting and diff checks pass.
Full query/workspace suites, benchmarks and native Windows comparisons were not
run for this slice. No performance or complete native compatibility claim is
made; the full backend objective and remaining P0 semantics stay ongoing.

## Contextual scalar trimming and casing

LTrim/RTrim/Trim and LCase/UCase now convert ordinary non-Text scalar inputs through
the shared CStr service. Text/NULL inputs retain their existing dispatch path.
Numeric/date formats remain explicit and casing still requires CaseLocale for
every non-NULL input. The converter re-enters only with proven Text, bounding
conversion to one materialization before the existing kernels. No source clone is
added for Text, and no new formatter, locale provider or dependency is introduced.

Source formatting and output share byte/work/Text/cancellation budgets. Trimming
still removes only U+0020, and ICU language/mapping behavior is unchanged. NULL
and eager errors retain their existing boundaries; Binary/GUID and expression-only
states remain outside this scalar conversion path. [The owning contract](TEXT_CASING.md#scalar-sources)
and builtin tracker retain complete Variant/native regional/error-precedence gaps.

Three expression and two query tests cover ordinary scalar carriers, reused/nested
format contexts, NULL/provider failures, preserved controls/nonbreaking spaces,
combined quotas and every observed conversion/output cancellation checkpoint.
The original [runner](testing/compare_trim_case_scalars.py) passes 336 independent
spelling/ASCII controls through scalar CLI modes and sequential/both index orders
on the four unchanged 20,000-row native fixtures. Jackcess supplies raw IDs only.
Another 562 existing stable-Unicode/language/refusal controls pass in scalar CLI
mode; the old runner's optional native branch was not rerun.

All 507 expression tests plus one doctest, five focused query tests and seven CLI
tests pass. All-target/all-feature expression/query/CLI Clippy, minimal expression/
query checks, debug CLI build, Python compilation, formatting and diff checks pass.
Full query/workspace suites, benchmarks and native Windows comparisons were not
run for this focused slice. No performance or complete native compatibility claim
is made; the full backend objective and remaining P0 semantics stay ongoing.

## Contextual Left/Right/Mid scalar string sources

Left/Right/Mid now format ordinary scalar sources through the shared CStr service.
Text stays borrowed; numeric/date formatting remains explicit and Boolean spelling
retains the existing profile. NULL still returns NULL after written expressions
have evaluated, without converting materialized counts. Other sources format
before checked count conversion. Binary/GUID and expression-only states remain
outside this ordinary scalar path. No new formatter, dependency or stored carrier
is introduced.

The existing slice kernel retains UTF-16 positions, nearest-even Long counts,
Mid start-before-length validation and strict surrogate boundaries. Source
formatting and final slice copying share byte/work/Text/cancellation limits.
Converted source strings stay local until the final owned result is complete;
prepared queries cache no spelling. The [contract](SLICE_COUNTS.md#scalar-string-sources)
and builtin tracker retain broader Variant, native error/locale and Unicode gaps.

Three expression and two query tests cover scalar source kinds, format rebinding,
nested positions, NULL/eager failures, combined byte quotas, surrogate boundaries
and every observed conversion/scan/output cancellation checkpoint. The original
[scalar runner](testing/compare_slice_scalars.py) passes 336 independent controls
using authored spellings and UTF-16 slices, with independent raw IDs from four
unchanged 20,000-row Jet 4/ACE fixtures through sequential and both index orders.
Another 1,086 existing rounded-count/UTF-16 controls pass in scalar CLI mode; their
optional native-fixture branch was not rerun in this slice.

All 504 expression tests plus one doctest, ten focused query tests and five CLI
tests pass. All-target/all-feature expression/query/CLI Clippy, minimal expression/
query checks, debug CLI build, Python compilation, formatting and diff checks pass.
Full query/workspace suites, benchmarks and native Windows comparisons were not
run for this slice. No performance or native compatibility claim is made; P0
semantics and the full backend roadmap remain ongoing.

## Contextual Replace scalar string operands

Replace now converts ordinary scalar source/find/replacement operands through the
shared CStr service. Text remains borrowed, numeric/date formatting stays explicit,
and Boolean spellings retain the existing profile. Required NULL and Binary/GUID
refusals remain. Optional start/count/mode validation precedes string conversion;
all written expressions still evaluate eagerly, including unused empty/count-zero
operands. No inferred locale, new formatter, provider or dependency is introduced.

Validated owned options are separated from borrowed operands in the existing
replacement feature. Converted strings remain local while the recipe streams into
the existing metered sink; only a complete Value is returned. No self-referential
recipe, cloned argument vector or match list is retained. Pure Text/NULL kernel
behavior, Unicode boundaries, provider spans and per-value/cumulative limits remain
explicit. [The contract](TEXT_REPLACEMENT.md#scalar-string-operands) and builtin
tracker retain native Variant, regional and error-precedence limitations.

Three expression and three query tests cover scalar positions, policy rebinding,
grouping/correlation, NULL/options/unused operands, eager failures, shared byte/Text
limits and every observed cancellation checkpoint. The original
[scalar runner](testing/compare_replacement_scalars.py) passes 336 independent
spelling/UTF-16 controls across scalar CLI paths and sequential/ascending/descending
indexed reads of the same unchanged four-family 20,000-row inputs. Jackcess supplies
raw IDs only; no native Access SQL is executed.

All 501 expression tests plus one doctest, eight focused query tests and eight CLI
tests pass. All-target/all-feature expression/query/CLI Clippy, minimal expression/
query checks, debug CLI build, formatting, Python compilation and diff checks pass.
Full query/workspace suites, benchmarks and native Windows comparisons were not
run for this focused slice. No performance, complete coercion or native Access
compatibility claim is made. The full backend objective remains ongoing.

## Contextual InStr/InStrRev scalar string operands

Substring search now reuses the existing CStr conversion for ordinary scalar
haystack/needle operands. Numeric/date formatting remains explicit; Boolean
spellings reuse the existing profile. Text operands remain borrowed. StrComp's
existing conversion helper moves beside CStr and is reused without changing its
semantics. No new formatter, locale provider, dependency or stored carrier exists.

The evaluator shares mode/start preparation with the strict kernel, validates
numeric options before string formatting, then preserves NULL without formatting
its partner. Eager expression failures still occur first. Per-value Text, cumulative
byte/work and cancellation limits remain active. Search units, forward/reverse
bounds, pure Text/NULL kernels and explicit LCID dispatch retain their contracts.
The [search contract](STRING_SEARCH.md#scalar-search-operands) links primary
references and separates materialized scalar conversion from full Variant/native
qualification; the builtin tracker retains Partial status.

Three expression and three query tests cover Boolean/numeric/fixed/date operands,
policy changes on reused plans, grouping/correlation, NULL/start/mode failures,
eager errors, byte/Text limits and every observed cancellation checkpoint.
The original [comparison runner](testing/compare_search_scalars.py) passes 336
independent spelling/UTF-16/native-source controls. It checks both scalar CLI paths
and sequential/ascending/descending index reads over the four unchanged 20,000-row
fixtures; Jackcess supplies raw IDs only and does not evaluate SQL.

All 498 expression tests plus one doctest, all 634 query tests plus one doctest,
and ten targeted CLI tests pass. All-target/all-feature expression/query/CLI
Clippy, minimal expression/query checks, debug CLI build, Python compilation,
formatting and diff checks pass. Full workspace suites, benchmarks and native
Access comparisons were not run. No performance or complete-coercion claim is
made; P0 value/text semantics and the full backend roadmap remain ongoing.

## Explicit larger query Text and output allowances

`ExecutionOptions::max_text_bytes` now controls individual Text admission, with
64 KiB unchanged by default and a 1 MiB hard maximum. Source/parameter copying
and scalar evaluators use the same selected limit across nested/domain/discovery
execution. Expression defaults, parser/binding literals, cumulative query budgets,
comparison-provider limits, native limits and writes remain separate.
`EvalLimits::DEFAULT_TEXT_BYTES` retains the old default while its explicit maximum
is now 1 MiB; oversized requests clamp, with focused boundary coverage.

CLI `query run --text-bytes` and `--output-bytes` expose separate explicit controls.
Output defaults to 1 MiB and permits up to 16 MiB across escaped display buffering
and final CSV TSV/table presentation. CSV quote expansion counts; table layout
keeps its own bound and can require TSV. The flags do not apply to eval, parse,
saved reconstruction or update. A boxed feature argument group keeps the command
enum bounded. No new dependencies or unbuffered publication are introduced.

The [larger payload runner](testing/compare_large_payloads.py) now passes 55 controls
against the same unchanged Jet 4 and ACE 2007/2016/2019 inputs. Full CLI Memo/OLE
and decoded attachment projections match independent getter/Python byte digests;
UTF-16 Len, NULL/empty distinctions, default refusals and resource gates also pass.
[The contract](LARGE_PAYLOADS.md) records independent evidence and remaining limits.

All 631 query tests plus one doctest, all 495 expression tests plus one doctest,
and 32 CLI unit/focused integration tests pass. The new tests cover prepared-plan
reuse, nested/domain calls, parameters/intermediates, UTF-8 byte bounds, hard-cap
clamping, budgets/cancellation, flag scope/ranges and final CSV expansion.
All-target/all-feature expression/query/CLI Clippy, minimal expression/query
checks, debug CLI build, formatting and Python compilation pass. Full workspace
suites, benchmarks and native Access comparisons were not run. No performance or
native conformance claim is made; remaining read/query semantics stay ongoing.

## Larger native stored payloads

Qualified long-value reads now admit 2 MiB encoded MEMO/OLE across at most 1,024
LVAL rows, with a separate 1 MiB decoded UTF-8 Memo ceiling. Attachment decoding
admits 1 MiB files in both stored and zlib modes, a 4 KiB header and 2 MiB encoded
storage. Cycle scanning charges its bounded work; Memo decoding charges logical
input passes and retains before/after cancellation checks. All caller budgets,
query Text limits, CLI output limits, source/image envelopes and write gates
remain explicit. [Larger payloads](LARGE_PAYLOADS.md) records the boundaries.

Original independently generated Jet 4 and ACE 2007/2016/2019 files pass 36 controls
against Jackcess read-only getters and authored Python bytes: exact large Memo/OLE,
empty/NULL Binary, actual attachment codecs and decoded file bytes, CLI Binary
counts, exhausted budgets, oversized query Text refusal and unchanged originals.
The original fixture recipe, hashes and runner are recorded in [FIXTURES.md](FIXTURES.md#original-larger-stored-payload-corpus).
Engineering examples gain explicit byte/work allowances; no dependencies change.

Twenty focused engine tests, seven adapter tests and fifteen CLI retrieval/native
budget tests pass. Full-size synthetic chains cover all four families, late
corruption, overlong chains, byte/work exhaustion and sampled cancellation.
All-target/all-feature ACE/access/CLI Clippy, minimal ACE check, debug CLI/example
builds, Python compilation and formatting pass. Full workspace suites, performance
benchmarks and native Windows comparisons were not run; no speedup or native
Access compatibility claim is made. Larger SQL Text and output transport remain
next acceptance work.

## Runtime domain-result text provenance

Internal query cells now carry expression values with host-issued text policies.
DLookup/DFirst/DLast/DMin/DMax retain the actually bound result policy through
selectors, aliases, derived/correlated rows, groups, sets, sort/TOP ties, pivots and
operation-local named-crosstab materialization. Public stored/expression values and
source contracts retain their existing shape; preparation still performs no domain
scans or provider calls. Static domain metadata stays Default without discarding
runtime provenance.

Sort/set-key contributors merge before comparison. Group/heading representatives
retain policies from matching discarded cells, so a neutral literal cannot hide
later policy conflicts. Grouped output and correlated expressions use the existing
structural identity rules through a shared metered plan service. Fixed headings
are rechecked when a runtime pivot policy becomes known. Raw projection and global
comparison overrides retain their prior behavior; fresh text functions keep the
existing provisional Default policy.

All 626 query tests and the documentation test pass, including 29 collation-routing
tests. Eighteen focused CLI retrieval/General/database tests pass. Query Clippy
with all targets/features, minimal query check, debug CLI build and warnings-denied
Rustdoc pass. The independent native-policy runner passes 1,460 model/source
controls on unchanged registered mixed-policy inputs. Rounded-predicate and narrow
index regression runners additionally pass 284 and 128 controls on unchanged
20,000-row inputs. Formatting/diff checks pass. Full workspace and Windows suites
were not run.

[The contract](RUNTIME_TEXT_POLICIES.md) records conservative merges and remaining
native qualification. Cells, keys and correlated frames charge their actual widths;
public expression publication uses additional fallible, metered vectors while
moving payloads. Existing byte/work caps can therefore be reached earlier; none
is silently raised. No speedup, concrete locale provider, new dependency or native
write qualification is claimed.

## Contextual StrComp scalar operands

Evaluator-backed StrComp now formats supported numeric, Boolean and date operands
through the existing CStr service before comparing strings. Existing Text is
borrowed; converted intermediates share cumulative work/byte limits and explicit
format policies. Mode validation precedes NULL propagation, and eager expression
failures remain visible. Low-level Text/NULL kernels and comparison providers
retain their original contracts; no locale is inferred.

All 495 expression unit tests and its documentation test pass. Eight focused
query tests cover comparison, option coercion and CStr integration. Expression/query
all-target/all-feature Clippy, standalone minimal expression checks, debug CLI
build, formatting and diff checks pass. The new independent runner passes 620
spelling/UTF-16/native-source controls on unchanged registered inputs; the original
StrComp runner still passes 85 controls. Quota and every observed cancellation
checkpoint are covered. Full workspace/query and native Windows suites were not run.

[The contract](STRING_COMPARISON.md#contextual-scalar-string-operands) records
conversion, ordering and native limits. The domain-policy audit also records the
remaining runtime result-lineage gap explicitly; no static guess was introduced.
No dependency, native storage, write gate or native Access qualification changes.

## Switch and Choose retain result text policies

Switch result values and Choose choices now contribute their static field-policy
identities to query outputs. Conditions and selector indexes do not contribute;
they still evaluate with their own normal rules. This extends the existing IIf
merge model: distinct result identities stay Conflict even for unselected branches.
Metadata preparation remains inert, and comparisons no longer silently use the
host default after a selector discarded its result's provenance. Raw projections
and explicit global comparison overrides keep their earlier behavior.

Sixty-three focused tests pass across collation routing, text metadata, conditionals
and named crosstabs. The generated contributor-set model now includes selectors,
with 256 fixed-seed bounded cases. A saved regression seed and explicit excessive
nesting test retain the original binding depth refusal; no production limit changes.
Final step-limit checks and cancellation at every observed checkpoint pass. Query
all-target/all-feature Clippy, minimal query check, debug CLI build, formatting
and diff checks pass. The extended independent native-policy runner passes 660
model/source controls on the four unchanged registered mixed-policy inputs.

[The selector contract](CONDITIONAL_FUNCTIONS.md#query-result-text-policy-lineage)
records the conservative policy. No new locale provider, native precedence claim,
dependency or write behavior is introduced. Full workspace/query and Windows
Access suites were not run for this slice; native Text/LCID and broader locale
coverage remain unfinished.

## Rounded LongLong floating and numeric-Text predicates

Contextual LongLong/Single, LongLong/Double and LongLong/Text comparisons now
convert both operands through the existing checked Double converter. Single
widens rather than forcing a LongLong back to Single precision. All six operators
and both written orders share this route across scalar and relational predicates.
Integer-only equality/order remains exact. Sorting/group/set keys, stored values,
source schemas and physical index hints retain their existing contracts.

All 493 expression unit tests and 613 query tests pass, plus each package's one
documentation test. The final focused quota/cancellation controls pass as well.
All-target/all-feature expression/query Clippy, standalone minimal expression
checks, debug CLI build, formatting and diff checks pass. The original independent
runner passes 284 controls, including seeded full-range Int64 values and raw ID
getters from four unchanged registered 20,000-row fixtures. The existing
source-free numeric-Text runner also passes 1,004 controls.

[The predicate contract](NUMERIC_PREDICATES.md#rounded-longlong-predicates) keeps
materialized numeric-Text choices distinct from complete declared/Variant typing.
Windows Access SQL equivalence remains unqualified; no dependency, writer or
native storage behavior changes. Full workspace/native Windows suites were not run.

## Rounded LongLong operands in Double arithmetic

Materialized arithmetic now uses the existing checked Double converter when
LongLong participates in a Double-selected operation: mixed floating +, - and *,
floating division and power. Integers beyond 53 significant bits round to nearest
even before the operation. Integer-only arithmetic and its overflow checks remain
exact; strict kernels, predicates and set/group key comparison retain their own
precision guards. Result metadata already describes these successful cases as
Double. [The contract](ARITHMETIC_PROMOTION.md#rounded-operands-for-double-selected-arithmetic)
records precedence, examples and remaining native qualification.

All 491 expression unit tests and 611 query tests pass, plus each package's
one documentation test. All-target/all-feature expression/query Clippy, standalone
minimal expression checks and a debug CLI build pass. The new runner passes 176
independent source-free/native-source controls on unchanged registered inputs.
Existing source-free promotion and division runners also pass 734 and 944 controls.
A fixed-seed independent significand model covers nearest-even conversion across
signed Int64 values. Formatting and diff checks pass.

No dependency, stored carrier, write gate or comparison policy changes. Full
workspace and Windows Access suites were not run; broader declared/Variant typing
and native query equivalence remain pending.

## Exact Byte and Int16 range reads

Typed unsigned Byte and signed Int16 bounds now pass from safe query constraints
through the Access adapter to matching native index requests. Out-of-width bounds
decline without clamping; original predicates, required-index errors, cancellation
and ephemeral selected snapshots remain intact. Only checked CByte/CInt closed
integral literal casts are newly admitted by integer hint analysis.

The focused batch passes 12 engine, 13 Access and 40 query retrieval tests, including
unchanged original narrow-index coverage. All-target/all-feature Clippy for those
packages, standalone minimal engine/query checks and a debug CLI build pass.
The original runner passes 128 independent controls on unchanged registered
20,000-row inputs. [The contract](NARROW_RANGES.md) records scope and evidence.
No dependency, write gate or performance claim changes. Full workspace and native
Windows suites were not run.

## Exact Currency range reads and CLI parameters

Single-Currency indexes now accept typed exact ranges/NULL selections through the
engine request API. Backend-neutral Currency constraints and the Access adapter
retain the original WHERE, strict native key-kind checks, required-index failures
and ephemeral selected snapshots. Safe hints admit non-NULL Currency parameters
and closed numeric CCur literals; unsafe/invalid constants decline hints without
new semantic failures on empty inputs. The CLI now accepts invariant `currency:`
parameters through the existing exact numeric Text converter and a shared budget.

Seventy-six focused tests pass across native Currency/index-node reads, Access
source/projection/selection, query retrieval and CLI retrieval. Updated Currency
checkpoint/NULL-parameter controls also pass. All-target/all-feature engine/access/
query/CLI Clippy, minimal engine/query checks and debug/release CLI builds pass.
The original comparison runner passes 136 controls in both debug and release,
using unchanged registered 20,000-row and full-range Currency inputs.

[Release measurements](CURRENCY_RANGES.md#release-measurements) retain 15 samples
per command, outliers and the excluded overlapping run. The 20,000-row point query
averages 11.888 ms sequentially and 7.848 ms indexed; the broad 700-row indexed scan
is slower, 4.032 ms versus 2.608 ms. These are process wall times with warm OS caches,
not scalar-kernel timings or a universal speedup. No automatic optimizer is added.

[The contract](CURRENCY_RANGES.md) records exact units, profile limits and pending
native Access qualification. No dependencies, stored values or write gates change.
Full workspace/native Windows suites were not run; unrelated promotion/NLS gaps
remain explicit rather than inferred from this read optimization.

## Expression extrema and remaining Error consumers

Min/Max and DMin/DMax now retain standard Error-only extrema under an explicit
provisional profile, ignoring NULL and rejecting mixed scalar/Error kinds or opaque
codes. Ordinary scalar extrema keep strict ordering and text-policy routing.
Numeric/statistical aggregates reject Error with TypeMismatch unless codes were
explicitly converted. Ampersand now evaluates both operands before Error rejection;
explicit CStr remains distinct. Scalar-only result publication still refuses raw
Error cells. Extrema have a separate feature module and retain one selected cell.

The full all-feature query and expression suites pass 603 and 490 tests respectively,
including doc tests. Focused tests cover scalar/expression APIs, code-zero/endpoints,
derived/grouped/domain/pivot results, NULL/singleton/empty cases, generated code
sequences, mixed kinds, late failures and every observed cancellation checkpoint.
Query/expression Clippy, separate minimal builds, CLI build, formatting and changed
contract links pass. The original comparison runner passes 410 independent controls
using Python models and read-only getters across four unchanged 20,000-row native
inputs, sequential and both indexed directions.

[The aggregate contract](EXPRESSION_AGGREGATES.md) records references, provisional
Error-only semantics and pending native Access subtype/error-order qualification.
Full workspace and native Windows comparisons were not run. No dependency,
stored carrier or native write behavior changed.

## Standard Error predicates and logical admission

Standard Error/Error pairs now compare their codes through the six relational
predicates, IN lists/subqueries, reversed BETWEEN and quantified comparisons.
Mixed Error/scalar pairs fail TypeMismatch; opaque Error/Error pairs remain
unsupported. Logical operators reject Error after eager binary operand evaluation.
Ordinary scalar predicates keep contextual conversion and per-field text policies.
Named preparation remains metadata-only; sort/group/set keys retain their existing
scalar restrictions. Shared predicate combinators now accept borrowed scalar or
expression operands without new buffering or hidden scans.

The full all-feature expression suite passes 488 tests including doc tests; an
additional shared-work/cancellation regression also passes. A full query batch
passed every target except a stale arithmetic Error assertion from the prior
slice. Correcting it and rerunning expression-result, Error-predicate and collation
routing targets passes all 28 focused tests. Separate initial predicate/logical/
arithmetic targets pass 14 tests. Expression/query Clippy, separate minimal builds,
CLI build, formatting and changed-document links pass.

The original runner passes 838 independent CLI/model/native-source controls across
both eval/run and four unchanged 20,000-row inputs, including sequential/indexed
filters, derived values, DCount and quantified/IN subqueries. The
[contract](VARIANT_ERRORS.md#error-predicates-and-logical-operators) records the
primary references and remaining declared/Variant, opaque-code and native Access
qualification gaps. Full workspace and native Windows comparisons were not run.
No dependencies, stored carriers or native write behavior changed.

## Arithmetic Error admission

Scalar and bound arithmetic now evaluate both operands before rejecting expression
Error data with TypeMismatch. Unary signs and all seven binary arithmetic operators
share the admission rule. Explicit numeric conversion and raw scalar-only result
publication keep their distinct contracts. Derived/correlated, encounter-aggregate
and crosstab operands retain the rule; operational failures are not caught.

The full all-feature expression suite passes 484 tests including doc tests.
Forty-one focused query tests pass, including arithmetic promotion, Error/NULL
pairs, eager failure ordering, composition, parameter reuse and every observed
query cancellation checkpoint. Expression/query Clippy, separate minimal builds,
CLI build, formatting and changed-document links pass. The original runner passes
1,190 independent CLI/model/native-source refusal controls with unchanged inputs.
[The contract](VARIANT_ERRORS.md#arithmetic-error-admission-and-eager-operands)
records the primary specification and pending native SQL qualification. The full
query suite passed 588 tests before this arithmetic slice; only the named focused
query targets were rerun afterward. Full workspace and Windows tests were not run.
No dependency, stored type or native write behavior changed.

## Counting expression Error values

Count now tallies non-NULL expression cells without converting Error codes or
requiring stored-value publication. Grouped/HAVING, derived/named and correlated
consumers, crosstab cells/totals and DCount share the bounded kernel. Empty counts
remain zero and missing pivot intersections remain NULL. Numeric aggregates retain
their existing refusal; invalid constructors and operational failures still fail
queries without buffered output.

The full all-feature query suite passes 588 tests including doc tests. Focused
aggregate/domain/crosstab/Variant targets pass 34 tests, including a fixed-seed
256-case NULL/Error/scalar sequence model and every observed cancellation
checkpoint. Query Clippy, minimal build, CLI build, formatting and changed-document
links pass. The original runner passes 98 independent controls across seeded
Python models and four unchanged 20,000-row native inputs, including both index
directions, selected ranges, DCount and late-error refusals.

[The contract](VARIANT_ERRORS.md#counting-expression-error-values) distinguishes
this explicit nullness profile from pending native Access Error-cell/error-order
qualification. No dependency, stored carrier or native write behavior changed.
Full workspace and native Windows comparisons were not run.

## Typed scalar crosstab headings

Crosstabs now accept Boolean, Single, Double, Currency, Decimal and both date
carriers through existing explicit CStr policies. Keys remain typed; name collisions
fail instead of merging values. Every candidate is validated under shared budgets.
Boolean fixed schemas bind without scans; contextual named schemas use explicit
execution-phase discovery and retain per-execution formatting. Binary/Guid remain
unsupported, and native heading spelling/ordering is not qualified.

The full all-feature query run passed every target except an old Decimal-heading
refusal assertion. Updating that assertion to require NumberFormat and rerunning
its target plus scalar-heading/named-crosstab targets passed all 38 focused tests. An additional named-date discovery regression also passes.
All-target/all-feature query Clippy and the minimal query build pass. The independent
runner passes 68 controls, including four immutable 20,000-row native inputs,
Python count/name models, missing policies and collision/length refusals.
[The contract](CROSSTABS.md#scalar-heading-verification) records evidence limits.
No dependencies or write behavior changed. Native Windows comparisons were not run.

## Explicit CStr expression Error and Empty conversion

CStr now consumes standard expression Error data as English `Error <code>` Text,
and Empty as empty Text, in scalar and bound queries. Derived/named/correlated,
encounter-aggregate/domain and crosstab consumers retain explicit conversion.
Missing and unmapped host codes fail; raw Error/Empty storage admission and implicit
operators remain unchanged. The renderer reuses bounded numeric output, with exact
byte accounting, cancellation and no operational-error catch or locale inference.

Full all-feature expression/query suites pass 1,061 tests including doc tests.
Focused generated code, byte/work/value limits, every observed cancellation checkpoint,
metadata-only binding, composition and eager-failure tests pass. All-target/
all-feature Clippy, minimal expression/query builds and the CLI build pass.
The independent runner passes 1,211 Error/Empty/model/native controls on unchanged
registered inputs. The existing CStr runner also passes 3,224 source-free
numeric/date/Text/refusal controls. [Scope](CSTR.md#explicit-expression-error-and-empty-conversion)
retains opaque-code mapping, localization and native Access qualification gaps.
Full workspace and native Windows comparisons were not run. No dependency or
native write behavior changed.

## Text/numeric UNION result conversion

Mixed Text/numeric UNION outputs now select unconstrained Text from metadata and
reuse CStr at execution. Existing Text and NULL cells stay unchanged. NumberFormat
is execution-local; conversion precedes duplicate removal and downstream consumers.
Known Text output prevents intermediate all-numeric rounding in a flat UNION.
Field-policy routing, eager errors, cancellation and shared allocation limits remain.

The full all-feature query suite passes 572 tests including doc tests; updated
eight-test conversion and nineteen-test collation targets also pass. Focused
controls prove exact payload-byte charging, full-width integer rendering, no-scan
metadata, context reuse, empty/NULL arms, grouped/named/derived/pivot composition and
all observed cancellation checkpoints. Query Clippy, minimal builds and the CLI
build pass. The independent runner passes 216 controls on unchanged original
native inputs, alongside the prior 180 numeric and 109 UNION controls.
[The explicit contract](UNION_TYPES.md#text-with-numeric-outputs) retains native
precedence, regional/first-arm and broader non-numeric/Variant gaps. Full workspace
and native Windows comparisons were not run. No dependency or write behavior changed.

## Mixed numeric UNION result carriers

UNION/ALL now select a shared result carrier across Boolean, integral, Currency,
Decimal and floating families without source scans. Fixed-point widening retains
exact values, including LongLong/Currency through Decimal. Floating mixes select
Double through the existing conversion kernel; rounding occurs before duplicate
removal and downstream consumers. IIf and accumulator policies remain separate.

The full all-feature query suite passes 565 tests including doc tests. Six focused
numeric-UNION tests cover 81 pairs, three-arm consistency, exact endpoints,
rounding/duplicate collapse, empty/NULL arms, eager failures, generated Decimal/IEEE
models and every observed cancellation checkpoint. The independent runner passes
180 controls and the existing UNION runner retains all 109 controls on unchanged
registered inputs. Query Clippy, minimal builds and the CLI build pass.
[The explicit matrix](UNION_TYPES.md#provisional-mixed-numeric-profile) remains
provisional: native first-arm/precedence/rounding and Windows qualification are
unrun. Full workspace suites were not run. No dependency or write behavior changed.

## Contextual scalar concatenation

The `&` evaluator now formats numeric and date operands through existing explicit
CStr policies, alongside Text/Boolean and the established one/two-NULL behavior.
It retains written operand evaluation, cumulative conversion/join allocation
charges, cancellation and buffered query publication. Known output descriptors
use Text with two-operand NULL allowances; metadata binding never formats or scans.
Contextual named crosstab headings resolve through explicit execution discovery.

The independent runner passes 190 eval/run/model/native-source controls, including
unchanged sequential/derived inputs and both directions of native indexed ranges.
[Contract and limits](CONCATENATION.md) retain broader Variant/byte-array handling
and native Access qualification gaps. No dependency or native write behavior changed.
Verification: full all-feature expression/query suites pass 1,037 tests including
doc tests; fifteen focused native constraint/batch-edit regressions also pass.
All-target/all-feature expression/query Clippy, minimal-feature builds, the CLI
build, formatting and Python compilation pass. Earlier regression links failed
for disk space; after clearing superseded generated artifacts, the rerun passed.
Full workspace and native Windows comparisons were not run.

## Stable arithmetic result descriptors

Metadata-only binding now retains successful fixed result carriers for qualified
binary arithmetic, including LongLong, Double, fixed point and rounded remainders.
Operand value constraints are removed; NULL allowances are retained. Narrow
integer/Single/DATE widening stays Dynamic. Derived and empty results, aggregates,
crosstabs and the existing UNION widening service consume the descriptors without
record scans or a new arithmetic coercion policy. Eager failures still surface.

The full all-feature query suite passes 554 tests including doc tests, followed
by the updated six-test arithmetic metadata target. Its 847 operand/operator
combinations check runtime cells against claimed descriptors. All-target/all-feature
query Clippy, the minimal query build and CLI build pass. The independent UNION
runner passes 109 controls on unchanged originals, including arithmetic arms.
[Metadata scope](QUERY_RESULT_TYPES.md#stable-binary-arithmetic-metadata) retains
static non-Variant and native SQL qualification gaps. Full workspace and Windows
comparison suites were not run.

## Larger indexed-table evidence

Original 20,000-row Jet 4/ACE 2007/ACE 16-family inputs now pass 288 independent
cursor/query controls across seventeen indexes per file plus ACE 16 Large Number.
Complete projections, counts, second output fields, small typed seeks, nested full
counts and buffered refusals preserve exact ID order and unchanged originals.
Synthetic large trees additionally check both directions, hidden-key corruption,
zero-width retention and legacy API ceilings across four headers.
[Measured scope](LARGER_NATIVE_QUERIES.md#larger-indexed-evidence) retains physical
read quotas, wider scale/locality gaps and pending native Windows qualification.
The eight-test index-node suite, focused all-feature Clippy, formatting and Python
compilation pass; the CLI builds. Full workspace suites and Windows comparisons
were not run for this evidence batch.

## Explicit indexed snapshot retention

The Access adapter now passes configured scalar row ceilings into native indexed
reads, including projected reads. Complete traversals retain declared-count checks;
partial seeks cap matching rows and cannot publish complete caches. Legacy engine
index APIs retain their table ceiling. Required-index errors still fail without
sequential fallback. [Scope and limits](LARGER_NATIVE_QUERIES.md) distinguish these
controls from qualification of genuinely larger indexed tables and native Access
comparisons, which remain pending.

Verification: 35 focused all-feature engine/adapter tests pass, including indexed
request bounds, projections, GUID/Int64 selection, metadata-only binding and
partial-cache isolation. Formatting, diff whitespace and all-feature library
Clippy pass. Full workspace suites and Windows comparisons were not run.

## Explicit larger native scalar query reads

Fresh native scalar cursors can now materialize complete snapshots under checked
row/batch limits, while default APIs retain 16,384 rows. The Access source accepts
explicit configuration for full/projected sequential loads and checks scalar
retention limits before indexed or sequential cache/visitor publication. Advanced
or failed cursors cannot turn a remaining suffix into a complete snapshot. Binding
stays metadata-only; complete caches retain nested/reentrant reads.

`query run --native-rows` and `--native-steps` now complement native byte controls.
Separate `--execution-steps` configures shared query work up to 100,000,000, retaining
its 1,000,000 default. All native budget flags explicitly reject SQLite before I/O;
normal query byte/row/output ceilings, index/collection structural limits and write
qualification remain independent. Defaults and buffered publication are preserved.

The full all-feature query suite passes 549 tests including doc tests; 46 focused
native/adapter/CLI regressions also pass. Authored 20,000-row layouts cover priority
headers, batch sizes, one read per data page, invalid/advanced/failed cursors,
projections, late count/corruption failures, every observed native cancellation
checkpoint and cache/retention boundaries. Independent read-only ID models pass
44 aggregate/order/group/subquery/UNION/limit controls on unchanged registered files.
All-target/all-feature engine/adapter/query/CLI Clippy and minimal engine/adapter/query
builds and warnings-denied public API documentation pass. [Limits and evidence](LARGER_NATIVE_QUERIES.md) distinguish this scalar
profile from larger indexed/collection reads, streaming SQL and native Access
qualification. Full workspace/engine suites and Windows comparisons were not run.

## Progressive CLI scalar rows

`rows --stream` now continues through one source-bound native cursor and publishes
CSV-encoded TSV batches with one header. `--limit` becomes the batch size; optional
ordinary scalar columns support mixed schemas. Explicit native byte/row/work limits
remain cumulative, with bounded batches and a separate total output ceiling.
Normal page/query/inspection publication stays buffered. Streaming read/format
failures can leave previous batches; write/flush errors stop continuation and keep
the CLI's existing broken-pipe/other-output-error distinction.

Forty-eight focused CLI/publisher/browsing/help/query/native-budget tests pass,
including first/late failures, count mismatches, metadata/projection errors, escaped
transport, output caps, write/flush errors and cancellation between batches. All-target CLI
Clippy and formatting pass. The independent read-only cursor/getter runner passes
20 controls over four new original 20,000-row Jet 4/ACE 2007/ACE 16-family files,
including outputs over 1 MiB, several batch sizes, projections and cumulative quota
failure prefixes. All originals remain unchanged. [Progressive contract](STREAMING_ROWS.md)
and [provenance](FIXTURES.md#original-progressive-row-inputs) retain native Access,
live-file, template/collection streaming and larger SQL execution gaps. Full
workspace/engine suites and Windows native comparisons were not run for this batch.

## UNION exact result widening

UNION/ALL now bind common integer-width and Single/Double result carriers without
reading records, and widen values in place before duplicate removal and ordering.
Different Text length ceilings and Decimal declarations also receive shared result
descriptors without changing strings or Decimal coefficients/scales. NULL/empty
arms, parameter rebinding, named/derived consumers, conditional branch behavior,
expression-only states and eager failures retain their documented boundaries.

The full all-feature query suite passes 548 tests including doc tests; query Clippy,
the minimal query build and CLI build pass. The independent CLI runner passes 85
integer/set/IEEE/native-row controls on unchanged registered inputs across the four
priority header families. [Result-type rules](UNION_TYPES.md) keep this exact profile
separate from broader mixed-family/native Access coercion and precedence, which
remain open. No new library or write behavior is introduced. Full workspace/engine
suites and native Windows comparisons were not run for this batch.

## Unicode saved-query source labels

Saved SELECT/crosstab join forests no longer require ASCII-only source labels.
Unicode table names and aliases retain the same ASCII-insensitive, otherwise exact
identity policy used by binding. Raw endpoint delimiters, logical duplicate checks,
original fragments and guarded expression/source parsing remain unchanged. Native
Unicode normalization/case equivalence and NLS matching are still unqualified.

The saved-query all-feature suite passes 63 tests, including Unicode named/derived
forests, priority-header execution, generated multibyte labels, delimiter/injection
refusals, every observed construction/publication cancellation checkpoint and
cumulative limits. The independent runner passes 112 normalized SQL-rendering and
direct/saved/named execution controls on fresh Jet 4/ACE 2007/ACE 16-family inputs;
all hashes remain unchanged. The CLI builds, all-target/all-feature adapter Clippy
and the minimal adapter build pass. Full workspace/engine suites and native Access
comparisons were not run. [Identity scope](NATIVE_SAVED_JOINS.md#unicode-source-labels)
and [provenance](FIXTURES.md#unicode-saved-source-inputs) retain those limits.

## Unprojected collection HAVING and sort inputs

Collection members referenced only by HAVING aggregates or ORDER BY now drive
bounded child-row expansion. WHERE membership classification and repeated pruning
share the same scoped dependencies, including correlated sort captures. Same-child
sibling fields remain aligned; independent row-producing collections use the
existing nullable product, while WHERE-only collections retain parent membership.
Grouping errors, DISTINCT sort restrictions, late provider/expression failures,
metadata-only binding and quotas remain explicit.

The full all-feature query suite passes 541 tests including doc tests. Focused
controls include hidden aggregate counts, duplicate/NULL/empty child rows, joins,
TOP ties, crosstabs, generic named members, malformed sibling alignment, parameter
rebinding, every observed cancellation checkpoint and generated independent product
models. All-target/all-feature query Clippy and the minimal query build pass; the
CLI builds. Independent stored-row runners pass 153 multivalue and 177 ordinal-policy
attachment controls across ACE 2007 and ACE 16-family inputs, verifying unchanged
hashes. [Semantics](MULTIVALUE_QUERIES.md#unprojected-having-and-sort-inputs) and
[fixture provenance](FIXTURES.md#hidden-collection-having-and-sort-controls) distinguish
this child-row profile from a new parent-membership fold and native Access SQL
qualification. Full workspace/engine suites and native Windows comparisons were
not run for this batch.

## GUID query point pushdown

Wholly safe GUID equality and IS NULL predicates now use explicitly selected
single-GUID indexes, including scalar projections of mixed schemas. Canonical
literals and typed parameters retain all bits; the original WHERE remains in place.
Parameter rebinding, NULL parameters, eager failures, mandatory-index refusal and
partial/complete cache separation retain their existing contracts. GUID ordering,
coercive predicates and unsafe partial conjunctions receive no point hint.

The independent cursor/filter runner passes 212 CLI query controls plus the existing
76 native reader controls on unchanged registered V2016/V2019 fixtures. Focused query,
adapter and native GUID tests cover metadata-only binding, duplicate/NULL/missing
points, mixed-schema ordinals, hidden-key corruption, callback/cancellation failures
and generated arbitrary 128-bit values. Relevant all-feature lint/build checks pass;
full workspace/native engine suites and Windows native Access comparisons were not
run for this batch. [Scope](NATIVE_QUERIES.md#explicit-native-index-access) keeps
independent query evidence separate from native Access qualification.

## Native GUID point selections

The standalone engine now exposes exact GUID/NULL selection through a mandatory
single-GUID index. It prunes disjoint physical spans conservatively across duplicate
separators and fetches matching roots with normal key/row agreement. All retains
complete count validation; partial selections audit only returned/traversed paths.
Other carriers/composites fail without fallback. GUID byte order remains a physical
codec concern; no SQL ordering or query pushdown is introduced by this slice.

Twenty-four focused GUID index/composite/write-preview regressions pass, including
node splits, zero/NULL distinctions, selected corruption, I/O fault injection,
budgets and every observed cancellation checkpoint. The read-only engineering host
and independent cursor/filter runner pass 76 controls on unchanged registered
V2016/V2019 files. [API/evidence scope](NATIVE_INDEXES.md#exact-guid-point-selections)
retains native Access and broader platform qualification gaps. All-target/all-feature
engine Clippy and the standalone minimal engine build pass; the full workspace/ACE
test suites and native Windows comparisons were not run for this batch.

## Field-free DISTINCTROW projections

DISTINCTROW record keys now require a nonempty proper subset of source owners.
Constants, parameters, closed subqueries and owner-free counts/group outputs
follow the existing ignored path and preserve joined-row/input multiplicities.
This applies the documented some-but-not-all source requirement within the current
profile; it does not invent where active record deduplication belongs relative to
grouping. That native semantic/qualification gap remains explicit.

The independent reference-row runner passes 46 sequential/indexed controls on the
unchanged registered ACCDB. Focused controls cover TOP, DISTINCT differences,
parameters, empty joins, grouping/counts, eager failures and metadata-only binding
with unavailable source payloads. Original native profile-family fixtures also
cover constant and grouped-count paths. [The contract](QUERY_EXECUTION.md#distinctrow-source-records)
keeps independent results separate from native Access SQL qualification.

Verification passes 530 all-feature query tests, twenty-three focused DISTINCTROW
controls, native-adapter fixture tests, all-target/all-feature query Clippy, the
minimal query build and focused retrieval/adapter Clippy. The former generic
binding-refusal case now verifies metadata-only success and empty execution.
Formatting and whitespace checks pass. Full workspace tests and native Windows
comparisons were not run for this batch.

## DISTINCTROW subquery projections

Active DISTINCTROW now uses bounded scoped capture analysis for scalar/predicate
subquery projections, including nested frames, inner joins, UNION and shadowed
local aliases. A projection identity/purity gate was unnecessary for record-key
construction and is removed. Every qualifying joined row still evaluates its
projection before deduplication; clocks/random state, cardinality, late failures
and first-class Error data retain their existing boundaries.

The independent reference-row runner passes 36 sequential/indexed controls on the
unchanged registered ACCDB. Focused tests count seven clock/random evaluations
before three records survive, preserve Error data only through the expression API
and test every observed cancellation checkpoint. Original synthetic native reads
cover one/both-source captures across priority profile families. [The contract](QUERY_EXECUTION.md#distinctrow-source-records)
keeps native Access owner/optimizer/evaluation-order qualification separate.

Verification passes 527 all-feature query tests, twenty focused DISTINCTROW
controls, the new native-adapter regression, all-target/all-feature query Clippy,
the minimal query build and focused adapter Clippy. The CLI was rebuilt before the
independent controls. Formatting and whitespace checks pass. Full workspace tests
and native Windows comparisons were not run for this batch.

## DISTINCTROW over complete logical query records

Active DISTINCTROW now accepts derived/named source rows, including complete
UNION, grouped and fixed/discovered crosstab outputs. Hidden exposed columns remain
part of record equality; columns explicitly omitted inside the source query do
not reappear from underlying tables. Dynamic scalar arithmetic works; unsupported
expression-only states and late errors fail without NULL replacement. Metadata-only
binding, existing ignored cases and per-field text policy remain intact.

Verification passes 521 all-feature query tests, fourteen focused DISTINCTROW
controls including generated complete/partial-record semijoin models, inherited
policy controls, all-target/all-feature query Clippy and the minimal query build.
Original native fixture tests pass through Jet 4/ACE 2007/ACE 16-family reads.
The rebuilt CLI passes 26 independent reference-row controls on the unchanged
registered ACCDB input. Focused adapter Clippy, formatting and whitespace checks
pass. [The contract](QUERY_EXECUTION.md#distinctrow-source-records) keeps native
Access optimizer/physical-record/updateability qualification separate. Full
workspace tests and native Access comparisons were not run for this batch.

## DISTINCT projected-expression ordering

DISTINCT and qualified DISTINCTROW reads now map structurally matching stable
ORDER BY expressions to projected output slots, retaining text policy and avoiding
a second evaluation. Qualified fields, computed scalars and aggregate trees work;
volatile/opaque/read-plan identities and algebraic rewrites remain refused unless
using an existing explicit output alias. Shared identity comparisons retain the
separate grouping-key rules and query-frame shifts. [Ordering scope](QUERY_ORDERING.md)
keeps native optimizer/typing qualification and broader alias expressions open.

Verification passes 515 all-feature query tests, focused native fixture and
per-field collation controls, all-target/all-feature query Clippy, the minimal
query build and focused adapter Clippy. The rebuilt CLI passes 144 independent
set/order controls. Formatting and whitespace checks pass. Full workspace tests
and native Access comparisons were not run for this batch.

## Stored grouped-subquery comparisons

Four fresh original V2000/V2007/V2016/V2019 files now supply fourteen additional
inert grouped definitions. The [comparison runner](testing/compare_saved_grouped.py)
passes 328 raw-getter/rendering/direct/saved/named and diagnostic-category/no-output
refusal controls, including parameter scopes, named/derived reads and crosstab
HAVING. Original file hashes remain unchanged. Reviewed Jackcess getters provide
metadata/SQL evidence; a Python partition model supplies results. [Fixture records](FIXTURES.md#grouped-saved-query-inputs)
retain provenance and precise limits. No Rust production code or dependency
changed, so the full Rust suite was not rerun. Java compilation and the complete
new comparison batch pass; native Access SQL/save-reopen remains unrun.

## Key-safe grouped subqueries

Grouped outputs and HAVING predicates now admit scalar/EXISTS/IN/quantified reads
whose correlations use grouping keys. Scoped structural validation follows nested
frames, UNIONs, joins and lowered parent-membership predicates, while rejecting
ungrouped captures before scans. Independent derived/named scopes remain isolated;
outer aggregate operands keep their per-record semantics. No runtime caching,
short-circuiting or changed scalar/subquery evaluation is introduced.

[The contract and evidence](GROUPED_SUBQUERIES.md) cover computed keys, nested frames,
NULL/cardinality, crosstab HAVING, budgets/cancellation and original synthetic native
rows across the four priority header families. The independent partition-model
CLI runner passes 96 generated controls and a no-output refusal. Native Access
static/aggregate-ownership/optimizer qualification remains separate.

Verification passes 511 all-feature query tests, the native grouped-query fixture
regression, all-target/all-feature query Clippy, focused native-adapter Clippy and
the minimal query build. The CLI was rebuilt before the independent controls.
Formatting and whitespace checks pass. The full workspace suite and Windows
Access comparisons were not run for this batch.

## NULL-aware query result composition

OutputType now distinguishes NULL-only results from Dynamic expression states.
Named/derived projections and UNION retain this metadata; a NULL-only alternative
relaxes a known scalar descriptor instead of erasing it. IIf, Switch/Choose and
explicit Nz combine supported result descriptors while preserving existing eager
evaluation and runtime values. Implicit Empty replacement, Error data and mixed
carriers remain distinct. [The result contract](QUERY_RESULT_TYPES.md#null-aware-composition)
records the public enum addition and remaining Variant/native typing gaps.

Verification passes the full all-feature query suite, the subsequently extended
six-test NULL-composition target, all-target/all-feature query Clippy, the minimal
query build and all-feature workspace check. Formatting and whitespace checks
pass. Focused cases also prove metadata-only preparation and continued eager
failure in unselected operands. Full workspace tests and native Access type
comparisons were not run for this metadata batch.

## Scalar function result metadata

Metadata-only preparation now describes successful text, date, numeric, financial,
inspection and session function results using their existing evaluator carriers.
Carrier-preserving math removes operand constraints; DateAdd retains the declared
legacy/extended date distinction; CVar preserves its input descriptor. GUIDFromString
reports Binary structure bytes. Byte intermediates and StrConv remain Dynamic.
No function executes during binding and no runtime semantics or provider is added.
[Result metadata scope](QUERY_RESULT_TYPES.md#function-metadata-boundaries) retains
conditional/Variant/domain typing and native qualification gaps.

Verification passes the full all-feature query suite, the subsequently extended
seven-test function-metadata target, all-target/all-feature query Clippy, the minimal
query build, formatting and whitespace checks. Native Access comparisons and the
full workspace suite were not run for this metadata-only batch.

## Native Decimal mean read/query evidence

Original synthetic Jet/ACE NUMERIC rows now exercise widened Avg/DAvg totals
through the native adapter, checking NULL elimination, declared result metadata
and metadata-only preparation for profile bytes 1/2/5/6. An original independent
Fraction/CLI runner passes 123 exact Decimal controls including cancellation,
halfway rounding and generated mixed scales. [Evidence and limits](AVERAGES.md#native-adapter-and-independent-rational-controls)
keep these checks separate from native Access qualification. No new database
binary, dependency or production behavior is introduced. Six native adapter
regressions, focused adapter Clippy with warnings denied, formatting and whitespace
checks pass; the CLI was rebuilt before the independent controls. The full workspace
suite and Windows comparisons were not run for this evidence-only batch.

## Wider exact Decimal means

Avg/DAvg retain exact aligned totals when intermediate Decimal sums overflow the
96-bit carrier or would discard mixed-scale digits. A bounded i128/BigInt fallback
uses the already-selected num libraries; num-integer is now a direct inherited
dependency for quotient/remainder and parity. Ordinary representable totals keep
the rust_decimal quotient path. Wide final Decimal results use explicit half-even
quantization inside the existing carrier. Sum and stored field semantics are
unchanged; mixed conversion order and native Access qualification remain open.

The three focused regressions check ordinary/domain means, repeated positive and
negative maximum coefficients, scale-28 extremes, tiny cancellation residuals and
halfway ties. Local verification passes the full all-feature query suite,
all-target/all-feature Clippy, the minimal query build, formatting, the dependency
guard and 32 script tests. The focused target was rerun after adding DAvg checks.
No Windows Access comparison or full workspace suite was run for this batch.
[The numerical contract and release measurements](AVERAGES.md) retain every raw
sample and the observed small one-group cost; no general speedup is claimed.


## Exact binary floating means

Floating Avg/DAvg now retain finite input significands in bounded i128 exponent
bins and use num-bigint/num-rational for one rounded final quotient. This avoids
intermediate Double overflow and preserves tiny cancellation residuals, including
subnormals and ties. A query-scoped scratch pool reuses buffers between groups;
argument expressions are evaluated once. Allocation/work/cancellation limits
remain explicit. Existing Decimal/mixed-type and native-precision gaps remain
separate in [the numerical contract](testing/FLOATING_MEANS.md).

Original exact Fraction/CLI checks pass 129 binary and 69 Currency cases; synthetic
ACE 16-family native rows cover extreme means with metadata-only binding. The
release query benchmark records both the one-group improvement and a roughly
0.12 ms cost for 128 small groups. [Raw timings](testing/results/floating-means-2026-10-22.json)
retain exploratory samples and full Hyperfine measurements. These are independent
checks and local timings, not native Access qualification.

Local checks pass the 485-test all-feature query batch, the subsequently extended
eight-test floating-mean target, 66 focused Access regressions, 11 CLI regressions,
the minimal query build, all-target/all-feature query/Access Clippy, the dependency
guard and 32 script tests. Formatting and documentation-link/whitespace checks pass.

## Wider Currency and integral means

Avg/DAvg now use a dedicated bounded accumulator through existing conversion,
rust_decimal-backed arithmetic and promoted division services. Currency means
work, including cases whose intermediate total exceeds Currency; integral means
also avoid the narrower Int64 sum limit. Explicit Decimal retains Decimal
precedence, other successful numeric means select Double, and NULL/empty behavior
is preserved. Sum and stored schemas are unchanged. [The contract](AVERAGES.md)
retains floating-total/Decimal-envelope limits and native precision qualification
as unfinished work.

Focused regressions first reproduced both the unsupported Currency quotient and
intermediate overflow failures. The corrected path passes Currency/domain means,
extremes, mixed Decimal/float cases and ACE 16-family native Currency fixture reads.
The original Fraction/CLI runner passes 69 independent rational controls. No new
dependency, source-row buffer, active content or native Access evidence is added.

Verification passes the 476-test all-feature query batch and the subsequently
extended four-test mean target, four native ACE 16 adapter regression tests, 11
selected CLI tests, the minimal query build and all-target/all-feature query/Access
Clippy. Formatting and documentation-link/whitespace checks also pass.

## Numeric literal and accumulator metadata

Conservative output metadata now describes current decimal/exponent and signed
hexadecimal/octal literal carriers through the existing bounded literal parser.
Stable unary results, typed Sum/Avg accumulators, statistical aggregates/domain
calls and DCount also retain their qualified carriers. Domain count allows NULL
for unresolved criteria fields. Sum/mean descriptors drop input field constraints;
value-dependent widening and unsupported combinations remain Dynamic. Binding
does not scan or evaluate arbitrary expressions. Numeric parsing is shared with
literal validation rather than reimplemented.

Focused controls cover metadata-only preparation, actual accumulator descriptors,
empty/all-NULL inputs, missing domain criteria and fixed sum crosstab NULL cells.
[The contract](QUERY_RESULT_TYPES.md) keeps native carrier qualification and the
discovered Currency Avg execution gap explicit; no aggregate arithmetic changes
are included in this batch. Local checks pass 36 focused/regression query tests,
all-target/all-feature Clippy, the minimal query build, formatting and whitespace
checks.

## Conservative result carrier metadata

The query binder now retains declared scalar descriptors and supported constraints
through qualified field/parameter projections, aliases, joins and named/derived
sources. Prepared plans and buffered scalar/expression results expose aligned
OutputType metadata. Outer joins, subqueries and empty aggregates account for NULL;
known conversions/predicates, matching UNION/IIf branches and crosstab cells retain
their supported carrier profiles. Named schema discovery carries result metadata.
Arithmetic that can widen and other unimplemented inference stay explicitly Dynamic,
without changing evaluation or claiming native declared-expression compatibility.
[The contract](QUERY_RESULT_TYPES.md) records exact scope and remaining work.

New metadata-only controls panic on a source scan; runtime checks validate declared
output descriptors against returned values, including empty results. Native adapter
controls bind an invalid stored DATE without decoding records and retain metadata
when reading another valid field. ACE 16-family Int32/Int64 index controls retain
typed metadata without extra data/index reads. These are independent checks;
native Access SQL result-type qualification remains pending.

Verification passes the 469-test all-feature query batch (including its API
doctest), the subsequently extended seven-test output-type target, 66 focused
native adapter regressions, all-target/all-feature query/Access Clippy and the
minimal query build. Eleven selected CLI regressions and 166 independent
template controls pass against the rebuilt CLI. This work adds no dependencies,
runtime or writes.

## Empty template directory records

ACCDT archives now accept bounded empty ZIP directory records without exposing
them as OPC parts or requiring content types. Header/CRC checks, physical-name
uniqueness, cancellation and resource charges still apply. Unsafe or nonempty
directories and symlinks (including empty slash-suffixed symlinks) fail explicitly.
Existing logical-part prefix and alias rules apply only to actual parts.

The focused batch passes all 34 adapter unit tests and all-target/all-feature
Clippy. A fresh CLI build passes 62 original Python package controls, 68 typed
template/table-query controls and 36 List Definition controls. Existing input
bytes remain unchanged. These are independent checks, not native Access evidence.
See [package limits](ACCESS_TEMPLATES.md).

## Escaped OPC part names and IRI references

ACCDT packages now map escaped UTF-8 ZIP names to logical Unicode part names,
retain opaque ASCII reserved escapes and resolve internal URI/IRI references
through iri-string 0.7.14. Invalid escapes/UTF-8, encoded separators, root escapes,
logical aliases and part-prefix collisions fail before package publication.
Relationship controls use the immediate parent segment; arbitrary nested `_rels`
paths are not mistaken for controls. The name and resource ceilings remain
explicit; estimated per-entry metadata increases to 8 KiB for mapped/folded names.
[Package scope](ACCESS_TEMPLATES.md) distinguishes raw UTF-8 ZIP compatibility,
Unicode normalization recommendations and unsupported directory/interleaved
features. Native Access qualification remains pending.

Local verification passes 171 all-feature adapter tests, all-target/all-feature
Clippy, the minimal adapter build, a fresh CLI build, 32 dependency-policy script
tests and the dependency guard. Independent temporary Python ZIP/XML controls
pass 50 package, 64 typed-table/query and 36 List Definition cases; 30 selected
CLI integration regressions also pass. Input bytes
remain unchanged; no third-party code/fixture or active content is executed.

## Consistent OPC ASCII part identity

ACCDT overrides, relationship control paths, internal targets and public part
lookup now share ASCII-case-insensitive identity with archive duplicate checks.
Original content-part spelling remains visible; relationship type URIs and
non-ASCII letters are not case folded. New controls cover mixed-case stored and
deflated packages, typed table reads, equivalent override refusal and bounded
allocation-free lookup. Escaped URI/IRI mapping remains unfinished, as does native
Access template qualification. See [package scope](ACCESS_TEMPLATES.md).

Verification passes all 26 adapter unit tests, all-target/all-feature adapter
Clippy with warnings denied, a fresh CLI build and 34 independent Python ZIP/OPC
controls, 48 existing typed-template controls and 36 List Definition controls.
Native Microsoft Access comparisons remain unrun.

## Stored List Definition declaration agreement

Qualified ACCDT List Definition tables now feed existing columns, snapshots,
browsing/counts and read-only queries. A bounded inert SOAP/CAML interpreter applies
ordered field declarations, base updates, display renames, property patches and
deletions, then checks final identities/types/constraints against inline XSD.
UUID identities reuse the existing uuid dependency. Binding still stops before
record events; defaults, validations, services and application code are not run.
[The contract](ACCESS_TEMPLATE_LISTS.md) retains missing/extra/default-dependent
schemas, broader types/property semantics and native Access qualification as
unfinished work. Hidden fields are not silently omitted.

The batch passes 162 all-feature adapter tests, including six new feature/generated
controls, and 36 selected CLI regressions. Original Python SOAP/XML/ZIP fixtures
pass 36 new CLI/query controls and replay 48 typed-template plus 30 package controls
with unchanged inputs. All-target all-feature Clippy, minimal Access checking,
warnings-denied rustdoc, formatting, dependency boundaries and 32 policy tests
pass. No registry version/closure change, native writer or runtime dependency is
introduced. Native Access remains unrun. Next is broader OPC part-name/URI handling
and remaining template/native read-query acceptance work.

## Typed stored template tables and CLI integration

The Access backend now reads object metadata, bounded inline XSD and complete typed
table-data snapshots for generic Object tables. Metadata-only schema discovery
stops before data-instance events; the shared query source supplies explicit scans
without evaluating application bodies. CLI catalog/column inspection, tables/counts,
rows/pages/all-table sections and query run use these services. Jiff, rust_decimal
and base64 are inherited from existing workspace dependencies.
[The contract](ACCESS_TEMPLATE_DATA.md) keeps SharePoint List Definition/schema
agreement, wider XSD/native field fidelity, named template queries, signatures and
Access-created qualification unfinished. Required agreement is enforced by refusal;
missing data never becomes an invented empty table.

The batch passes 154 all-feature adapter tests, followed by 16 final template unit
tests including content-selection and interrupted-schema reuse regressions. The selected CLI batch
passes 36 controls. Original Python XML/ZIP fixtures pass 48 typed-template CLI/query
controls and replay all 30 package controls with unchanged inputs. All-target
all-feature Clippy, minimal Access checking, warnings-denied rustdoc, formatting,
dependency boundaries and 32 policy tests pass. A failed link caused by full disk
was rerun after removing obsolete generated executable variants; source/fixtures
were preserved. Native Access remains unrun. Next is SharePoint List Definition
construction/schema agreement, then remaining template and native read/query
acceptance work.

## Bounded ACCDT package inspection

The Access backend now captures bounded stored/deflated OPC templates, validates
ZIP geometry/CRC/name uniqueness and resolves content types/internal relationships.
Basic CLI inspect reports inert parts and metadata; signatures stay unverified.
[The contract](ACCESS_TEMPLATES.md) keeps template schema/data reading, table/query
integration, wider OPC layouts and native Access qualification unfinished. No
active content, extraction or database instantiation occurs.

The batch passes all 146 Access adapter tests, including six focused template
tests and generated mutation controls, plus 24 existing CLI browsing/inspection/
distribution/CAB regressions. The original Python ZIP/OPC runner passes 30 CLI
controls with unchanged inputs. All-target all-feature Clippy, minimal Access
checking, warnings-denied rustdoc, formatting, dependency boundaries and its
32 policy tests pass. Two stale adapter assertions from prior predicate/arithmetic
changes now follow the documented operation-limit and Int32 result contracts.
The dependency record covers zip and its three-package locked addition. Next is
ACCDT metadata/schema/data interpretation and table/query integration, alongside
remaining native read/query and backend acceptance work.

## Contextual legacy DATE/Text predicates

Legacy DATE/Text comparisons now reuse CDate under explicit date syntax, year
controls and optional numeric serial fallback. Scalar and bound filters, joins,
membership/quantified sources, domain criteria and closed Eval share the service.
Extended DateTime/Text remains refused to avoid implicit precision loss. Strict
set/group keys, stored carriers and physical writes are unchanged; the
[contract](NUMERIC_PREDICATES.md#legacy-date-and-text) keeps full declared/Variant
coercion and native qualification open.

The batch passes 936 all-feature expression/query tests including doctests and
21 selected CLI controls. The original runner passes 310 independent Gregorian/
serial/native-field controls over four unchanged priority-family files. All-target
all-feature Clippy, separate minimal expression Clippy, warnings-denied rustdoc,
formatting and dependency boundaries pass. Native Access remains unrun. Next is
bounded ACCDT package/schema/data reading, then remaining backend acceptance work.

## Contextual numeric predicate comparisons

Mixed numeric/Text predicates now share the explicit number policy, with target-
specific Currency/Decimal conversion and checked fixed/floating comparison. Binary
comparisons, BETWEEN, membership/quantified sources, joins, filters, HAVING, domain
criteria and closed Eval use the same expression service. Text/Text retains field
collation routing; strict set/group/sort keys retain their separate contract. A
source-spy regression proves that partial hints cannot hide a later bad value.
[The contract](NUMERIC_PREDICATES.md) keeps materialized versus declared/Variant
semantics, temporal/Text coercion and native SQL qualification open. No dependency,
stored carrier, physical writer or schema-admission changes.

The batch passes 470 all-feature expression tests and 460 query tests including
doctests, plus 21 selected CLI/help/output controls. Conversion quota/cancellation
checks cover every observed checkpoint; the query boundary preserves operation-level
interrupt/limit identity. The original runner passes 1,060 independent Decimal/
rounding/predicate/native-field controls over four unchanged priority-family files.
All-target all-feature Clippy, separate minimal expression checking/Clippy,
warnings-denied rustdoc and dependency-boundary checks pass. Native Access typing,
coercion, optimizer and error order remain unqualified. Next is date/Text predicate
conversion through existing explicit date policies, then remaining read/query and
backend acceptance work.

## Bounded hexadecimal and octal query literals

Existing &H/&O syntax now evaluates through a feature-local numeric literal
service. Checked unsigned magnitudes select signed Integer/Long carriers without
floating conversion or locale providers. Prefix adjacency and unquoted digits
are validated at the syntax boundary; host-built Number spellings retain complete
runtime validation. Fixed crosstab headings use the same policy-independent values
without scanning. [The contract](RADIX_LITERALS.md) keeps 32-bit/128-byte ceilings,
suffixes, decimal inference and native SQL typing qualification explicit. No
package dependency, core carrier, serialization or native writer changes.

The batch passes 922 all-feature expression/query tests including doctests,
59 parser tests and one CLI control, followed by six final literal/query checks
covering the exact size boundary and equivalent heading collisions. Generated
signed-value models and the original runner pass 742 independent type/native-field
controls over unchanged priority-family inputs. All-target all-feature Clippy,
standalone minimal expression checking/Clippy, warnings-denied rustdoc and dependency
boundaries pass. Native Access literal typing, static overflow, optimizer behavior
and save/reopen remain unqualified. Next is numeric/Text comparison behavior for
local-table filters and the remaining read/query/backend acceptance work.

## Unary and division-family promotion

Materialized unary minus now preserves qualified widths, widens Integer/Long minima
and checks LongLong/Currency minima. Division retains qualified Single results and
supplies checked Currency/DATE ratios. Power now accepts qualified Currency/DATE
operands through Double. Integer division/Mod select operand/result widths from
original carriers before contextual Text conversion, with direct lexical half-even
rounding and range checks. [The contract](ARITHMETIC_PROMOTION.md#unary-minus-and-division-family-selection)
retains dividend-signed remainders, current zero/zero errors, literal typing,
declared-expression inference and native qualification gaps. Direct checked kernels
and aggregate contracts remain separate. No dependency, stored carrier or writer
changes.

The batch passes 464 all-feature expression tests and 453 query tests including
doctests, 322 separately run minimal expression tests and three CLI controls.
The original division runner passes 1,040 independent Fraction/rounding/type/native
field controls over four unchanged priority-family inputs; the existing arithmetic
Text recipe also passes 672 regression controls. All-feature/minimal Clippy,
warnings-denied rustdoc, formatting and dependency-boundary checks pass. Native
Access expression typing, rounding and optimizer/error equivalence remain unrun.
Next is literal/result typing and the remaining scalar/read-query acceptance work;
the full backend-before-UI roadmap remains open.

## Materialized arithmetic result promotion

SQL addition/subtraction/multiplication now use a separate materialized-value
promotion service. Qualified Byte/Integer/Long/Single results retain their carriers
and widen on overflow; LongLong overflow remains checked. Currency multiplication
uses the existing Decimal kernel and four-place rounding. Currency/floating and
Decimal/DATE selection now have explicit working routes; DATE multiplication
returns a raw serial Double. Direct checked numeric kernels and aggregate result
contracts remain intact. [The contract](ARITHMETIC_PROMOTION.md) keeps declared
expression typing, other operator-specific promotion, literal inference and native
Access qualification open. No dependency, stored carrier or writer changes.

The batch passes 462 all-feature expression tests and 451 query tests including
doctests, 320 separately run minimal expression tests and two CLI controls. The
new original range/type/Fraction runner passes 818 controls over unchanged original
Jet 4, ACE 2007 and ACE 16-family files. The DATE runner passes 62 updated serial
controls. Native getters never execute SQL. All-feature/minimal Clippy,
warnings-denied rustdoc and dependency-boundary checks pass. Native Access result
types, optimizer/rounding/error order remain unqualified. Next is unary and other
operator-specific promotion, then the remaining read/query and backend roadmap.

## Contextual numeric Text arithmetic

Mixed Text/numeric addition, other numeric arithmetic and unary signs now share
execution-local numeric conversion. Text + Text remains concatenation. Decimal
and Currency routes preserve their checked targets; integer division/Mod convert
lexical Text directly to Int64 without a bounded Decimal or Double intermediate.
Legacy DATE operands retain raw serial rules. [The contract](ARITHMETIC_TEXT.md)
keeps the existing numeric result profile, full Variant promotion and native
qualification gaps explicit. Metadata-only crosstab binding classifies mixed
Text arithmetic as contextual and leaves it to explicit execution staging. No
package dependency, core carrier or native read/write behavior changes.

The batch passes 906 all-feature expression/query tests including doctests, followed
by final four-test scalar, 29-test query and one-test CLI rechecks after the lexical
midpoint correction and heading regression. Generated integer models, direct
Decimal/Currency/DATE goldens, policies, domains, eager errors, limits and every
observed conversion cancellation checkpoint are covered. All-feature Clippy,
standalone minimal expression checking/Clippy and warnings-denied rustdoc pass.
The original runner passes 672 independent Decimal/half-even/native-field controls
over four unchanged priority-family sources, with existing CSV/TSV output policy
preserved. Native Access SQL qualification remains pending. Next is remaining
Variant result promotion/operator and read/query acceptance work.

## Runtime-only domain schema discovery

The explicit execution service now stages crosstabs first requested by runtime
domain functions, including targets behind named SELECT wrappers. Parameterless
admission is checked during binding; declared domain parameters remain refused
before the requested discovery scan. Runtime and relational requests share one
completed-result cache, active dependency path, providers and cumulative budget.
Nested execution can extend that cache without invalidating existing plan indices.
[The contract](DOMAIN_FUNCTIONS.md#runtime-discovery-controls) retains the separate
metadata-only preparation and legacy PreparedQuery execution profiles, inline
TRANSFORM limits and native volatile/optimizer qualification gaps. No dependency,
scalar storage carrier or writer changes.

The full query suite passes 445 tests including doctests; selected CLI tests pass
35 controls. Five runtime-domain tests and a field-policy routing test cover nested
requests, first-use/repeated calls, policies, cycles/parameters, every observed
cancellation checkpoint, cumulative quotas and late failures/changed definitions.
All-target query/CLI Clippy, minimal query checking and warnings-denied rustdoc pass.
The original engineering runner passes 148 independent grid/getter/inert/refusal
controls across four unchanged priority-family files. Only TableCraft executes SQL;
native Access execution remains unrun. Next is broader query value/operator and
Variant acceptance work; the complete backend-before-UI roadmap remains open.

## Domain reuse of discovered crosstab results

Runtime domain binding now receives the operation's completed relational crosstabs.
Domain aggregates/lookups retain their schemas, policies and expression-valued cells
without rereading underlying tables or rerunning volatile cells. Existing transitive
parameter refusals, criteria behavior and shared budgets remain intact. Runtime-only
targets still cannot initiate discovery. [The scope](DOMAIN_FUNCTIONS.md) retains
that remaining integration work and native qualification; no dependency or write
behavior changes.

Two new tests cover aggregation, missing criteria, volatile/Error identity,
parameter refusals and runtime-only boundaries. Selected named/domain/text-routing
tests and query/CLI Clippy pass. The extended independent native runner passes
112 controls over the same four unchanged sources; only TableCraft executes SQL.
Next is discovery initiated by runtime domain targets, followed by remaining query/
Variant/read acceptance work. The complete backend roadmap remains open.

## Explicit named-crosstab schema discovery

Explicit query execution now stages named crosstabs whose headings depend on data,
parameters or execution policies. Completed per-run results retain expression Error
cells and text-policy provenance; repeated references reuse them without rerunning
volatile cells. Binding remains metadata-only. Shared budgets and cancellation
cover discovery, rebinding and outer execution. [The contract](NAMED_SCHEMA_DISCOVERY.md)
keeps runtime domain-target discovery, inline TRANSFORM, broader headings and native
Access equivalence open. The CLI composes the library service; no dependency or
storage/write behavior changes.

The batch passes 437 all-feature query tests including doctests, 35 selected CLI
tests and a final 18-test named-crosstab recheck. All-target query/CLI Clippy,
minimal query checking and warnings-denied query rustdoc pass. Original engineering
recipes pass 100 new discovery controls and 204 computed-heading regression controls
across unchanged Jet 4, ACE 2007 and ACE 16-family files. These compare explicit grids
and independent getters, not native Access SQL. Next is integrating discovered
schemas with runtime domain targets under the same operation budget; the full
backend-before-UI roadmap remains open.

## Computed fixed schemas for named crosstabs

Named crosstabs now derive fixed Text/integral headings from qualified constant
operators, numeric conversions/CVar and nonvolatile numeric math, reusing existing
scalar kernels with no source or provider context. The original expressions still
execute and result names are verified; metadata computation does not prune inner
cells or suppress later failures. [The contract](NAMED_CROSSTABS.md#policy-independent-computed-fixed-headings)
keeps dynamic/parameter/policy-dependent schemas, further computed calls and native
qualification open. No dependency, native mutation or new runtime host is added.

The full query suite passes 429 tests including doctests, with a final eleven-test
named-crosstab recheck after adding the empty-table assertion. Three internal
boundary tests cover parent resource budgets, no-source guards, structural limits
and every observed cancellation checkpoint. The original runner passes 204 independent
native-grid/inert/refusal controls over four unchanged priority-family files.
All-target query/CLI Clippy, minimal query checking, warnings-denied rustdoc and
formatting/diff checks pass. Native Access execution remains unrun. Next is the
explicit execution-phase discovery boundary for context/data-dependent named schemas;
binding must not scan records to invent them. The wider backend roadmap remains open.

## Read/query acceptance review

The roadmap's local-file status now separates working encryption opening/management
from remaining codec/native qualification, and distinguishes current General text
providers from ordinal/plain LIKE modes. Current kernel/provider/CLI code and
named-crosstab no-scan refusal tests were inspected; no new compatibility result
or capability is claimed by this documentation review. [The acceptance table](ROADMAP.md#current-acceptance-status)
keeps Variant, named-query composition, advanced fields, ACCDT/signatures, scale,
coherent reads and native comparisons visible. The full backend-before-UI milestones
and production write gates remain unchanged. No runtime tests were repeated for
this documentation-only review; diff checks pass.

## Contextual DateSerial/TimeSerial components

Date/time constructors now reuse checked Integer conversion for raw DATE and
explicit-policy SQL numeric Text components. Components validate progressively
before the existing relative calendar calculation; Int16 limits, NULL errors,
eager operands and execution-local year windows remain intact. [The contract](DATE_CONSTRUCTORS.md#argument-coercion)
retains full Variant/native/calendar qualification gaps. No date parser, dependency
or storage/write behavior is added.

The batch passes 454 all-feature expression tests including doctests, 316 separate
minimal-profile tests, eleven selected query tests, three selected CLI tests and
388 independent Decimal/Gregorian/Fraction/native-field controls over unchanged
priority-family sources. All-feature/minimal Clippy, warnings-denied rustdoc and
formatting/diff checks pass. Native Access SQL qualification remains pending.
Next is the read/query acceptance review and remaining named-query composition,
Variant and native-text gaps, rather than treating Partial built-ins as completion.

## Bounded DateAdd/DatePart/DateDiff sources

Date interval functions now prepare their owning controls before sharing bounded
CDate conversion for numeric/Text sources. DateAdd also accepts checked raw DATE
and explicit-policy numeric Text counts. Existing date/Extended/NULL sources stay
borrowed; DateDiff validates both sources before NULL propagation. [The contract](DATE_INTERVALS.md)
retains the current calendar algorithms, validation order and full Variant/native
qualification gaps. No dependency, native storage change or write expansion is added.

The batch passes 868 expression/query tests including doctests, 315 separately run
minimal-profile tests, three selected CLI tests and 13,662 independent Gregorian/
Decimal/native-field controls over unchanged priority-family sources. The broader
query suite exposed an obsolete Year(1) refusal assertion from the component-source
change; it now checks the supported 1899 result while preserving eager/error/shape
coverage. All-feature/minimal Clippy, warnings-denied rustdoc and formatting/diff
checks pass. Native Access SQL qualification remains pending. Next is date-constructor
argument coercion and the remaining read/query acceptance work.

## Bounded date component source conversion

Year/Month/Day/Hour/Minute/Second now reuse the evaluator's bounded CDate service
for numeric/Text sources. Existing date/Extended/NULL sources retain their carrier
and precision rules without narrowing or policy discovery. [The scope](EXPRESSIONS.md#date-component-source-coercion)
keeps full Variant/native/calendar semantics separate and leaves DateAdd/DatePart/
DateDiff source coercion as next work. No dependency, parser or storage change is added.

The batch passes 443 all-feature expression tests including doctests, six selected
query tests, three CLI tests and 1,406 independent Gregorian/native-field controls
over unchanged priority-family inputs. Standalone minimal and all-feature Clippy,
warnings-denied rustdoc and formatting/diff checks pass. The minimal runtime source
profile is unchanged; its tests were not repeated for these SQL-only evaluator
edits. Native Access SQL qualification remains pending.

## Named numeric formatting of legacy DATE sources

FormatNumber/Percent/Currency now format raw legacy DATE serials through the
existing Double conversion and bounded magnitude renderer. Negative epoch aliases
retain their numeric sign; no civil rounding or date-output profile is inferred.
Extended numeric dates remain refused. [The scope](STYLED_NUMBER_FORMATTING.md#legacy-date-source-values)
retains existing display precision, rust_decimal rounding, options and native
Variant/format qualification gaps. No dependency or native read/write change is added.

The batch passes 439 all-feature expression tests including doctests, 314 separate
minimal-profile tests, seven selected query tests, three CLI tests and 1,636
independent Decimal/calendar/native-field controls over unchanged priority-family
sources. All-feature/minimal Clippy and formatting/diff checks pass. Native Access
SQL qualification remains pending. Next is broader date-function source coercion
through the existing bounded date conversion service.

## Contextual week options across read/query functions

DatePart/DateDiff/Weekday and general Format now share checked raw DATE and
explicit-policy SQL numeric Text week options. The owning parser validates each
option progressively; interval, NULL/default/system rules and eager written
operands retain their function-specific order. Original source values stay
borrowed. [The contract](WEEK_SETTINGS.md#contextual-week-option-conversion)
keeps WeekdayName, full Variant/enum/native behavior and the existing DateDiff w
anchor gap separate. No calendar algorithm, write path or dependency is added.

The batch passes 849 expression/query tests including doctests, 312 separately
run minimal-profile tests, four selected CLI tests and 928 independent Decimal/
enumerated-week/native-field controls over unchanged priority-family sources.
All-feature/minimal Clippy, warnings-denied rustdoc and formatting/diff checks
pass. Native Access SQL qualification remains pending. Next is named numeric
formatter DATE source coercion, then remaining source/Variant acceptance work.

## Contextual numeric/date formatting options

FormatNumber/Percent/Currency options and FormatDateTime named selectors now
accept explicit-policy SQL numeric Text through checked Long conversion. Their
owning parsers preserve progressive validation and function-specific NULL/profile
precedence without cloning the formatted value. Display precision/rounding,
regional defaults, supported styles and date policies are unchanged. [The contract](STYLED_NUMBER_FORMATTING.md#contextual-numeric-options)
retains full Variant/NLS/native gaps and adds no dependency.

The batch passes 840 expression/query tests including doctests, 309 separately
run minimal-profile tests, three selected CLI tests and 420 independent
Decimal/Gregorian/options/native-field controls. Original native hashes remain
unchanged. All-feature/minimal Clippy, warnings-denied rustdoc and formatting/diff
checks pass; native Access SQL qualification remains pending. Next is remaining
week-option and scalar source-coercion acceptance work.

## Contextual StrConv flags and LCIDs

StrConv now reuses the checked numeric service for Integer flags and Long LCIDs,
including raw DATE serials and explicit-policy SQL numeric Text. Required flag/
locale/source validation order, supported locale identities and raw encoding
boundaries remain intact. [The contract](STRCONV.md#contextual-flag-and-lcid-conversion)
records the primary declaration and full native enum/Variant/NLS gaps; no new
locale/flag provider, runtime or dependency is added.

The batch passes 833 expression/query tests including doctests, 309 separately
run minimal-profile tests, three selected CLI tests and the tightened eight-byte
resource control. The original runner passes 938 independent Decimal/representation/
locale/native-field controls over unchanged priority-family inputs. All-feature/
minimal Clippy, warnings-denied rustdoc and formatting/diff checks pass. Native
Access SQL/NLS qualification remains pending. Next is remaining formatting-option
and scalar source-coercion acceptance work.

## Contextual comparison modes and character/byte codes

Comparison modes and Chr/ChrW codes now accept raw DATE and explicit-policy SQL
numeric Text. ChrB, raw byte slicing and InStrB numeric slots share the checked
conversion without weakening raw-byte publication. Function-specific mode/NULL/
start/count validation order, closed-versus-LCID dispatch, page and surrogate
checks remain intact. String/Asc source argument identity and StrConv's separate
profile are preserved. [The scope](STRING_COMPARISON.md#contextual-comparison-mode-conversion)
keeps native provider/Variant gaps visible and adds no dependency.

The batch passes 828 expression/query tests including doctests, 309 separately
run minimal-profile tests, three selected CLI tests and 840 independent
Decimal/character/codec/mode/native-field controls. Priority-family sources retain
their hashes. All-feature/minimal Clippy, warnings-denied rustdoc and format/diff
checks pass. Native Access SQL/NLS qualification remains pending. Next is StrConv
flag/LCID coercion and remaining scalar option/source semantics.

## Contextual Replace start/count conversion

Replacement start/count slots now accept raw legacy DATE and explicit-policy SQL
numeric Text through existing checked Long conversion. Original source/match/
replacement strings remain borrowed; normalization, scans and output share limits.
Required validation order, NULL failures, defaults, suffix semantics, surrogate
checks and eager written operands remain explicit. [The contract](TEXT_REPLACEMENT.md#contextual-start-and-count-conversion)
retains Partial status and native subtype/error/locale gaps. No dependency is added.

The batch passes 819 expression/query tests including doctests, 307 separately
run minimal-profile tests, three selected CLI tests and 798 independent
Decimal/UTF-16/replacement/native-field controls. Original priority-family source
hashes remain unchanged. All-feature/minimal Clippy, warnings-denied rustdoc and
formatting/diff checks pass; native Access SQL qualification remains pending.
Next is comparison/character argument coercion and remaining read/query scalar gaps.

## Contextual repetition counts and search starts

Space/String counts and numeric character inputs now use checked legacy DATE
Long conversion. SQL Text counts and InStr/InStrRev starts use the explicit
number policy; String's Text character argument remains literal. Prepared
borrowed search operands retain comparison-mode validation before start
conversion, and repetition retains its different NULL shortcut. Normalization,
scans and streamed output share budgets without source cloning. [The scope](TEXT_CONSTRUCTION.md#contextual-repetition-counts)
keeps source/mode/Variant/native gaps explicit and adds no dependency.

The batch passes 812 expression/query tests including doctests, 306 separately
run minimal-profile tests, four selected CLI tests and 1,200 independent
repetition/UTF-16/native-field controls. Original native hashes are unchanged.
All-feature/minimal Clippy, warnings-denied rustdoc and formatting/diff checks
pass. Native Access SQL qualification remains pending. Next is replacement
start/count coercion and remaining character/comparison argument slots.

## Rounded Left/Right/Mid counts

Slicing positions and lengths now reuse checked half-even Long conversion for
numeric/legacy DATE carriers. SQL numeric Text counts use the selected number
format and shared normalization/output budgets. A feature-local slice kernel
borrows the original source; the evaluator converts count slots without cloning
it. Current NULL/domain precedence, eager written operands and surrogate-boundary
refusals remain explicit. [The contract](SLICE_COUNTS.md) retains Partial status,
source/Variant gaps and pending native Access qualification.

The batch passes 803 expression/query tests including doctests, 304 separately
run minimal-profile tests, three selected CLI tests and 1,122 independent
Decimal/UTF-16/native-field controls. The four priority native source families
retain their hashes. All-feature/minimal Clippy, warnings-denied rustdoc,
formatting and diff checks pass. No dependency or native write profile is added.
Next is repetition/search argument coercion and remaining read/query scalar gaps.

## Rounded logical operands and contextual Text predicates

Logical operators now reuse checked half-even conversion for Single/Double,
Currency/Decimal and raw legacy DATE inputs. Unary Not uses Long; binary inputs
use Long or original-Int64-selected LongLong. Numeric Text shares the explicit
number format, exact integer parser and cumulative budgets. Read-only final Text
predicates use contextual CBool instead of integer rounding, consistently through
joins, HAVING/crosstabs and collection membership. Pure helpers retain their
policy-free Text refusal. No dependency, stored carrier or write-format gate is
added. [The contract](INTEGER_LOGIC.md#rounded-numeric-and-text-operands) keeps
literal/Variant subtype and native Access qualification gaps explicit.

The batch passes 796 expression/query tests including doctests, 302 separately
run minimal-profile expression tests and five selected CLI tests. The new runner
passes 2,658 independent Decimal/per-bit/calendar/native-field controls; the
1,394-control integer baseline also passes. Jet 4/ACE 2007/ACE 16-family source
hashes remain unchanged. Minimal/all-feature Clippy, warnings-denied rustdoc,
formatting and diff checks pass. No native Access SQL comparison was run.
Next is string-function count coercion and remaining read/query scalar gaps.

## Exact integer logic and numeric read predicates

Not/And/Or/Xor/Eqv/Imp now preserve exact integer widths and sign extension,
with explicit mixed Boolean/integer and integral NULL policies. Read-only
WHERE/HAVING/ON uses numeric/legacy DATE zero/nonzero only at the final predicate
boundary. Collection criteria retain scalar bits and execute item masks before
membership folding; independent Boolean memberships remain separate.
[The contract](INTEGER_LOGIC.md) records unsupported rounded operator coercions
and pending native Access qualification. No stored value type, dependency or
native write profile is added. UPDATE's final predicates remain Boolean/NULL.

The expression/query batch passes 785 tests including doctests, with 299
separately run minimal-profile tests, three selected CLI tests and 68 final focused
edit/collection controls. The original runner passes 1,394 independent controls,
including unchanged Jet 4/ACE 2007/ACE 16-family field queries. Minimal/all-feature
Clippy, warnings-denied rustdoc, formatting and diff checks pass. No Microsoft
Access comparison was run. Next is rounded float/fixed/date/Text logical operator
coercion and the remaining Variant/predicate acceptance work.

## Contextual conditional coercion

IIf/Switch/Choose now accept legacy DATE selector inputs and execution-local
numeric Text policies, while retaining eager operands and selected Error/Text
payloads. Typed Empty remains distinct from stored NULL; Error/Missing selectors
fail. Operational conversion failures stop immediately and remain latched.
[The contract](CONDITIONAL_FUNCTIONS.md#date-text-and-explicit-expression-states)
records 776 expression/query tests, 296 separately run minimal-profile tests,
seven selected CLI tests and 1,072 independent controls over unchanged
Jet 4/ACE 2007/ACE 16-family inputs. No native Access compatibility or broader
logical/bitwise/Variant completion is claimed.

## Legacy DATE numeric conversion

All nine explicit numeric/Boolean conversion functions now reuse the stored
legacy DATE serial and their existing destination kernels. Format numeric date
masks share that Double conversion; typed numerical consumers retain their domain
checks. Date identity, IsNumeric/IsDate, NULL failure and Extended-date refusal
remain explicit. [The contract](DATE_NUMERIC.md) records current carriers and
limits separately from native Access qualification. The expression/query batch
passes 766 tests including doctests, with 293 separately run minimal-profile
tests, ten selected CLI tests and 848 independent calendar/Decimal/IEEE/native-field
controls. Original Jet 4/ACE 2007/ACE 16-family source hashes remain unchanged.

## Interleaved numeric Format layouts

Numeric Format now retains literals among whole, fractional and exponent slots,
including percentage placement, extra leading digits and optional positions after
required whole slots. Borrowed masks and scalar layout metadata feed a streamed,
bounded renderer using the existing Decimal/IEEE display magnitude. No token heap
or arithmetic engine is introduced. [The profile](FORMAT.md#interleaved-numeric-literals)
records exact limits, 48 focused SQL-enabled Format tests, 35 separately run
parser-free Format tests and 17 query/CLI mask tests. The new runner passes 816 independent controls; fixed/scientific recipes
retain 2,470/1,432 controls. Reference/native getter controls are independent evidence; native Access/NLS qualification remains open.

## Numeric formatting of stored Date/Time

Numeric Format masks and General Number now display legacy DATE serials through
the existing bounded Double renderer, preserving negative-fraction interpretation
and the source bits. Explicit number policies apply without requiring a date
profile. CStr/Str and Extended-date numeric conversion retain their boundaries.
[The contract and verification](FORMAT.md#numeric-display-of-legacy-datetime-serials)
record 44 SQL-enabled and 32 parser-free focused Format tests, six query/CLI
tests and 252 independent Decimal/calendar/native-field controls with unchanged
Jet 4/ACE 2007/ACE 16-family inputs. Native Access qualification remains open.

## Numbered error-message lookup

Explicit Error(number)/Error$(number) now produce budgeted Text in scalar, bound,
nested/domain/closed Eval and grouped query paths. Hosts select immutable message
providers; missing context fails, while zero needs no catalog. The CLI selects
an optional Fluent-backed English reference catalog with 86 candidate classic
VBA messages. Worker requests keep owned catalogs isolated. Omitted-number VBA
runtime state remains deferred. [The contract](ERROR_MESSAGES.md) separates
reference data, its CC-BY attribution and pending native Access qualification.
The owning document records 752 expression/query tests, 285 minimal-profile tests,
35 SQLite checks, 16 selected CLI tests and 560 reference/native-field controls.

## Explicit CurrentUser expression sessions

CurrentUser now materializes an immutable caller-issued AccessSessionContext name
through the ordinary budgeted Text boundary. Libraries fail without context; the
CLI explicitly declares an unsecured Admin expression session for each eval/run
command. Names retain exact spelling and are not inferred from environment,
headers or encryption passwords. Grouping, nested/domain/closed Eval and named
query paths share execution-local data, never plan-captured identity. This metadata
grants no authentication or permissions. [The contract](SESSION_FUNCTIONS.md) keeps
MDW/ACL/native host qualification separate; CurrentUser moves to Partial.

All 745 expression/query tests including doctests pass, together with 285 standalone
parser-free expression tests and the CLI control covering both commands. One pure
context control and two scalar controls check name validity, independent byte
boundaries, exact output charges, narrower Text limits and every observed dynamic
cancellation checkpoint. Three bound controls cover plan/context reuse, Unicode,
group/domain/derived/correlated calls, arity/eager failures, quotas and cancellation.
Minimal/all-feature Clippy, warnings-denied rustdoc, dependency boundaries and
format/diff checks pass.

An all-feature optional-worker consumer check found stale ExecutionOptions fields.
The SQLite worker now forwards owned session names, builds optional request-local
Rnd state from validated seeds and explicitly leaves field collation resolution
unselected. All 33 SQLite tests and its async API doctest pass, including invalid
control cleanup, missing-provider refusal and request name/seed isolation. This is
shared-context integration, not expanded SQLite storage or write scope.

The original runner passes 58 declared-session/type/refusal/native-field/group/
domain controls across four unchanged original ScalarFixture files, even with
other OS username variables. Getters disable expression evaluation. These checks
do not execute native CurrentUser or authenticate a workgroup account. Windows
comparisons and whole-workspace runtime tests remain unrun. No dependency, stored
carrier or native format changed, and no third-party source or fixture was copied.

## Closed query Eval and strict expression parsing

Eval now evaluates one closed expression string through the existing scalar
engine, retaining caller policies, clock/Rnd providers, Error result data,
cancellation and cumulative budgets. Numeric input uses explicit CStr policy;
dynamic fields, parameters, members, subqueries, domain/UDF/application calls
remain refused before host lookup. Dedicated bound argument walks retain visible
source/group/aggregate/correlated dependencies. Runtime group/pivot keys reject
hidden clock/Rnd calls, including nested Eval; ordinary projections retain supplied
host context. [The contract](DYNAMIC_EVALUATION.md) keeps full host/Variant/native
qualification gaps explicit. Eval moves to Partial.

The shared strict parser entry point refuses SQL terminators without constructing
a second parenthesized input. Domain expressions also consume it under their
existing source/shape/budget rules. A parser control covers quoted semicolons,
comments, extra statements and the complete 65,536-byte ceiling. Chr$ is a specific
checked Chr spelling, matching the documented Eval example; other typed aliases
are not implied.

All 797 SQL/expression/query tests including doctests pass, together with 284
standalone parser-free expression tests and the CLI control covering both commands.
Four scalar and four bound controls cover nested/context/Error results, host-name
and effect preflight, field/parameter/group/aggregate/correlated dependencies,
source-predicate safety, volatile pivot/group refusal, depth, quotas and every
observed scalar/query cancellation checkpoint. Focused final controls additionally
confirm repeated calls exhaust one shared allowance. Initial pivot test examples
omitted required grouping and then used a nonexistent fixture field; corrected
examples exercise the intended runtime refusal without changing product behavior.
Minimal/all-feature Clippy, warnings-denied rustdoc, dependency boundaries and
format/diff checks pass.

The original runner passes 1084 independent integer/Boolean/civil/context/refusal/
native-field controls across four unchanged original ScalarFixture files. Getters
disable expression evaluation. It does not execute Access Eval or qualify native
saved/application formulas. Windows comparisons and whole-workspace runtime tests
remain unrun. No dependency, stored carrier or native format changed; no third-party
source or binary fixture was copied. Inspection never executes stored formulas.

## Expression-preserving encounter aggregates and domains

First/Last now select expression data, including Error codes and NULL, through a
single encounter-order slice. The same selector supplies DFirst, DLast and DLookup;
the domain result path no longer forces Error through scalar publication. Grouped
HAVING, ordered derived inputs, explicit consumers and crosstab publication retain
the selected carrier. Every operand still evaluates, so later division-by-zero,
constructor, cancellation and source failures remain query failures. Stored/source
contracts and the scalar-only execute publication check are unchanged.
See [scope and remaining propagation work](VARIANT_ERRORS.md#encounter-order-aggregates-and-domains).

All 365 query tests including doctests pass, with two CLI Error controls. Four
bound controls include a fixed-seed 256-case endpoint model over empty/scalar/NULL/
Error sequences, plus grouping/derived/crosstab/domain results, eager late failures,
scalar publication refusal, shared quotas and every observed cancellation
checkpoint. All-target/all-feature Clippy, warnings-denied query rustdoc,
dependency boundaries and formatting/diff checks pass. Expression kernel code and
its feature/dependency configuration were unchanged by this chunk.

The original runner passes 1058 independent list-endpoint/NULL/Error/refusal/
native-field controls. Reviewed getters provide encounter IDs from four unchanged
original ScalarFixture files with expressions disabled. These controls do not
execute Access aggregates or qualify native optimizer/index/Error order. Numeric
aggregates, Count(Error), Error comparison/sort/group semantics and complete
Variant propagation remain unfinished. Windows qualification and whole-workspace
runtime tests remain unrun. No dependency, stored carrier or native format changed;
no third-party source or fixture was copied.

## Provisional StringFromGUID query conversion

StringFromGUID now accepts exactly sixteen GUID-structure Binary bytes and stored
Guid fields through a shared fixed-size formatter/evaluator, returning uppercase
hex inside the provisional {guid {…}} Text envelope. It uses the existing uuid
dependency, explicit byte-order conversion, fallible charged Text allocation and
caller cancellation/limits. A dedicated bound child node retains field/parameter,
group/aggregate, correlation and collection dependency walks. [The contract](GUID_CONVERSIONS.md)
separates Microsoft's inverse-operation reference from unqualified exact spelling,
coercion and direct-query behavior; no native Access evidence was invented.

All 727 expression/query tests including doctests pass, together with 284 standalone
parser-free expression tests and both CLI GUID controls. The new independent
integer-hex property has 256 fixed-seed cases. Focused controls cover native versus
canonical byte order, exact Text bytes/ceilings, invalid widths/NULL/carriers,
parameters, grouping/aggregates/correlations, eager failures and every observed
bound cancellation checkpoint. Minimal/all-feature Clippy, warnings-denied rustdoc,
dependency boundaries and format/diff checks pass.

The original inverse runner passes 1594 provisional formatting/byte/type/refusal/
native-field controls across four unchanged original ScalarFixture files. Getters
disable expression evaluation. Native Access output, query admission and full
coercion qualification remain pending, as do whole-workspace runtime tests.
No dependency, stored carrier or native format changed; no third-party source or
fixture was copied. StringFromGUID moves to Partial.

## Explicit expression-valued query results

PreparedQuery::execute_expression now returns QueryResult<ExpressionValue> through
its ExpressionQueryResult alias. Buffered runtime rows retain Error codes through
projection, derived/named queries, UNION ALL, joins, scalar/correlated subqueries
and crosstab publication. Explicit field consumers can inspect or convert those
codes. The existing execute API performs checked scalar-only final publication;
source/parameter/snapshot/write carriers remain Value. No Error side table or
NULL/integer replacement was introduced. Both CLI query commands display raw Error
cells as #ERROR(code), separate from quoted Text. [The contract](VARIANT_ERRORS.md)
retains unfinished operator/aggregate, array, sorting/set-comparison and native
qualification work.

All 591 tests in the combined query/CLI suite pass; five focused expression-result
tests and six
editing tests also pass after the final publication/tracing adjustment. Controls
cover explicit versus scalar APIs, eager failures, derived/join/correlated/named
rows, crosstab cells, state reuse, quotas and every observed result/edit
cancellation checkpoint. The old editing test assumed a particular cancellation
checkpoint; it now checks every observed checkpoint and accepts cancellation
from either query or expression work, while verifying no backend candidate call.
All-feature/all-target Clippy, warnings-denied query rustdoc,
dependency boundaries and formatting/diff checks pass.

The extended original CVErr runner passes 4522 controls, including raw Error cells
and native ID-field results across four unchanged ScalarFixture files. Getter
values are independent data, not Access function execution. No dependency or native
format changed. Windows Access comparisons and whole-workspace runtime tests
remain unrun; the existing scalar-only adapter APIs retain their explicit limits.

## CVErr construction and explicit Error consumers

CVErr now constructs the core's existing expression-only Error carrier through a
shared conversion kernel. Checked Long coercion precedes the standard-code range
check; existing opaque Error identity is retained. Explicit numeric conversion,
type/state inspection, CVar and eager IIf/Nz/Switch/Choose selection consume or
preserve Error data without catching operational failures. Query binding tracks
the new child node through field/group/aggregate/correlation and collection walks.
The public scalar expression-result API can return Error data deliberately;
normal scalar-only query cells/storage still reject it. See [the result-cell
contract](VARIANT_ERRORS.md). CVErr moves to Partial.

All 716 expression/query tests including doctests pass. Three new kernel controls
include a fixed-seed 256-case code/explicit-conversion/storage-refusal model. Two
scalar and three bound controls cover selection, eager failures, explicit numeric
conversion, parameters, grouping/aggregate/correlated dependencies, predicate
safety, Text conversion budgets and observed scalar cancellation checkpoints.
All 283 standalone parser-free expression tests and the CLI control covering both
commands pass. Minimal and all-feature Clippy, warnings-denied rustdoc, dependency
boundaries and format/diff checks pass. A complete suite repeat includes the eager Error-condition cases and
array-admission guards. Subsequent focused scalar/query checks cover the final
Binary/NULL selector array-view preservation; array bounds retain Long carriers.

The independent runner passes 3120 code/half-even/inspection/selection/refusal/
native-field controls across four unchanged original ScalarFixture files. Initial
CLI/model expectations used lowercase Boolean output and were corrected to the
existing uppercase transport. Reviewed getters supply ID fields without evaluating
functions. These checks do not establish native Access Error/coercion/query-cell
behavior; Windows comparisons and whole-workspace runtime tests remain unrun.
No dependency, stored carrier, schema or native format changed, and no third-party
source or fixture was copied.

## Explicit Rnd query sequences

Rnd now uses caller-owned sequential state through the math kernel, scalar
evaluator and query execution options. Omitted/positive calls advance, zero
repeats, and negative numeric input reseeds through checked Single bits. Plans
retain no state. The CLI creates one state per eval/run command and exposes a
validated raw `--random-seed`, defaulting to the reviewed Basic initial seed.
See [Rnd ownership and limits](RANDOM_FUNCTIONS.md) for query-stage ordering,
volatile-key restrictions, error-state consumption and native qualification gaps.
Rnd moves to Partial; stored carriers, native I/O and dependencies are unchanged.

All 709 expression/query tests including doctests pass, together with 280
standalone parser-free expression tests and a CLI test covering both commands.
Three kernel tests include a fixed-seed 256-case wide-integer model. Two scalar
controls cover every observed pre-draw cancellation checkpoint, state absence,
invalid arity and work refusal. Three bound controls cover plan/state reuse,
nested calls, filters, sort aliases, aggregates and cancellation/work refusal.
The initial filter/projection test expected interleaved stages; the corrected
expectation follows the existing complete-filter-then-project pipeline. The final
complete suite passes. Minimal/all-feature Clippy, warnings-denied rustdoc,
dependency boundaries and format/diff checks pass.

The independent runner passes 1680 state/seed/Single/refusal/native-field controls,
with exact Single-bit comparisons and unchanged hashes across four original
ScalarFixture files. Getters supply ID fields without evaluating expressions.
The Basic recurrence/default mapping is reference evidence, not an Access
sequence oracle. Windows Access optimizer/sequence qualification, wider saved-query
qualification and whole-workspace runtime tests remain unrun. No third-party
source or fixture was copied. OS getrandom remains the encryption entropy source;
it does not replace the stateful Rnd algorithm.

## Fixed EuroConvert query calls

EuroConvert now runs through the shared financial expression/query call path with
Double input/results, the 12-code Access table, exact recorded ECB factors,
target calculation-place rounding and explicit significant-digit triangulation.
Omitted slots remain distinct from NULL; equal-code identity retains the converted
amount. Finite extremes and unintended full-precision underflow have explicit
results/errors. The feature lives in a financial slice, with no CLI/backend
rate-table duplication. See [the contract](EURO_CONVERSION.md) for precision,
code-admission and native qualification gaps. EuroConvert moves to Partial.

All 701 expression/query tests including doctests pass, together with 277
standalone parser-free expression tests and a CLI test covering both commands.
Four kernel tests include a fixed-seed 256-case identity model. Two scalar and
two bound tests cover optional slots, eager failures, grouping/aggregate/correlated
dependencies, parameter reuse, byte/work limits and every observed scalar
cancellation checkpoint. Minimal and all-feature Clippy, warnings-denied rustdoc,
dependency boundaries and format/diff checks pass.

The original independent runner passes 4646 controls across all 144 currency
pairs, precision policies, optional/error cases and native Currency fields in
four original ScalarFixture files. All source hashes remain unchanged. Decimal
models allow eight ULPs or 1e-12 absolute error for Double comparison; this does
not establish native bitwise precision. Getters disable expression evaluation.
Windows Access function/NLS and complete native saved-query qualification remain
unrun, as do whole-workspace runtime tests. No dependency, stored carrier or
native I/O changed; no third-party source or binary fixture was copied.

## Format scientific notation and comma scaling

Format now renders scientific exponent sign/case/width, mantissa normalization
and rounding carry, the named Scientific style and whole-part comma scaling.
Internal commas still group; scaling composes with percentage shifts and sections.
The shared numeric display magnitude keeps exact coefficients and finite IEEE
exponents separate, using rust_decimal rounding without narrowing scaled output.
Finite subnormals bypass fixed-place rounding before normalization. See
[the scientific/scaling contract](SCIENTIFIC_FORMATTING.md). Format stays Partial;
no dependency, stored carrier or native I/O changed.

All 693 expression/query library tests including doctests pass. Four new kernel
controls include exact carrier boundaries, finite extremes and a fixed-seed
256-case independent digit/exponent model. A scalar control checks each observed
cancellation checkpoint, byte/work/input/output limits and eager failure. A query
control covers grouping/aggregate/correlated dependencies and mask/profile reuse.
All 273 standalone parser-free expression tests and five focused CLI tests pass.
Minimal and all-feature Clippy, warnings-denied rustdoc, dependency boundaries and
format/diff checks pass. A focused repeat verifies the final additional
output-expansion limit assertion after the complete library run.

The original independent runner passes 1456 Decimal/scientific/scaling/refusal/
native-field controls over four unchanged ScalarFixture files. Its first native
probe used unbracketed Money and failed parsing; the corrected `[Money]` query
retains all probes. The existing numeric/styled-number models replay all 2570
and 1334 controls. These are models and reviewed getters, not Access function
execution. Windows Access/NLS and whole-workspace runtime tests remain unrun.
No third-party source or fixture was copied.

## Format text placeholders and combined casing

Format now renders required/optional UTF-16 slots, explicit fill direction,
NULL/empty sections, literals and excess input through a borrowed text feature
slice. Combined casing reuses full-context ICU before placement, with two metered
passes and no intermediate String. Adjacent surrogate halves publish as a pair;
interposed literals fail explicitly. See [the text-layout contract](TEXT_FORMAT_MASKS.md)
for portable policies and remaining native/coercion/raw-string gaps. Format stays
Partial; no stored value, I/O profile or dependency changed.

All 685 expression/query library tests including doctests pass, followed by two
additional focused query tests for grouped/aggregate/correlated dependencies and
language/mask reuse. Four new kernel tests include a fixed-seed 256-case position
model. Two scalar tests exercise sections, Unicode, eager failures, output/work/
allocation limits and every observed cancellation checkpoint. All 269 standalone
parser-free expression tests and four focused CLI tests pass. Minimal and
all-feature Clippy, warnings-denied rustdoc, dependency boundaries and format/diff
checks pass. The first query test needed explicit text collation for a Text group
key; its revised numeric Len group key tests the dependency without inventing a
comparison provider. The initial failed check is not passing evidence.

The independent text-layout runner passes 1326 controls, including original native
Short Text/NULL/empty fields from four ScalarFixture files with unchanged hashes.
Numeric and date-format runners replay all 2570 and 13020 controls respectively.
These models/getters do not execute Access functions. Windows Access/NLS and
whole-workspace runtime tests remain unrun. No third-party code or fixture was
copied.

## Gregorian Format date and time masks

Format now routes custom date/time fields and named date styles through Jiff,
shared CDate conversion, calendar names, whole-second output and internal week
settings. Quoted/escaped data stays literal; month/minute context and meridiem
selection are explicit. Numeric-only calendar fields need no ambient locale;
regional names/separators require supplied date profiles. Extended range/day
carry remains checked. See [the date-mask profile](DATE_FORMAT_MASKS.md) for
medium-style choices and remaining section/calendar/NLS/coercion gaps. Format
stays Partial; no stored carrier or native I/O interpretation changed.

All 679 expression/query library tests including doctests pass. Six new kernel
controls cover token/context/literal goldens, named profiles, refusal cases,
negative serials, Extended boundaries, shared week rules and a fixed-seed
256-case component model. Two scalar and three query controls cover policy and
parameter reuse, group/aggregate/correlated dependencies, eager errors and every
observed cancellation checkpoint. New date-mask CLI and existing Format/date
formatting tests pass in both commands. All 265 standalone parser-free expression
tests pass, alongside minimal and all-feature Clippy, warnings-denied rustdoc,
dependency boundaries and format/diff checks.

The independent calendar/week runner passes 13020 controls using Python models
and four original native ScalarFixture files with unchanged hashes. The existing
numeric Format runner also replays all 2570 controls successfully. The date runner's initial
22-field probe exceeded the CLI work ceiling; the passing run retains all probes
in six-field batches without raising limits. Ordinary stored DATE/NULL values
come from reviewed getters, not native function execution. Windows Access/NLS
and native Extended field-format qualification remain unrun. No dependency was
added; whole-workspace runtime tests were not run.

## Numeric Format masks and sections

Format now executes borrowed custom fixed numeric masks with required/optional
digits, grouping, percentage scaling, literal quoting/escaping and distinct
positive/negative/zero/NULL sections. Named numeric/Boolean styles and existing
CStr/standalone casing paths share explicit policies. Missing SQL masks retain
an empty-Text default instead of becoming NULL; week options use existing typed
ranges and host settings. A shared numeric-format magnitude slice now supplies
both this renderer and the existing styled-number functions without arithmetic
changes. See [the supported profile and remaining mask families](FORMAT.md).
Format remains Partial; custom date/text layouts and native qualification remain
unfinished.

All 668 expression/query library tests including doctests pass. Six new kernel
controls include independent literal/section goldens, a fixed-seed 256-case
integer-padding model and truncation boundaries. Three scalar and three query
controls cover default/missing/NULL arguments, eager errors, policies and reusable
parameters, group/aggregate/correlated dependencies, output limits and every
observed cancellation checkpoint. New CLI Format and existing styled-number
controls pass in both query commands. The independent Decimal/model runner passes
2570 controls, including native Int32/NULL fields across the four original
Hyperlink fixtures with unchanged hashes; Microsoft Access evaluation is unrun.

The standalone parser-free expression profile passes all 259 tests. Standalone
minimal expression Clippy and all-feature expression/query/CLI Clippy,
warnings-denied rustdoc, dependency boundaries and format/diff checks pass. No dependency or stored carrier changed;
whole-workspace runtime tests and Windows comparisons were not run.

## Explicit StrConv casing and encoding

StrConv now executes upper/lower/proper casing and strict Unicode/single-byte
string conversion through the existing owning byte-call paths. Casing recipes
reuse ICU and bounded TextOutput; exact encoding/decoding reuse the eleven
existing character pages with a bounded ByteOutput sink. Conversion 128 retains
raw String bytes, distinct from Binary arrays; conversion 64 explicitly accepts
ANSI Byte-array input. Fourteen exact LCID profiles override language/page settings
for a call without changing subsequent calls or reused plans. See
[the conversion contract](STRCONV.md) for mode combinations, byte publication,
NULL/error order, and remaining width/kana/DBCS/locale/coercion/NLS gaps.
StrConv remains Partial and does not execute stored VBA.

The complete expression/query suites pass, followed by focused checks after
simplifying proper-case error propagation and adding output-expansion controls;
together these cover 656 library tests including doctests. Nine new kernel/scalar
controls cover conversion flags, exact locale identities, strict mapping goldens,
all documented separators, a fixed-seed 256-case proper-case model, nested raw
bytes, eager failures, byte/output ceilings and every observed scalar cancellation
checkpoint. Three query tests cover group/correlated/aggregate dependencies,
Binary inputs, parameter/policy reuse, call-local overrides and every observed
query cancellation checkpoint. Both new CLI tests and the existing casing/byte
controls pass in eval/run modes, including empty stdout on refused output.
A separate parser-free expression run now passes all 253 tests and warnings-denied
Clippy. This exposed and repaired earlier unconditional evaluator imports in
Hyperlink/Partition tests and an SQL-only byte-search helper export; combined
query/expression checks had enabled SQL transitively. Pure kernel coverage is
retained without SQL, and development guidance now requires separate invocations.

The original independent runner passes 12178 mapping/casing/refusal/native-field
controls using Python codecs, documented WHATWG exceptions, selected casing models
and the four original Hyperlink fixtures. All source hashes remain unchanged;
Jackcess decodes values without evaluating StrConv. Actual Windows Access
execution, full native NLS/coercion/error behavior and best-fit policies remain
unqualified. No library version, dependency feature or stored carrier changed.

All-target/all-feature expression/query/CLI Clippy, minimal expression/query
builds, warnings-denied rustdoc, dependency boundaries and format/diff checks pass.
Whole-workspace runtime tests and Windows Access comparisons were not run.

## Binary InStrB execution

InStrB now searches Unicode UTF-16LE and lossless byte-string operands through an
allocation-free binary kernel and shared evaluator. One-based byte starts/results
retain zero-byte, odd-byte and unaligned matches. SQL normalization, written-order
eager evaluation and the existing text-call plan visitors preserve dependency,
grouping and correlation behavior. Explicit compare 0 works; nontrivial omitted
comparison needs a supplied Binary Option Compare policy. Text/database and LCID
byte-search providers remain unsupported without fallback. See
[the byte-search profile](BYTE_STRING_FUNCTIONS.md#binary-byte-search).

Complete expression/query suites pass, followed by focused byte tests after an
added cancellation control; together these cover 644 library tests including
doctests. New controls cover independent byte goldens, a fixed-seed 256-case
raw search model, every observed kernel/query cancellation checkpoint, explicit
policy refusal, eager errors and group/aggregate/correlated dependencies. All
three CLI byte tests pass in eval/run modes. The extended independent Python
runner passes 3952 byte/refusal/native-field controls, including all byte-code
needle searches and zero-byte searches over all four original native fixtures
with unchanged hashes. Native Microsoft Access evaluation remains unrun.

Expression/query/CLI all-target/all-feature Clippy, minimal expression/query
builds, warnings-denied rustdoc, dependency boundaries and format/diff checks
pass. No dependency changed; whole-workspace runtime tests were not run.

## Lossless byte-string construction and slicing

ChrB, LeftB, RightB and MidB now execute through the existing text-call binding
and shared owning byte kernels. Odd byte counts and unpaired UTF-16 units survive
nested byte calls, CVar and type inspection. String subtype identity remains
distinct from Binary arrays. Consuming raw slices reuse their original buffer;
Unicode input encoding and strict Text publication use cumulative work/allocation
controls. Unsupported Unicode publication fails without partial CLI stdout.
See [the byte-string contract](BYTE_STRING_FUNCTIONS.md) for remaining arbitrary
consumer/result-carrier, alias and coercion gaps; all four functions stay Partial.

The complete expression/query test suites pass, with focused reruns after the
strict decoder idiom change and an added query cancellation control. In total,
639 library tests including doctests are covered. Eight new kernel/scalar/query
controls cover independent raw-byte goldens, a fixed-seed 256-case slicing model,
NULL/errors, eager evaluation, grouping identity, aggregate dependencies,
correlation, reusable parameters, budgets and every observed cancellation point.
Both CLI byte tests pass in eval/run modes with empty stdout on refused output.
The extended independent runner passes 3208 controls, including every ChrB code,
odd/unaligned slices and independently decoded native fields from the four
original Hyperlink fixtures; all source hashes remain unchanged.

All-target/all-feature expression/query/CLI Clippy, minimal expression/query
builds, warnings-denied rustdoc, dependency boundaries and format/diff checks pass.
No dependency or stored carrier changed. Whole-workspace runtime tests and actual
Microsoft Access evaluation were not run.

## LenB and AscB Unicode byte observations

LenB now returns checked UTF-16 byte length for Text/NULL, while AscB returns the
first raw UTF-16LE byte as Integer. They do not count UTF-8 bytes or infer an ANSI
page. Supplementary characters, combining sequences and leading zero bytes retain
their exact Unicode representation. Existing call-shape, input, work and output
boundaries remain active; no dependency or stored carrier changed. See
[the byte-string contract and remaining representation work](BYTE_STRING_FUNCTIONS.md).

All 301 expression tests including doctests and 330 query tests including doctests
pass, along with the new CLI byte observation test in both execution paths.
Four kernel controls include Unicode/NUL/supplementary goldens, code-page
independence, NULL/carrier checks, scan cancellation and a fixed-seed 256-case
code-point property. Two query controls cover fields, numeric subtypes,
filters/groups/correlations, aggregates, parameter reuse and eager errors.
The independent Python UTF-16LE runner passes 628 byte/refusal/native-field
controls using reviewed getters on the four original Hyperlink fixtures, with
unchanged hashes. Actual Access evaluation, full Variant/storage overloads and
raw byte-string behavior remain unqualified; both functions stay Partial.

Expression/query/CLI all-target/all-feature Clippy, minimal expression/query
builds, warnings-denied rustdoc, dependency boundaries and format/diff checks pass.
Whole-workspace runtime tests and Windows Access comparisons were not run.

## Stored HyperlinkPart components

HyperlinkPart now exposes all six numeric selectors through a borrowed text
kernel and existing metered Text output. SQL requires both arguments, following
Microsoft's query-specific contract. Display fallback, empty/internal-only full
addresses and raw Unicode components work without URI normalization or resource
access. Selector conversion, NULL/error ordering and incomplete/extra-delimiter
policies are explicit; native edge behavior remains unqualified. See the
[component contract](HYPERLINK_PARTS.md).

All 297 expression tests including doctests and 328 query tests including doctests
pass, along with the new CLI Hyperlink test in both execution paths. Four new
kernel controls cover all selectors, independent component goldens, scan
cancellation/output budgets and a fixed-seed 256-case generated-field property.
Two query controls cover grouping, correlations, filtering, aggregate/parameter
dependencies, required SQL arity and eager failures. The original Java recipe
creates actual Hyperlink-marked Memo fields in Jet 4/ACE 2007/2016/2019 files.
The corrected independent runner passes 1090 component/refusal/native-field
controls, including Jackcess getters and unchanged source hashes. Native
Microsoft Access function evaluation and save/reopen remain unrun.

Expression/query/CLI all-target/all-feature Clippy, minimal expression/query
builds, warnings-denied rustdoc, dependency boundaries and format/diff checks pass.
No dependency changed. Whole-workspace runtime tests were not run.

## GUIDFromString byte arrays

GUIDFromString now parses a bounded GUID spelling and returns sixteen Binary
GUID-structure bytes through scalar and bound queries. The fixed-size public
kernel uses uuid without allocation; evaluation charges work/output before
fallible materialization. The byte result is distinct from the canonical stored
Guid carrier and existing SQL GUID literals. See
[spelling, byte order and explicit comparison gaps](GUID_CONVERSIONS.md).
StringFromGUID remains pending query-specific evidence and output-format
qualification; application documentation alone is not counted as query execution.

All 293 expression tests including doctests and 326 query tests including doctests
pass, along with the new CLI GUID conversion test in both execution paths.
Four new kernel/evaluator controls cover independent mixed-endian goldens,
accepted/refused shapes, a Unicode slice-boundary regression, a fixed-seed
256-case byte-group property and output/work/cancellation/NULL limits. Two query
controls cover parameter reuse, correlations, aggregate dependencies, eager
failure and every observed cancellation checkpoint. The independent runner
passes 422 GUID-byte/refusal/native-row controls across four original Jet 4/
ACE 2007/2016/2019 files with unchanged bytes. Microsoft Access execution,
NULL/errors, accepted grammar and exact Byte-array mapping remain unqualified.

The existing uuid dependency is now required in standalone expressions for its
working public conversion kernel; version/features are unchanged and no package
was added. All-target/all-feature expression/query/CLI Clippy, minimal expression/
query builds, warnings-denied rustdoc, dependency boundaries and all 32 boundary
script tests pass, along with format/diff checks. Whole-workspace runtime tests
and Windows Access comparisons were not run.

## Materialized CVar values

CVar now moves its evaluated argument unchanged through scalar and bound queries,
retaining numeric widths, exact representations, NULL and payload ownership. An
explicit unary plan node preserves work/cancellation accounting, grouping and
operand dependencies; Text keeps its original static comparison policy. No VBA
variable/object runtime or new stored carrier was introduced. See the
[scope and representation contract](CVAR.md).

All 289 expression tests including doctests and 324 query tests including doctests
pass, along with the new CLI CVar test in both execution paths. Two kernel tests
cover representation and allocation-address preservation. Four query controls
cover scopes, aggregate/domain/group walks, reusable Decimal parameters,
Binary/native extension carriers, missing clock context and every observed
cancellation checkpoint. Existing field-policy controls now include CVar
comparisons/conflicts. The original independent runner passes 346 subtype/value/
refusal/native-row controls across four original Jet 4/ACE 2007/2016/2019 files
with unchanged bytes. Arrays/expression-only states and full Variant/native
behavior remain unqualified; Microsoft Access execution was not run.

Expression/query/CLI all-target/all-feature Clippy, minimal expression/query
builds, warnings-denied expression/query rustdoc, dependency boundaries and
format/diff checks pass. No dependency changed. Whole-workspace runtime tests
were not run.

## Materialized CVDate alias

CVDate now resolves to the existing CDate conversion operation for materialized
query values, following the MS-VBAL equivalence contract. Results retain the
legacy DATE carrier and Date subtype code; no Variant variable runtime or stored
carrier was added. Explicit date/numeric policies, NULL failure, range checks,
resource accounting and eager errors are inherited. See
[the CVDate boundary](DATE_CONVERSIONS.md#cvdate-query-alias).

All four CLI date-conversion tests and three query date-conversion tests pass.
New controls cover subtype, epoch/negative serials, explicit ISO input, correlated
field conversion, aggregates, late NULL failure and malformed calls with no
stdout. The independent Gregorian/Automation runner passes 2470 controls under
CVDate and 2470 under the unchanged default CDate spelling. Optional native-row
comparisons were not run for this alias change. Existing conversion semantics
remain Partial; full Variant/regional behavior and native Access comparisons
remain unqualified.

Expression/query/CLI all-target/all-feature Clippy, the minimal standalone
expression build, dependency boundaries and format/diff checks pass. No dependency
changed. Whole-workspace runtime tests and Windows Access comparisons were not run.

## Numeric Partition labels

Partition now produces padded range labels through a stack-only text recipe,
checked Long/half-even conversion and shared metered output. Scalar/bound query
paths retain eager operand evaluation, NULL propagation, domain checks, parameter
reuse and buffered failures. Widened internal endpoints avoid Long arithmetic
overflow; stored carriers and dependencies are unchanged. See the
[contract and native padding/edge limits](PARTITION.md). The interval-one rule
follows Microsoft's documentation; exact outside-range behavior remains
unqualified rather than inferred from a related runtime implementation.

All 287 expression tests including doctests and 319 query tests including doctests
pass, along with the new CLI Partition test in both execution paths. Four new
kernel controls cover space/boundary goldens, NULL/rounding/carriers, budgets and
a fixed-seed 256-case enumerated-range property. Three query controls cover
relations/scopes/group identity, aggregates, parameters, eager/late failures,
syntax refusal, work limits and every observed cancellation checkpoint. The
independent runner passes 294 range/rounding/refusal/native-row controls across
four original Jet 4/ACE 2007/2016/2019 fixtures with unchanged bytes. It executes
TableCraft against original native files, not Microsoft Access formulas, and adds
no saved Partition fixture.

Expression/query/CLI all-target/all-feature Clippy, standalone minimal expression/
query checks, warnings-denied expression/query rustdoc, dependency boundaries and
format/diff checks pass. Whole-workspace runtime tests and Windows Access
comparisons were not run.

## Query-accessible packed colors

RGB and QBColor now return packed Access Long values through the pure numeric
expression feature slice. Checked numeric conversion, channel upper saturation
and the sixteen-entry Basic palette feed existing scalar/bound retrieval without
UI or runtime dependencies. Round keeps its two-argument bound after expanding
the shared math-call stack buffer for three-channel RGB. See the
[color contract](COLOR_FUNCTIONS.md). Microsoft's explicit sandbox query list
establishes query availability; coercion, input-width and native error boundaries
remain unqualified. The tracker records query availability separately from
implementation scope: IsObject remains `Not planned` as VBA object-reference
inspection, despite being callable in Access queries. No object runtime was added.

All 283 expression tests including doctests and 316 query tests including doctests
pass, along with the new CLI color test in both execution modes. Four new kernel
controls include independent palette/channel goldens and a fixed-seed 256-case
arithmetic property. Three query controls exercise metadata-only binding,
dependency/grouping identity, correlations, aggregates/domains, plan reuse,
every observed cancellation checkpoint and eager/late failures. The independent
runner passes 340 packed-color/palette/refusal/native-row controls across four
original Jet 4/ACE 2007/2016/2019 files with unchanged database bytes. These are
local models and TableCraft execution, not native Access comparisons.

Expression/query/CLI all-target/all-feature Clippy, standalone minimal expression/
query checks, dependency boundaries and format/diff checks pass. No dependency
changed. Whole-workspace runtime tests and Windows Access comparisons were not run.

## Query-accessible Timer

Timer now returns Single seconds since local midnight through the existing
host-issued clock contract in scalar and bound read-only queries. CLI fixed and
system readings retain 100-nanosecond fractions before Single rounding; existing
Date/Time/Now results still use whole seconds. Separate readings remain volatile,
binding stays inert, and missing context, cancellation, budgets and provider
failures retain their existing boundaries. See [the clock contract](CLOCK_FUNCTIONS.md).
Microsoft's Timer query example establishes intended query availability; native
Access resolution, invocation behavior and rounding remain unqualified.

All 279 expression tests including doctests and 313 query tests including doctests
pass, along with all four CLI clock tests. New controls cover fractional values,
distant dates, Single boundary goldens, midnight, plan reuse, volatile grouping,
eager errors and buffered late failures. Existing callback controls now also
exercise Timer. The independent rational binary32 runner passes 560 rounding,
DATE-regression and original native-row controls across Jet 4/ACE 2007/2016/2019
fixtures, with unchanged database bytes. The separate Date/Time/Now model passes
2456 regression controls. These are independent/local execution evidence, not
Microsoft Access comparisons; no saved Timer fixture or native execution is claimed.

Expression/query/CLI all-target/all-feature Clippy, standalone minimal expression/
query checks, warnings-denied expression/query rustdoc, dependency boundaries and
format/diff checks pass. No dependency changed. Whole-workspace runtime tests and
Windows Access comparisons were not run.

## Type/state inspection and explicit array bounds

IsArray/IsEmpty/IsError/TypeName/VarType/LBound/UBound now consume supported
materialized values and direct Array/VBA.Array intermediates in scalar and bound
retrieval. A bounded immutable expression-only array preserves explicit Long bounds
and original scalar elements; stored Value, schema and scalar parameter carriers
remain unchanged. Type inspection retains actual numeric widths and distinguishes
NULL, host-issued Empty/Error and optional Missing. IsError never catches operational
failures. Native-only GUID/extended DateTime subtype guesses are rejected. See
[value/array contracts and explicit gaps](VALUE_INSPECTION.md).

All 277 expression tests including doctests and 311 query tests including doctests
pass. Eleven new expression controls include independent subtype/name goldens,
state distinctions, a fixed-seed 256-case wide-bound property, dimension/range/
shape checks and allocation-free direct host-bound Binary inspection with exactly
one field resolution. Four new query controls cover metadata-only binding,
pruning/grouping/correlations/domains/aggregates, parameter reuse, every observed
cancellation checkpoint, shared budgets and late failures. Text grouping retains
its explicit collation requirement.

The original fixture recipe and independent runner pass 284 subtype/bounds/scalar/
getter/rendering/direct/saved/named/refusal controls across four fresh original
Jet 4/ACE 2007/2016/2019 files with unchanged bytes. Jackcess constructs and reads
metadata without executing formulas or SQL. Native query admission, promotion,
binary mapping, empty bounds, coercion/errors, objects/default properties and
broader Variant arrays remain unqualified. SQL CVErr and language variables remain
unimplemented; their explicit host-issued states can be inspected without storing
them or running VBA.

Expression/query/Access/CLI all-target/all-feature Clippy, minimal expression/query
checks, warnings-denied expression/query rustdoc, dependency boundaries and
format/diff checks pass. No dependency changed. Whole-workspace runtime tests and
Windows Access comparisons were not run.


## Explicit periodic cashflows

NPV/IRR/MIRR now consume bounded explicit Array/VBA.Array numeric sequences in
scalar and bound read-only retrieval. The immutable cashflow input retains all
zeros and periods, uses fallible budgeted allocation and rejects NULL/nonnumeric/
non-finite elements. Compensated discounting, a shared bounded IRR/Rate solver and
logarithmic MIRR accumulation retain explicit range/convergence failures. Query
binding reads metadata only; normal operand walks, grouping range identity,
correlations, domains and aggregate composition remain active. See the
[cashflow contract and open native gaps](CASHFLOW_FUNCTIONS.md).

All 266 expression tests including doctests and 307 query tests including doctests
pass. Eleven new expression controls include two fixed-seed 256-case independent
ledger/root properties and an independent polynomial derivative control. Four new
query controls cover metadata-only binding, structural grouping identity,
parameters, scopes, every observed cancellation checkpoint and late failures.
All 280 independent cashflow/certificate/scalar/getter/rendering/direct/saved/named/
refusal controls pass across four fresh original Jet 4/ACE 2007/2016/2019 files,
with unchanged bytes. General Variant arrays, native SQL Array admissibility,
coercion, algorithm/precision and the conflicting NPV sign boundary remain
explicitly unqualified. Jackcess metadata/rendering does not execute formulas.

The shared solver extraction also passes the existing 294 independent Rate
controls against unchanged retained fixtures. Expression/query/Access/CLI
all-target/all-feature Clippy, minimal expression/query checks, warnings-denied
expression/query rustdoc, dependency boundaries and formatting/diff checks pass.

The module entry points compose separate input, function, metering and numerical
slices. The repository guide now requires readable spacing from the first edit,
one statement per line, logical blank lines and manual review beyond formatter
success. No dependency changed; whole-workspace runtime tests and Microsoft Access
comparisons were not run.


## Bounded iterative annuity rates

Rate now shares financial argument conversion and optional normalization, with a
separate 0.1 guess default. A guess-directed log-growth solver uses safeguarded
Newton/secant/bracket steps, at most twenty iterations, representable-rate residual
qualification and explicit convergence failures. Scalar/bound SQL share a metered
evaluator bridge, charging the existing cumulative budget and checking cancellation
before iterations/publication; supplied guesses are not replaced by automatic
restarts. See [solver contract and native gaps](RATE.md).

All 255 expression tests including doctests and 36 selected query tests pass.
Six new expression controls include a fixed-seed 256-case independent ledger
property, an independent polynomial derivative check, multiple roots, explicit
exhaustion and every observed solver checkpoint. A new query control covers
defaults, reusable parameters, cumulative work, cancellation and failure buffering.
All 294 independent rate/certificate/scalar/getter/rendering/direct/saved/named/
refusal controls pass across four fresh original Jet 4/ACE 2007/2016/2019 files with
unchanged hashes. Native algorithm, precision, basins/root selection and coercion
remain explicitly unqualified; Jackcess does not execute queries or formulas.

Expression/query/Access/CLI all-target/all-feature Clippy, minimal expression/query
checks, warnings-denied expression/query rustdoc, dependency guard and formatting/
diff checks pass. No dependency changed. Whole-workspace runtime suites and native
Windows comparisons were not run.

## Scalar depreciation

SLN/SYD/DDB now share the closed financial dispatch, numeric conversion and
function-specific argument validation. DDB defaults its optional factor to two
without changing annuity zero defaults. Constant-work algebraic kernels cover
straight-line, digit-weighted and declining-book calculations; original schedule
controls remain separate from provisional Microsoft runtime edge behavior. See
[depreciation scope, references and limits](DEPRECIATION.md).

All 249 expression tests including doctests and 35 selected query tests pass.
Six new expression controls include two fixed-seed 256-case schedule properties,
carrier/domain/default checks and representable extreme ranges. A new query control
covers factors, dependencies, aggregate/domain/correlated composition and eager
late failures. All 278 independent rational-ledger/scalar/getter/rendering/direct/
saved/named/refusal controls pass across four fresh original Jet 4/ACE 2007/2016/
2019 files with unchanged hashes. Jackcess metadata/SQL rendering does not qualify
Access execution. Signed/short-life/fractional/custom-factor boundaries, rounding,
coercion and error order remain explicitly unqualified.

Expression/query/Access/CLI all-target/all-feature Clippy, minimal expression/query
checks, warnings-denied expression/query rustdoc, dependency guard and format/diff
checks pass. No dependency or external implementation was adopted. Whole-workspace
runtime tests and Windows comparisons were not run.

## Financial periods and payment components

NPer/IPmt/PPmt now share the financial dispatch, numeric conversion and optional
slot rules. NPer uses a normalized log1p period solver with explicit impossible/
ambiguous-target errors. IPmt/PPmt validate selected periods, derive fixed payments
and calculate one component without scanning an amortization schedule. Positive
rates use discounted remaining payments after the first period to avoid late
cancellation found by the independent ledger. See [scope and limits](PERIOD_PAYMENTS.md).

All 243 expression tests including doctests and 34 selected query tests pass.
Five new expression controls include two fixed-seed 256-case ledger properties;
the prior annuity property also now has 256 fixed-seed cases. A new query control
covers six-argument calls, aggregate/domain/correlated composition and late errors.
All 306 independent scalar/getter/rendering/direct/saved/named/refusal controls
pass across four fresh original Jet 4/ACE 2007/2016/2019 files with unchanged
hashes and the original numeric comparison tolerance. No Access execution is
claimed from Jackcess metadata/SQL rendering or independent Decimal ledgers.

Expression/query/Access/CLI all-target/all-feature Clippy, minimal expression/query
checks, warnings-denied expression/query rustdoc, dependency guard and formatting/
diff checks pass. No dependency changed. Whole-workspace runtime suites and native
Windows comparisons were not run. Full native coercion, period admissibility,
ill-conditioned inversion and extreme precision/range remain explicit gaps.

## Fixed-rate annuity functions

Pmt/PV/FV now share an allocation-free financial kernel with Double results,
explicit numeric conversion, optional zero defaults and eager SQL evaluation.
The kernel handles zero/near-zero rates, timing, normalized payments and directly
discounted PV/positive-growth Pmt without an unnecessary overflowing growth power.
Negative-base real-domain and non-finite/divisor errors remain explicit. Bound
argument walks retain pruning/group/correlation/collection dependencies. See
[the financial profile and precision/coercion gaps](FINANCIAL_FUNCTIONS.md).

All 238 expression tests including doctests and 300 query tests including doctests
pass. Five new expression controls include 128 generated independent ledger cases;
three query controls cover scope/dependency/group identity, reusable parameters,
eager errors, budgets and every observed cancellation checkpoint. The original
native recipe passes 246 independent scalar/getter/rendering/direct/saved/named/
refusal controls across four fresh Jet 4/ACE 2007/2016/2019 files with unchanged
hashes. Jackcess only handles inert metadata/rows/SQL rendering. No native Windows
execution or whole-workspace runtime suite was run.

Expression/query/Access/CLI all-target/all-feature Clippy, standalone minimal
expression/query checks, warnings-denied expression/query rustdoc, dependency
guard and formatting/diff checks pass. No dependency changed.

## Stored domain calls and optional criteria

Read-only domain calls now accept a missing final optional criteria slot while
retaining errors for explicit NULL and missing required arguments. Tests cover
all twelve optional calls and compound scalar criteria, including nested domain
calls. Native saved-query controls retain omitted syntax and reusable caller
parameters across the priority shared header layouts.

The original SavedDomainFixture recipe adds fifteen inert definitions and a
Unicode-named local table. All 300 independent getter/rendering/direct/saved/named/
refusal controls pass across four fresh Jet 4/ACE 2007/2016/2019 files with
unchanged hashes. The covered paths include grouping, derived reads, UNION,
fixed crosstabs, recursive definitions, unknown/parameter-requiring domains and
buffered late errors. See [domain evidence and limits](DOMAIN_FUNCTIONS.md).

The wider saved-select suite exposed a stale Unicode rejection assertion left
from before Unicode prompt support; commit 3dd7167 replaces it with an explicit
acceptance check. All 57 saved-select tests pass. Focused query/domain/policy tests,
query/Access all-target/all-feature Clippy, minimal query, dependency guard and
format/diff checks pass. No dependency changed. No Windows comparison or full
workspace runtime suite was run. Broader criteria subquery/collection/host and
native coercion/ordering behavior remain gaps rather than completed features.

## Bounded read-only domain functions

All twelve domain functions now execute through the caller-selected source,
reusing aggregate/numerical kernels under cumulative query budgets. Dynamic
expression/criteria strings are bounded and validated before binding; missing
criteria fields return NULL while source failures remain errors. Parameterless
named reads work without capturing outer parameters. Preparation retains outer
argument dependencies without reading domain rows. Nested calls have explicit
recursion limits. Field text policies apply inside domain reads; returned text
has default static provenance. See [the profile and remaining work](DOMAIN_FUNCTIONS.md).

Microsoft documents a two-value minimum for DStDevP, so its singleton returns
NULL separately from SQL StDevP's singleton-zero profile. All twelve tracker
entries are Partial: broader criteria/collection/host contexts, coercion/subtypes,
error order and native Windows qualification remain incomplete.

All 295 query tests including doctests and 13 selected native-query/statistical
CLI tests pass. Eight new query controls cover result semantics, dynamic argument
dependencies, named/parameter/refusal paths, shared quotas, every observed
cancellation checkpoint and per-field policy routing. The native CLI control
checks read-only bytes and no partial output after errors. All 36 independent controls
across four original Jet 4/ACE 2007/2016/2019 inputs retain their hashes. Query
all-target/all-feature and selected CLI Clippy, minimal query check, warnings-denied
query rustdoc, dependency guard and formatting/diff checks pass. No dependency
changed; whole-workspace runtime tests and Windows execution were not repeated.

## Statistical query aggregates

Var/VarP/StDev/StDevP now execute over finite numeric/Boolean operands using the
existing conversion kernel and query aggregate plan. Each argument evaluates
once, with a budgeted numeric buffer, compensated shifted/scaled centered passes
and per-pass cancellation/work checks. Function-specific NULL/singleton rules,
Double results, finite/range errors and native qualification limits are recorded
in [the statistical contract](STATISTICAL_AGGREGATES.md).

All 287 query tests, 37 selected CLI tests and six Access collection tests pass,
including nine new statistical controls, two CLI controls and a stored collection
control across three priority ACE headers. Four fresh original files pass 180
independent getter/rendering/direct/saved/named/refusal controls with unchanged
hashes. No runtime dependency was added. Native Windows precision/error-order
qualification remains deferred rather than treated as a blocker. Query/CLI/Access
all-target/all-feature Clippy, warnings-denied query rustdoc, minimal-feature query
check, dependency guard and formatting/diff checks pass. Whole-workspace runtime
tests were not repeated.

## Zero TOP roadmap correction

Reference review found Access reports rejecting TOP 0, so zero-count execution
has been removed from the planned missing-feature list. Existing metadata-only
representation and execution refusal remain unchanged; no SQL Server-style
empty-result extension was introduced. Native Windows edge-case qualification
remains deferred. [Evidence and boundary](QUERY_EXECUTION.md#zero-top-counts)
separate the Microsoft reference, community observation and our current behavior.

## Unicode native saved-query prompts

The Access adapter now retains Unicode parameter prompts rather than imposing an
ASCII-only restriction on metadata already accepted by the parser and binder.
Raw/quoted accented, Cyrillic, CJK and supplementary names reconstruct, bind and
execute through native saved/named query paths. Shared matching remains ASCII
case-insensitive and otherwise exact, without normalization or inferred NLS rules.
Existing name/syntax/resource ceilings and control-reference refusals remain.

Original synthetic controls verify all four priority header layouts, plan reuse,
exact composed/decomposed/case-distinct identities, duplicates, missing bindings,
read-only source bytes and publication cancellation. CLI controls exercise saved
and named retrieval. Independent fixtures additionally cover derived reads, UNION
and crosstabs. Native Windows identity/execution qualification remains deferred;
Unicode name ceilings and complete regional semantics are still explicit gaps.

All 56 Access saved-query tests and 30 CLI saved-query tests pass. Four fresh
independent-reader files pass 464 getter/rendering/direct/saved/named/refusal
controls with unchanged hashes. Access/CLI all-target/all-feature Clippy,
warnings-denied Access rustdoc, minimal-feature Access check and formatting/diff
checks pass. No dependency changed; whole-workspace tests were not repeated.

## ICU collation candidate evidence

An original isolated ICU4X 2.3.1 probe now records capability assertions, 1,681
ordered sample comparisons against each General family, counterexamples and
separate direct/kernel timings. Even after trailing-space handling, 118 legacy
and 106 modern comparisons differ. The upstream API lacks substring search and
replaces unpaired UTF-16 surrogates. This does not supply an Access Text/LCID
provider. The dependency remains unadopted; no workspace lockfile or runtime
behavior changed. [Probe evidence](testing/ICU_COLLATION_PROBE.md) separates model
disagreements, raw costs and pending native Windows qualification.

Release execution, isolated warnings-denied Clippy, argument controls and source
format checks pass. Hyperfine retains five measured runs after two warmups per
contender. Different result checksums and budget contracts prevent a replacement
speedup claim. Full workspace runtime tests were not repeated for this isolated
engineering/documentation change.

## Explicit InStr LCID dispatch

InStr now accepts exact structurally valid LCID arguments through a separate
host `find_locale` callback, retaining language and alternate-sort bits. Inputs
use the existing checked numeric conversion; reserved high bits fail before
NULL short-circuiting. The callback shares UTF-16 match bounds validation and
input metering, with a cancellation checkpoint after return. Existing standard
modes and other string functions retain their closed mode set.

This implements caller-supplied locale dispatch, not concrete Windows locale
comparers. CLI ordinal/General policies do not silently substitute their database
semantics for LCID search. Text and Database remain separate contexts; the current
binary function profile uses UTF-16 units. Native Windows comparison stays deferred.

All 232 expression unit tests, eight focused query tests, five CLI tests and two
General-search adapter tests pass. The new controls cover LCID structure/sort-bit
retention, checked rounding, callback routing, provider failures, invalid spans,
NULL/empty rules, parameter reuse, grouped/correlated reads and cancellation after
return. Expression/Query/CLI all-target/all-feature Clippy, minimal-feature checks,
warnings-denied expression rustdoc, dependency guard and format/diff checks pass.
No dependency was added; whole-workspace tests and native Windows comparisons
were not repeated.

## Native collection policy composition

Native sources now publish exact flat value/member text identities from engine
collection declarations. Metadata vectors allocate fallibly and charge the source
budget. Complete collection snapshots must match those descriptors before caching;
decoded FileData has no text policy. Both collection/resolver builder orders work,
while the older homogeneous-policy builder retains its collection restrictions.
CLI `--collation database` now accepts collection opt-ins and uses the same shared
comparison pool as scalar fields and the stored default.

Original synthetic controls cover ACE 2007/16-family headers, contrasting
flat/template/default orders, legacy-versus-2010 element comparisons, membership,
LIKE, grouping, crosstabs, per-member policies, unknown-policy projection/errors,
metadata-only binding and snapshot sort drift. CLI controls check real member
queries, decoded Binary payloads, buffered failures and unchanged source bytes.
These are composition controls, not native Access SQL conformance evidence;
Windows comparison remains deferred.

All 23 focused Access tests and 11 CLI tests pass, including four new adapter
controls and one CLI control covering all three priority ACE header families.
The existing original attachment corpus also passes 162 independent stored-record
model controls in database mode, with all three hashes unchanged. Access/CLI
all-target/all-feature Clippy, warnings-denied Query/Access rustdoc, minimal Access
check, dependency guard, 32 Python tests and format/diff checks pass. Whole-workspace
runtime tests were not repeated.

## Query collection text policies

Sources may now declare aligned optional text-policy identities for multivalue
elements and named collection members. Binding validates and owns both vectors,
including unused fields, and retains identity in expanded slots and hidden
membership items through correlated scopes. All three table execution paths
check complete collection policy metadata before scanning. The existing explicit
resolver handles collection predicates, LIKE, grouping, DISTINCT, ordering,
aggregates and crosstab headings; global overrides remain explicit.

Focused controls use different folded/reverse/ordinal policies to expose fallback,
cover member conflicts and unknown policies, metadata-only binding, malformed
metadata and changes before ordinary/expanded/parent/joined-parent scans. Native
adapter/CLI collection integration follows separately. Windows comparisons remain
deferred; this host policy model is provisional.

All 100 focused query tests pass (15 routing, 19 named-member, 58 multivalue,
8 scalar-policy controls), including seven new collection-policy tests. Query
all-target/all-feature Clippy, minimal-feature check, dependency guard and
format/diff checks pass. Whole-workspace tests were not repeated.

## Native collection declaration sort metadata

The engine now supplies working multivalue and attachment declaration APIs with
exact flat value/member text descriptors, retaining unknown and zero orders.
Completed collection snapshots expose the same metadata without rescanning rows.
Shared scalar declarations keep their existing types; native descriptors remain
engine-owned and select no comparison policy. Original inspectors preserve their
return shapes and use the same qualified walkers with result-width allocation
accounting and fallible reservation, without an intermediate wrapper vector.

Four focused synthetic controls cover contrasting template/flat orders, unknown
metadata, non-Text absence, old/new declaration agreement, borrowed stored values,
read budgets and late cancellation. A guarded reader proves discovery reads no
parent/child records while actual snapshot reads trip the guard. The reviewed
Jackcess source confirms the flat-column mapping reference; this is not native
Access SQL comparison evidence. [Collection metadata scope](NATIVE_TEXT.md#stored-collection-text-declarations)
records the APIs and limits. Query/adapter collection policy integration is still
pending, so native database-mode collection guards remain in place.

All 29 selected native complex/collection tests and 20 Access consumer tests pass,
including the four new declaration controls. ACE/Access all-target/all-feature
Clippy, warnings-denied ACE rustdoc, minimal-feature ACE build, dependency guard,
32 Python tests and format/diff checks pass. Whole-workspace runtime tests were
not repeated; native Windows comparisons remain deferred.

## Native per-field General resolver and CLI composition

`NativeCollationResolver` now supplies exact legacy/2010 field policies plus the
stored database default through one caller-owned comparison pool. Construction,
lookup, full-string comparison, search and LIKE share cumulative quotas across
families and executions; borrow conflicts/cancellation fail explicitly. Providers
borrow inline quota state, introducing no runtime, heap cache or dependency.
Module Option Compare remains an explicit host choice.

`NativeSource::with_database_collation_resolver` configures the guarded scalar
source before binding. CLI `--collation database` now composes this resolver,
retaining 16 MiB/1,000,000 comparison steps shared by all policies. Supported
families may coexist in a query; required unknown field identities and conflicting
expressions fail without fallback. Unknown defaults fail configuration; inert
field projection stays available. Collections remain blocked in both builder
orders until child policies resolve. The older single-policy library builder
retains its explicit homogeneous contract. [Native scope](NATIVE_TEXT.md#stored-database-general-policy)
records limitations, including unqualified native expression precedence.

Original mixed-column fixture/oracle recipes pass 172 independent weight-model/
source controls across Jet 4, ACE 2007 and both ACE 16 file families, sequentially
and with selected text indexes. U+0080/U+0301 provide opposite legacy/2010 order,
exposing accidental default fallback. The existing short-key corpus adds 40
passing controls, including defaults differing from field policies. All twelve
source hashes remain unchanged. These are independent integration/model checks;
native Access SQL/Windows qualification remains deferred.

A controlled equivalent-query Hyperfine comparison over 55 rows reports about
2.7 milliseconds per CLI invocation for both explicit-family and per-field modes,
including process launch/I/O/sorting. Treat it as a tie. Raw samples and an earlier
concurrent-process sample excluded from comparison remain retained. Focused tests
cover pooled quotas, reentrant borrowing, cancellation checkpoints, metadata-only
binding, unknown projection/comparison and separate source I/O budgets. All 133
Access package tests and 51 selected CLI tests pass. Query/Access/CLI all-target/
all-feature Clippy, warnings-denied Access rustdoc, minimal release CLI, dependency
guard, 32 Python tests and format/diff checks pass. Whole-workspace runtime tests
were not repeated. Native qualification remains separate from these checks.

## Explicit per-operation comparer routing

The read executor now accepts a host `CollationResolver` through execution options.
Text/Text comparisons use bound policy provenance; default uses the host comparer,
a single identity resolves exactly, and conflicts fail explicitly. Unknown keys
cannot fall back to a default. Existing global overrides retain their behavior
when no resolver is supplied, and string built-ins retain the explicit host
module/database policy. NULL/type/LIKE-mode guards and eager failures stay explicit.

The same routing covers predicates, joins, grouping/HAVING, DISTINCT/DISTINCTROW,
UNION, ordering/TOP ties, min/max and crosstab pivot/headings. Incremental UNION
comparison uses only contributing branches at that point. Traversal/temporary
vectors and callback boundaries retain query budgets/cancellation, with provider
quotas owned by the host. A new payload-free resolution span records no policy
keys or data. [Routing scope](QUERY_EXECUTION.md#explicit-per-operation-collation-resolver)
records the provisional model. Native resolver/CLI integration and native mixed
precedence remain open; the existing homogeneous database guard stays in place.

Eight focused tests use deliberately different folded/reverse/ordinal providers
to expose global fallback, along with unknown/conflicting policies, fresh built-in
results, correlation, crosstabs, complete-record distinctness, callback quotas and
every observed query cancellation checkpoint. All 394 affected query/Access tests
and 49 selected CLI regressions pass; the final expanded eight-test routing matrix
also passes. All-target/all-feature query/Access/CLI Clippy, warnings-denied query
rustdoc, minimal release CLI, dependency guard, 32 Python tests and format/diff
checks pass. Forty independent file query/source controls pass with all eight
source hashes unchanged. Those file controls regress existing explicit General
policies; they do not qualify native per-field routing. Whole-workspace runtime
tests were not repeated; native Windows qualification remains deferred.

## Bound expression policy provenance

Bound field and output slots now retain `TextPolicy` provenance through pruning,
aliases, derived/named relations and correlated scalar subqueries. The provisional
model merges concatenation/text-capable Add, IIf result branches, Nz alternatives
and corresponding UNION outputs; First/Last/Min/Max retain argument provenance.
Conflicts remain explicit. Built-in/conversion results currently use host-default
provenance; that choice is visible rather than native-qualified.

`PreparedQuery::output_text_policies()` exposes one static policy per known output
without scanning records or selecting comparers. Fixed crosstabs repeat cell
provenance and retain row-key policies; dynamic schemas return None. Policy vector
allocation is fallible and charged; separate bounded visits/depth constrain the
lineage analysis. Global comparer overrides and result values retain their existing
behavior. [Policy lineage](QUERY_EXECUTION.md#bound-expression-text-policy-lineage)
records exact scope. Per-operation routing/native mixed precedence remain open.

Focused goldens cover expression/query boundaries, crosstab widths, correlation,
conflicts, fixed named schemas and unchanged explicit override execution. A
128-case generated expression check compares lineage with an independent set of
contributing field IDs, including fresh-result resets. All 386 affected query/
Access tests and 49 selected CLI regressions pass, along with all-target/all-feature
query/Access/CLI Clippy, warnings-denied query rustdoc, minimal release CLI,
dependency guard, 32 Python tests and format/diff checks. Forty independent native
query/source controls pass with all eight source hashes unchanged. Whole-workspace
runtime tests were not repeated; native Windows qualification remains deferred.

## Stable source text-policy metadata

The query Source contract now supplies optional aligned opaque `TextCollationId`s
for scalar Text fields. Preparation validates width/type declarations and captures
a budgeted owned copy with the complete table schema. Projection pruning retains
it; ordinary, expanded, parent and joined-parent execution checks for changes
before row visitors. Provider errors propagate, and source identity changes fail
as `SchemaChanged`, including through named/derived/union/group/subquery paths.
Existing providers retain an empty metadata default.

The Access adapter now publishes exact field descriptor identities, including
unknown LCIDs/variants/versions, and validates identity consistency in newly read
full/projected native snapshots. Native details remain in that adapter; query code
never decodes the keys or imports an engine/provider. Metadata does not select a
comparer: unknown orders still allow inert scalar projection, existing explicit
comparison controls remain required, and the homogeneous database guard remains.
This is the implemented metadata/stability prerequisite for per-expression
inheritance/resolution, which remains unfinished. See [source policy identity](QUERY_EXECUTION.md#source-text-policy-identity).

Six new focused tests cover malformed declarations, exact opaque keys, provider
errors, changed/unprojected policies, all three collection scan paths, native
unknown descriptors and absence of implicit comparison activation. All 382
query/Access tests and 49 selected CLI regressions pass. Final focused metadata/
collection tests also pass after making the metadata copy explicitly fallible.
All-target/all-feature query/Access/CLI Clippy, warnings-denied query/Access rustdoc,
minimal release CLI, dependency guard, 32 Python tests and format/diff checks pass.
Forty independent cross-format query/source controls pass with all eight source
hashes unchanged. Whole-workspace runtime tests were not repeated. Windows native
qualification remains deferred.

## Explicit stored database collation

Native `query run --collation database` now selects the exact stored General
legacy/2010 descriptor. The Access adapter owns the working
`NativeSource::with_database_collation` service; the CLI only composes it with
comparison budgets and query execution. Scalar schema discovery enforces the
same descriptor on all available Text fields of each bound table, including
unprojected fields. A guarded reader proves selection/binding reads no user
records; unknown/mixed settings fail before row scanning, without guessing or
ordinal fallback. Direct, saved and named retrieval share the policy.

This is a conservative homogeneous scope, not complete mixed-expression policy
resolution. Collection opt-ins remain unsupported in this mode because their
child text descriptors are unresolved; both library builder orders retain that
guard. Existing ordinal/family overrides remain explicit. The new CLI mode
requires native input, retains buffered/payload-free errors and cannot enable
writes. Comparison and source budgets remain separate, with shared cancellation.
All 124 Access package tests and 60 selected CLI tests pass, including seven new
adapter tests and four CLI/saved-query controls. All-target/all-feature Access/
CLI Clippy, warnings-denied Access rustdoc, the minimal release CLI, dependency
guard, 32 Python tests and format/diff checks pass. Whole-workspace runtime tests
were not repeated.
See [stored policy scope](NATIVE_TEXT.md#stored-database-general-policy).

Forty independent controls reuse eight unchanged TextIndexFixture inputs across
Jet 4, ACE 2007 and both ACE 16 file families. A new original read-only Jackcess
metadata oracle supplies exact database/column descriptors; homogeneous queries
match existing independent short-key order, while mixed schemas fail atomically,
both sequentially and with selected text indexes. These are portable metadata/
model checks, not Microsoft Access SQL qualification. Windows comparisons remain
deferred and do not block implementation. Broader locales, full mixed-expression
resolution, child policies and safe text predicate pushdown remain open.

## General UTF-16 LIKE execution

Explicit General legacy/2010 query policies now use the expressions layer's
iterative UTF-16 matcher: unit wildcards, complete weighted literal runs and
single-element lists/ranges, retaining literal spaces and raw surrogate units.
Negated lists use excluded-prefix precedence; token boundaries do not split
partial character weights. These are explicit provisional model choices, not
native Access qualification. Ordinary collations retain their scalar matcher.

General callbacks share cumulative normalization/lookup budgets and cancellation;
expression buffers/transitions retain their own limits. Safe primary-prefix/FST
proofs prune impossible extensions. A 128-unit contains regression now succeeds
without raising budgets; large ambiguous patterns can still fail atomically.
Grouped, correlated, parameterized and native-source/index-selected CLI cases
exercise the same matcher. [Pattern matching](PATTERN_MATCHING.md) records scope.

An independent recursive reference model using reviewed Jackcess raw weights
passes 1,792 controls across both families and ANSI modes. Original licensed
reference source remains temporary, with source/JAR hash checks. Generated ASCII
identity and raw-prefix extension controls provide separate properties.
[Native text evidence](NATIVE_TEXT.md#general-utf-16-like-matching) includes the
bounded harness and measured microseconds per call; twelve family/workload
combinations retain five Hyperfine process samples plus a warm-up. Earlier
unpruned samples remain retained separately. Windows comparisons are deferred.
Final verification passes 596 affected expression/query/Access tests and 30
selected CLI tests. Thirteen filtered native collation tests pass with the
engineering generator intentionally ignored. All-target/all-feature Clippy for
the five affected packages, warnings-denied ACE/Access/expression rustdoc,
minimal release CLI, dependency guard, 32 Python tests and format/diff checks
pass. Whole-workspace runtime tests were not repeated.

Automatic field/database policy resolution, safe native text predicate pushdown,
other locales and broader search/coercion semantics remain open; text P0 is not
complete.

## Derived General LIKE element lookup

`is_general_like_unit` is a working standalone engine recognizer for complete
raw UTF-16 spans equivalent to one unit under the selected General weight model.
It distinguishes expansion equivalents such as `ss`/`ß` from arbitrary words,
retains pattern-literal spaces/surrogates and charges cumulative encoding/lookup
work with cancellation checks before publication. It parses no wildcard syntax.
The lookup now supplies General list/range transitions through the matcher above;
dictionary readiness alone did not establish query behavior or complete text P0.

The selected `fst` 0.4.7 dependency now owns an actual derived dictionary consumer,
with workspace inheritance/defaults disabled, one new locked registry package
and no transitive runtime packages. Source/feature/build-script/license and
headless boundaries are recorded in [DEPENDENCIES.md](DEPENDENCIES.md). Original
regeneration tooling emits immutable General legacy/2010 sets from all raw
units, with fixed SHA-256/CRC checks, exact counts and existing weight-data
provenance/notices. Their total size is 109,185 bytes. Regeneration reproduces
both byte-for-byte; independent hash expectations are not automatically rewritten.

Eleven filtered library tests pass with one intentionally ignored engineering
generator, including the four new artifact/membership/bounds checks and existing
exhaustive independent key digests. All raw units are present and exact distinct
counts exclude extra keys. Thirty-seven selected native comparison/search/index/
projection tests pass. ACE/Access/CLI all-target/all-feature Clippy, warnings-denied
ACE rustdoc, minimal release CLI, the dependency guard, 32 Python tests and
format/diff checks pass. Whole-workspace runtime tests were not repeated.

The validated harness compares FST/BTreeSet/sorted-vector lookups on identical
member and nearby nonmember probes, separately from complete classification.
Standalone samples place complete classification around 0.53 microseconds;
lookup-only samples are tens to low hundreds of nanoseconds. All eight family/
mode combinations also completed five Hyperfine process runs plus one warm-up,
with raw samples retained. Construction is outside the lookup timer but inside
process measurements; no full-query gain or confidence interval is claimed.
[The owning text document](NATIVE_TEXT.md#general-like-element-dictionary) records
the sample values, scope and next matcher work. Windows comparisons remain
deferred under the user's current direction.

## General substring search and consumed replacement spans

The standalone engine now supplies `find_general_text` and explicit forward/
reverse UTF-16 bounds. It selects the earliest/latest valid Unicode start, then
the shortest nonempty substring with equal complete General weights. The Access
adapter implements database-mode `Collation::find` under the same cumulative
budget/cancellation as comparisons. CLI General policies now support database/
default InStr, explicit database InStrRev and database Replace. Unequal-length
matches consume their actual spans; binary defaults and explicit text-mode/LCID
gaps remain unchanged. See [the model](NATIVE_TEXT.md#general-substring-spans).

Needles encode once; boundary tables charge UTF-8/UTF-16 offsets and conservative
primary bounds. Insufficient suffixes are skipped, exponential/binary probing
avoids every shorter endpoint, and append-only primary mismatches prune extensions.
Complete weights retain accents, context-dependent repeats and secondary handling.
The initially expensive 1,025-unit common-prefix miss is retained and now completes
after conservative suffix-bound pruning; no work allowance was raised. Independent
legacy key vectors ignore the tested emoji while General 2010 retains it, so
earliest consumed spans deliberately preserve that model distinction.

Thirty-seven selected engine integration tests and seven filtered library tests
pass, including exhaustive existing UTF-16 key digests and index projections.
Five General search tests cover examples, empty/ignorable needles, UTF-16 bounds,
limits/cancellation and generated agreement with exhaustive unpruned span
enumeration. Seven selected Access tests and twenty selected CLI tests pass;
the CLI search target was extended and rerun to retain both the successful
32-row and failed 64-row long-needle controls. Late cumulative failures publish
no prior rows. The release CLI passes 268 independent short-key position/
replacement controls through the reviewed Jackcess encoder, with no generated
database or copied external implementation. Native Access search qualification
remains separate and deferred by user direction.

All-target/all-feature ACE/Access/CLI Clippy, warnings-denied ACE/Access rustdoc,
minimal release CLI, dependency guard, 32 Python script tests, and formatting/diff
checks pass. Whole-workspace runtime suites were not repeated. The standalone
search benchmark validates expected outputs before timing and accepts Cargo's
harness marker through clap. All six family/workload combinations ran through
Hyperfine with five process runs/one warm-up and retained raw samples. Timings
include resource checks and temporary allocation/destruction; process wall time
also includes launch/scheduling. No comparative speedup is claimed. Native LIKE,
broader text/LCID search, mixed field/database policy resolution, broader locales
and safe SQL text pushdown remain text P0 work.

## Explicit General comparison in queries and CLI

The Access adapter now supplies `GeneralCollation`, connecting the standalone
full-string weight model to the backend-neutral comparison interface. One
caller-issued allocation/work budget and cancellation callback cover all calls,
including reused executions. Typed limits, cancellation and allocation errors
survive adaptation; provider/reentrancy failures return a payload-free typed
error without panics. The adapter supplies no implicit module, text-mode or
substring-search policy. Access now directly inherits the existing expression
crate; the reviewed dependency guard reflects this working consumer, with no
new registry dependency or feature changes.

Read-only CLI `query run` exposes `--collation general-legacy` and
`--collation general-2010` alongside unchanged ordinal. General comparisons share
a separate cumulative 16 MiB/1,000,000-step allowance per run. Filtering, joins,
sort/TOP ties, grouping/HAVING, DISTINCT/UNION, IN/BETWEEN, scalar subqueries and
Min/Max use the selected policy. Inline, native/index-selected and SQLite inputs
use the same host path; predicates remain intact and text pushdown is not enabled.
CLI default/database StrComp modes use the explicit policy. General substring
search remains unsupported; LIKE retains its explicit Unicode-scalar profile.
UPDATE help accepts only ordinal and rejects General before any database I/O.

All 587 tests including doctests pass across expressions/query/Access. Twenty-five
selected CLI tests pass; the General target also passes after adding explicit/
default StrComp checks. Coverage includes full Long Text parameter suffixes,
late cumulative-limit failures without partial stdout, native indexed predicates,
SQLite host comparison, unknown descriptors, cancellation and reentrant callbacks.
All 32 Python script tests, the dependency guard, workspace all-target/all-feature
Clippy, warnings-denied expression/Access rustdoc, minimal release CLI and
format/diff checks pass. Whole-workspace runtime suites were not repeated.

The release CLI passes 24 independent General query/source controls across the
eight existing Jet 4/ACE 2007/2016/2019 General fixtures, preserving all hashes.
Short complete-key order agrees with the independent reader for both sequential
and explicitly indexed queries. This is weight-model integration evidence.
Native Access comparisons remain deferred at the user's direction. Native text
P0 still includes mixed field/database policy resolution, native LIKE/search,
broader locales and safe SQL text-predicate pushdown; it is not marked complete.

## Provisional full-string General weight comparison

The standalone engine now exposes `compare_general_text` for recognized General
legacy/General 2010 descriptors. It processes complete UTF-16 strings and trims
trailing ASCII spaces under an explicit provisional weight model. Long Text
suffixes are retained; internal unprintable positions are widened with checked
arithmetic. No physical prefix/digest or ordinal fallback supplies equality.
Existing native index payloads retain their original compact representation.
Both operands are completely encoded under caller-issued allocation/work limits
and cancellation before their temporary buffers are compared. This working API
does not yet replace CLI/query comparison or implement LIKE.

Five focused tests pass, including pairwise agreement with applicable independent
short-key vectors, 20,000-unit suffix/punctuation checks, generated Unicode order
laws, unknown descriptors, limits and every measured cancellation checkpoint.
Seven filtered library tests retain exhaustive UTF-16 key digests and existing
physical-key properties. Thirty-five selected integration tests pass across
comparison, native indexes, index projections and text descriptors. All-feature
ACE Clippy passes. Native Windows comparisons remain deferred by user direction;
the [full-string model](NATIVE_TEXT.md#provisional-full-string-general-comparison)
is implemented, while native SQL qualification and query integration remain open.

## Native text capture replay

The [native text recipe](testing/NATIVE_TEXT_ORACLE.md#replay-against-tablecraft)
now has a portable CLI replay tool. It validates capture provenance/completeness
before running all 76 plain/indexed query cases under explicit collation and LIKE
policies. Exact displayed typed cells retain NULL/empty/text distinctions,
control escapes, supplementary characters and Long Text suffixes. Ordered results
retain order; DISTINCT uses multisets and accepts different representatives only
when the recorded equality partition explains the complete native DISTINCT
result. Unexplained operation-specific behavior remains unresolved.

Native errors never become passes, even if the CLI fails too. Candidate errors,
partial output, malformed TSV, timeouts, schema/value differences and unresolved
representatives remain separate report outcomes. Input and executable hashes are
rechecked after execution. No runtime collation, dependencies or native SQL
compatibility claims changed. Native text comparison P0 remains open; Windows
capture/replay is unrun. Synthetic replay and capture tests cover the tooling,
not Microsoft Access execution.

All 32 Python script tests pass, including 19 native-capture/replay regressions.
Formatting and diff checks pass; the release CLI accepts the replay command's
explicit TSV/collation/LIKE options. Rust runtime code did not change, so the
workspace runtime suite was not repeated.

## Several hidden members in one WHERE atom

WHERE atoms now support several hidden members: members of one collection/alias
stay on the same child, while independent collections form bounded actual-item
products. Each atom folds to one parent/pair truth and never multiplies output by
matching tuple count. Separate Boolean atoms retain independent folds.
[The explicit contract](MULTIVALUE_QUERIES.md#several-hidden-members-in-one-atom)
records root negation, empty/NULL/absent behavior and native qualification gaps.

Private plan members carry group positions and stable metadata keys. Item
references retain tuple position, frame width and correlated scope depth through
pruning, nested reads and inner single/multiple membership. All selected lists
are validated even when another group is empty or absent; combinations use
position vectors and one temporary tuple. A successful match never bypasses later
errors. Single-member execution remains on its existing path. Source contracts,
parser syntax, registry dependencies, native formats and work ceilings did not
change. CLI help now describes the current joined/mixed/subquery/crosstab profile
instead of retaining its stale unsupported claim.

The complete query package passes 252 tests including its doctest. Six new
multivalue tests and three aligned-member tests cover independent generated
nullable models, zipped children, alias separation, duplicate scalar parents,
empty/NULL/absent owners, nested tuple captures, output aliases, mixed/derived/
crosstab/named scopes, zero-width parents, late errors, malformed providers,
parameter rebinding, metadata changes, budgets and every measured execution
cancellation checkpoint. The final focused run also passes after extending
zero-width and named-tuple cases. Eight targeted Access and five CLI tests pass,
including aligned decoded-data/flags predicates, complete cache reuse and buffered
late checksum errors. All-feature targeted Clippy, warnings-denied query/Access
rustdoc, minimal release CLI and formatting/diff checks pass. Whole-workspace
runtime suites were not repeated.

The release CLI passes 225 independent parent/child controls and 162 attachment
controls across unchanged original ACE 2007/2016/2019 hashes. Thirty-three/twenty-one
controls extend the previous 192/141 with nullable pair products, wrapped/deep
captures, mixed/crosstab results, zipped metadata/data and buffered errors. The
full deeply correlated 100-item product exceeds the existing 100,000-row work
ceiling and is retained as an explicit refusal; a bounded scalar ON domain checks
the same deep scopes positively. No budget was raised or failing input removed.
These remain stored-record/model evidence, not native Access SQL, NULL/negative/
absent rules, cross-collection admissibility, binary representation or error order.
Independent hidden HAVING/ORDER BY, history-member SQL, scale and native evidence
remain open.

## Unprojected collection fields in JOIN ON

ON member references now drive child-row expansion without requiring a member in
output/grouping. Duplicate matching children remain duplicate rows, both outer
joins preserve unmatched expanded inputs, and WHERE siblings filter the same
child. Independent hidden WHERE collections keep the existing parent membership
folds. [The explicit join contract](MULTIVALUE_QUERIES.md#collection-members-in-join-conditions)
distinguishes child-value joins from parent membership. Native Access ON
admissibility/cardinality remains unqualified; this does not add parent folding
inside ON or claim native support from a row model.

Binding and projection pruning share their existing scoped relation-dependency
walker to retain ON fields, aliases and local join offsets. No record scan occurs
while binding, and no runtime join implementation, public source contract,
parser syntax, storage format or dependency changed. Shared budgets, eager errors,
complete caches, selected-index refusals and decoded-data opt-in remain intact.

The complete query package passes 243 tests including its doctest. Six new
multivalue tests and one interleaved-member test cover projection independence,
duplicates, two-collection products, LEFT/RIGHT and empty/NULL inputs, ON versus
WHERE placement, aligned siblings, independent membership, grouped counts,
nested/derived/subquery/crosstab scopes, generated independent models, malformed
providers, parameters, metadata changes, budgets and every measured cancellation
checkpoint. A final 51-test multivalue run also passes after test-only cleanup.
Eight targeted Access and five CLI tests pass, including decoded ON fields,
separate alias membership, complete cache reuse and buffered late payload failure.
Targeted all-feature Clippy, warnings-denied query/Access rustdoc, minimal release
CLI and formatting/diff checks pass. Whole-workspace runtime suites were not
repeated.

The release CLI passes 192 independent parent/child-model and 141 attachment
controls across unchanged original ACE 2007/2016/2019 hashes. Thirty-three/eighteen
new controls extend the previous 159/123 with inner/outer child joins, duplicates,
projection/sibling behavior, hidden membership, grouped/crosstab counts and eager
non-finite/checksum errors. These remain inert getter/model comparisons, not
Microsoft Access collection join/empty/duplicate, decoded-binary or error-order
qualification. Independent hidden HAVING/ORDER BY, history-member SQL and
complete native evidence remain open; multi-member WHERE atoms are implemented
above.

## Scoped subqueries inside collection criteria

Membership WHERE atoms now support scalar subqueries, IN/NOT IN, ANY/ALL/SOME
and EXISTS, including nested correlated item references and inner queries with
their own collections. Joined/mixed, crosstab and fixed named-crosstab queries use
the same scoped profile. [The contract](MULTIVALUE_QUERIES.md#subqueries-inside-membership-criteria)
retains explicit empty/NULL/absence and root-negative rules pending native Access
qualification.

A private item expression records query-scope depth separately from ordinary
fields/output aliases. A feature-local binder traversal replaces only the selected
outer member through correlated reads, inner predicates and joins; independent
derived/named scopes remain separate. Projection pruning retains all other scalar
dependencies. Execution captures an owned item frame only for nested item
references; ordinary and uncorrelated membership keep the borrowed parent-row
path. Frames, copies, inner scans, materialization and cancellation share the
existing ceilings. No parser syntax, public source contract, stored carrier,
dependency or native format changed.

The complete query package passes 236 tests including its doctest. Six new
multivalue tests and one named-member test cover scalar/set/quantified forms,
several captured scopes, inner membership, derived/UNION/grouped/sorted subqueries,
output aliases, joined/mixed/outer/crosstab behavior, stored NULL/empty queries,
cardinality/late errors, independent generated models, parameters, metadata
changes, byte/row/work limits and every measured cancellation checkpoint. The
fixed named-crosstab regression now rebinds a captured item through its subquery.
Eight targeted Access and five CLI tests pass, retaining explicit decoded data,
complete cache reuse and buffered late failures. Targeted all-feature Clippy,
warnings-denied query/Access rustdoc, minimal release CLI and formatting/diff
checks pass; whole-workspace runtime suites were not repeated.

The release CLI passes 159 independent parent-model and 123 attachment controls
across unchanged original ACE 2007/2016/2019 hashes. Thirty/twelve new controls
extend the previous 129/111 with nested correlations/local membership, joined/
mixed/crosstab criteria, decoded NULL tests and eager arithmetic/checksum errors.
These remain inert getter/model comparisons, not Microsoft Access SQL,
NULL/negative/empty-set, decoded-binary or error-order qualification. Hidden
collection HAVING/ORDER BY, two-hidden-member atoms, history-member SQL and
native qualification remain open; ON-only fields now expand children as above.

## Collection membership criteria in crosstabs

Crosstab WHERE clauses now use the explicit parent/mixed membership profile for
wholly hidden collections, including joined and literal fixed-heading named
queries. Binding includes PIVOT dependencies before classifying displayed versus
hidden members, so pivot-only collections expand into aligned children. Matching
hidden items do not multiply cells. [The contract](CROSSTABS.md#collection-membership-criteria)
retains the existing NULL/empty/absent-parent rules and native qualification gaps.

Dynamic discovery evaluates each source-row pivot once before membership, retains
headings from rejected parents, and carries the computed value privately through
matching. Fixed IN mode filters before evaluating retained-row pivots. The shared
parent scanner accepts a private pre-filter callback; ordinary SELECTs retain a
no-op path. No source API, parser syntax, storage format or dependency changed.

The complete query package passes 229 tests including its doctest. Eight new
multivalue tests and one named-member test cover fixed/dynamic/pivot-only,
joined/outer/mixed/named scopes, independent nullable grids, discovery-once
observations, discarded-parent pivot failures/heading limits, parameters, bad
providers, metadata changes, budgets and every measured execution cancellation
checkpoint. A subsequent focused run also checks a provider ignoring discovery
callback failure. Eight targeted Access and five CLI tests pass, covering cached
explicit decoded views, original counts and buffered late decoder errors.
Targeted all-feature Clippy, warnings-denied query/Access rustdoc, minimal release
CLI and formatting/diff checks pass. Whole-workspace runtime suites were not
repeated.

The release CLI passes 129 independent parent-model and 111 attachment controls
over unchanged original ACE 2007/2016/2019 hashes. Thirty/fifteen new controls
extend the previous 99/96 with complete heading domains, fixed order, empty
matches, aligned pivot-only children, mixed/joined counts and buffered failures.
These are inert getter/model comparisons, not native Microsoft Access SQL,
NULL/negation/absence, decoded-binary or error-order qualification. Hidden member
criteria in ON/HAVING/ORDER BY and two-hidden-member atoms remain open,
alongside history-member SQL and native qualification; membership subqueries
are implemented above.

## Displayed children with independent parent membership

Queries now combine displayed/grouped collection children with WHERE membership
on wholly hidden independent collections or aliases. For example, displaying
attachment names while testing a separate tag list retains each matching child
once, regardless of hidden match count. Displayed sibling fields keep aligned
scalar behavior; an atom can also use a displayed child as a scalar input to one
hidden-member fold. [The explicit contract](MULTIVALUE_QUERIES.md#displayed-children-with-independent-parent-membership)
retains separate empty/NULL/absent-parent rules pending native Access evidence.

Binding records only wholly hidden fields as membership inputs without scanning.
Affected leaves scan the sorted union of displayed and hidden fields once per
parent/alias. Complete validated hidden lists use private handles beside child
rows; ordinary and mixed scans share child alignment/expansion code. Parent
identity never depends on scalar equality, and hidden matches do not multiply
rows. Parameters, joins, derived/correlated scopes, grouping and pruning keep their
existing limits. No parser syntax, public source API, dependency or native storage
format changed.

The complete query package passes 220 tests including its doctest, with nine new
mixed-query tests (one generated) plus expanded legacy/named-index-space coverage.
They cover duplicate parents/children, several displayed products, interleaved
members, nullable folds, outer absence, eager late errors, malformed providers,
metadata changes, parameter rebinding, byte/row/work ceilings and every measured
execution cancellation checkpoint. Eight targeted Access tests and five CLI tests
pass, including complete cache reuse, decoded Binary/NULL/empty behavior and
buffered late failures. Targeted all-feature Clippy, warnings-denied query/Access
rustdoc, minimal release CLI and formatting/diff checks pass. Whole-workspace
runtime suites were not repeated.

The final release CLI passes 99 independent parent-model controls and 96
attachment controls across original ACE 2007/2016/2019 inputs, with unchanged
hashes. These extend the previous 78/84 controls with mixed child/parent,
derived/grouped/alias and eager-failure cases. Microsoft Access SQL,
NULL/negation/absent-parent rules, native FileData representation and error-order
qualification remain unrun. Wholly hidden membership in ON/HAVING/ORDER BY,
comparisons between two hidden member fields and history-member SQL remain open; collection crosstabs are implemented above.

## Scalar-parent collection predicates over joins

Scalar-only parent queries now evaluate hidden collection membership over comma
products and INNER/LEFT/RIGHT joins, nested joins and independent derived sources.
Atoms may use scalars from another joined parent or correlated outer scopes;
separate aliases keep separate memberships. Binding records relation-leaf/local
metadata positions independently of scalar projection pruning. Execution owns
complete checked lists and carries private query-local handles beside scalar join
rows, preserving duplicate parents/pairs without child multiplication. The
[joined-parent contract](MULTIVALUE_QUERIES.md#parent-membership-over-joins)
distinguishes absent outer parents (UNKNOWN membership) from present empty lists
and stored NULL. This explicit policy is not native Access qualification.

Six authored/generated query tests cover three-relation and derived/correlated
scopes, duplicate rows, outer absence, independent nullable models, parameter
rebinding, metadata changes, byte/row/work ceilings, every measured cancellation
checkpoint and late errors even under TOP/decisive Boolean operands. Existing
single-table membership tests retain their bounded path. The complete query run
passes 211 tests including its doctest; the final 24-test multivalue target also
passes after extended nested/correlated cases. Eight Access source and five CLI
query tests pass, including joined attachment NULL/metadata criteria and buffered
late decoder errors. Targeted all-feature Clippy, warnings-denied query/Access
rustdoc, minimal release CLI and formatting/diff checks pass. Whole-workspace
runtime suites were not repeated. No public source API, parser syntax,
registry dependency or native storage format changed.

The release CLI passes 78 independent multivalue parent controls and 84 attachment
controls over original ACE 2007/2016/2019 files. The former retain the previous
45 controls and add joined/outer/derived/grouped/parameter/error cases; the latter
add joined metadata/decoded-data membership and late checksum refusals. Original
and disposable-copy hashes remain unchanged by reads. These are explicit stored
record/nullable-model comparisons, not Microsoft Access SQL, absent-parent,
binary-representation or native error-order qualification. Wholly hidden
collection-valued ON conditions and comparisons between two hidden members
remain open; mixed displayed/hidden criteria are implemented above.

## Unprojected siblings of displayed collection members

Queries can now filter/order on an unprojected member of a collection already
projected or grouped, with aligned child behavior in otherwise valid joins,
correlated reads, derived queries and HAVING expressions. For example,
`SELECT Files.FileName FROM Items WHERE Files.FileType='txt'` no longer requires
FileType in output. Binding and repeated projection pruning share bounded sorted
borrowed identities keyed by relation qualifier and collection name. Hidden
dependencies remain in native scan projections; self-join aliases stay distinct.
[The contract](MULTIVALUE_QUERIES.md#unprojected-members-of-displayed-collections)
retains the existing scalar-parent NULL/negative-atom policy. Independent hidden
collections mixed with displayed children now use the separate policy above. No public source
API, parser syntax, dependency or native storage format changed.

Six new feature-local tests include a generated zipped-child model, 256-slot
metadata, multiple aliases, nested pruning/correlation, grouping/sorting/HAVING,
NULL/empty rows, eager failures, invalid providers, parameter rebinding and all
measured execution-cancellation checkpoints. The complete query run passes 203
tests including its doctest; two subsequent focused cases bring verified distinct
query coverage to 205 (the final collection target passes eleven). Eight targeted
Access source tests and five CLI query tests pass. Targeted all-feature Clippy,
warnings-denied query/Access rustdoc, minimal release CLI and formatting/diff
checks pass. Workspace runtime suites were not repeated.

The release CLI passes 78 independent attachment controls plus 45 unchanged
scalar-parent membership controls over original ACE 2007/2016/2019 inputs.
These verify aligned hidden metadata/data predicates, opt-in decoded getters,
late checksum failures and unchanged source hashes. Native Microsoft Access SQL,
empty-set/negative predicates and binary representation remain unqualified.

## Explicit decoded attachment query views

`NativeSource::with_decoded_attachments()` and CLI
`--collections --decoded-attachments` now expose original file bytes as nullable
Binary FileData members beside aligned stored metadata. The feature slice uses
the existing qualified native decoder and a separately published complete cache;
metadata-only binding/queries stay inert. All selected payloads decode before
parent visits, preserving late errors under TOP/filtering. NULL, valid empty
files and invalid wrappers remain distinct. [The contract](NATIVE_QUERIES.md#explicit-decoded-attachment-values)
keeps raw native SQL representation and Windows qualification explicitly open.
No dependency or engine-to-query coupling was added.

Three adapter tests cover all current attachment families, metadata-only binding,
native snapshot reuse, aligned metadata/bytes, count/NULL predicates, opt-in and
index refusals, later malformed data, resource ceilings and early/middle/final
execution cancellation. One CLI regression adds explicit flag/conflict checks,
exact hex/NULL/empty output and buffered late failures. The complete Access adapter
suite passes 107 tests, CLI multivalue queries five, engine attachment suites
twelve, and Python engineering/policy checks 25. Targeted all-feature Clippy,
warnings-denied Access rustdoc, minimal release CLI and formatting/diff checks pass. Workspace runtime
tests were not repeated. An initial broader build ran out of disk; after clearing
generated incremental artifacts, affected checks completed successfully.

The independent original attachment corpus passes 69 controls across ACE 2007,
2016 and 2019, including public decoded getters, maximum/empty files, alignment,
explicit parent criteria, late checksum failures and unchanged source hashes.
The engineering TSV adapter now permits a maximum 64 KiB Binary value's 131,074
hex characters under the existing CLI output ceiling and restores Python's prior
CSV field limit even on errors. Native SQL binary representation and collection
semantics remain unqualified. This slice initially refused hidden unprojected
criteria mixed with displayed members; the subsequent aligned-sibling feature
extends that boundary as described in MULTIVALUE_QUERIES.md.

## Native text comparison capture tooling

The [Windows ACE capture recipe](testing/NATIVE_TEXT_ORACLE.md) now prepares
original Short/Long Text inputs and 38 SQL probes over unindexed/indexed files.
It captures lossless UTF-16, typed results/errors, actual DAO/DLL/Windows builds,
collating orders and unchanged read hashes. The portable validator verifies
recipe/runner provenance, complete case/pair/ID coverage and exact stored values;
native errors remain errors and partial failed results are refused. No native
SQL comparison policy, production dependency or compatibility claim was added.

Twelve focused evidence-integrity tests plus ten existing dependency-policy
tests pass. The PowerShell 7.6.7 parser and seven actual helper checks run through
an isolated .NET host on macOS; these qualify syntax and UTF-16/carrier handling,
not Windows COM or DAO execution. Native Windows execution, physical format
selection, query outcomes and TableCraft/native equivalence remain unrun. No
Rust code changed; Rust runtime/lint/build suites were not repeated.

## Named crosstabs with literal fixed headings

Named saved crosstabs now bind metadata-only schemas from literal Text/signed
integral IN headings. [NAMED_CROSSTABS.md](NAMED_CROSSTABS.md) records supported
composition and remaining dynamic/computed/inline scope. A private read-plan node
uses the existing evaluator, preserves pivot-only fields through enclosing pruning
and retains every inner cell/error. Shared heading validation/integer spelling
keeps binding and runtime names aligned; runtime checks the resulting schema.
Punctuated generated columns now have bracketed qualified dot-field syntax while
object/callable/bang paths retain their guards. No dependency, provider, engine
format, writer or GUI was introduced.

Seven focused query tests cover no-scan binding, repeated pruning, joins/UNION/
derived/correlated reads and another crosstab, parameter scopes/cycles, signed/
text headings, 256/257-column boundaries, dynamic/computed refusals, TOP/ties,
unreturned-cell errors, limits and all measured cancellation checkpoints. One
parser test checks literal qualified fields; native authored layouts and CLI
regressions now exercise named fixed schemas and buffered errors. Parser suites
pass 57 tests including doctests, query suites 199, adapter suites 104 and CLI
saved queries 26. Workspace all-target/all-feature Clippy, minimal release CLI,
warnings-denied SQL/query/Access rustdoc, formatting and diff checks pass. The full
workspace runtime suite was not repeated.

Four fresh local Jet 4 / ACE 2007/2016/2019 files add nineteen inert definitions
to the fourteen-query crosstab baseline. Independent getters/rendering and explicit
expected grids pass 380 direct/saved/named/getter/refusal controls for dependencies,
punctuated headings, parameters, joins, correlation, nested reads/UNION/crosstabs,
inner TOP/ties and cycle/later-cell failures. Existing baseline/TOP inputs retain
320/440 controls. Sources remain unchanged; [fixture records](FIXTURES.md#named-fixed-crosstab-inputs)
retain provenance/hashes. Native Windows Access execution, heading/type/collation/
parameter rules, error order and save/reopen remain unrun. The backend roadmap
remains incomplete.

## Literal parameter prompts and bracketed CLI assignments

Bracketed punctuation now remains a single literal name in parameter declarations
and unqualified expressions. [PARAMETER_PROMPTS.md](PARAMETER_PROMPTS.md) describes
dotted-field distinction, object/member/call guards, native raw/quoted prompt versus
control-reference handling, and bracketed CLI assignments for names containing
equals signs. Type/length checking, field precedence, named scopes, shared limits
and buffered publication remain. No dependency, engine format, provider, writer or
implicit interactive prompt was added.

Four parser tests include generated prompts and byte/token/role guards. Four query
tests cover checked plan reuse, dotted fields, field precedence, nested literals
and named scopes. Five adapter tests cover unchanged native names/rows, four header
layouts, unsupported control/Unicode names, duplicate and resource/cancellation
checks plus generated prompts. Three CLI tests cover inline/saved/named paths,
assignment delimiters, unchanged text values and missing/malformed/duplicate
bindings. Parser suites pass 56 tests including doctests, query suites 192, adapter
suites 104 (saved-query target 55) and CLI saved queries 24. Workspace all-target/
all-feature Clippy, minimal release CLI, warnings-denied SQL/query/Access rustdoc,
formatting and diff checks pass. The full workspace runtime suite was not repeated.

Four fresh local Jet 4 / ACE 2007/2016/2019 files add thirteen inert definitions to
our fourteen-query baseline. Independent raw getters/rendering and explicit rows
pass 264 direct/saved/named/getter/refusal controls, including literal bang/dot/
grave/equals prompts, code-looking literal names, nested reads, UNION/crosstabs
and a deferred control reference. Source bytes stay unchanged;
[fixture records](FIXTURES.md#literal-parameter-prompt-inputs) retain provenance and
hashes. Native Windows Access prompt/Unicode/regional/error-order and save/reopen
qualification remain unrun. The broader backend roadmap is still incomplete.

## Native saved-query fragment ordering

Saved SELECT, UNION and crosstab reconstruction now share a stable borrowed
ordered view. [NATIVE_SAVED_ORDERING.md](NATIVE_SAVED_ORDERING.md) records attribute/
unsigned-key ordering, physical ties, whole-definition NULL behavior, native carrier
limits and pending qualification. A cancellable/work-bounded merge sorts references
without altering native metadata. Clause/source/parameter extraction uses that one
view. No engine format, write path, query model or dependency changed.

Five feature tests cover ordered clause families, arbitrary valid fixture key bytes,
prefixes/ties/empty/NULL behavior, native nullable joins, every measured sort/
publication cancellation checkpoint and resource ceilings. One CLI test compares
physical inspection with saved/named output header and row order. Adapter suites
pass 99 tests including doctests (saved-query target 50); CLI saved queries pass
21. Workspace all-target/all-feature Clippy, minimal release CLI and example
builds, warnings-denied Access rustdoc, formatting and diff checks pass. The full
workspace runtime suite was not repeated.

Four original local Jet 4 / ACE 2007/2016/2019 files add eleven inert definitions
to the fourteen-query forest baseline. Independent getters/rendering and explicit
row controls pass 252 direct/saved/named/getter/refusal comparisons. Fresh originals
satisfy native system-index key constraints; copy-only duplicate/NULL key cases
deliberately disagree with that index and establish reader behavior, not native
admissibility. Original/copy bytes stay unchanged; [fixture records](FIXTURES.md#native-fragment-ordering-inputs)
retain provenance and hashes. Earlier SELECT, parameter and compound-join recipes
pass on their unchanged originals through both hosts, with ordering copies now
compared positively and other refusals retained. Actual Windows Access execution,
index/admissibility rules and save/reopen remain unrun; the backend roadmap is
still incomplete.

## Already-bracketed native saved names

Stored table/alias/projection/TRANSFORM/ASCII parameter names now accept one
complete bracket pair and retain their literal owned spelling.
[NATIVE_SAVED_NAMES.md](NATIVE_SAVED_NAMES.md) describes delimiter normalization,
raw join-label matching, logical duplicate detection and remaining qualified-name/
prompt/Unicode limits. A borrowed helper preserves original fragments and does not
introduce name constructors, dependencies, execution authority or write behavior.

Five feature tests include generated symbol-bearing aliases, expected syntax,
native parameter rows and malformed/injection/duplicate checks. One crosstab test
and one CLI test cover shared aliases, saved/named execution, prompt binding and
missing-value buffering. The Access adapter suite passes 94 tests including
doctests (saved-query target 45); CLI saved queries pass 20. Workspace all-target/
all-feature Clippy, minimal release CLI, warnings-denied Access rustdoc, formatting
and diff checks pass. The full workspace runtime suite was not repeated.

Four original local Jet 4 / ACE 2007/2016/2019 inputs add ten inert name definitions
to our fourteen-query forest baseline. Independent renderer and explicit row
controls pass 160 reconstruction/direct/saved/named checks, including reserved/
symbol names, raw quoted join labels and single Unicode/derived aliases. Earlier
forest/derived originals retain their 240/176 controls: 576 combined. Sources stay
unchanged; [fixture records](FIXTURES.md#stored-identifier-inputs) retain provenance
and hashes. Native Windows Access execution, naming/collation and save/reopen
remain unrun. The broader backend roadmap remains incomplete.

## Native stored derived-query sources

Native saved SELECT/crosstab Table rows with NULL Name1 and a read Expression now
reconstruct as derived sources. [NATIVE_SAVED_DERIVED.md](NATIVE_SAVED_DERIVED.md)
describes original/guarded read equality, anonymous/aliased roots, joined sources
and execution boundaries. Named tables with expression qualifiers remain remote
refusals. The adapter moves a parser-owned boxed read, retains original fragments
and verifies its complete final source tree without executing content. No package
dependency, engine format, backend-neutral query model or writer changed.

Four focused feature tests and one crosstab test cover syntax, nested SELECT/UNION,
comments, anonymous/aliased roots, injection/action/remote refusals, native nullable
join results, budgets and every measured cancellation checkpoint. CLI coverage
checks saved/named execution and buffered later arithmetic/UDF failures. The Access
adapter suite passes 88 tests including doctests (saved-query target 39); CLI saved
queries pass 19. Workspace all-target/all-feature Clippy, minimal release CLI,
warnings-denied Access rustdoc, formatting and diff checks pass. The full workspace
runtime suite was not repeated.

Four original Jet 4 / ACE 2007/2016/2019 files add eleven inert definitions to the
fourteen-query forest baseline. Independent rendering and explicit expected rows
pass 176 direct/saved/named reconstruction/execution/refusal controls for nested
reads, UNION, aggregates, anonymous/aliased roots, parameters and named dependencies.
Lateral/owner/UDF execution refuses without stdout. All source bytes remain
unchanged; [fixture records](FIXTURES.md#saved-derived-source-inputs) record provenance
and hashes. The unchanged forest originals also retain their 240 controls. These
are independent metadata/row checks; native Windows Access qualification remains
unrun and the broader backend roadmap is incomplete.

## Native saved join forests

Native saved SELECT and crosstab reconstruction now supports sixteen declared
sources and sixteen ordered join fragments, including disconnected comma products,
nested INNER/LEFT/RIGHT trees and later predicates attached to inner or matching
ancestor joins. [NATIVE_SAVED_JOINS.md](NATIVE_SAVED_JOINS.md) records placement,
orientation, ceilings and remaining qualification. The adapter uses a borrowed
arena, guarded parsing and complete topology/condition verification; native I/O,
query contracts and package dependencies are unchanged. Inert inspection and
explicit saved/named retrieval retain their existing boundaries.

Five focused forest tests cover expected trees, sixteen-source membership and
condition ceilings, metadata/injection failures, native nullable rows and every
measured reconstruction cancellation checkpoint. A crosstab regression checks the
shared forest, and CLI tests check saved/named retrieval plus buffered UDF refusals.
The Access adapter suite passes 83 tests including doctests, saved-query target 34
and CLI saved-query target 18. Workspace all-target/all-feature Clippy, minimal
release CLI and example builds, warnings-denied Access rustdoc, formatting and diff
checks pass. The full workspace runtime suite was not repeated.

Four original local/untracked Jet 4 / ACE 2007/2016/2019 files contain fourteen
definitions each. Independent renderer, getter and Python row models pass 240
assembly/execution/refusal controls through direct/saved/named CLI paths. Existing
twenty-eight-definition compound-join originals also pass on Jet 4 and both ACE 16
layouts, with seven refused mutations and one newly accepted reversed-endpoint
copy per file through both hosts. Original/copy bytes remain unchanged;
[FIXTURES.md](FIXTURES.md#saved-join-forest-inputs) records provenance and hashes.
Actual Windows Access join admissibility, rows, errors and save/reopen remain
unrun. This completes the former two-source restriction, not the backend roadmap.

## Crosstab TOP counts, percentages and ties

Positive crosstab TOP/PERCENT now execute through the same final row limiter as
ordinary SELECT. The helper borrows complete sorted output, rounds percentage
counts upward and retains full ORDER BY ties without cloning rows or allocating
another result buffer. [CROSSTABS.md](CROSSTABS.md#top-row-groups) records row/column,
error, resource and native qualification boundaries. TOP 0, DISTINCT/owner options
and named/derived crosstab schemas remain explicit gaps.

TOP is applied after WHERE/HAVING, all row totals/cells and sorting. It does not
shrink heading discovery or fixed columns, bypass source/output budgets, push down
source reads or suppress omitted-group errors. Ordinary SELECT retains its existing
selection semantics while sharing the helper; explicit checks surround truncation
and tie comparison failures/cancellation remain visible. The native adapter now
executes retained stored counts/PERCENT rather than silently dropping options.
No package dependency, parser model, engine format or write behavior changed.

Four query tests cover deterministic count/percentage/tie cases, independent
nullable rank properties, errors in omitted cells/totals/HAVING/keys and provider
failure/final tie cancellation. Native encoded count/percentage/tie tests cover
four current header layouts and unchanged source bytes. A CLI regression checks
saved TOP and buffered later-cell failures. Query suites pass 188 tests including
doctests, latest crosstab batch 16, native saved-query batch 28 and CLI saved-query
batch 17. Workspace all-target/all-feature Clippy, minimal release CLI build,
warnings-denied query rustdoc, formatting and diff checks pass. The full workspace
runtime suite was not repeated.

SavedCrosstabFixture --top adds six original inert definitions to the fourteen-query
baseline, on fresh Jet 4 / ACE 2007/2016/2019 files. Independent raw getters control
stored counts/PERCENT that Jackcess omits from its renderer. The extended recipe
passes 440 assembly/execution/getter/refusal controls; original inputs still pass
320. A separate Python nullable-rank recipe passes 782 controls for ASC/DESC,
secondary keys, NULL ties, count/percentage ceilings and buffered refusals. All
original/mutation-copy bytes remain unchanged; [FIXTURES.md](FIXTURES.md#crosstab-top-inputs)
records hashes and reproduction. These are independent metadata/model checks;
native Windows Access crosstab TOP, percentage/tie and error-order evidence remains
unrun. The broader backend roadmap remains incomplete.

## Native saved crosstab reconstruction and explicit retrieval

Native crosstabs now reconstruct through reconstruct_saved_read and
NativeSource::read_saved_query and feed the existing top-level grouped/pivot
executor through query run --saved. [NATIVE_SAVED_CROSSTABS.md](NATIVE_SAVED_CROSSTABS.md)
records flag roles, expression/ordering rules, stored options, evidence and remaining
native scope. No inventory/open path activates a query or application content.

The adapter validates the crosstab family/type, single transform/pivot roles,
ordinary SELECT columns/groups and optional matching pivot-group metadata. SELECT
and crosstab now share clause composition without modifying native definitions.
Parameters, qualified joins, WHERE/HAVING, aliases, fixed IN headings, original
identity/fragments, shared budgets and publication cancellation remain explicit.
The SQL model/parser/formatter now retain aggregate AS aliases and trailing owner
syntax. No new engine, foundation, runtime, write path or dependency was added.

Stored SELECT options remain visible. Reviewed Jackcess 5.0.4 omits crosstab TOP
from SQL rendering despite retaining its flag/count; TableCraft preserves that
option and currently refuses execution rather than discarding it. Owner/UDF and
other unsupported forms remain blocked. Named/derived crosstab schemas, omitted
TRANSFORM, wider role layouts/joins/parameters and native heading/type/alias rules
remain incomplete. Dynamic discovery still uses the current pre-WHERE source
domain; this is not represented as native Access qualification.

One parser regression checks alias/owner round trips, and one query regression
checks generated headings and owner refusal. Four adapter tests cover role/option
preservation, four header layouts, explicit fixed/dynamic execution, malformed
metadata/statement refusals, budgets and early/late cancellation. Two CLI tests
cover inert printing, explicit execution and buffered, payload-free failures. The
Access suite passes 76 tests, latest saved-query batch 27, SQL suite 52 including
doctests, query crosstab batch 12 and CLI saved-query batch 16. Workspace all-target/
all-feature Clippy, minimal release CLI build, warnings-denied Access/SQL rustdoc,
formatting and diff checks pass. The full workspace runtime suite was not repeated.

Four fresh Jet 4 / ACE 2007/2016/2019 files contain fourteen original inert
SELECT/crosstab definitions each. The independent recipe passes 320 raw metadata,
assembly, getter/grid, saved/direct and refusal controls, including raw fixed-field
mutations on copies. The existing saved UNION recipe additionally passes 268
regression controls. Source and mutation-copy bytes remain unchanged; fixtures,
original recipes and hashes are recorded in [FIXTURES.md](FIXTURES.md#saved-crosstab-inputs).
The external reference assembles syntax and reads scalars; it never executes SQL.
Windows Access parsing/execution and wider native behavior remain unrun.

## Native saved UNION reconstruction and retrieval

Saved native UNION definitions now reconstruct through a separate labelled-fragment
adapter and the new reconstruct_saved_read / NativeSource::read_saved_query APIs.
SavedRead/SavedReadError/NativeSavedReadError are the primary broad result/error
names; existing SELECT type/error names remain aliases and SELECT-only entry
points retain their family restriction. [NATIVE_SAVED_UNIONS.md](NATIVE_SAVED_UNIONS.md)
records layout, parsing, resource, execution and native qualification boundaries.

The adapter checks family/type agreement, two exact branch labels, duplicate flags,
owner syntax, ordered parameters and final sort metadata. Wrappers and a complete
composition parse reject statement/action/declaration/ordering injection without
repairing fragment boundaries. Mixed UNION/ALL association remains textual and
bounded. Original scoped identity, flags, strings and row order are retained.
Native/query/CLI work shares existing limits and buffering; no storage, evaluator,
runtime, write path, source pushdown or dependency was added.

Explicit query saved / query run --saved and named-source retrieval now use the
broader APIs. Inert queries can retain unsupported calls/owner flags but execution
still rejects them. Existing named scopes, type checks, dependency cycles and
planning/source limits remain in force. Header inspection and file opening do not
activate queries. Native saved crosstabs and wider native fragment/type behavior
remain incomplete; no generic UNION carrier promotion is inferred.

Five authored adapter tests cover owned syntax, reordered labels, mixed chains,
parameters, four header families, same-source/named execution, metadata and action
refusals, parser/work/byte/row limits and early/late cancellation. Two CLI tests
cover inert output, explicit/named execution and buffered refusals. The Access
suite passes 72 tests, latest focused adapter regressions 25, named/retrieval
regressions 34 and the saved-query CLI batch 14. Workspace all-target/all-feature
Clippy, minimal release CLI build, warnings-denied Access rustdoc and formatting/
diff checks pass. The full workspace runtime suite was not repeated.

The original SavedUnionFixture and read-only SavedReadOracle recipes supply
fourteen definitions each on fresh Jet 4 / ACE 2007/2016/2019 files. The independent
comparison passes 268 assembly/execution/getter/refusal controls, including copy-only
raw metadata mutations, with original bytes unchanged. [FIXTURES.md](FIXTURES.md#saved-union-inputs)
records sources/hashes. The oracle assembles SQL and reads scalar inputs; it does
not execute SQL. Native Microsoft Access parsing/execution remains unrun, including
wider metadata layouts, type promotion and native error order.

## Explicit Unicode LCase/UCase

LCase and UCase now execute over Text/NULL through the parser-independent text
kernel, scalar evaluator and generic read-only TextCall path. A validated explicit
CaseLocale selects ICU Unicode casing; the CLI exposes --case-locale and the
optional SQLite worker owns request-local settings. No OS language discovery,
formatting or SQL collation is inferred. [TEXT_CASING.md](TEXT_CASING.md) records
context/expansion, NULL/type/arity behavior and remaining native acceptance gaps.
Both tracker rows remain Partial; native Windows NLS/ACE comparison is unrun.

The feature uses borrowed recipes and streamed Writeable output with original
host errors retained through a fmt::Write bridge. Input/context work, output
allocation, expansion and cancellation are bounded. Greek uppercase reserves
conservative extra context scans, which can exhaust work before the text byte
ceiling. Existing typed TextCall binding retains fields, grouped/aggregate and
correlated expressions and unused computed errors. No UPDATE or source pushdown
was added; implicit locale policy remains deferred with sys-locale.

Workspace dependencies now declare icu_casemap 2.3.0 (compiled_data),
icu_locale_core 2.3.0 (alloc) and writeable 0.6.4. The 23 added locked packages,
licenses, toolchain requirements, compiled-data and unsafe boundaries are recorded
in DEPENDENCIES/DEPENDENCY_REVIEW/LIBRARY_POLICY. Existing versions are unchanged;
parser-free feature inspection activates no serde/datagen/serialized data loader.
Reviewed upstream ULE advisories are fixed by locked zerovec 0.11.8 and
zerovec-derive 0.11.6. No performance improvement or exhaustive security audit is
claimed from adoption.

Four kernel tests cover Unicode/context/language goldens, shape/policy validation,
NULL/type behavior, generated ASCII mappings and injected sink errors. An evaluator
test checks output expansion, cumulative bytes, work and cancellation. Two query,
two CLI and one worker tests cover execution-local policy, recovery, grouping,
correlations and buffered errors. Expression/query suites pass 406 tests including
doctests; parser-free expressions pass 178, selected CLI regressions seven and the
worker regression one. Workspace all-target/all-feature Clippy, minimal release CLI
build, warnings-denied expression/query rustdoc, formatting/diff checks, the
12-package boundary guard and ten guard tests pass. The full workspace runtime
suite was not repeated.

The [independent recipe](testing/compare_casing.py) passes 714 stable-Unicode,
language-golden, refusal and native-row controls against the current release.
Sequential/indexed projections use unchanged Jackcess-decoded source inputs.
Native Access locale/version mappings, coercion, aliases and error ordering still
need Windows evidence; independent Unicode/reader results do not qualify them.

## Explicit clock functions and CLI local-clock host

Date(), Time() and Now() now use caller-issued Clock providers through the
parser-independent date slice, scalar evaluation and owned query clock nodes.
ClockReading validates the legacy calendar envelope and truncates fractions before
projecting date/time components. Libraries do not read the system clock or timezone;
the CLI supplies a lazy fallible Jiff local-clock provider or --clock fixed reading.
[CLOCK_FUNCTIONS.md](CLOCK_FUNCTIONS.md) records precision, provider, volatility and
remaining native scope; all three built-ins remain Partial.

Every executed call reads once. Binding captures no reading, provider errors are
not retried/fallbacked, and eager/unused-computed failures remain visible. Group-key
eligibility now has a separate bounded structural walk: clock keys can execute
without treating separate clock expressions as equivalent. Field-bearing volatile
group projections retain a qualification gap. Pure-only DISTINCTROW/crosstab/parent
criteria gates remain restricted. Clock providers are request-local in the optional
worker; no UPDATE/system-clock mutation or active database content is introduced.

The CLI inherits Jiff with tz-system, tzdb-zoneinfo and tzdb-bundle-platform. The
lock adds reviewed jiff-tzdb-platform 0.1.3 and jiff-tzdb 0.1.9, both Unlicense OR MIT,
MSRV 1.70. Existing package versions/shared Jiff defaults remain unchanged. Standalone
expression feature trees retain civil-only Jiff; configured Windows closure contains
platform timezone data. No Windows build, GUI/network runtime, serializer or fixture
was added. Development/library/dependency records describe this consumer boundary.

Three kernel tests cover projections, truncation, endpoints, epoch signs and callback
failure/no-retry behavior; an evaluator test checks limits/cancellation before calls.
Five query tests cover inert binding, repeated/correlated reads, grouping/equivalence,
unused failures and field-name preservation. Three CLI tests cover fixed readings,
strict offset-free input, lazy/invalid timezone behavior and a UTC system-day check.
A worker test covers request isolation/recovery. Expression/query suites pass 398
tests, parser-free expressions 174, selected CLI regressions seven, latest query
regressions five and the worker regression one. Workspace all-target/all-feature
check/Clippy, minimal release CLI build, warnings-denied expression/query rustdoc,
formatting/diff checks, the 12-package boundary guard and ten guard tests pass.
The full workspace runtime suite was not repeated.

The [independent recipe](testing/compare_clock.py) passes 2,684 Gregorian/fixed-clock/
refusal/native-row controls against the current release binary. Sequential/indexed
projections use unchanged Jackcess-decoded inputs. Native Windows Access remains
unrun, including native timezone rules, clock invocation timing, precision, calendars
and volatile grouping/error ordering. Independent model/reader and feature-tree
checks do not establish native compatibility.

## Explicit fixed-place numeric formatting

FormatNumber, FormatPercent and FormatCurrency now return bounded Text under
explicit invariant/en-us/de-de profiles, with checked optional digits/tristate
controls and missing-slot defaults. Exact fixed inputs reuse rust_decimal rounding;
finite IEEE inputs share Str/CStr's seven/fifteen-digit normalization with a
separate exponent. Percentage display can exceed stored carrier range without
constructing an invalid stored value. [Formatting contracts](STYLED_NUMBER_FORMATTING.md)
record profile defaults, half-away output rounding, zero rules, the 255-place
ceiling and continuing native/NLS/Variant limits; all three calls remain Partial.

The feature slice separates option validation, bounded magnitude preparation and
streamed grouping/adornments. Text conversion has private charged scratch; display
allocation/work/cancellation remains metered. Existing typed Text nodes retain all
value/options through binding, grouping, aggregates and correlations. Profiles
remain execution-local, including worker reuse after failure. CLI help and existing
dependency records describe the additional working consumers. No registry package,
feature, serializer, core carrier, fixture, UPDATE support or pushdown was added.

Five kernel tests cover defaults/overrides, exact/floating bounds, maximal Decimal
percentages, 255 places, scratch isolation, refusals and injected host failures,
including generated Currency checks against independent integer rounding. An
evaluator test covers scratch/output bytes, work, input size and cancellation.
Two query and two CLI tests cover fields, grouping, aggregates, correlations,
optional slots, profiles and buffered errors; a worker regression checks request
isolation/recovery. The text-kernel batch passes 60 tests, parser-free expressions
171, selected query/CLI batches eight/seven, the worker regression one and the
focused latest formatting/evaluator batch six. Workspace all-target/all-feature
Clippy, minimal release CLI build, warnings-denied expression/query rustdoc,
formatting and diff checks pass. The full workspace runtime suite was not repeated.

The [independent recipe](testing/compare_styled_number.py) passes 1,790 Decimal/
normalized-float/refusal/native-row controls. Existing CStr and numeric-text recipes
pass 3,908 and 3,936 controls after sharing the float helper. Sequential/indexed
projections use unchanged Jackcess-decoded source inputs. Native Windows Access
remains unrun, including regional settings, spacing, coercion, precision/rounding
and error ordering. Independent models/readers do not qualify native SQL behavior.

## Standalone Weekday execution

Weekday now returns Int16 Gregorian relative weekdays or NULL through scalar,
bound/nested and qualified saved SELECT execution. Omitted first-day options use
Sunday, zero resolves caller-issued context, and concrete SQL options override it.
Existing date carriers share the calendar kernel; numeric/Boolean/Text inputs use
the checked CDate profile. [Weekday contracts](WEEKDAY.md) record exact extended
components, current legacy rounding, optional slots and remaining native gaps;
the built-in tracker marks the function Partial.

An owned Weekday node preserves both date and optional first-day dependencies
through pruning/remapping, grouping identity, aggregate detection, correlations
and qualified multivalue parent criteria. The evaluator supplies bounded input,
work/cancellation and private charged normalization. Plans capture no policy and
worker settings remain request-local. No dependency, fixture, predicate pushdown
or UPDATE support was added.

Four kernel tests include independent generated serial-day controls, defaults,
context/overrides, NULL/invalid options, shared conversions, date-boundary precision
and host failures. An evaluator regression checks input/work/cancellation limits.
Two query tests cover fields, grouping, aggregates, correlations and settings reuse;
a multivalue test checks hidden member dependencies. CLI and worker tests cover
optional slots, eager failures, buffered output, isolation and recovery. Expression/
query suites pass 381 tests; parser-free expressions pass 166, selected CLI
regressions nine, and the worker and later evaluator regressions one each.
Workspace all-target/all-feature check/Clippy, minimal release CLI build,
warnings-denied expression/query rustdoc, formatting and diff checks pass. The full
workspace runtime suite was not repeated.

The [independent recipe](testing/compare_weekday.py) passes 2,666 Gregorian/serial/
refusal/native-row controls. The existing week-policy recipe passes 1,368. Native
sequential/indexed projections use unchanged Jackcess-decoded source inputs.
Native Windows Access remains unrun, including full NLS/calendar/Variant coercion,
precision and error ordering. Independent models/readers do not establish native
SQL compatibility.

## Shared host week context for DatePart and DateDiff

DatePart/DateDiff now resolve explicit zero week options through the host setting
already used by WeekdayName and a new validated FirstWeekOfYear. Omitted options
retain Sunday/Jan-1 defaults; concrete SQL values override host context. The public
WeekContext and context-aware kernel entry points retain the original context-free
functions without behavior changes. [Week settings](WEEK_SETTINGS.md) document
independent day/week resolution, NULL/error behavior and remaining native limits.

Evaluator, query and optional worker settings carry request-local first-day and
first-week values. CLI eval/run expose --first-week-of-year alongside the existing
first-day flag, with parser validation and inert parse/inspect boundaries. Eval
controls now use a typed struct instead of an expanding positional argument list.
No dependencies, fixtures, predicate pushdown or UPDATE support were added.
DateDiff w non-Sunday anchors remain explicitly unsupported, and the existing
DatePart final-Monday anomaly is preserved rather than replaced with ISO rules.

Four kernel tests cover first-week validation, zero resolution, defaults, overrides,
incomplete context, NULL/options, signed boundaries and every day/week combination
around year boundaries. Two query and two CLI tests cover fields, grouping,
correlations, plan reuse, flags, optional slots and buffered errors. The worker
regression checks settings isolation and reuse after missing context. The date
kernel batch passes 35 tests, parser-free expressions 162, selected query/CLI
batches nine each and the worker regression one. Workspace all-target/all-feature
check/Clippy, minimal release CLI build, warnings-denied expression rustdoc,
formatting and diff checks pass. The full workspace runtime suite was not repeated.

The [independent week recipe](testing/compare_week_context.py) passes 1,368 Python
calendar-membership, boundary, refusal and native-row controls. The existing
calendar-name recipe passes 1,922 after the CLI control changes. Sequential/indexed
projections use unchanged Jackcess-decoded row inputs. Native Windows Access remains
unrun; these independent models/readers do not qualify NLS, full coercion, error
precedence, alternate calendars or DateDiff w anchor behavior.

## Explicit MonthName and WeekdayName output

MonthName and WeekdayName now emit bounded full/short English and German names
through the existing typed Text node. Jiff supplies English names; shared German
labels also serve FormatDateTime. Numeric ordinals/abbreviation operands use the
current checked conversion profile. Optional slots preserve defaults without
coercing NULL to omission. [Calendar-name contracts](CALENDAR_NAMES.md) retain
regional/calendar/Variant/native gaps; both calls are Partial in the tracker.

A validated FirstDayOfWeek and --first-day-of-week provide WeekdayName's system/
default context without reading host preferences or inferring it from date syntax.
Explicit SQL first-day values override that context. Settings remain request-local
across evaluators, bound/nested queries and the optional worker. DatePart/DateDiff
now share this setting for explicit zero options under the host week contract. Library policy reiterates that
sys-locale discovers language identifiers and cannot replace calendar data or
first-day policy. No dependency or fixture was added.

Four kernel tests exhaust month names and every first-day/weekday combination,
including abbreviations, overrides, checked coercion, NULL/domain failures and host
errors. Separate tests cover first-day validation and evaluator limits. Two query
and two CLI tests cover fields, grouping identity, correlations, optional slots,
flag validation and failure buffering; a worker test checks settings isolation and
reuse after missing context. Expression/query suites pass 368 tests; parser-free
expressions pass 158, selected CLI regressions nine and the worker regression one.
Workspace all-target/all-feature check/Clippy, the minimal CLI release build,
formatting and diff checks pass. The full workspace runtime suite was not repeated.

The [independent recipe](testing/compare_calendar_names.py) passes 1,922 Gregorian/
refusal/native-row controls, and the existing date-format recipe passes 3,238 after
shared-label extraction. Sequential/indexed native projections use Jackcess-decoded
inputs and preserve source bytes. Native Windows Access remains unrun, including
NLS spellings, default numbering, coercion and error precedence. These model/reader
checks do not qualify native SQL compatibility.

## Explicit FormatDateTime named output

FormatDateTime now implements selectors 0–4, the omitted General default and a
missing optional final slot through the shared typed Text node. The parser-free
date-format slice owns checked input, five named modes and a shared renderer;
CStr retains its General behavior and public DateText export. Explicit iso/en-us/
de-de profiles supply date/time patterns, with intrinsic 24-hour short time.
Numeric/Boolean/Text inputs reuse CDate policy; extended dates retain components.
Input scratch and output are charged separately, preventing normalization bytes
from entering results. [Named-format contracts](DATE_FORMATTING.md) document
rounding, profile choices and continuing native/regional/Variant gaps. The
[built-in tracker](ACCESS_BUILTINS.md) marks the function Partial.

Five kernel tests cover modes, carriers, rounding, scratch isolation, selector
conversion, long-date buffer bounds and host failures. An evaluator regression
checks allocation/work/input/output limits and cancellation. Two query tests cover
fields, grouping identity, correlations and per-execution settings; two new CLI
tests cover both execution paths, optional slots and buffered errors. A worker test
checks request-local input/output settings and reuse after missing context.
Expression and selected query batches pass; parser-free expressions pass 153 tests,
nine selected CLI regressions and the worker regression pass, and six focused
formatting tests cover the latest additions. Workspace all-target/all-feature
Clippy, the minimal release CLI build, expression rustdoc, formatting and diff
checks pass. The full workspace runtime suite was not repeated.

The new [independent recipe](testing/compare_date_format.py) passes 3,238 Gregorian/
weekday/refusal/native-row controls against the release binary; the existing CStr
recipe passes 3,908 controls after renderer sharing. Sequential/indexed projections
use unchanged Jackcess-decoded source inputs. Native Windows Access remains unrun;
these results do not establish native SQL/formatting compatibility. No dependency
or fixture was added.

## Explicit en-US dotted time recognition

The shared date parser now accepts H.MM.SS under en-us, including optional AM/PM
and a whitespace-separated date. This matches Microsoft's IsDate example while
preserving de-de dotted date ordering. Invalid fourth components, ranges and
four-digit seconds are rejected; dotted fractional-second syntax is unsupported.
CDate, DateValue, TimeValue and IsDate all consume the same parsing change.
[Date conversion](DATE_CONVERSIONS.md) and [recognition](ISDATE.md) describe the
explicit policy and continuing native/regional limits.

The 120-test expression-function batch and five selected CLI regressions pass,
including exact time-fraction goldens, invalid discarded dates, Unicode boundaries,
profile separation and all four calls in both CLI execution paths. Relevant
all-target/all-feature Clippy, minimal release build, formatting and diff checks
pass. The updated independent inspection recipe passes 5,946 controls, including
generated dotted times and native-table projections; the date-conversion recipe
passes 2,698 controls against the release binary. Source bytes remain unchanged.
Native Windows Access remains unrun. No dependencies or fixtures were added.

## Explicit IsDate recognition

IsDate now uses the parser-independent date kernel through the existing Inspect
plan node, with request-local Gregorian format, two-digit-year and reference-year
settings. Invalid values return FALSE; missing policies and operational failures
propagate. NULL is FALSE; typed numeric/date inputs use the documented checked
profile. Numeric Text fallback is disabled. [IsDate contracts](ISDATE.md) and the
[built-in tracker](ACCESS_BUILTINS.md) retain partial/native qualification limits.

Four kernel tests include generated calendar spellings and host failures; a new
evaluator test checks shared work/input/cancellation limits. Query, CLI and optional
worker regressions cover grouping identity, correlations, request isolation,
recovery after missing context and buffered failures. Expression/query suites pass
350 tests; parser-free expressions pass 147, with the later evaluator addition
checked in the 10-test focused inspection run. Four selected CLI regressions and
the new worker test pass. Relevant all-target/all-feature Clippy, the minimal CLI
release build, formatting and diff checks pass. The independent inspection recipe
passes 5,686 generated/refusal/native-row controls using unchanged source bytes.
Native Windows Access and the full workspace runtime suite remain unrun for this
slice. No dependencies or fixtures were added.

## Explicit CDate, DateValue and TimeValue conversion

Three date conversion/extraction calls now share deterministic Gregorian parsing,
Jiff validation and explicit syntax/reference-year controls across scalar, bound,
nested and qualified saved SELECT execution. CDate preserves existing DATE bits;
DateValue/TimeValue project exact stored day/fraction components without a calendar
round trip. Text and extended components project before legacy narrowing. Numeric
CDate and optional explicit numeric Text fallback reuse the existing Double
conversion. [Date-conversion contracts](DATE_CONVERSIONS.md) record scope and
native/locale/Variant limits; all three are Partial in the
[built-in tracker](ACCESS_BUILTINS.md).

DateTextFormat now has working parsing/output consumers in the date slice, with its
old CStr export retained. ReferenceYear supplies omitted-year context through
--reference-year and request-local library settings; no clock is read. Two-digit
years share DateSerial's separate window. The host-error tracker now also protects
numeric date fallback from reclassifying provider failures. New DateConvert nodes
retain fields, grouping identity, aggregates, correlations and qualified multivalue
parent dependencies. No new dependency, fixture, host runtime or UPDATE call is added.

Six kernel tests cover exact bits/aliases, numeric bounds, regional/month/time
spelling, invalid discarded components, Unicode boundaries, omitted/two-digit years,
extended projection and operational errors, including generated day models. An
evaluator test checks work/input/cancellation limits. Two query and a multivalue
regression exercise dependencies and settings; two CLI tests and a worker test
check validation, buffering, request isolation and reuse after failure.
Expression/query suites pass 345 tests with all features; standalone parser-free
expressions pass 143, selected CLI regressions 22 and the all-feature SQLite package
25 including its rustdoc example.

The [independent recipe](testing/compare_date_conversion.py) passes 2,698 Python
Gregorian/Automation-serial, refusal and native-row controls. Sequential/indexed
projections agree over Jackcess-decoded source inputs, with the original source
hash unchanged. Native Windows Access remains unrun, including full regional/year,
NULL, coercion, precision and error-order behavior. Workspace all-target/all-feature
Clippy, minimal release builds, warnings-denied expression/query rustdoc, formatting
and diff checks pass. The full workspace runtime suite was not repeated. Development
guidance now reflects Jiff's existing parser-independent expression dependency.

## CStr numeric and Gregorian date/time output

CStr now copies Text, spells Boolean values and rejects NULL explicitly. Numeric
output uses the selected NumberFormat with no reserved sign space; exact integer,
Currency and Decimal digits remain intact. A streaming adapter reuses Str's existing
bounded numeric renderer. DateTextFormat independently selects iso/en-us/de-de
Gregorian short-date/long-time output via Jiff, with explicit nearest-second rounding,
epoch/time-only, midnight/date-only, carry and overflow rules. CLI eval/run expose
--date-format, and library/query/optional worker settings remain execution-local.
[CStr contracts](CSTR.md) distinguish these explicit profiles from complete native
regional/Variant behavior; the [built-in tracker](ACCESS_BUILTINS.md) marks it Partial.

The fixed formatting buffer now has working numeric and date consumers in the text
slice. Text/Boolean/numeric/date output shares byte/work/cancellation limits and
failure buffering, charging only emitted numeric bytes. Stored values/bits, row
display, implicit arithmetic, Str/Val and parameter transport are unchanged. No
new dependency, fixture or host runtime is introduced; Jiff remains the selected
calendar implementation.

Five kernel tests cover exact values, numeric/date profiles, NULL/type/policy errors,
epoch/midnight carry, bounds, limits and generated Int64 values. An evaluator test
checks byte accounting and failures. A query regression covers grouped numeric
parameters, nested date parameters and profile reuse. Two CLI and one worker
regressions verify independent policies, validation, buffering and failure reuse.
Expression/query suites pass 335 tests with all features; standalone parser-free
expressions pass 137, selected CLI regressions 22 and the all-feature SQLite package
24 including its rustdoc example.

The [independent recipe](testing/compare_cstr.py) passes 3,908 Python integer,
exact Decimal, floating/calendar, refusal and native-row controls. It uses exact
Currency inputs to isolate formatting from the separate IEEE import policy.
Sequential/indexed projections agree over Jackcess-decoded rows and the original
source hash is unchanged. Native Windows Access remains unrun, including date
rounding/patterns, extended-type behavior and complete numeric/Boolean locale
semantics. Workspace all-target/all-feature Clippy, minimal release builds,
warnings-denied expression/query rustdoc, formatting and diff checks pass. The full
workspace runtime suite was not repeated.

## Numeric type and Text inspection

IsNumeric now returns Boolean across scalar SELECTs, bound queries and qualified
saved SELECTs. Numeric/Boolean carriers are recognized by type; NULL, both date
carriers, binary and GUID return FALSE. Text uses the existing explicit whole-string
number-format parser, with finite range and no prefix truncation. Its typed Inspect
node preserves argument dependencies in pruning/remapping, grouping, aggregates,
correlation and qualified multivalue predicates. [Inspection contracts](ISNUMERIC.md)
record current behavior, reference discrepancies and remaining native/locale gaps;
the [built-in tracker](ACCESS_BUILTINS.md) marks it Partial.

Expected lexical/range failures yield FALSE; host-buffer, cancellation, allocation,
input/work/byte and missing-policy failures propagate. A checked buffer retains
error provenance even if a provider returns TypeMismatch or Overflow. Argument
errors remain eager, and no new function call is allowed in bounded UPDATE.
No dependency, fixture or host runtime is added.

Four kernel tests include all current carriers, generated integer text, explicit
profiles and injected host errors. An evaluator test covers resource/cancellation
failures, two query tests cover binding/grouping/correlation/parameters and a
multivalue regression checks hidden dependencies and parent semantics. CLI and
optional worker regressions check transport, buffering, request isolation and
failure reuse. Expression/query suites pass 328 tests with all features;
standalone parser-free expressions pass 132, selected CLI regressions 20 and the
all-feature SQLite package 23 including its rustdoc example.

The [independent recipe](testing/compare_inspection.py) passes 3,638 generated
numeric-text, typed/negative, refusal and native-row controls across three profiles
and both CLI paths. Sequential/indexed native projections agree over independently
decoded Jackcess rows and the original source hash remains unchanged. Native
Windows Access remains unrun; independent models/readers do not settle the documented
formal-reference/numeric-string discrepancy or establish complete compatibility.
Workspace all-target/all-feature Clippy, minimal release builds, warnings-denied
expression/query rustdoc, formatting and diff checks pass. The full workspace
runtime suite was not repeated.

## Explicit numeric Text profiles

Nine existing conversion calls now accept complete numeric Text through explicit
invariant, en-us and de-de syntax profiles. Evaluator, bound/nested query and
optional worker hosts carry per-execution settings; the CLI exposes --number-format
only on eval/run. No ambient OS/database locale is read. Missing policy is a typed
failure. Numeric and Boolean carrier conversions, native decoding, implicit
arithmetic, Val grammar and CLI parameter transport retain separate contracts.
[Numeric Text contracts](NUMBER_FORMAT.md) describe accepted syntax and current gaps;
the [built-in tracker](ACCESS_BUILTINS.md) keeps all nine Partial.

The shared exact Val digit scanner now handles lexical rounding for text integer,
Currency and Decimal targets without an intermediate binary float or bounded
Decimal pre-round. CDec rounds directly from the original spelling into its current
96-bit/scale-28 envelope. CSng parses normalized text directly into binary32;
CBool additionally recognizes the documented Boolean spellings. The selected
rust_decimal numeric/IEEE conversion and arithmetic kernels remain in place. No
additional arithmetic dependency, fixture or host runtime is introduced.

Five kernel tests include generated Int64 controls, grouping/currency/exponents,
96-bit Decimal edges, midpoint guard digits, direct Single parsing, overflow and
resource limits. An evaluator test covers input/allocation/cancellation limits;
two query tests check parameter reuse, separate grouped/nested cases, eager failures
and work ceilings. Two CLI regressions cover both paths, profile validation and
buffering. A worker regression verifies request isolation, failure reuse and an
unchanged SQLite file. Expression/query suites pass 320 tests with all features,
standalone parser-free expressions 128, selected CLI regressions 21 and the
all-feature SQLite package 22 including its rustdoc example.

The [independent recipe](testing/compare_number_format.py) passes 6,462 integer,
exact Python Decimal, refusal and native-row controls across all profiles and both
CLI paths. Sequential/indexed native projections agree over Jackcess-decoded
inputs; the original source hash remains unchanged. Native Windows Access remains
unrun, including regional acceptance, precision retention, rounding and error order.
Workspace all-target/all-feature Clippy, minimal release builds, warnings-denied
expression/query rustdoc, formatting and diff checks pass. Dependency-boundary
validation and its ten Python regressions pass. The full workspace runtime suite
was not repeated. These are implemented-profile/independent-reader
results, not complete native compatibility.

## Numeric-to-Text Hex, Oct and Str

Hex and Oct now share checked half-even Long conversion with an explicit Int64
exemption, bounded stack formatting and metered Text output. Their current negative
width intervals follow the published MS-VBAL rules; known inconsistencies and
native qualification needs are explicit in [numeric-text contracts](NUMERIC_TEXT.md).
Str preserves every integer/Currency/Decimal digit and uses an explicit finite
7/15-significant-digit floating profile. Its native notation, broader Variant
coercion and locale/date behavior remain incomplete. All three propagate NULL and
are Partial in the [built-in tracker](ACCESS_BUILTINS.md).

Six focused kernel tests cover examples, negative-width boundaries, half-even
conversion, exact fixed values, floating extremes and generated models. An evaluator
test checks byte ceilings/cancellation. Three query tests cover fields, NULLs,
grouping, nested aggregates, parameter reuse, refusals and caller-supplied work
limits. A CLI regression covers both paths, failure buffering and the scalar work
ceiling. Expression/query suites pass 311 tests with all features, followed by all
three numeric-text query tests after adding the explicit work-limit check; standalone
parser-free expressions pass 123. Selected CLI regressions pass 20 tests.

The [independent recipe](testing/compare_numeric_text.py) passes 3,936 integer,
exact-Decimal floating-rounding, refusal and native-row controls. Sequential/indexed
projections agree over independently decoded Jackcess inputs and the original
fixture hash stays unchanged. These do not establish native expression compatibility;
native Windows Access remains unrun. Workspace all-target/all-feature Clippy,
minimal release builds, warnings-denied expression/query rustdoc, formatting and
diff checks pass. The complete workspace runtime suite was not repeated. No
dependency or fixture is added.

## Bounded Val numeric prefixes

Val now parses Text prefixes in scalar and bound queries, with period-only decimal
syntax, documented whitespace removal and checked hexadecimal/octal accumulation.
Integer suffix checks precede floating-point conversion; Currency suffixes validate
the exact decimal coefficient and reuse the shared Currency conversion. The current
result carrier is finite Double. [Val contracts](VAL.md) distinguish implemented
behavior from unqualified native subtypes, coercions and error precedence; the
[built-in tracker](ACCESS_BUILTINS.md) marks it Partial.

Five focused kernel tests cover examples, ranges, whitespace/exponents, generated
integer/radix models, normalization byte limits and cancellation. Two query tests
exercise grouping, aggregates, parameter reuse and eager refusal. One CLI regression
checks both execution paths and empty stdout on failed conversions. Expression/query
suites pass 302 tests with all features; standalone parser-free expressions pass
117. Selected CLI regressions pass 19 tests. The
[independent recipe](testing/compare_val.py) passes 3,172 integer/radix/Currency,
refusal and native-row controls, including sequential/indexed projections over
Jackcess-decoded source rows. The original fixture hash remains unchanged. These
are independent-model/reader checks; native Windows Access remains unrun.

Workspace all-target/all-feature Clippy, minimal release builds, warnings-denied
expression/query rustdoc, formatting and diff checks pass. The complete workspace
runtime suite was not repeated. No dependency or fixture is added.

## Checked Chr, Asc and Unicode unit functions

Chr/Asc now construct and inspect characters under explicit single-byte pages,
with the existing ASCII-only default. Chr rejects out-of-byte-range inputs rather
than borrowing String's modulo rule. Asc uses non-replacing fixed-buffer encoding
and validates an exact decoded round trip; unmappable characters fail without
best-fit substitution. ChrW/AscW use an explicit signed/unsigned UTF-16 profile,
independent of selected code pages. AscW can return a signed leading surrogate
from supplementary input; ChrW refuses unrepresentable surrogate Text output.
[Character-function contracts](CHARACTER_FUNCTIONS.md) record ranges, NULL/type
rules, strict mapping and remaining native gaps. All four are Partial in the
[built-in tracker](ACCESS_BUILTINS.md).

Four kernel tests cover explicit pages, numeric conversion, NULL/domain/type
errors and every representable BMP unit against independent signed models. A
separate encoder test exercises all mapped bytes across eleven pages and unmappable
text. An evaluator test covers allocation-byte charging and cancellation; two
query tests cover grouped/nested calls, parameter/page reuse and eager refusals.
The new CLI regression checks both execution paths and uses csv decoding for
existing Text display cells.

Expression/query suites pass 295 tests with all features and 295 without defaults;
standalone parser-free expressions pass 112. Selected CLI regressions pass 25 tests.
The [independent recipe](testing/compare_character_functions.py) passes 11,304
byte-page/inverse/UTF-16/refusal/native-row controls against Python models, including
sequential/indexed projections, with the original database hash unchanged.
Workspace all-target/all-feature Clippy, minimal release builds, warnings-denied
expression/query rustdoc and formatting/diff checks pass. The full workspace runtime
suite was not repeated and native Windows Access remains unrun. No dependency or
fixture is added. DBCS, full Variant/string coercion, lossless UTF-16 output and
native default/error/mapping behavior remain incomplete.

## Execution-local character-code pages

String now decodes numeric byte codes using explicitly validated pages 866, 874
or 1250–1258. A selected page uses encoding_rs's non-replacing decoder with a fixed
UTF-16 stack buffer; unmapped bytes fail without substitution. Omission retains
ASCII-only behavior. WHATWG mappings, including documented C1/Hebrew extensions,
are the selected contract, not observed native Access byte behavior. Existing Text,
physical field decoding, SQL collation and inert inspection remain separate.
[Character-code contracts](CHARACTER_CODE_PAGES.md) document the supported profile.

Library callers select TextContext/Evaluator/ExecutionOptions policies. Prepared
plans do not capture them, and existing SQLite workers forward the value per
request without enabling CLI async execution or expanding SQLite functionality.
CLI query eval/run expose --character-code-page ID; invalid settings fail before
native I/O. CLI retrieval now composes its existing comparison/calendar controls
with this setting in a single controls value rather than extending positional
argument chains. The built-in tracker retains String as Partial.

Two decoder tests, one evaluator test, two query tests, two CLI tests and one
worker test cover independent goldens, every supported ASCII byte, bad IDs/unmapped
bytes, modulo/Text separation, nested/grouped execution and plan/request reuse.
Expression/query/SQLite all-feature suites pass 308 tests; minimal expression/query
suites pass 287 and standalone parser-free expressions pass 107. Selected CLI
regressions pass 24 tests. The [independent recipe](testing/compare_character_code_pages.py)
passes 5,672 controls, including all bytes across eleven pages in both CLI query
paths, 46 unmapped-byte refusals, and sequential/indexed native projections with
unchanged original database hash.

The selected encoding_rs 0.8.42 dependency, disabled default features, locked
transitives, upstream unsafe/build assumptions and licenses are recorded in
[DEPENDENCIES.md](DEPENDENCIES.md). Dependency boundaries and all ten guard tests
pass; feature/tree review retains minimal CLI GUI/network/async-runtime isolation.
Workspace all-target/all-feature Clippy, minimal release builds, warnings-denied
expression/query/worker rustdoc and formatting/diff checks pass. Final naming of
code-page identifiers is covered by focused decoder tests. Full workspace runtime
tests and native Windows Access execution remain unrun for this slice. No fixture
or copied mapping table is added; DBCS, full Variant and native mapping/default
qualification remain incomplete.

## Bounded Space, String and BMP reversal

Space, String and StrReverse now share a parser-free construction recipe across
scalar SELECT evaluation and retrieval. Repeat counts use checked half-even Int32
conversion; String selects a first BMP unit or an explicit numeric ASCII code after
modulo 256. Space/String retain their explicit NULL profile; StrReverse rejects
NULL and reverses BMP units without normalization or grapheme inference. High
numeric bytes require an unimplemented code-page policy and unrepresentable
surrogate results fail explicitly. [Construction contracts](TEXT_CONSTRUCTION.md)
record these limits rather than claiming complete native Unicode/coercion behavior.

The working TextOutput trait now lives in its own text feature module and is shared
by construction and Replace. The former replacement::TextOutput path remains a
re-export. New recipes allocate no intermediate String or match list, and the
existing evaluator sink checks cancellation, work, payload bytes and per-value
limits before growth. Huge repeat counts stop at the applicable allowance;
failed operations discard partial output. All three functions are Partial in
the [built-in tracker](ACCESS_BUILTINS.md).

Five authored/generated kernel tests, one evaluator test, two query tests and
one CLI test cover repeated/reversed BMP models, conversion, modulo/control
characters, NULL and eager errors, invalid code pages/surrogates, resource limits,
grouping/correlation and parameter reuse. Expression/query suites pass 282 tests
with all features and 282 without defaults; standalone parser-free expressions
pass 105 tests. Selected CLI regressions pass 22 tests. The
[independent recipe](testing/compare_text_construction.py) passes 154 repetition,
UTF-16 reversal, NULL/refusal and sequential/indexed native-row controls with the
original registered database hash unchanged.

Workspace all-target/all-feature Clippy, minimal workspace release build,
warnings-denied expression/query rustdoc and formatting/diff checks pass. No
dependency or database fixture is added. The full workspace runtime suite was not
repeated and Windows Access execution remains unrun. Extended byte-code policies,
lossless UTF-16 results and complete Variant/native character semantics remain open.

## Bounded Replace suffix construction

Replace now shares a borrowed parser-free recipe between scalar SELECT evaluation
and retrieval. It emits the requested UTF-16 suffix, replaces non-overlapping
source matches, never searches inserted text, and retains explicit start/count/
comparison defaults and eager argument evaluation. NULL source/find/replacement
values fail rather than becoming stored NULL or empty text. Locale/database
providers can supply unequal-length source spans; malformed/nonadvancing spans
and unrepresentable surrogate boundaries fail explicitly.

A private evaluator sink charges payload bytes before fallible output growth,
checks work/cancellation and per-value limits, and discards partial output on
failure. Kernel scans do not allocate a match list or intermediate String.
[Replacement contracts](TEXT_REPLACEMENT.md) record the borrowed output API,
remaining coercion/locale/default qualification and allocator-accounting limits.
The [built-in tracker](ACCESS_BUILTINS.md) marks Replace Partial.

Five kernel tests, one evaluator test, two query tests and two CLI tests cover
suffix/count/empty/deletion/overlap behavior, generated Unicode models, provider
spans, grouping/correlation, parameter reuse, optional slots, eager errors and
bounded construction. Expression/query suites pass 274 tests with all features
and 274 without defaults; standalone parser-free expressions pass 100 tests.
Selected CLI regressions pass 21 tests. CLI expectations decode CSV/TSV transport
before checking the existing quoted Text display policy; production output is
unchanged. The [independent UTF-16 recipe](testing/compare_replacement.py) passes
416 controls, including sequential/indexed native rows and both CLI resource
profiles, with the original example database hash unchanged.

Workspace all-target/all-feature Clippy, minimal workspace release build,
warnings-denied expression/query rustdoc and formatting/diff checks pass. Final
CSV-based CLI assertions are also covered by CLI lint/tests; removal of an
unreachable optional-mode default arm is covered by focused replacement tests.
The full workspace runtime suite was not repeated and native Windows Access
execution remains unrun. No dependency or database fixture is added.

## Bounded InStr and InStrRev search

InStr and InStrRev now share metered UTF-16 substring search across scalar SELECTs
and retrieval. Results are one-based Int32 offsets, zero when not found, or NULL.
Forward default comparison requires a declared host policy; reverse default
comparison is binary. Reverse bounds constrain the end of an entire match, and
UTF-16 positions preserve supplementary-character widths and overlapping matches.
A stack-only SQL argument normalizer keeps written NULL, optional missing slots
and function-specific defaults distinct without cloning expression trees.

Host text/database search uses a separate callback with validated UTF-16 match
ranges, allowing unequal source/needle lengths instead of inferring locale search
from equality. CLI ordinal comparison/search now lives in its own feature module;
it supplies exact database/default search, with surrogate-safe UTF-16 bounds.
Locale/code-page comparison, LCID modes, complete Variant coercion and native
Access qualification remain incomplete. [Search contracts](STRING_SEARCH.md)
record boundary/empty-input policies and provider responsibilities. Both functions
are marked Partial in the [built-in tracker](ACCESS_BUILTINS.md).

Five authored/generated kernel tests, two query tests and two CLI tests cover
Unicode, overlapping/reverse matches, provider bounds, eager errors, optional
slots, grouped/correlated expressions, parameter reuse and cancelled scans.
Expression/query suites pass 266 tests with all features and 266 without defaults;
standalone parser-free expressions pass 95 tests. Selected CLI regressions pass
19 tests. The [independent recipe](testing/compare_string_search.py) passes 284
UTF-16/ordinal/default/NULL/refusal/native-row controls with unchanged source hash.

Workspace all-target/all-feature Clippy, minimal workspace release build,
warnings-denied expression/query rustdoc and formatting/diff checks pass. Final
edits after those builds are documentation/comments and the bracketed reserved
alias in a test; the corrected test passes in both feature suites. Full workspace
runtime tests and Windows Access execution were not repeated/run respectively.
No dependency or fixture is added; performance claims are not made for the
bounded candidate scan.

## Explicit StrComp comparison modes

StrComp now shares a metered, allocation-free text kernel across scalar SELECTs
and retrieval. Explicit binary comparison uses UTF-16 units and returns Int16
-1/0/1 or NULL. Text and database comparison use distinct host callbacks; omitted
or -1 modes require a declared host default. CLI ordinal selection supplies the
database/default policy without pretending to provide locale-aware text ordering.
Optional omitted slots stay distinct from explicit NULL comparison arguments.
[Comparison contracts](STRING_COMPARISON.md) record argument conversion, eager
errors, callback responsibilities and the remaining native code-page/locale gaps.
The [built-in tracker](ACCESS_BUILTINS.md) now marks this implementation Partial.

Five kernel tests, including a generated independent UTF-16 model, one evaluator
test and two retrieval tests cover exact carriers, NULL/mode errors, distinct
providers, eager failures, grouping, correlations, parameter reuse and resource
limits. Expression/query packages pass 259 tests with all features and 259 without
default features. The standalone parser-free expression profile passes 90 tests.
The selected CLI eval/retrieval/StrComp regressions pass 17 tests.
The [original reference recipe](testing/compare_strcomp.py) passes 85 independent
UTF-16/ordinal/NULL/refusal controls, including sequential/indexed native projections;
the registered database hash remains unchanged.

Workspace all-target/all-feature Clippy, minimal workspace release build,
warnings-denied expression/query rustdoc and formatting/diff checks pass. Final
comment/documentation edits do not change executable behavior. The full workspace
runtime suite was not repeated; Microsoft Access execution remains unrun. No
new dependency or database fixture is added. Native database/text collation and
complete Variant coercion remain open.

## Execution-scoped DateSerial year windows

Hosts can now select a validated two-digit-year window through
`Evaluator::with_year_window` and `ExecutionOptions::year_window`. The execution
setting applies to nested queries and constructor calls without changing source
metadata or being captured during preparation. A reused prepared plan can switch
windows and return to the named default without leaking the prior setting.
Four-digit years and TimeSerial remain unaffected.

CLI `query eval` and `query run` now expose `--two-digit-year-max YEAR` for the
same policy. The shared constructor validates the final year, 199–9999, before
file I/O. Omission retains 2029 (1930–2029). No machine preference is inferred;
inert parse/inspection commands remain separate. [Constructor contracts and
examples](DATE_CONSTRUCTORS.md#explicit-execution-windows) record the public API.

Expression/query packages pass 251 tests with all features and 251 without default
features. Five authored scalar/query/CLI/worker tests cover evaluator-local policies,
prepared-plan reuse, nested/grouped expressions, boundaries, buffered errors and
argument rejection before native I/O; the CLI regressions total 22 passing tests.
The optional SQLite worker now forwards `AsyncOptions::year_window`, fixing the
explicit execution-options consumer found by workspace-wide linting. Its complete
all-feature package suite passes 20 tests, including request-local policy reuse
and unchanged source bytes. This maintains the existing worker boundary without
adding SQLite functionality or a CLI runtime dependency.
The [independent constructor recipe](testing/compare_date_constructors.py) now
passes 126 controls on the optimized CLI, including four windows, boundary-year
mapping, native projections and indexed filters whose result depends on the
chosen window. The registered Access example's original hash remains unchanged.

Workspace all-target/all-feature Clippy, minimal workspace release builds,
warnings-denied expression/query/worker rustdoc and formatting/diff checks pass.
Final import-only cleanup and optional worker forwarding are covered by focused
compile/test checks; the full workspace runtime suite was not repeated. No dependency or database fixture is
added. Hijri, locale parsing and native Windows qualification remain pending.

## Gregorian DateSerial / TimeSerial constructors

Closed constructor dispatch now shares DateSerial and TimeSerial between scalar
evaluation and retrieval. Three eager arguments undergo checked half-even Int16
conversion; NULL is an invalid conversion. DateSerial normalizes relative
Gregorian months/days, including final-valid dates whose intermediate year exceeds
the Date envelope. TimeSerial retains bounded signed second/day carries. Both
return validated legacy Date carriers without clocks, locale parsing or VBA.

The named default two-digit-year window is 1930–2029. Library callers can validate
another 100-year window for direct construction; SQL currently uses the explicit
named default. Query/CLI host-policy overrides, Hijri support and native Access
NULL/negative-carry/rounding qualification remain pending. The feature does not
claim that an ambient Windows preference was detected.

Five value tests cover relative units, year-window boundaries, numeric conversion,
errors and generated independent calendar/second models. Extreme-input testing
caught an infallible Jiff span-construction panic; the implementation now uses its
fallible API and returns typed Overflow instead. Scalar dispatch and two retrieval
tests cover eager errors, grouping, pruning, constructor identity, correlations,
parameter reuse and nested date functions. The expression/query packages pass
249 tests with all features and 249 without default features. Relevant all-target/
all-feature Clippy, minimal workspace release builds, warnings-denied rustdoc and
formatting/diff checks pass. The full workspace runtime suite was not repeated.

The [independent CLI recipe](testing/compare_date_constructors.py) passes 74
Gregorian/rational constructor controls on the optimized CLI, including
sequential/indexed reads of original Int32 rows from the registered Access
example. Its original hash remains unchanged. No dependency or database fixture is
added. Microsoft references, current policies and limits are recorded in
[DATE_CONSTRUCTORS.md](DATE_CONSTRUCTORS.md); Windows Access remains unrun.

## Explicit DATE-serial arithmetic and adjacent date operators

Legacy DateTime addition/subtraction and unary signs now share an explicit
Automation serial kernel across scalar evaluation and retrieval. Numeric operands
use the existing CDbl conversion; representable results retain Date, Date-minus-
Date returns Double and finite out-of-envelope results promote to Double.
Stored serial bits/fractions are used without civil-calendar rounding. DateAdd/
DateDiff, ordinary numeric arithmetic and unsupported Date aggregate accumulation
retain their separate contracts. Native pre-epoch/operator/promotion fidelity still
requires Access comparison; the [current policy](DATE_ARITHMETIC.md) is explicit.

Date arithmetic tests exposed an upstream lexical collision: #- and #> consumed a
closing hash with its adjacent operator. The Access adapter now separates those
compound tokens, retains exact spans and reconstructs adjacent >=. A regression
checks date subtraction/comparison, spaced invalid operators and inert quoted
text/bracketed names. Hash operators from other dialects are not added to Access.

Three value tests include an independent generated quarter-day model; a scalar
and two retrieval tests check exact carriers, boundaries, NULL/error eagerness,
pruning, correlation, grouping, parameter reuse and unchanged aggregate limits.
Parser/expression/query packages pass 292 tests with all features and 292 without
default features. Relevant all-target/all-feature Clippy, minimal workspace release
builds, warnings-denied rustdoc and formatting/diff checks pass. The full workspace
runtime suite was not repeated for this parser/expression batch.

The [DATE arithmetic recipe](testing/compare_date_arithmetic.py) passes 62 controls
on the optimized CLI, including inert raw-bit reads from existing original Jet 4,
ACE 2007, ACE 2016 and ACE 2019 scalar fixtures. All input hashes remain unchanged.
These compare an explicit serial/IEEE model, not native Access SQL. No database
fixtures, dependencies, ambient calendar/clock access or runtime host are added.
The supplied W3Schools index is separately recorded as a secondary coverage
checklist in [REFERENCES.md](REFERENCES.md); Microsoft remains the semantic source.

## Double-valued mathematical functions

The closed numeric API now implements Sqr, Exp, Log, Sin, Cos, Tan and Atn using
the shared checked numeric-to-Double conversion. All non-NULL results are Double;
Sqr/Log enforce mathematical domains, non-finite inputs fail and non-finite
results report overflow. Signed zero, finite subnormals and exponential underflow
retain the explicit IEEE policy. Exact integer/Decimal/Currency arithmetic outside
this function boundary is unchanged. No dependency, runtime or VBA host is added.

Three value tests include generated trig identities and carrier/domain/NULL/error
checks. The scalar dispatcher and two retrieval tests cover composed and grouped
expressions, pruning, correlations, parameter reuse and eager failures. Expression
and query packages pass 235 tests with all features and 234 without default
features. Relevant all-target/all-feature Clippy, the minimal workspace release
build, warnings-denied expression/query rustdoc and formatting/diff checks pass.
The full workspace runtime suite was not repeated for this expression-only batch.

The [high-precision comparison](testing/compare_transcendental_functions.py)
passes 134 mathematical/NULL/refusal controls on both development and optimized
CLIs. Decimal provides high-precision root/exponential/logarithm references;
independent power series and Machin's identity provide trigonometric references
without calling the platform trig functions. Comparison uses an explicit four-ULP
regression tolerance. These are mathematical model checks, not native Access
rounding, coercion, NULL or Windows qualification. Microsoft's primary function
references and remaining limits are recorded in [MATH_FUNCTIONS.md](MATH_FUNCTIONS.md).
Read/query items 1–3 and the broader backend roadmap remain active.

## Aligned attachment metadata query members

`CollectionMember` now supplies a shared named child-field declaration without
adding collection variants to core `Value`. Query sources expose aligned member
views alongside existing `.Value` lists. Binding resolves two/three-part member
names, retains source pruning and validates the combined schema. Execution aligns
fields of one child record, checks list lengths/types and takes products only
across separate collections; provider failures remain latched and budgeted.

The ACE engine now inspects attachment members without user records and borrows
stored metadata at checked child ordinals. `NativeSource::with_collections()` and
CLI `query run --collections` expose FileURL, FileName, FileType, FileTimeStamp and
FileFlags on ACE 2007/2016/2019-family inputs. One complete checked child snapshot
is cached per attachment field. Encoded FileData is currently still read within
that snapshot, but never decoded, extracted or exposed as an SQL value. Plain
parent/wildcard projection and selected collection-parent indexes remain rejected.

Five query tests cover same-child alignment, interleaved metadata, independent
products, legacy/named index spaces, empty/NULL states, membership, invalid
providers, metadata drift and execution limits. New engine/adapter/CLI checks
cover no-data binding, borrowing, complete cache reuse, opt-in and refusal
boundaries. Thirty-nine independent attachment model controls pass on development
and optimized CLIs across all three original fixture families, preserving hashes.
The optimized CLI also passes all 75 existing value-expansion controls and 45
parent-membership controls. These are explicit stored-record/model comparisons;
Microsoft Access SQL and native Windows qualification remain pending.

This batch passes 1,115 workspace all-feature tests, 131 query-package tests without
default features, workspace all-target/all-feature Clippy, minimal release builds,
warnings-denied rustdoc, formatting/diff checks, twelve-package dependency guards
and ten policy tests. It adds no dependencies or database fixtures. The existing
original Java recipe gains an inert parent-ID metadata report; its prior metadata
output and generation dataset remain unchanged. Documentation-only status and
provenance updates follow the runtime checks.

See [query contracts](NATIVE_QUERIES.md#attachment-metadata-member-queries) and
[shared aligned views](MULTIVALUE_QUERIES.md#aligned-named-collection-members).
The read/query roadmap remains active; FileData/history SQL, broader scalar and
native text behavior still need implementation/evidence before items 1–3 close.

## Scalar-parent multivalue membership criteria

Direct single-table retrieval now lowers hidden `.Value` WHERE criteria into a
private parent predicate tree. Independent membership atoms combine with eager
three-valued logic; conjunction can match different stored children, negation
acts on the parent match and duplicate parent records remain separate. Ordinary
scalar projection/grouping/counts retain parent cardinality without flattened
rows, fake keys or output-value deduplication. Scalar dependencies, parameters
and correlated scalar outer fields retain bounded pruning/slot rewriting.
Existing explicit `.Value` output keeps its flattened behavior.

Six authored/generated query tests and a CLI test cover parent counts, independent
atoms, NULL/empty policy, negative operators, parameter reuse, scalar correlation,
derived composition, eager errors and rejected shapes. Native cache assertions
also pass. Forty-five independent parent/member model controls pass in development
and optimized builds; the existing 75 expansion controls pass on the optimized
CLI too. All three original fixture hashes remain unchanged.

The batch passes 1,106 workspace all-feature tests, 126 query-package tests without
default features, workspace all-target/all-feature Clippy, minimal release builds,
warnings-denied rustdoc, formatting/diff checks, twelve-package dependency guards
and ten policy tests. A final CLI test rename/help wording correction changes no
execution logic; the full suite was not repeated for those text-only edits.
No dependency or fixture is added.

[Membership contracts](MULTIVALUE_QUERIES.md#parent-membership-criteria) explicitly
retain native NULL/negative-operator/error-order qualification gaps. Joined/mixed
parent modes, cross-collection atoms, subqueries inside parent criteria, ambiguous
member sorting and parent-criteria crosstabs remain rejected. Scalar-parent
selection is implemented; whole-collection display and attachment/history member
SQL remain open. Independent nullable-set models are not Microsoft Access SQL
evidence. Items 1–3 stay active; attachment metadata query access is next.

## Native multivalue query adapter and CLI

Qualified ACE 2007/2016/2019-family stored collections now connect to scalar
`.Value` query expansion through `NativeSource::with_multivalues()` and explicit
`query run --database PATH --multivalues`. Ordinary scalar metadata behavior stays
on its previous path by default. Element metadata discovery reads no parent/child
data records; only referenced collections load complete validated native links
and child snapshots. Borrowed constant-time value lookup avoids payload clones.
Collection caches are separate from scalar/index snapshots, parent counts remain
unchanged, and an index selected on an expanded parent table is rejected without
fallback. Lookup queries, formulas and active content do not execute.

One new engine test, three adapter tests and two CLI tests cover all qualified
families, metadata-only binding, correct parent association, complete cache reuse,
NULL/empty cases, buffered errors, cancellation and unchanged inputs. Seventy-five
independent stored-row/model controls pass against actual parent IDs and six
finite scalar collection families, with source hashes unchanged. Both current
development and optimized CLI builds pass those controls; non-finite refusals also
check their diagnostic category. No new fixture or dependency is added.

The workspace batch passes 1,099 all-feature tests. A final qualification-helper
refinement retains validation against the loaded child snapshot rather than only
earlier declared metadata; afterward all nine focused engine, three adapter and
two CLI tests pass without repeating the full workspace suite. Current minimal
release builds, workspace all-target/all-feature Clippy, warnings-denied rustdoc,
formatting/diff checks, twelve-package dependency guards and ten policy tests pass.

[Native contracts](NATIVE_QUERIES.md#explicit-multivalue-value-queries) retain
parent-level criteria, display-value conversion, attachment/history member SQL,
indexed expansion, native empty/product semantics and Windows Access comparison
gaps. Independent storage/row-model agreement is not native Access SQL evidence.
Read/query roadmap items 1–3 remain incomplete; parent-level multivalue matching
is the next collection-query slice.

## Backend-neutral scalar multivalue queries

Explicit host sources can now expose typed borrowed multivalue lists, preserving
parent scalar projections and empty/NULL/duplicate memberships. Query binding
resolves qualified `.Value` slots without records, pruning requests only used
collections. Execution owns bounded expansion, residual filtering, grouping,
joins, aggregation and crosstab inputs. Member references only in criteria are
rejected rather than guessed as parent-level matching. Plain collection fields
remain unavailable scalars; core Value and the native engine dependency boundary
stay unchanged. Ordinary scalar sources retain default empty collection metadata.

Nine authored/generated query tests and one parser test cover qualified/repeated
members, empty parents, exact carriers, products, joins/correlations, grouping,
NULL/count behavior, metadata changes, malformed providers, ignored visitor errors
and limits. The parser permits reserved Value after a dot without relaxing bare
names or bang access. The generated model independently expands parent/value data.
These are host-contract tests, not native Access SQL/fixture comparisons.

The workspace batch passes 1,093 all-feature tests. After the final borrowed-value
validation refinement, all 120 query-package tests pass with all features and
again without default features; the full engine suite was not repeated for that
query-local change. Workspace all-target/all-feature Clippy, current minimal
release builds, warnings-denied rustdoc, formatting/diff checks, twelve-package
dependency guards and ten policy tests pass. No dependency is added.

[Contracts and limits](MULTIVALUE_QUERIES.md) keep native adapter/CLI integration,
parent-level criteria, collection display values, attachment/history member SQL,
empty/product semantics and Windows comparisons explicit. Existing native
collection browsing remains separate; `query run --database` does not expose
these new members yet. Connecting that adapter is the next implementation slice.

## Eager Switch/Choose retrieval

Switch and Choose now share a streaming materialized-value selector across scalar
and query evaluation. Every expression evaluates in written order; only the chosen
payload is retained, with no argument-value buffer or selected-value clone.
Switch keeps the first true pair including NULL; Choose uses bounded half-even
one-based indexing and returns NULL for unsupported index ranges. Conversion
errors wait until all expressions have evaluated, preserving later failures.
The selector latches argument-count violations and omits payload Debug/Serde.

All operands participate in metadata-only binding, source pruning, correlated-slot
rewriting, grouping/aggregate validation, parameters and crosstab expressions.
Four authored/generated value tests, two scalar order/budget tests and five query
tests pass. Twenty-six independent CLI selection/reference-row controls pass in
development and optimized builds; native input bytes remain unchanged. Text
controls use the existing TSV display adapter rather than assuming raw strings.

The batch passes 1,083 workspace all-feature tests, 74 standalone expression tests
without SQL, workspace all-target/all-feature Clippy, minimal release builds,
warnings-denied rustdoc, formatting/diff checks, twelve-package dependency guards
and ten policy tests. No dependency is added. [Conditional contracts](CONDITIONAL_FUNCTIONS.md)
retain native NULL-index/error-order, return-promotion and Variant/locale coercion
qualification gaps. Native Windows Access comparisons remain unrun. Read/query
roadmap items 1–3 remain active; advanced-field query access is the next slice.

## Numeric magnitude, sign, floor and rounding functions

Abs/Sgn/Int/Fix/Round now share an allocation-free materialized-value API across
scalar evaluation and retrieval. Integer widths and exact Decimal/Currency paths
are retained; signed minimum/rounding overflow, non-finite values and invalid
place arguments fail explicitly. Round uses the existing rust_decimal library
for exact carriers. Numeric arguments participate in bounded binding, pruning,
aggregate/group validation, correlations and crosstab cells. Both supplied Round
operands evaluate before NULL propagation or result publication.

Seven authored/generated value tests, one scalar-dispatch test and three query
tests pass. Twenty-one independent Python arithmetic/reference-row CLI controls
pass in development and optimized builds, preserving the registered native input
hash. The batch passes 1,072 workspace all-feature tests, 70 standalone expression
tests without SQL, workspace all-target/all-feature Clippy, minimal release builds,
warnings-denied rustdoc, formatting/diff checks, twelve-package dependency guards
and ten policy tests. The final Clippy cleanup only replaces temporary test clones
with borrowed slices; no full-suite repetition was needed. No dependency is added.

[Numeric contracts](MATH_FUNCTIONS.md) document the 0–28 place bound, explicit
Boolean/result carriers, NULL policy and scaled IEEE rounding. Native Access
midpoint/subtype/evaluation-order qualification and broader math/financial,
locale/Variant, text and advanced-field query completeness remain open. This
advances read/query roadmap item 1 without claiming items 1–3 are complete.

## Whole-record DISTINCTROW retrieval

Bounded scalar-table DISTINCTROW now retains all fields of selected source
records, including hidden fields, through pruning and comparison. Joined records
with different hidden values remain distinct despite equal output; identical whole
records collapse. Single-source/all-source-output cases ignore the predicate.
Aliases, nested/outer joins, output ordering, TOP ties and UNION retain their
existing contracts. Shared comparison, allocation/work limits and cancellation
apply; projection errors are not suppressed by earlier duplicates.

Eight authored/generated tests, a hidden-field projection-provider check and
fourteen independent CLI reference-row controls pass. Both development and
optimized CLI runs preserve the registered input hash. The completed batch passes
1,061 workspace all-feature tests, workspace all-target/all-feature Clippy, minimal
release builds, warnings-denied rustdoc, formatting/diff checks, twelve-package
dependency boundaries and ten policy tests. No dependency is added.

[Scope and limitations](QUERY_EXECUTION.md#distinctrow-source-records) explicitly
retain active grouping, selected derived/named records, unavailable full records,
locale/Memo/OLE comparison and native Windows qualification gaps. This advances
roadmap item 1; it does not complete scalar/query, native text or collection-query
work. The independent row model is not native Access SQL evidence.

## Top-level crosstab retrieval

TRANSFORM/PIVOT now binds without row scans and executes bounded fixed/discovered
headings with grouped row totals, WHERE/HAVING, parameters and row ordering. Dynamic
headings retain the source domain before WHERE; fixed headings preserve order and
missing columns. Payloads move into pivot buckets, and every allocation/comparison
retains shared limits/cancellation. Complete names and rows publish together.
Eleven authored/generated tests cover grids, NULL/missing cells, metadata-only binding,
parameter reuse, name/width errors and filtering. Ten independent reference-row
CLI controls pass on sequential/indexed registered input, with unchanged bytes.
[Contracts](CROSSTABS.md) retain heading-type/naming, saved/named crosstab and native
Access semantic/optimizer qualification gaps. No SQL collation or advanced-field
completion is inferred from this feature.

The batch passes 1,051 workspace all-feature tests, workspace all-target/all-feature
Clippy, minimal release builds, warnings-denied rustdoc, formatting/diff checks,
dependency boundaries and ten policy tests. A final added noncontiguous-slot pruning
check passes with all eleven focused crosstab tests, without repeating the full
workspace suite. The optimized CLI also passes the ten reference-row controls.
No dependency is added; the larger read/query workstreams remain incomplete.


## Computed grouping and First/Last retrieval

Read-only queries now group on supported pure scalar expressions and recognize
those bound expressions in projection, HAVING and sorting. They retain source
pruning and reject hidden ungrouped fields; validation uses shared binder limits.
First/Last return encounter endpoints including NULL and keep complete carriers.
They evaluate every operand, preserving later errors/cancellation, and do not
infer minimum/maximum or native optimizer order. Five authored/generated query
tests and eight independent reference-row CLI controls pass, including sequential
and explicit native-index input. Source bytes remain unchanged.
[Contracts and limits](QUERY_EXECUTION.md#computed-group-keys-and-encounter-aggregates)
keep native Access, type/locale coercion and evaluation-order qualification separate.
The unfinished Windows oracle prototype is retained under ignored private planning;
it is not a production dependency or completed native comparison.

All 1,041 workspace all-feature tests, workspace all-target/all-feature Clippy,
minimal release builds, warnings-denied rustdoc, formatting/diff checks, dependency
boundaries and ten policy tests pass. No dependency is added. SQL completeness,
text semantics and advanced-field queries remain the active implementation scope;
this batch does not mark those workstreams complete.


## Compound Short Binary indexes

Text-free mixed tuples containing Short Binary now use the same bounded owned
physical-key reader as text-bearing tuples. Complete components are joined before
one whole-entry overflow digest; NULL/empty, native segment tags, descending bytes
and all returned row/key checks remain explicit. The shared native entry ceiling
now belongs to format layout, and component encoding is a separate feature module.
Exact short-key constraints use full-row NULL facts; digested uniqueness remains
unqualified. No compound seek, Binary predicate pushdown, OLE key or native Binary
mutation is added. Primary/foreign Binary fixture qualification remains pending.

Four original Jet 4 / ACE 2007/2016/2019 inputs pass 176 independent controls with
unchanged sources, including mixed ten-column keys, node roots, full/projected
reads and corruption refusals. The prior Binary corpus passes 84 controls, with
its existing compound case now positive. Native Microsoft Access remains unrun.
[Contracts](testing/BINARY_INDEX_TUPLES.md) and
[fixture identities](FIXTURES.md#compound-binary-index-inputs) record exact scope.
Full-string SQL collation remains a separate native-observation requirement;
physical index weights do not silently replace the explicit ordinal query policy.

The batch passes 1,036 workspace all-feature tests, workspace Clippy, minimal
release builds, warnings-denied rustdoc, formatting/diff checks, the dependency
guard and ten policy tests. Final review added rejection of keys shorter than
one marker per component; all five focused tuple tests pass after that guard.
No dependency is added.


## Compound General text indexes

Qualified text-bearing native indexes now read two-to-ten-column mixed tuples
through ACE, the Access adapter and explicit CLI index selection. Short Text and
Memo retain per-column General descriptors and directions alongside numeric,
Boolean, legacy DateTime, GUID, Decimal and Short Binary components. Complete
component bytes are concatenated before the single whole-entry overflow digest;
Jet 4 Decimal sign bytes are reconstructed without expanding write scope.
All returned rows reproduce their physical key, including omitted key fields.
Row-only NULL facts support checked constraints; Memo/full-width uniqueness,
unknown collations, ignore-NULL indexes, compound seeks and SQL locale semantics
remain explicit gaps. Binary-only compounds and native text writes remain pending.

All 408 independent tuple controls pass across eight Jet 4 / ACE 2007/2016/2019
General combinations, with unchanged sources. Registered MSysObjects.ParentIdName
now supports complete/projected reads; authored corruption checks retain mandatory
key verification. [Contracts](NATIVE_TEXT.md#compound-text-index-reads) and
[fixture identities](FIXTURES.md#compound-general-text-index-inputs) distinguish
these observations from unrun native Microsoft Access qualification.

All 1,034 workspace all-feature tests and workspace all-target/all-feature Clippy
pass. Minimal release workspace builds, warnings-denied rustdoc, formatting/diff
checks, dependency boundaries and ten policy tests pass. The prior eight-file
Short Text corpus also passes with its compound case now a positive control.
No dependency is added. Related changes were verified as one implementation batch.


## Physical Memo index reads

Single General Memo/Long Text indexes now use the bounded native index reader,
projections and explicit physical ranges. Full stored values remain intact;
only key construction retains the native 255-UTF-16-unit prefix. Short Text
still rejects values beyond its declared width. Memo prefix equality never
establishes full-value equality, so Memo unique constraints remain explicitly
unqualified. This does not enable compound text indexes, native Memo index
creation/writes or SQL locale comparison.

Eight original generated profile/collation combinations pass 648 independent
read/query/range/output-limit controls, with unchanged sources. Full-value
Len/Right expressions and projected key verification retain their existing
contracts. All 488 prior Short Text range controls also pass. Authored tests
cover prefix bytes, distinct suffixes and nullable Memo metadata without requiring
Java or claiming Access-authored fixture coverage. The engineering range helper
retains its 1 MiB default buffered-output ceiling and accepts an explicit allowance
through 16 MiB; failed default-output runs publish no partial stdout.
[Memo scope](NATIVE_TEXT.md#memo-index-reads) records Microsoft's differing current
and older indexing documentation separately from unrun native Access evidence;
[fixture records](FIXTURES.md#general-memo-index-reads) retain exact source hashes.

All 1,029 workspace all-feature tests and workspace all-target/all-feature Clippy
pass. Minimal release workspace builds, warnings-denied rustdoc, formatting/diff
checks, dependency boundaries and ten policy tests pass. No dependency is added.

## Explicit physical Short Text ranges

Library hosts now use `read_table_text_index_range` or projected Text selections
for explicit General Short Text start/end intervals. Bounds follow physical index
direction and include native overflow-digest framing; they are not SQL string
comparison endpoints. All/Null/range variants require the exact index profile.
UTF-16 endpoint limits and initial cancellation are checked before I/O. Encoded
bounds use the caller's budget, conservative fences retain duplicate endpoint
keys, and projected rows retain exact key verification. Selected snapshots never
acquire complete-table semantics; the query adapter adds no text pushdown.

All 488 independent physical cursor/projection/pruning/refusal controls pass on
the eight General fixtures, with unchanged sources and temporary mutation copies.
Five new authored/generated Rust tests cover public ranges/projections, typed
refusal, early cancellation/limits, equivalent/exclusive endpoints and 512 opaque
interval hulls. [Contracts](NATIVE_TEXT.md#physical-text-cursor-intervals)
keep native Access qualification and SQL locale behavior separate.

All 1,027 workspace all-feature tests, workspace all-target/all-feature Clippy,
minimal release workspace builds, warnings-denied rustdoc, formatting/diff checks,
dependency boundaries and ten policy tests pass. No dependency is added.

## Checked native Short Text indexes

General legacy/General 2010 single-column Short Text indexes now feed checked
complete native snapshots and scalar projections through ACE, the Access adapter
and explicit CLI index selection. Key decoding/re-encoding shares caller budgets
and cancellation. Every returned row must reproduce its physical key, including
unprojected text fields. Native entry-level 510-byte truncation/digest framing is
separate from character encoding; 24 reference vectors protect the original
bitwise implementation. Short-key unique constraints participate in read-only
checks; full-width unique keys stay explicitly unqualified rather than treating
digest collisions as proof of equal values. No text seek, SQL locale comparison,
text compound/Memo index or native text-index write is enabled.

All eight original Jet 4 / ACE 2007/2016/2019 General-version combinations agree
with independently executed reference cursors on exact rows and full/projected
queries. The expensive General 2010 cumulative check hits the fixed CLI work
limit without partial output; smaller checks complete. Three fresh mixed-schema
inputs pass 198 indexed projection/order/range/refusal controls. Source hashes
remain unchanged. [Text contracts](NATIVE_TEXT.md#checked-short-text-index-reads)
and [fixture records](FIXTURES.md#general-short-text-indexed-reads) retain exact
scope and distinguish this evidence from unrun native Microsoft Access tests.

All 1,022 workspace all-feature tests and workspace all-target/all-feature Clippy
pass, including registered text cursor order, row/key disagreement under projection,
budgets/final cancellation and unknown-sort refusal. Minimal-feature ACE checking,
minimal release workspace builds, warnings-denied rustdoc, formatting/diff checks,
dependency boundaries and ten policy tests pass. No dependency is added.

## Index key ownership

The private native index read policies no longer require keys to implement
`Copy` or `Clone`. Page ordering, parent fences, selection and NULL checks borrow
keys; selected leaf entries move into the completed traversal buffer. Unique
constraint checks borrow the previous key before replacing it. Physical mutation
profiles explicitly retain their existing `Copy` requirement. This is preparation
for owned variable-length text keys, not working text-index retrieval or writes.

Authored numeric-page tests exercise a deliberately non-Clone key across parent
fences, leaf ordering, range pruning and parent/sibling corruption. An owned
variable-length key test also retains duplicate-key and NULL constraint behavior.
These are traversal/ownership regressions, not native Microsoft Access evidence
or text-index qualification. [Remaining text work](NATIVE_TEXT.md#qualification-and-next-implementation)
still includes connecting the encoder to checked indexed reads and seeks.

All 1,015 workspace all-feature tests pass. Workspace all-target/all-feature
Clippy, the minimal-feature ACE check, formatting/diff checks, dependency
boundaries and ten policy tests pass. No public API, dependency or native
compatibility claim changed.

## General native text-key encoding

ACE now encodes General legacy/General 2010 text-index payloads with bounded
owned output, static borrowed weight lookups and native UTF-16 prefix/space,
diacritic, expansion, kana/repeat, surrogate, overflow-tail and descending rules.
Unknown descriptors fail without locale fallback. The payload API does not
activate text-index B-tree reads or SQL comparison, and never supplies truncated
index equality as full-string query equality. [Scope and evidence](NATIVE_TEXT.md#general-text-index-payload-encoding)
keep those remaining tasks distinct.

An independently executed reference encoder agrees on all 262,144 single-unit
keys and 560 multi-character keys. Generated raw-unit tests, resource/final
cancellation checks and the UTF-8 wrapper also pass. All 1,011 workspace
all-feature tests pass, along with workspace all-target/all-feature Clippy,
minimal release build, warnings-denied rustdoc, formatting/diff checks, dependency
boundaries and ten policy tests. The weight data and two adapted Rust encoder
files retain Apache-2.0 notices; ACE's SPDX metadata reflects the combined MIT /
Apache obligations. No registry dependency, runtime or native Access claim is added.

## Native text-order metadata

Roadmap item 2 now has working raw database/column sort-order inspection. ACE
owns the LCID/variant/version types and General-family recognition; unknown/zero
values stay explicit. CLI inspection reports those settings without changing
ordinal query comparison, LIKE or text-index eligibility. The root header mask
is shared with encryption rewrites; numeric/complex descriptor bytes are not
reinterpreted as text settings.

Focused schema/header, CLI, cancellation/budget tests and workspace Clippy pass;
ten independent plain/encoded inputs agree on database and selected text-column
descriptors with unchanged source hashes. [Native text scope](NATIVE_TEXT.md)
records the exact reference source and remaining General comparison/index/LIKE
work. No native Access text equivalence is claimed.

All 1,006 workspace all-feature tests pass. Workspace all-target/all-feature
Clippy, minimal release build, warnings-denied rustdoc, formatting/diff checks,
dependency boundaries and ten policy tests pass. Seven original encryption
inputs also pass independent add/remove/change and full-image recovery again
after the shared header-mask extraction.

## Native encryption creation

`encryption add` creates fresh native output copies, using Agile AES-256/SHA-512
for ACE and recoverable native encoding/passwords for Jet 4. It shares bounded
capture, SID remasking, credential prompts and non-clobbering output publication
with remove/change-password. ACE 2007 upgrades its header to the Agile-capable
profile; later families retain their bytes. No production table-write or signed
package-rewrite path is opened. [Creation scope](NATIVE_ENCRYPTION.md#add-native-encryption)
records the limits and independent comparison recipe; native Access save/reopen
remains unqualified.

Validation: all 1,002 workspace all-feature tests pass, with workspace all-target/
all-feature Clippy, minimal release build, warnings-denied rustdoc, formatting,
dependency boundaries and ten policy tests. All 43 independent corpus inputs
pass add/remove/change checks, including independently recovered complete images.
MacOS pseudo-terminal checks include add success and confirmation failure with
hidden input and restored echo. Windows console and native Access remain unrun.

## Bounded ACCDC database reads

`tablecraft-access::AccessInput` now detects CAB content, checks bounded single-file
geometry and reads its contained ACE database in memory. CLI inspection, tables,
rows, saved queries and SELECT reuse this input, including ordinary native password
codecs. Inspection reports the package signature as unverified. No disk extraction,
installation, execution, package rewrite or trust grant occurs.

Backend and CLI tests pass for independently constructed uncompressed/MSZIP CABs,
exact database bytes, encrypted payloads, rows/counts/SELECT, credential failures,
corruption, reservations/overlays, limits and cancellation. Native signed Access
packages, LZX package fixtures and Windows behavior remain unqualified. The
[exact profile](features/ACCESS_DISTRIBUTION.md#working-accdc-read-profile) and
[dependency review](DEPENDENCIES.md#access-deployment-packages-20-october-2026)
separate those limits from the implemented reader.

## Runtime/compiled filenames and package read scope

The native reader already selects profiles from content, without an extension
allowlist. Two CLI regressions cover tables/counts, row browsing, SELECT and
inspection under `.accdr`, `.accde`, uppercase `.ACCDE` and Jet 4 `.mde` names,
unchanged originals and invalid-content refusal. These are renamed original
fixtures, not native compiled-project evidence. The expanded
[read scope](features/ACCESS_DISTRIBUTION.md#required-read-scope) includes `.accdt`
template schema/stored data and `.accdc` contained databases; ACCDC now has the
bounded reader above. ACCDT reading and native compiled/package fixture
qualification remain pending. No VBA execution,
template instantiation or signature verification is added. Focused tests,
Clippy and formatting pass; no production code or dependencies change.

## Native remove/change-password copies

`encryption remove` and `encryption change-password` now produce fresh explicit
outputs for Jet 4 and ACE codes 2/3/5/6 using every implemented read cipher.
The engine owns password/header transforms, bounded image capture, same-width
unindexed SID remasking and cipher changes; the CLI owns hidden credentials and
tempfile publication without overwriting. The 64 MiB image limit is distinct
from the larger streaming-read envelope. No original-source, normal table-edit,
journal or active-content execution path is enabled by this operation.

Agile changes rewrap the same file key and preserve data ciphertext; legacy
Office changes re-encrypt data under fresh password-derived material. Jet
password changes and encryption removal remask native SIDs using shared checked
record geometry and physical relocation resolution. Plain ACE subversion one is
qualified without changing its native version bytes. [Exact boundaries and
verification](NATIVE_ENCRYPTION.md#remove-encryption-and-change-password) include
43 independently reopened inputs / 86 outputs, complete decoded SID comparisons,
exact user rows, unchanged sources, old-password/output protection and original
JCE mixed-parameter vectors. Windows/native Access tests remain unrun.

Validation: all 991 workspace all-feature tests across 135 suites pass. Workspace
all-target/all-feature Clippy, minimal release build, warnings-denied rustdoc,
formatting/diff checks, dependency boundaries and ten policy tests pass. Focused
CLI/native rewrite and plain-subversion regressions also pass after source-first
credential prompting and payload-free tracing. MacOS terminal checks cover hidden
old/new entries, confirmation mismatch, echo restoration and reopened output;
Windows console/native Access qualification remains pending.

## Legacy encoded native reads

`EncryptedSource` now selects qualified Jet 4 RC4 encoding, Office Binary RC4,
CryptoAPI RC4/SHA-1, standard AES-ECB/SHA-1 and compatibility AES alongside the
existing Agile codec. The CLI uses this source for encrypted reads and prompts
only when the selected codec requires credentials. Jet encoding uses its stored
header key and does not authenticate legacy database passwords/workgroup rights.
Raw reads, encrypted-image refusal and cumulative limits remain intact.

Independent checks match all 240 data pages across the three original legacy
inputs, exact rows/queries/counts, credential failures and unchanged source bytes.
All 985 workspace all-feature tests and workspace all-target/all-feature Clippy
pass, with formatting/diff checks, dependency boundaries and ten policy tests.
JCE/BC known answers cover all key widths and shared header families; derivative
routing is distinct from native-authored profile qualification. [Coverage and
remaining encryption management work](NATIVE_ENCRYPTION.md#profile-and-codec-coverage)
remain explicit; creation now works under the separate add contract above.

## Encryption commands and hidden password input

`encryption inspect PATH` owns the detailed inert encryption report, replacing
`inspect --encryption`. Basic `inspect PATH` adds page encryption status/type.
Neither authenticates or prompts. Qualified native reads prompt once with echo
disabled when standard input is a terminal and `-p` is absent; explicit `-p`
continues to support noninteractive callers. Plain, malformed and unsupported
sources do not prompt; wrong credentials publish no records. CLI prompt strings
are zeroized after opening the source. Remove/change-password copies now work under the explicit rewrite contract above;
add-encryption now shares that fresh-copy publication path.

Validation: 121 CLI tests across 23 suites; focused inspection assertions; CLI
all-target/all-feature Clippy; formatting/diff and dependency-policy checks pass.
Independent envelope/table/query comparisons remain unchanged. A real macOS
pseudo-terminal test verifies hidden entry, echo restoration, one prompt, correct
and incorrect passwords, and original-byte preservation. Windows terminal and
native Access tests remain pending.

## Password-enabled native table/query sources

Global CLI `--password` / `-p` now opens qualified Agile ACCDB sources for tables,
row pages/all-table browsing, queries/saved SELECTs and detailed inspection.
Unlocking and native operations share the command's cumulative allowance. Raw
header/encryption inspection needs no credentials. Wrong/missing passwords publish
no partial output or credential diagnostics; owned CLI password strings are wiped
at dispatch completion. Other encryption schemes and encoded UPDATE copies stay
blocked; explicit remove/change-password copies are now available.

`NativeReadSource` adds explicit native-page hooks to raw Read + Seek, and
`AgileSource` binds one verified decoder to its owned stable/offline input.
Native readers and Access adapters now use that boundary. File/Cursor/SharedFile/
offline transactions retain plain defaults; custom raw sources add an empty impl.
Raw I/O always returns original ciphertext, so whole-image preservation cannot
silently capture plaintext and still refuses encoded images. Catalog headers keep
the original encoding flag with a private authenticated read context. No Write
implementation or plaintext disk copy is introduced.

The original Agile inputs require ACE 2010 ordinary-table geometry, now explicitly
selected as `ReadProfile::Ace2010`. Advanced fields and further index variants
remain separately gated. Deterministic in-memory encrypted derivatives cover codes
2/3/5/6 and subversion 1 in CI, including Unicode credentials, cursor paging, raw
byte preservation and resource/cancellation failures. Those are routing/layout
controls, not native-authored generation evidence. The original encrypted inputs
also match independent exact-row/inventory/schema/count/ordered-query/aggregate
and credential controls, retaining their hashes. [Scope and evidence](NATIVE_ENCRYPTION.md)
remain distinct from unrun native Access/Windows/save qualification.

All 982 workspace all-feature tests passed. Dependency boundaries and ten policy
tests passed. Workspace all-target/all-feature Clippy, minimal release build and
warnings-denied rustdoc passed, with formatting/diff checks clean. The focused
journal/write regression suite also passes after the source-boundary migration.
CLI test fixtures reuse existing
workspace crypto dependencies; zeroize adds its alloc feature for the host-owned
password String. No package or registry version was added.

## Agile password verification and explicit page decoding

The native engine now validates bounded namespace-aware Agile XML descriptors,
verifies explicit passwords and returns a source-specific AES-CBC data-page
decoder. AES-128/192/256 and SHA-1/256/384/512 support separate password/data
settings. Native page IVs include the encoding key. Resource/cancellation checks
bound XML, iteration and decoding work; failed page decoding leaves ciphertext
unchanged. Typed errors distinguish unsupported profiles, malformed descriptors,
missing/incorrect credentials and excessive password length. No plaintext key or
credential is exposed through metadata/Debug/serialization. Maintained primitives
and supported owned-buffer zeroization are recorded in [dependencies](DEPENDENCIES.md).

All 102 engine library tests and ten CLI inspection tests pass. Known-answer CBC
tests and original independent JCE vectors cover AES-192, Unicode, all four hashes,
mixed settings and SHA-1 key padding. The engineering decoder matches independent
Jackcess Encrypt hashes for all 198 data pages across the two unchanged original
Agile inputs, including credential/page refusals. Those files have ACE 2010-family
headers; this is not ACE 16 table/query, native Access/Windows or rewrite evidence.
Envelope inspection retains its narrower inert contract. Workspace all-target/
all-feature Clippy, minimal release build and warnings-denied rustdoc pass;
formatting/diff checks, dependency boundaries and ten
policy tests pass. [Scope, references and millisecond opening measurements](NATIVE_ENCRYPTION.md)
record the remaining limits, including unverified ciphertext/file HMAC integrity.

Next: source-bound native reader integration, global CLI `--password`/`-p`, and
explicit remove/change-password commands with separate output files. Normal
encrypted table/query reads and rewrites remain blocked. Native Access comparisons
and the rest of the backend roadmap remain outstanding.

## Inert encryption envelope inspection

`tablecraft-ace::inspect_encryption` and CLI `encryption inspect PATH` now
distinguish zero-key pages, Jet 4 file encoding and bounded Office encryption
envelopes. Reports expose version/family/length and applicable outer flags, with
no password, key, salt, verifier or descriptor payload. Invalid bounds/fixed
prefixes have a typed encryption-envelope error. Unknown versions remain visible
without cipher inference. Reads remain cumulative-budgeted, cancellable and
read-only; CLI output remains atomic and supports table/TSV presentation.

ACE envelope inspection accepts compatibility subversions zero and one without
changing table-read qualification. Three original upstream encrypted fixtures
actually carry ACE 2010-family code 3/subversion 1 despite their 2007/2013 names;
the fourth has ACE 2007 code 2/subversion 0. All four match expected envelope
metadata and retain their source hashes; ordinary catalog reads remain refused.
This evidence does not establish an ACE 16 encrypted layout or native Access
compatibility. See [scope and references](NATIVE_ENCRYPTION.md) and
[fixture provenance](FIXTURES.md#encryption-envelope-reference-inputs).

All 94 native engine library tests and ten CLI inspection tests passed, including
seven new engine tests and three CLI tests for malformed/unknown envelopes,
format independence, subversion handling, resource/cancellation failures,
metadata-only output and unchanged sources. Workspace all-target/all-feature
Clippy, minimal release build, warnings-denied rustdoc, formatting and diff checks
passed. Dependency boundaries and all ten policy tests passed; no dependency
changed. The release CLI repeats the same four original-input controls.

Next: parse and validate Agile XML/cipher/KDF parameters, adopt maintained crypto
primitives for actual password verification and per-page decoding, and compare
decrypted rows with independent reference output. Standard/CryptoAPI and Jet 4
schemes need separate qualification. No encrypted data page is readable yet;
password input, authentication, encryption/password changes and encrypted writes
remain unimplemented. Broader backend acceptance and native Windows comparisons
remain outstanding.

## Typed Short Binary index ranges

`BinaryIndexSelection` now supplies borrowed All/Null/raw-byte intervals to
`read_table_binary_index_range` and projected index reads. Included/excluded/
unbounded endpoints use ascending byte order in either physical direction; NULL
and empty Binary remain distinct, and reversed ranges return no rows. Endpoints
are bounded before traversal without copying them. All/Null still require a
single qualified Short Binary index. The projection selector carries the bounds'
lifetime while returned snapshots remain owned.

The reader conservatively prunes disjoint physical spans and fetches only matching
roots. Ranges that include empty keys use a continuous physical hull, preserving
leaf-chain checks through descending-order gaps. Skipped subtrees/payloads remain
unvalidated; only All retains complete count checks. Existing limits, metering,
cancellation, write blockers and no-fallback contracts remain. Normal query sources
retain complete Binary snapshots and residual predicates; this adds explicit
library range controls, not automatic Binary predicate pushdown.

All 960 workspace all-feature tests passed. New goldens/generated hull properties
and four-profile integration cases cover exact/projection reads, boundary forms,
NULL/empty, reversed/oversized ranges, malformed selected keys, type qualification
and resource/cancellation failures. Workspace all-target Clippy, minimal release
build, warnings-denied rustdoc, formatting and diff checks passed. Dependency
boundaries and all ten policy tests passed; no dependency changed.

The unchanged original Binary inputs pass 200 independent interval/projection/
disjoint-subtree controls. A temporary damaged rightmost subtree remains unread
for NULL selection but fails full traversal without partial output. Expected rows
come from independent native cursors filtered by raw byte bounds. [Evidence](testing/BINARY_INDEX_RANGES.md)
distinguishes partial traversal from native Access/Windows/write qualification,
which remains unrun.
The original 72 complete Binary controls and 1,100 scalar CLI controls also pass
again with unchanged hashes.

Next, reprioritized 20 October: encrypted local-file reads, starting with ACE 16
descriptor/password/page-codec qualification, followed by the applicable ACE 2007
and Jet 4 schemes. Encryption is not implemented; the existing nonzero encoding-key
gate still rejects encoded table reads. See the [read/query roadmap](ROADMAP.md#remaining-local-file-readquery-work)
for acceptance checks. Binary compound keys, binary primary/foreign fixture
qualification and automatic Binary query predicate pushdown remain subsequent
work. Broad function-sidecar and SQLite expansion stay behind native reader/query
reliability.

## Single Short Binary index reads and checks

Single-column native Short Binary indexes now use bounded segmented key decoding
for full/projected traversal and read-only constraint checks in every priority
profile. Existing tree ownership, separators, prefix reconstruction, sibling links,
root references, counts and exact row/key checks remain. Empty Binary is distinct
from NULL and precedes nonempty physical keys even descending, because its marker
is an encoded prefix. Descending continuation/final tags and padding are checked.
The shared page decoder now passes only key bytes, excluding row/child trailers.
Typed integer seeks, binary compound keys, ignore-NULL indexes and OLE/Text keys
remain unavailable. Native binary-indexed writes are explicitly blocked.

All 956 workspace all-feature tests passed, including codec goldens/generated
properties, four-profile synthetic reads/checks, malformed padding, key/row
disagreement, typed seek rejection and write refusal. Workspace all-target Clippy,
minimal release build, warnings-denied rustdoc, formatting and diff checks passed.
Dependency boundaries and all ten policy tests passed; no dependency changed.

Four new original inert Jackcess inputs pass 72 independent tagged/full/projected/
check/refusal controls, including 1,000-row ascending/descending node-root trees,
NULL/empty/duplicate values and lengths through 255 bytes. All hashes remain
unchanged. Existing GUID comparisons retain 2,034 ordered rows per ACE profile,
and original scalar CLI inputs retain 1,100 controls. [Evidence](testing/BINARY_INDEX_READS.md)
and [fixture provenance](FIXTURES.md#native-binary-index-inputs) distinguish this
read scope from pending native Access/Windows/save/primary/foreign qualification.

Next: binary compound keys and typed binary range reads before locale-dependent
Text index work. Broad function-sidecar and SQLite expansion remain behind native
reader/query reliability.

## Qualified deleted-column row histories

Older native rows can now retain deleted fixed/variable identities without
renumbering surviving fields. Present fixed offsets and surviving variable indexes,
together with past/future missing logical identities, bound the stored row tail.
Deleted payloads stay ignored; absent current values remain NULL/FALSE. Ambiguous
fixed-only/variable-tail histories still fail explicitly, including zero-width
projections. Duplicate surviving variable slots are now malformed metadata.
Equal-count decoding retains its previous acceptance rules. Native edit gates
explicitly reject deleted/gapped declarations, so this read work does not qualify
historical writes, DDL or type changes.

The full workspace all-feature run passed 949 tests. The final focused five-test
history suite also passes, adding a further variable-count interval regression
after that run. Coverage includes all four profiles, removed fixed/variable bytes,
absent Boolean bits, empty projections, impossible tail counts, duplicate slots,
ordinary/batched/projected reads and write refusal even for a current target row.
The malformed history-template regression now expects the earlier typed metadata
failure. Workspace Clippy and final engine all-target/all-feature Clippy, minimal
release build, warnings-denied rustdoc, formatting and diff checks passed. Dependency
boundaries and all ten policy tests passed; no dependency changed.

Four original append-only inputs and separate deleted-declaration derivatives pass
60 full/cursor/CLI/query/refusal controls against independent getters, with all
base/derivative hashes unchanged. Original scalar CLI checks retain 1,100 controls;
modern ACE exact-carrier comparisons also pass unchanged. [Evidence](testing/COLUMN_HISTORIES.md)
and [fixture provenance](FIXTURES.md#deleted-column-history-derivatives) distinguish
engineering derivatives from unrun native Access deletion/save/Windows qualification.
The private guides remain ignored.

Next: broaden native binary index reads before locale-dependent text index work;
ambiguous schema histories still need additional native evidence. Broad function-
sidecar and SQLite expansion remain behind native reader/query reliability.

## Explicit CLI native query byte allowance

Native `query run --database PATH` now accepts `--native-bytes BYTES` for cumulative
native read/allocation accounting, including `--saved` retrieval. Clap validates
decimal bytes in 1..=268435456 before file I/O. Omission retains 16 MiB; the option
requires a native database and is unavailable to SQLite, source-free retrieval,
parsing/evaluation, inspection, syntax-only saved-query output and writes. Native
row/work/structural limits, query execution budgets and buffered output limits
are unchanged. No budget is reset per table, saved dependency or cursor batch.
The executable owns this policy; library budget controls remain explicit.

All 116 CLI tests passed, including four-profile 6,000-page queries that fail
under defaults and succeed at 64 MiB, inclusive range endpoints, invalid/scoped
arguments rejected before I/O, exact output and immutable inputs. Existing saved
execution also runs with the explicit allowance. Workspace all-target/all-feature
Clippy, minimal release build, CLI warnings-denied rustdoc, formatting and diff
checks passed. Dependency boundaries and all ten policy tests passed; no dependency changed.

Independent large-map fixtures retain 24 engineering query controls and now pass
32 CLI controls: five complete query results, default-budget refusal and separate
execution/output-budget refusals per profile. Original scalar CLI inputs retain
1,100 controls. All hashes are unchanged. [Reproduction](testing/BATCHED_QUERY_SNAPSHOTS.md)
distinguishes stored getter comparison from native Access qualification, which
remains unrun. [Query documentation](NATIVE_QUERIES.md) records option scope.

Next: audit and qualify remaining native row/schema-history variants, starting
with deleted/gapped columns, before broad function-sidecar or SQLite expansion.

## Batched sequential query snapshot loading

NativeSource now uses `read_table_batched` / `read_table_projection_batched` for
sequential full and projected loads. Checked 256-row cursor batches build one
complete snapshot under the same cumulative native controls. This removes the
4,096-address vector from those query paths while preserving the 16,384-row
snapshot ceiling, schema/count validation, cache reuse and no partial visitor or
stdout publication. Aggregate vector allocations are charged in addition to
batch allocations; scalar payloads move without cloning. Existing direct full
read APIs, explicit index/range reads and write consumers keep their own limits.

All 942 workspace all-feature tests passed. New cases cover all four profiles,
6,000-page scalar/mixed query reads, full/projected cache reuse, late failure with
no rows/cache, explicit successful retry, cumulative byte/row exhaustion, exact
256-row boundaries, the snapshot row cap and final cancellation. The CLI large-map
test now checks byte-limit refusal and empty stdout, replacing the obsolete
address-list structural diagnostic. Workspace all-target Clippy, minimal release
build, warnings-denied rustdoc, formatting and diff checks passed. Dependency
boundaries and all ten policy tests passed; no dependency changed.

Existing independent 6,000-page fixtures pass six query controls each: full sorted
IDs, descending TOP, filtered tail, Count(*), a bounded correlated query and
default-budget refusal. Successful engineering calls explicitly supply 64 MiB
native bytes; the ordinary CLI budget remains 16 MiB. All fixture hashes remain
unchanged. [Evidence and reproduction](testing/BATCHED_QUERY_SNAPSHOTS.md) distinguish
bounded complete snapshot loading from streaming queries and pending native
Microsoft Access/Windows/live-file qualification.
The original 1,100 scalar CLI controls and 28 large-table page/count/refusal
controls also passed again with unchanged hashes.

Next: expose deliberate native read-budget configuration at the CLI boundary so
larger qualified files can use the new library path without weakening defaults;
then continue native read variants and query scaling. Broad function-sidecar and
SQLite expansion remain behind native reader/query reliability.

## Explicit projected scalar cursors for mixed schemas

`TableCursor::open_projection` and `NativeBrowser::projected_cursor` now select
ordinary scalar fields without qualifying unrelated unavailable payloads. They
retain all native schema declarations and return selected values in native logical
order, using the existing ASCII-insensitive name policy. Missing/duplicate names
and unavailable selected fields fail before user record reads, including on empty
tables. Empty selections return zero-width rows while fetched row framing and live
counts are checked. Full-snapshot and cursor selection qualification share one
fallible, metered implementation. Cumulative host budgets, cancellation, terminal
fetch failures and the ordinary all-column contract remain intact.

All 937 workspace all-feature tests passed, including four new projection cases
and the shared browser control case. Tests cover 6,000-row mixed tables in every
read profile, native-order subsets, unavailable declarations, duplicate/missing
names, empty selections and malformed fetched framing. Workspace all-target
Clippy, minimal release build, warnings-denied rustdoc, formatting and diff checks
passed. Dependency boundaries and all ten policy tests passed. No library or
fixture bytes changed.

All 6,000 full and projected getter rows match Jackcess in each existing large-map
fixture at batch sizes 1, 127 and 250. Existing ACE 2016/2019 calculated-scalar
fixtures pass five controls each: ordinary Id results in three batch sizes and
explicit full/calculated-selection refusals. All hashes remain unchanged. The
original 1,100 inventory/page/projection/query controls and modern ACE exact-carrier
comparisons also pass unchanged. The
[cursor evidence](testing/RESUMABLE_TABLE_READS.md) distinguishes library APIs and
engineering output from ordinary CLI behavior. Native Access and live-file
qualification remain pending.

Next: wire bounded native continuation into query-source consumption, retaining
its schema, budget, repeat-read and atomic-output contracts. Broad function-sidecar
and SQLite work remain behind reader/query reliability.

## Source-bound resumable scalar batches

`TableCursor` now retains qualified schema, map position and current page/slot
across bounded scalar fetches. `NativeBrowser::cursor` exposes the same traversal
with its original cumulative controls. Independent offset paging shares the
physical traversal implementation. Errors after range validation make continuation
terminal, publish no partial batch and preserve the count of previously delivered
rows; budgets are not refunded. Invalid sizes leave a ready cursor unchanged.
All usage metadata is still checked before user data. Stable/offline ownership
remains required; no native snapshot, transferable bookmark or shared-file lock
is claimed. CLI offsets and materialized queries retain their existing limits.

The workspace all-feature run passed 929 tests; three additional final cursor
regressions passed in the focused 16-test page suite. Coverage includes all four
profiles, 6,000-page inline/indirect traversal, exact-once data-page reads, generated
row sequences, deletion/relocation boundaries, EOF count mismatches, cancellation,
failed-fetch closure and the browser's cumulative row budget. Workspace all-target
Clippy, minimal release build, warnings-denied rustdoc, formatting and diff checks
passed. Dependency boundaries and all ten policy tests passed; no dependency changed.

All 6,000 rows in each unchanged independent large-map fixture match Jackcess
at batch sizes 1, 127 and 250. Original scalar fixtures retain 1,100 controls;
the original page/count/refusal checks remain separate from complete cursor reads.
[Reproduction and measurements](testing/RESUMABLE_TABLE_READS.md) record a 1,000-row
scan in 25-row batches: 10.627 ms using repeated offsets versus 3.563 ms through
continuation, about 2.98 times faster in the memory-backed harness. These are
milliseconds per scan, not single-value timings or disk performance measurements.
Native Access and live-file qualification remain pending.

Next: integrate bounded continuation with ordinary host row consumption and
projected reads before expanding query materialization. Broad function-sidecar
and SQLite work remain behind native reader/query reliability.

## Incremental owned-page addresses for bounded row paging

Bounded row pages now validate complete usage metadata, then enumerate sorted
owned addresses without a per-address vector. The cursor retains the small usage
row and one bitmap page, caching the first nonempty indirect bucket and checking
any later reread before yielding. Shared bitmap validation preserves signatures,
widths, bounds, ordering, byte/work metering and cancellation. Full snapshots,
catalogs, indexes and writes keep their existing bounded materialized ownership
contracts; physical reads and deep-offset limits remain. No dependency changed.

All 923 workspace all-feature tests passed, including synthetic 6,000-page paging
for all profiles, later-map corruption before data reads, reread corruption,
malformed/empty map roots, dense bitmaps, independent bit properties and CLI
limits/refusals. Final CLI assertions passed again. Workspace all-target Clippy,
minimal release build, warnings-denied rustdoc, formatting and diff checks passed.
Dependency boundaries and all ten policy tests passed.

New independent Jet 4/ACE 2007/2016/2019 fixtures each contain 6,000 rows on 6,000
owned pages. All 28 selected-page/count/refusal controls passed with unchanged
hashes. Existing scalar inputs retained 1,100 controls, and modern ACE checks
passed unchanged. [Evidence](testing/ALLOCATION_MAP_PAGING.md) separates stored
getter comparison from pending native Access/SQL/live-file qualification.

Hyperfine measured the small example at 57.49 µs/page before and 57.51 µs/page
after, with variation much larger than that difference; no speedup is established.
The newly readable ACE 2019 large table measured 32.88 µs/page under the memory-
backed harness, a different workload. Prior production code refused that table.
Next: resumable bounded paging, so reaching later records need not rescan every
earlier root; full query streaming/materialization remains separate work.

## Larger read sources with separate image bounds

Qualified native read sources now accept page-aligned lengths up to 2 GiB.
Complete NativeImage capture and OfflineReadTransaction retain an explicit 64 MiB
image ceiling before allocation, including when a caller supplies a larger budget.
Existing page/map/row/payload/work/output limits and offline-source requirements
remain. No layout, encoding, query materialization or write qualification expanded.
[Large-source evidence](testing/LARGE_FILE_READS.md) separates the larger envelope
from unrestricted large-table or native Access compatibility.

All 914 workspace all-feature tests passed. Synthetic cases fetch a row from the
final page of a 2 GiB source under a 1 MiB budget in all four read profiles, with
no whole-source buffer and less than 128 KiB physical reads. CLI tests cover
128 MiB far-page discovery, counts, inspection, paging and SELECT; oversized Jet 4
updates publish no copy. Image capture rejects before bulk reads/allocation, and
transaction rejection releases the OS lock. Workspace all-target Clippy, minimal
release build, warnings-denied rustdoc, formatting and diff checks passed.
Dependency boundaries and all ten policy tests passed; no dependency changed.

Independent checks passed 1,100 controls on original scalar inputs plus 1,100 on
128 MiB padded derivatives, with all hashes unchanged. Modern ACE checks passed
unchanged too. [Fixture records](FIXTURES.md#larger-read-source-derivatives) retain
provenance and explain that padding does not establish native global allocation
validity. Native Access, live-file consistency and authentic large-database
qualification remain pending. Next: lazy allocation-map traversal for large-table
paging; eager maps and complete query snapshots still retain their existing limits.

## Checked Decimal arithmetic and aggregates

The private rust_decimal boundary now supplies checked +, -, *, / when a Decimal
operand participates, including exact fixed and normalized floating imports.
Exponentiation selects finite Double; integer division/Mod retain the existing
Int64 coercion profile. Bounded intermediate scale reduction, half-even rounding,
overflow, zero-divisor and non-finite errors have explicit contracts and goldens.
The shared query Sum/Avg fold now handles Decimal, including grouped/correlated
queries, SQL equality across result scales and eager conversion failures.
[Contracts](EXPRESSIONS.md#checked-decimal-arithmetic) distinguish this implemented
profile from full Variant promotion and native Access/OleAut qualification.
Native field codecs, core representation/Serde and dependency features are unchanged.

All 908 workspace all-feature tests passed. The final Decimal query suite passed
again after adding duplicate-scale and nullable-conversion coverage. Workspace
all-target Clippy, parser-free expression tests, minimal release build, warnings-
denied rustdoc, formatting and diff checks passed. Dependency boundaries and ten
policy tests passed. Existing scalar files passed 1,100 independent controls across
Jet 4/ACE 2007/2016/2019; modern ACE checks passed too, with source hashes unchanged.
[Fixture evidence](FIXTURES.md#checked-decimal-arithmetic-read-regression) remains
independent getter/rational evidence rather than native Access SQL conformance.

[Hyperfine measurements](testing/DECIMAL_ARITHMETIC.md) over ten million operations
per run measured about 12.6 ns per operation for the Decimal add/subtract/multiply
mix and 38.9 ns per Decimal division, versus 5.1 ns for the same Int64 arithmetic
mix. Whole-process amortized timings include overhead and do not establish engine
throughput. Reader/query reliability remains ahead of broad function-catalog work.

## Full Decimal scalar envelope and explicit CDec

Core Decimal now validates signed 96-bit magnitude with retained scale 0..=28,
including the bounded 29-digit coefficient band. Optional Serde keeps its wire
shape and validates the widened constructor. Native schema/codec precision stays
1..=28; field bounds and the legacy index sign band no longer depend on the
scalar maximum. Native write-preview regressions reject wider values unchanged.
[Carrier contracts](CORE_TYPES.md#scalar-representation-and-sources) explicitly
record this scalar validity expansion and the separate field boundary.

CDec now shares the closed conversion layer across scalar, retrieval and qualified
saved SELECTs. Exact numeric inputs retain digits and Decimal/Currency scale.
Floating inputs use rust_decimal normalized conversion; CCur retains its prior
retained-IEEE rule. NULL/arity/type/range/eager-error/UPDATE gates remain explicit.
[Expression contracts](EXPRESSIONS.md#explicit-decimal-conversion-and-expression-range)
distinguish the engineering profile from pending native Access/OleAut comparison.
General Decimal arithmetic, Decimal aggregates and Variant promotion remain absent.

All 899 workspace all-feature tests passed. After the diagnostic text update,
core all-feature tests passed again. Workspace all-target Clippy, parser-free
expression tests, minimal release build, warnings-denied rustdoc, formatting and
diff checks passed. Dependency boundaries and ten policy tests passed; no package
or feature was added. Original Jet 4/ACE 2007/2016/2019 scalar files passed 1,080
controls with unchanged hashes. Modern ACE scalar controls also passed unchanged.
[Fixture evidence](FIXTURES.md#explicit-decimal-conversion-read-regression) uses
independent getter/exact-carrier models, not native Access SQL equivalence.

## Shared declared Decimal field constraints

Shared schema descriptors now retain validated field Decimal precision and exact
retained scale. Native column, full/index/page/projected schemas carry these rules
into query source validation; reusable plans distinguish precision/scale changes.
Different-scale values fail without hidden rounding or rescaling, including zero.
NULL and bare unconstrained carriers retain their previous rules.
[Schema contracts](SCHEMA.md#declared-decimal-field-constraints) separate the field
precision limit from future wider expression ranges. CDec, the wider expression
carrier and Decimal arithmetic remain unimplemented in this slice.

All 892 workspace all-feature tests passed, including every valid precision/scale
pair's signed boundaries, provider row/schema drift failures and native declaration
preservation for all four read profiles. The final projected-read regression also
passed after expansion. Workspace all-target Clippy, minimal release build,
warnings-denied rustdoc, formatting, documentation file links and diff checks
passed. Dependency boundaries and all ten policy tests passed; no dependency changed.
Original scalar inputs retained 1,036 cross-profile controls and unchanged hashes;
modern ACE checks also passed unchanged. These independent regressions do not
qualify native Microsoft Access expression or field-assignment behavior.

## Shared explicit Currency conversion

CCur now extends the shared conversion layer across scalar SELECTs, retrieval and
qualified saved SELECTs. A private rust_decimal boundary supplies checked numeric
conversion and explicit four-place midpoint-to-even rounding; existing Currency
and Decimal carriers/codecs remain unchanged. Currency identity, Boolean -1/0,
exact field Decimal inputs, retained IEEE inputs, signed-i64 bounds, NULL/errors
and unsupported input types have focused tests. Broader Decimal arithmetic and
Variant promotion remain unsupported. [Contracts](EXPRESSIONS.md#explicit-currency-conversion)
separate the implemented model from pending native Access/OleAut qualification.

All 889 workspace all-feature tests passed. Workspace Clippy, parser-free expression
tests, default release build, warnings-denied rustdoc, formatting and diff checks
passed. Original scalar files passed 1,036 cross-profile controls, and modern ACE
scalar checks passed; original file hashes remain unchanged. These independent
getter/model checks do not establish native Access equivalence.

Hyperfine measured approximately 53 ns per Decimal conversion and 511 ns per
floating-input conversion, versus 14 ns for the existing integer-kernel Decimal
reference. These whole-process amortized costs cover different fixed workloads;
they do not establish query-engine speed. [Benchmark details](testing/CURRENCY_CONVERSIONS.md)
and [dependency records](DEPENDENCIES.md#explicit-currency-conversion-dependency)
record scope and enabled features.

## Shared explicit Single and Double conversions

CSng/CDbl now extend the shared conversion layer across scalar SELECTs, retrieval
and qualified native saved SELECTs. Explicit requested IEEE rounding is separate
from implicit arithmetic PrecisionLoss checks. Integer inputs convert directly
to the requested width; finite widening/same-width operations preserve exact bits,
including signed zero/subnormals. Currency/Decimal use their exact existing
canonical spelling in a bounded stack buffer and Rust parsing at the target width,
avoiding coefficient-as-float division and an intermediate Double for CSng.
This adds no heap-backed formatting or general decimal engine.

NULL/non-finite/unsupported input remains an explicit error; finite Double values
outside Single's finite magnitude range fail with Overflow. In-range narrowing
uses gradual IEEE underflow to signed subnormal/zero. [Contracts and primary
references](EXPRESSIONS.md#explicit-single-and-double-conversions) distinguish the
implemented rounding model from pending native Access/OleAut edge-range behavior.
All existing call/traversal, grouping, eager-error and UPDATE restrictions remain.

All 883 workspace all-feature tests passed, including Decimal/Currency double-
rounding goldens, generated finite Single bit round trips, exact target widths,
range/NULL failures, explicit versus implicit precision, grouped/correlated
retrieval and buffered CLI/saved queries. Workspace Clippy, parser-free expression
tests, release build, rustdoc, formatting and diff checks passed. Two independent
engineering-oracle tests passed. No dependency is added.

Existing original Jet 4/ACE 2007/2016/2019 scalar inputs passed 980 controls with
unchanged hashes, including exact IEEE result comparisons and NULL refusals.
Original modern ACE scalar inputs retained carrier/index/query checks and passed
both explicit float conversions at Large Number extrema/beyond double precision,
unchanged. [Fixture evidence](FIXTURES.md#explicit-floating-conversion-read-regression)
records the independent rational/IEEE model; native Access SQL remains unrun.

## Shared checked integer and Boolean conversion functions

CByte/CInt/CLng/CLngLng/CBool now share parser-independent conversion kernels
across scalar SELECTs, retrieval and qualified native saved SELECTs. Integer
results preserve Byte/Int16/Int32/Int64 widths with existing exact half-even
rounding and checked bounds. CBool tests exact zero/nonzero without narrowing.
Concrete NULL conversion fails explicitly; the internal nullable coercion helper
retains its prior behavior. CByte(Boolean true) returns 255 while numeric -1
still overflows Byte; other signed targets retain true=-1. No locale-sensitive
Text/date/binary/GUID coercion, pointer-width conversion or VBA runtime is enabled.
[Rules and primary references](EXPRESSIONS.md#checked-numeric-and-boolean-conversion-functions)
separate the implemented numeric profile from unqualified native/platform behavior.

Every operand retains projection/correlation and aggregate/group validation;
wrong shapes and UPDATE calls still fail, and unselected eager errors remain
visible. All 878 workspace all-feature tests passed, with generated Int64 identity/
truth checks, exact Currency/Decimal/IEEE boundaries, NULL/error distinctions,
grouped/correlated retrieval and buffered CLI/saved-query regressions. Workspace
Clippy, release build, parser-free expression tests, rustdoc, formatting and diff
checks passed. No dependency is added.

Existing original Jet 4/ACE 2007/2016/2019 files passed 868 controls with unchanged
hashes. Original modern ACE scalar inputs additionally retained exact carrier/
index/query checks and passed exact Large Number conversion, numeric truth and
NULL-refusal controls, unchanged. [Fixture evidence](FIXTURES.md#checked-numeric-conversion-read-regression)
uses independent exact rational/getter models, not native Access SQL. Windows
NULL/Boolean/coercion/platform qualification remains pending.

## Shared Gregorian DatePart and function binding slice

DatePart now shares all ten interval selectors across scalar SELECTs, retrieval
and qualified native saved SELECTs, returning Int16 or NULL. Shared interval
identities distinguish day of year from day of month without changing DateAdd/
DateDiff day arithmetic. Shared week settings preserve omission versus NULL,
validate explicit first-day/week rules and reject system policy without a host.
Jiff supplies civil validation/day/weekday facts; bounded week-label arithmetic
handles previous-year labels and both Extended year endpoints without storing
out-of-envelope dates. The Monday/four-day model preserves Microsoft's documented
2003 final-Monday 53-versus-1 anomaly; it is not advertised as ISO numbering.
[Contracts and primary references](EXPRESSIONS.md#gregorian-datepart) distinguish
this explicit model from pending native Access qualification of other boundaries,
rounding/coercion/NULL precedence and Extended behavior.

Every provided setting participates in projection/correlation, aggregate/group
checks and eager evaluation. Invalid call shapes and UPDATE calls remain rejected.
Function/aggregate binding now lives in binding/calls.rs, keeping the expression
walker focused on traversal and preserving retrieval/editing gates.

All 873 workspace all-feature tests passed; final affected-package and parser-free
tests also passed, including all week settings at Extended year 1/9999 January/
December endpoints. Workspace Clippy, release build, rustdoc, formatting and diff
checks passed. Existing original Jet 4/ACE 2007/2016/2019 scalar files passed 584
controls with unchanged hashes; original modern ACE scalar inputs also retained
exact carrier/index/query checks and passed nine Extended DatePart projections,
unchanged. [Fixture evidence](FIXTURES.md#gregorian-datepart-read-regression) separates
independent getter/model results and documentation goldens from native Access
runs, which remain unperformed. No dependency, fixture or third-party code added.

## Shared Gregorian DateDiff

DateDiff now shares signed Int32 boundary counting across scalar SELECTs,
retrieval and qualified saved SELECTs. All ten interval spellings use bounded
Gregorian calendar arithmetic. Calendar ww supports explicit first-day boundaries;
w counts complete starting-weekday periods. Three required arguments and two
optional settings retain omitted positions separately from NULL, and every
provided argument participates in pruning/correlation/group validation. NULL date
propagation follows interval/settings and date-carrier checks; eager expression
errors remain visible. UPDATE still rejects calls.

System week settings now resolve caller-issued context; missing settings still
fail explicitly under the [host week contract](WEEK_SETTINGS.md).
Nondefault first-day settings for w remain unsupported because primary-reference
anchor wording needs native resolution. Week labels do not change the sequence
of ww boundaries. Legacy rounding, optional-setting coercion/NULL precedence and
mixed/Extended date behavior remain explicit implemented contracts awaiting
native Access qualification. [Rules and primary references](EXPRESSIONS.md#gregorian-datediff)
record these limits. Shared interval lookup retains DateAdd's distinct w-as-day
behavior, and Long overflow never widens or wraps.

All 867 final workspace all-feature tests passed, including generated day counts,
calendar versus elapsed boundaries, week defaults/omission, NULL/type/overflow
checks, optional-argument grouping, correlated retrieval and buffered CLI/saved
execution. Workspace Clippy/rustdoc and final affected-package checks, parser-free
tests, release build, formatting and diff checks passed. Existing original
Jet 4/ACE 2007/2016/2019 scalar files passed 536 controls with unchanged hashes.
Existing modern ACE inputs retained carrier/index/query checks and passed Extended
zero/one-second and Long-overflow refusal controls, also unchanged.
[Fixture evidence](FIXTURES.md#gregorian-datediff-read-regression) remains independent
getter/model comparison, not native Access SQL. No dependency or fixture added.

## Shared Gregorian DateAdd

DateAdd now shares a parser-independent eager kernel across scalar SELECTs,
retrieval and qualified native saved SELECTs. All ten Access interval spellings
use checked Jiff civil arithmetic; month/year changes constrain to valid month
ends and w adds calendar days rather than workdays. Counts reuse exact signed
Long conversion with half-even rounding. Valid interval/count with NULL date
returns NULL; invalid interval/count/shape or unsupported date coercion fails.
All operands retain projection, aggregate/group and correlation traversal, and
UPDATE still rejects calls.

Legacy results use the shared whole-second date-function profile within years
100–9999. Extended results preserve their carrier, 100 ns fraction and years
1–9999. Overflow is explicit. Zero-count legacy adjustments can canonicalize the
new value; original stored input bytes remain untouched. [Contracts and Microsoft
references](EXPRESSIONS.md#gregorian-dateadd) distinguish implemented rounding,
NULL precedence and Extended behavior from pending native Access qualification.

All 861 workspace all-feature tests passed, with focused interval/month-end,
leap-year, fractional/negative count, overflow, exact Extended fraction, generated
reversibility, grouped/correlated query and buffered CLI/saved-query coverage.
Workspace Clippy, parser-free expression tests, release build, rustdoc, formatting
and diff checks passed. Existing original Jet 4/ACE 2007/2016/2019 scalar files
passed 488 controls with unchanged hashes. Existing modern ACE inputs retained
exact tagged carriers/index/query checks and passed zero-count/fraction-preserving
leap-day Extended DateAdd projections, also unchanged. [Fixture evidence](FIXTURES.md#gregorian-dateadd-read-regression)
is independent getter/model comparison; native Access SQL remains unrun. No new
dependency, third-party code or fixture is added.

## Shared Gregorian date extraction

Year/Month/Day/Hour/Minute/Second now share parser-independent date kernels across
scalar SELECT evaluation, table retrieval and qualified native saved SELECTs.
They return Int16 or NULL without implicit text/numeric conversion. Legacy DATE
reuses core's Jiff calendar conversion then explicit nearest-second HalfExpand
rounding with checked carry; Extended Date/Time reads exact validated components
without legacy conversion or fractional rounding. Stored bits remain unchanged.
Arguments retain pruning/correlation/group rules, wrong call shapes fail binding
and UPDATE still rejects calls. Native rounding/coercion/alternate-calendar and
Extended Date/Time SQL qualification remains pending; see
[contracts and primary references](EXPRESSIONS.md#gregorian-date-component-built-ins).

All 855 workspace all-feature tests passed, including whole-second generated
round trips, negative dates, Gregorian boundaries/carry, NULL/type failures,
exact Extended components, grouped/correlated retrieval and buffered CLI errors.
Workspace Clippy, parser-free expression tests, default release build, rustdoc,
formatting and diff checks passed. Dependency-boundary checks and ten policy tests
passed. Expressions now directly uses the already reviewed Jiff dependency for
working non-parser date kernels; versions/features/lockfile are unchanged.

Existing original Jet 4/ACE 2007/2016/2019 scalar fixtures passed 448 controls with
unchanged bytes. Original modern ACE scalar files additionally passed six
Extended component projections, exact tagged carrier/index/query checks and
unchanged-hash checks, including years 1/9999 and 100 ns fractions.
[Fixture evidence](FIXTURES.md#gregorian-date-component-read-regression) remains
independent inert getter/model comparison, not native Access SQL evidence.
No new third-party code, fixture or runtime is adopted.

## Shared bounded text built-ins

Len, LTrim/RTrim/Trim, Left/Right and two-/three-argument Mid now share
parser-independent borrowed text kernels across scalar SELECT evaluation, bound
retrieval and qualified native saved SELECTs. Len returns Int32 UTF-16 length;
trims remove U+0020 only; slices use UTF-16 positions with exact Long-range integer
arguments. NULL input stays NULL, while supplied expressions still evaluate in
order. Scans consume steps/check cancellation and outputs reserve fallibly after
charging shared byte limits. Binding retains argument fields, correlations and
aggregate/group rules; UPDATE still rejects calls.

Slices that would produce an unpaired surrogate fail with UnrepresentableText;
text cannot yet represent those results losslessly. Complete Variant coercion,
locale behavior, byte/$ variants and native Access SQL equivalence remain gaps.
[Contracts and primary references](EXPRESSIONS.md#bounded-text-built-ins) describe
these explicit limits without claiming native qualification.

All 849 workspace all-feature tests passed, including generated Unicode slices
against independent UTF-16 buffers, typed domain/boundary failures, budgets,
cancellation, grouped/correlated retrieval and CLI buffered failures. Workspace
Clippy, parser-free expression tests, default release build, rustdoc, formatting
and diff checks passed. Existing original Jet 4/ACE 2007/2016/2019 scalar files
passed 424 inventory/page/projection/query controls with unchanged bytes, including
eight text operations over Text and Memo getters. [Fixture evidence](FIXTURES.md#text-built-in-read-regression)
remains independent storage/getter/model comparison, not native Access SQL.
No dependency or fixture is added.

## Shared query-context Nz

`Nz(value[, fallback])` now works through the parser-independent expressions
kernel, scalar SELECT evaluation, bound retrieval and qualified native saved
SELECTs. Non-NULL values retain their exact carriers; NULL selects the explicit
fallback, including NULL, or empty Text when omitted. Supplied operands evaluate
in order under the existing eager call profile. Projection/correlation mapping
and aggregate/group checks retain both operands. Wrong arities, omitted positions,
qualifiers, external calls and UPDATE calls remain rejected.
[Contracts and Microsoft reference](EXPRESSIONS.md#query-context-nz) distinguish
query defaults from VBA Empty and unqualified native optimizer/coercion behavior.

The full all-feature workspace run passed 840 tests before the final cancellation
regression was added; subsequent affected-package tests cover the final code.
Workspace Clippy, default release build, rustdoc, parser-free expression tests,
formatting and diff checks passed. Four existing original Jet 4/ACE 2007/2016/2019
scalar files passed 360 inventory/page/projection/query controls, including both
Nz forms around fourteen stored carriers, with unchanged source bytes.
[Fixture evidence](FIXTURES.md#query-context-nz-scalar-regression) remains independent
getter/model comparison, not native Access SQL qualification. No dependency added.

## Shared IsNull and eager IIf built-ins

The expressions library now owns closed built-in lookup, NULL testing and exact
conditional value selection. Scalar evaluation and retrieval use the same kernels.
IIf evaluates both branches; unselected errors/allocations remain visible. Boolean,
exact numeric and finite floating conditions use zero/nonzero truth, with NULL
selecting falsepart. Chosen values retain their carriers without promotion; text/
date/GUID/binary truth conversion remains unsupported. These are pure built-ins,
not VBA/UDF execution. UPDATE retains its existing no-call policy.

Binding/group validation and projection/correlation mapping retain every built-in
argument, including aggregate arguments and both conditional branches. Wrong arity,
qualifiers, omitted arguments and arbitrary calls fail explicitly. Tests exercise
argument order, exact numeric boundaries, limits/cancellation, grouped/correlated
queries, saved SELECTs and buffered CLI errors. [Contracts and Microsoft references](EXPRESSIONS.md#explicit-isnull-and-iif-built-ins)
separate documentation-derived semantics from unrun native Access qualification.

Independent original Jet 4/ACE 2007/2016/2019 scalar fixtures pass 248 inventory/
page/projection/query controls with unchanged bytes, including IsNull/IIf around
all fourteen stored carriers and exact numeric condition models. All 835 workspace
all-feature tests passed; subsequent affected-package/focused checks, Clippy,
release build, rustdoc, formatting and parser-free expression tests also passed.
No new dependency
or fixture is added. [Fixture evidence](FIXTURES.md#built-in-scalar-read-regression)
retains the oracle's inert getter/model scope.


## Named local saved SELECT dependencies

The backend-neutral Source contract can now expose stable named retrieval metadata.
The binder captures it as independent derived operators, retains original/alias
qualifiers, resolves supported UNION references and detects active dependency
cycles. Each definition keeps its declared parameter scope; compatible same-name
TypeDescriptors share a supplied value, while conflicts and undeclared parent/
child leakage fail before scans. Separate dependency/syntax depth and combined
node/allocation ceilings bound the expanded graph. Parameter/name reservations
are fallible. UPDATE never resolves named query targets.

NativeSource inventories query identities beside table metadata, then reads and
caches only referenced qualified SELECT definitions under cumulative native
controls. Loading checks native identity and table/query ambiguity; user records
remain unread during binding. Explicit SQL and --saved retrieval share this path;
inspection and inert reconstruction do not execute dependencies. Plans retain
prepared definitions and require reprepare after metadata changes. This does not
supply revision invalidation, external/UDF/owner authority or new write support.
See [shared contracts](QUERY_EXECUTION.md#named-retrieval-sources) and
[native scope](NATIVE_SAVED_QUERIES.md#nested-saved-select-sources).

All 825 workspace all-feature tests, Clippy, default release build, rustdoc,
formatting and standalone no-default query/Access checks passed. Independent original Jet 4/ACE
2007/2016/2019 dependency files and existing parameter inputs passed 362
saved/direct execution/getter/refusal controls over 171 definitions with unchanged
bytes. [Fixture records](FIXTURES.md#saved-select-dependency-inputs) separate the
corrected original recipe, synthetic checks and independent evidence from native
Access parameter/scope qualification, which remains unrun. No dependency added.


## Explicit native saved SELECT execution

`query run --saved NAME --database PATH` now reads/reconstructs one selected local
saved SELECT and executes it through the existing bounded query engine. It
accepts the same parameter, collation, wildcard, explicit-index and output
controls as SQL input; positional SQL and saved names are mutually exclusive.
`query saved` remains reconstruction only. NativeSource exposes the shared inert
`read_saved_select` API with typed native/reconstruction failures; metadata,
reconstruction and later table reads share cumulative source controls. No implicit
saved-source expansion, owner authority, startup, UDF/provider or VBA execution
is enabled. Reconstructable unsupported forms still fail binding without output.
[Execution contracts](NATIVE_SAVED_QUERIES.md#explicit-saved-select-execution)
record these boundaries.

All 817 workspace all-feature tests, Clippy, default release builds, rustdoc,
formatting and standalone no-default Access checks passed. Focused tests verify
same-source parameter/filter/order execution, preserved fragments, cancellation,
cumulative budget exhaustion, CLI argument/presentation policy and payload-free
diagnostics. Seven original Jet 4/ACE 2007/2016/2019 inputs passed 358 saved/direct
execution/getter/refusal controls over 168 definitions with unchanged bytes.
[Fixture records](FIXTURES.md#explicit-saved-select-execution-inputs) retain the new
original ACE 2007 hash and existing-input provenance. No dependency or write
qualification changed; native Access SQL comparison remains unrun.


## CLI table counts and all-table browsing

`tables --count` inserts stored local Row Count metadata after Kind and reports
N/A for linked/unknown sources. It uses existing shared schema descriptions without
record decoding; it does not audit live counts. `rows PATH --all` renders named
sections from every user table in catalog order, with per-table limit/offset and
existing stored-value opt-ins. Linked/unknown tables are explicitly unopened;
system tables remain hidden. Empty sections retain headers. A shared browser
budget, cumulative output limit and buffered failure behavior cover all sections.
See [usage and scope](CLI_BROWSING.md#inventory-counts-and-all-table-pages).

All 95 CLI tests, Clippy, formatting and the release CLI build passed, including counts, linked/empty tables,
flag conflicts, per-table paging, bordered/TSV output and late-read failure with
no partial stdout. The registered ACE 2007 example completed counts and two-row
pages across all eleven user tables without modifying input bytes. No dependency
or native compatibility/write claim is added.


## Cross-family projected read regression

Expanded the existing original scalar/index comparison runners across Jet 4 and
ACE 2007/2016/2019. All fourteen scalar carriers now pass individual projected
SELECTs; 124 inventory/page/projection/query controls cover long payloads, NULLs,
filter/sort inputs, outer joins, grouping, correlation, derived reads, UNION and
TOP. Another 576 indexed/sequential predicate/projection controls retain narrow
outputs, constants/counts, parameter hints and residual filters. Input bytes stayed
unchanged. [Fixture provenance](FIXTURES.md#cross-family-projected-query-regression)
records fresh hashes and the existing original inputs. Python compilation passed;
no production query code/dependency was changed. Native Access remains unrun.


## Query projection pruning

Retrieval binding now computes required source fields after complete validation,
including filters, sorting, grouping, join conditions and nested correlations.
Plans rewrite dense runtime positions while preserving original source schemas
and constraint ordinals. Derived output expressions and eager evaluation errors
remain intact; unavailable references/wildcards still fail before reading rows.
The new borrowed projected-scan contract defaults to validated complete-row scans;
NativeSource overrides it with actual selected native payload decoding.

Native sequential/indexed reads skip unused ordinary payloads while retaining
structural checks and required index-key decoding. Complete field-set snapshots
can satisfy narrower requests; bounded range snapshots never populate complete
caches. Zero-width rows retain record counts without fabricated values. All
projection planning/storage uses existing fallible allocation and resource limits.
UPDATE and write qualification are unchanged. This is not streaming/TOP pushdown
or a whole-table payload audit. See [query contracts](QUERY_EXECUTION.md#requested-source-fields)
and [native scope](NATIVE_QUERIES.md#query-projection-pruning).

Local workspace all-feature tests (810), Clippy, default release build, rustdoc,
formatting and dependency-boundary checks passed. Independent original ACE
2007/2016/2019 inputs passed 189 indexed projection/order/range/refusal controls
and 45 mixed scalar query/refusal controls with unchanged bytes. Narrow cases
retain non-output filter/sort fields and omit ordinary Memo/other payloads.
Synthetic regressions cover nested correlation/join mappings, derived expression
errors, swallowed callback failures, malformed unused dates and cache field sets.
Native Access qualification remains unrun; no new dependency or fixture is added.


## Indexed scalar views of mixed native tables

`read_table_index_projection` now accepts named ordinary output fields and a
width-qualified `IndexProjectionSelection`. Required key fields join the decode
projection even when absent from output. Existing key policies borrow a checked
native-position view of dense decoded values, validate each entry/row key, then
remove unrequested fields in place. No unread field is padded with NULL. Empty
output projections still verify keys/counts; full and typed range scope remains
explicit. Ordinary full-column index APIs retain their prior profiles/order.

NativeSource now uses projected index reads for mixed scalar views and maps
constraint columns by native identity/name to dense scalar positions. Eligible
single-Int32/Int64 ranges prune safely; other predicates use the complete selected
index and the original query filter. Missing/ineligible indexes never fall back
to sequential reads. Partial snapshots remain uncached; complete snapshots retain
their existing cache. Unavailable SQL references/wildcards remain unsupported.
This removes the prior indexed-mixed-view limitation without changing writes,
key domains, collations or whole-collection semantics.

Three fresh original 1,000-row ACE 2007/2016/2019 inputs passed 157 independent
cursor/model controls: full ascending/descending/composite order, counts, NULLs,
duplicate/endpoints, exact Int64 values beyond 2^53, range and non-key predicates,
and buffered missing/ignore-NULL/text-index refusal. Inputs remain unchanged.
[Fixture records](FIXTURES.md#mixed-schema-index-projection-inputs) record hashes,
reference-builder warnings, original recipes and scope. Native Access remains
unrun. Synthetic cases also verify key reconstruction across GUID/Decimal/tuple
profiles, zero-width outputs, hidden-key corruption and actual node pruning with
a corrupt disjoint leaf under a shifted native/dense key position.

Local workspace all-feature tests (802), Clippy, rustdoc, default release build,
formatting and dependency-boundary checks passed. No registry dependency or write
qualification changes are introduced. Unselected payloads and pruned subtrees
retain their explicit unaudited scope.

## Scalar queries over mixed native schemas

Query Source now exposes optional explicit unavailable field names beside its
ordinary scalar columns. Binding retains these names for ambiguity, unsupported
references and wildcard refusal, with no omitted wildcard fields or fabricated
NULL cells. Prepared contracts check unavailable metadata again on execution.
Joins, outer padding and correlated lookups use dense available row positions.
Existing scalar-only hosts retain their default contract. A wholly unavailable
source can still supply zero-width rows for COUNT(*) and constant projections.

The engine adds bounded explicit `read_table_projection` snapshots, retaining
native metadata/order, complete row framing/allocation/count checks and existing
controls while skipping unselected payload decoding. NativeSource atomically
captures ordinary/unavailable schema views and caches ordinary scalar projections
of mixed tables. It never expands collections or evaluates stored formulas. A
matching unavailable field or wildcard fails before scanning, including on empty
sources. Explicit native indexes for these mixed projected views remain unsupported
without fallback; ordinary scalar indexed paths are unchanged. This is not SQL
projection pruning: unused ordinary scalar payloads still load and can fail.
See [contracts and limits](NATIVE_QUERIES.md#scalar-views-of-mixed-native-schemas).

Original ACE 2007/2016/2019 mixed inputs pass 45 independent getter/model controls
for scalar fields after native collection columns, filters/counts, outer joins,
correlation, derived wildcards and buffered refusal. Input bytes remain unchanged.
[Fixture records](FIXTURES.md#scalar-queries-over-mixed-native-tables) distinguish
these results from unrun native Microsoft Access SQL/locale qualification.
Synthetic checks additionally cover unknown carriers, zero-width native rows,
missing/duplicate selections, shared controls, schema changes, unavailable name
ambiguity and rejected extra NULL placeholders.

Local workspace all-feature tests (797), Clippy, rustdoc, default release build,
formatting and dependency-boundary checks passed. No registry dependency, active
content or write-profile change is introduced.

## Mixed typed stored-collection pages

The native engine now provides `StoredCollectionTablePage` with explicit scalar
or collection cells and borrowed typed Value/Attachment/Version items. Complete
checked links feed the existing strict validators; field count only routes the
validator, without qualifying a type or falling back on malformed input. Visible
Memo/history binding remains checked. Attachment payloads stay encoded and history
stays in stored order. Parent layout/key checks, full collection snapshots,
budgets/cancellation and selected scalar page decoding share one implementation.
The existing multivalue-only API wraps it under its unchanged restricted policy.

`NativeBrowser::collection_page` exposes this API to hosts. `rows --collections`
displays typed lists beside ordinary fields, including nullable attachment
metadata/encoded lengths and exact timestamp bits. Hidden history fields retain
their native name and visible-Memo label. The flag conflicts with `--multivalues`;
qualified cached results still require `--stored-calculations`. Unknown columns
fail without omission; no core Value or SQL collection semantics change.
See [scope and limits](CLI_BROWSING.md#mixed-scalar-and-stored-collection-pages).

The original inert Java oracle and nine existing ACE 2007/2016/2019 inputs passed
72 page/offset/empty/bordered controls, with unchanged bytes. The original
multivalue/cache matrix now passes 102 controls, retaining all 57 earlier cases;
attachment/history corrupt-copy runners additionally check empty general-page
refusal. Existing stored metadata/history canonical hashes remain unchanged.
[Fixture records](FIXTURES.md#mixed-stored-collection-page-verification) retain
independent evidence separately from unrun native Microsoft Access qualification.

Local workspace all-feature tests (789), Clippy, rustdoc, default release build,
formatting and dependency-boundary checks passed. New automatic cases cover all
three collection variants, exact DATE bits, NULL/empty/absent states, paged visible
Memo bindings, invalid metadata on empty pages, shared controls and CLI conflicts.
No dependency or write-profile changes are introduced.

## Shared stored-content browsing and CLI inspection

`NativeBrowser::attachment_column` and `version_history_column` now expose the
qualified engine readers under the browser's cumulative budget and cancellation
controls, with public typed/borrowed record re-exports. Feature code lives under
`browsing/collections.rs`. The CLI adds explicit mutually exclusive
`inspect --table NAME --attachments COLUMN` and `--version-history MEMO` views.
Both show complete parent membership and stored child order. Attachment output
contains nullable metadata, encoded byte lengths and raw flags without decoding,
extracting or opening content. History retains NULL/empty distinctions without
synthesizing current/latest values. Both show exact DATE bits beside calendar
presentation. These complete snapshot operations retain current resource limits
and buffered output; ordinary scalar pages/queries are unchanged.
See the [contract](CLI_BROWSING.md#stored-attachments-and-memo-history).

The existing independent attachment/history runners now check the CLI using
standard CSV parsing. Original ACE 2007 and both ACE 16-family inputs and the
existing nullable/corrupt disposable copies passed, preserving original bytes
and canonical hashes. Checks include metadata/encoded lengths, empty membership,
NULLs, signed-zero/subnormal/alias/calendar-extreme DATE bits, stored ordering,
and no-output rejection. Separate bordered attachment/history reports passed.
[Fixture records](FIXTURES.md#stored-collection-cli-verification-using-existing-inputs)
retain independent evidence separately from unrun native Access qualification.

Local workspace all-feature tests (785), Clippy, rustdoc, default release build,
formatting and dependency-boundary checks passed. New automatic checks cover
shared cancellation/resource ceilings and all new flag requirements/conflicts.
No dependency, native profile or write behavior changes are introduced.

## CLI command source organization

All command arguments, dispatch and handlers now live under
`apps/tablecraft-cli/src/commands/`, split into table inventory, row browsing,
inspection and query features. Inspection and query arguments have dedicated
modules; native command errors have one shared owner. `main.rs` retains clap
Parser/global options, process startup, diagnostics and output/exit handling.
Shared native controls and output/presentation helpers remain outside commands.
Command syntax, filtering, output formats and database behavior are unchanged.

Local validation passed all 90 CLI tests, Clippy, the release build, formatting
and dependency-boundary checks. Fifteen
help/version/argument-validation/evaluation cases matched the prior release
binary exactly, including stdout, stderr and exit status.

## Default user-table listings and CSV-backed TSV

CLI `tables` and `inspect --catalog` now hide native system tables by default.
`--show-system` includes them; it replaces `--hide-system` and requires `--catalog`
on inspection. Hidden user tables and names beginning with MSys remain visible;
other catalog object kinds are unaffected. Reader/library inventories retain all
objects.

The CLI now uses workspace-owned csv 1.4.0 `WriterBuilder` with a tab delimiter,
LF endings, standard necessary quoting and flexible record widths. TSV fields
contain the existing escaped display representations, including NULL/empty/text
distinctions; consumers must decode CSV transport quoting before interpreting
those cells. Pretty tables retain their presentation. Final quote expansion is
subject to the 1 MiB bounded fallible output sink before stdout publication.
[The CLI contract](CLI_BROWSING.md) documents the changed bytes and parsing example;
[dependency records](DEPENDENCIES.md) record licensing/features/transitives.
Independent comparison adapters now decode TSV with Python csv, and inventory
oracles request system entries explicitly.

Local workspace all-feature tests (783), Clippy, rustdoc and the default release
build passed, including CSV-reader
round trips and quote-expansion limits. Dependency-boundary checks, ten policy
tests, two comparison-adapter tests and comparison-script syntax checks passed.
Independent original Jet 4, ACE 2007 and both ACE 16-family fixtures each passed
complete inventory, five scalar pages and SELECT comparisons, preserving input
bytes. The mixed-value matrix retained all 57 controls and cached-page checks
retained all 38 controls. Native Microsoft Access comparison remains unrun.

## ACE 2007 stored attachment and history reads

The typed stored attachment and append-only Memo history APIs now accept the
qualified ACE 2007 profile alongside ACE 16. Existing shape, flag, parent/child
reference and exact DATE checks remain in force; no active content is executed.
These APIs do not yet render attachment/history cells through CLI rows.

Fresh original V2007 inputs passed independent Jackcess comparisons for six
attachments (65,810 decoded bytes), exact stored metadata and eighteen history
versions across six parent collections, retaining ACE 16 controls and corruption
cases. [Fixture evidence](FIXTURES.md#ace-2007-stored-attachment-and-history-inputs)
records hashes and limitations. Local workspace tests, Clippy, rustdoc and release
build passed; native Microsoft Access comparison remains unrun.

## ACE 2007 complex keys and multivalue qualification

The existing ACE 2007 profile now permits qualified four-byte complex parent keys,
fixed Int32 child foreign-key fields carrying exactly the complex-FK extension,
and typed multivalue collections through the existing key/value/page decoders.
Its observed empty Decimal conceptual template uses the same narrowly checked
9-byte hint with precision 18/scale 0 and ordinary 17-byte child storage.
Ordinary nine-byte Decimal records remain unsupported. Header/subversion/encoding
selection, strict layout/flag checks and native storage semantics are unchanged.
No generation is inferred from extensions or resemblance alone.

`rows --multivalues`, shared browsing and the raw/typed collection APIs now accept
this qualified legacy family alongside both ACE 16 families. Scalar query Source
can read the newly qualified raw Int32 child-FK carrier; parent collections still
require their explicit browsing API and do not acquire `.Value` query semantics.
Large Number and cached calculation gates remain newer-family-only. Typed
attachment/history APIs now state their existing ACE 16 generation gates explicitly
so broadening the shared key primitive cannot implicitly qualify those operations.
[Profile contracts](CLI_BROWSING.md#explicit-multivalue-cells) retain complete
parent/child limits, reference checks and separate native Access qualification.

Fresh original V2007 inputs compare 32 collections/137 exact stored values and
twelve corrupted orphan/template copies, plus nine raw parent/child memberships
and twelve corrupted-reference copies. Optional ACE 2007 mixed-page controls add
eleven cases, retaining the original 46 controls (57 total). Original ACE 16
canonical collection/reference hashes and all input bytes remain unchanged.
[Fixture records](FIXTURES.md#ace-2007-complex-key-and-multivalue-inputs) identify the
final fresh inputs, reviewed artifact, original sources/oracles and exact scope.
Evaluation is disabled; no native Access provider or active content runs.

Positive synthetic key/collection/page tests now cover all three families. Raw FK
schema tests update the expected ACE 2007 carrier, while ACE 2010/unknown flags,
width/type mismatches, orphan/duplicate/NULL/reference errors, capacities, budgets
and cancellation retain rejection coverage. Added tests keep ACE 2007 typed
attachment reads and Large Number collection templates unsupported; existing
legacy calculated/history guards also remain green.
All 779 all-feature workspace tests/doc tests and 351 standalone default ACE tests/
doc tests pass. Warnings-denied all-feature/default Clippy, rustdoc, default release
build, formatting, twelve-package dependency checks, ten Python regressions and
changed documentation links pass. No dependency or package boundary changed;
full backend acceptance and native Access comparison remain unfinished.


## Explicit mixed multivalue table pages

The engine now exposes `read_multivalue_table_page`/`MultivalueTablePage`, consumed
by shared `NativeBrowser::multivalue_page` and CLI `rows --multivalues`.
Cells explicitly distinguish core scalar values from stored collections; native
complex types remain in the original schema and no collection enters scalar
query/edit contracts. Collection keys, duplicate/NULL values and flat-table order
are preserved. CLI headers identify multivalue columns and format assigned empty
lists, absent keys and NULL elements distinctly. Lookup/display queries never run.

Combined `--multivalues --stored-calculations` enables qualified cached results
beside those cells, with retained inert metadata and existing cached labels.
Without cache opt-in this path does not read property maps. It shares existing
scalar/page and stored-calculation gates; unsupported attachments/history/other
fields reject rather than disappear. [Contract](CLI_BROWSING.md#explicit-multivalue-cells)
retains explicit profiles and unchanged ordinary query/write eligibility.

Each collection reuses complete native parent-key and child snapshots, reference/
identity validation and the 100-item policy. Complete-snapshot ceilings and shared
budgets still apply even for tiny or out-of-range page requests. Observed native
layouts must match the initial schema, and selected keys must match their full
parent snapshots before publication. This establishes checked joins on stable/
offline sources, not native shared-file coherence or lazy child acquisition.
Ordinary/cached payload validation remains scoped to selected rows.

Five engine tests and two host integration tests cover mixed cell types, NULL
versus assigned-empty states, duplicates/NULL elements, paging, absent coordinates,
unknown columns, global orphan/overflow rejection, budgets and final cancellation.
A fixed-seed 128-case property and saved regression check collection membership
across parent offsets. New original mixed fixtures keep deliberately stale caches,
while independent readers cover eight collection carriers through existing inputs.
All 46 independent mixed-page controls pass with unchanged originals;
[provenance](FIXTURES.md#mixed-multivalue-page-inputs) records hashes, original
recipes/oracles and the distinction from native Access qualification.

All 777 all-feature workspace tests/doc tests and 349 standalone default ACE tests/
doc tests pass. Warnings-denied all-feature/default Clippy, rustdoc, default release
build, formatting, twelve-package dependency checks and ten Python regressions pass.
The original typed-multivalue comparison also remains green. No dependency or
crate boundary was added. Broader backend acceptance and native Access comparison
remain incomplete; this completes the stated mixed-page read profile.


## Structured inspection tables and system-table filtering

CLI inspection now follows the existing `--format auto|table|tsv` policy.
Terminals render compact comfy-table sections: explicit Property/Value summaries
plus independent column/index/property/fragment/check/relationship tables.
Report producers record byte boundaries while writing escaped text; names,
contents and varying column counts are never guessed as section markers.
Redirected/explicit TSV retains the prior exact report structure. All sections
share the final 1 MiB publication ceiling and existing conservative layout checks;
format failures still publish no partial stdout. Other SQL/edit reports are unchanged.

`tables --hide-system` filters native system-marked entries using the shared
classifier. `inspect --catalog --hide-system` filters only system tables and
retains other object kinds, including system-marked queries. The inspect flag
requires catalog mode. Hidden-only user flags and MSys-prefixed user names do
not qualify as system. Neither option adds table/property/provider reads.

Names remain native. The registered example's actual name is tblContacts and its
decoded groups contain no alternate Caption. Microsoft distinguishes TableDef
Name from field/control Caption metadata; no guessed prefix removal or implicit
alias mapping is added. [Presentation/name contracts](CLI_BROWSING.md#inspection-presentation-and-object-names)
record the primary sources and behavior.

CLI integration tests pass, including both explicit formats, separate summary/
column sections, header/catalog tables, metadata-based filtering, hidden/prefixed
user names, retained system queries, invalid flag combinations and atomic read
failures. Three presentation unit tests include a shared output ceiling across
multiple individually valid sections. Nine registered-example inspection modes
were exercised in forced-table mode; header auto-format was also observed in a
PTY. The original fixture hash remains unchanged. The independent Jet 4 and both
ACE 16 metadata runner passes with unchanged original inputs/TSV.
Warnings-denied all-feature/default Clippy, rustdoc, default release build,
formatting and documentation-link checks pass. This is host presentation/filtering
work; native decoding capabilities and native Access qualification are unchanged.


## Explicit cached-calculation table pages

The engine now composes qualified cached calculation values with ordinary scalar
cells through `read_stored_table_page` and its distinct `StoredTablePage` result.
It retains original schema/inert Expression/ResultType declarations, native column
order, per-column cache markers, physical request/end metadata and exact stored
values. The shared Access `NativeBrowser::stored_page` owns orchestration; CLI
`rows --stored-calculations` only opens the file, selects the operation and labels
calculated headers `[cached]` in TSV/bordered output. Formula execution and cache
freshness are not implied. Ordinary rows/query/write eligibility remains intact.

The profile reuses existing code-5/code-6 stored Boolean/Int32/Double/Currency/
legacy DateTime/Short Text wrapper decoders and the single-column declaration gate.
Every native column must qualify before reading records, including empty/large-
offset requests. Missing declarations, mismatched result types, unsupported
carriers/layouts and selected invalid payloads fail without a partial page.
The same physical walker now serves ordinary and cached pages, preserving skipped
payload scope, cumulative limits, fallible charged allocations and cancellation.
[Contract](CLI_BROWSING.md#explicit-stored-calculations) keeps other generations,
complex/other calculation carriers and recalculation outside this profile.

Six new engine tests cover both families, seven result encodings, origin metadata,
paging/NULLs, declaration failures on empty/out-of-range requests, unsupported
ordinary columns, skipped/selected corruption, older profiles, limits and final
cancellation. A fixed-seed 128-case property checks nullable Int32 pages against
an independent ordinal model. Shared-library and CLI tests cover explicit opt-in,
headers in both formats and deliberately stale results that must not be recomputed.
The independent runner matches 38 controls on four unchanged original fixtures;
previous Int32/six-scalar inspection runners also remain green.
[Provenance](FIXTURES.md#cached-calculation-page-regression) distinguishes inert
Jackcess evidence from native Microsoft Access/formula/locale qualification.

All 767 all-feature workspace tests/doc tests and 344 standalone default ACE tests/
doc tests pass, with warnings-denied all-feature/default Clippy, rustdoc, default
release build, formatting, twelve-package boundary checks and ten Python regressions.
No dependency or package boundary was added. The broad backend roadmap remains
unfinished; this completes the stated cached browsing profile only.


## CLI table presentation and ACE 2007 read priority

CLI `tables`, `rows`, `query run` and `query eval` now use comfy-table 8.0.1
Unicode borders on terminals, retaining escaped TSV for redirected stdout.
Global `--format table|tsv|auto` overrides that choice; inspect/SQL/edit reports
retain their original formats. Rendering stays at the application boundary,
with the original scalar representations/escaping and 1 MiB final output limit.
A conservative layout-expansion check runs before dependency formatting; layout
rejections suggest TSV. Internal dependency allocations are not fallible APIs.
[Browsing contracts](CLI_BROWSING.md) describe wrapping and resource scope;
[dependency review](DEPENDENCIES.md) records the minimal CLI-only feature graph.

Table inventory now exposes native system classification through engine
`NativeObjectOrigin::is_system` and backend `TableEntry::is_system`.
The CLI always includes `Kind` (`System`/`User`), independent of names, hidden
status and local/linked storage. Either native system bit qualifies; tests
separately cover both bits, hidden-only flags, unrelated flags and user names
beginning with MSys. No table/external provider is opened to classify inventory.
Scripted inventory consumers must account for the added Kind column.

ACE 2007/ACE 12 joins ACE 16 and Jet 4 as an immediate read/query priority.
The registered example already reports Ace2007 and remains readable through its
existing profile. Fresh V2007 independent inventory, five page cases and ordered
SELECT agree on fourteen scalar carriers; original bytes remain unchanged.
The Jet 4 and both ACE 16 scalar and metadata matrices also pass with Kind labels.
[Fixture records](FIXTURES.md#ace-2007-browsing-priority-regression) retain the new
hash/reproduction; [profile explanation](NATIVE_READING.md#read-profile-selection)
distinguishes shared decoding from header recognition and capability qualification.
No native Microsoft Access comparison or broader compatibility claim is added.

All-feature workspace tests/doc tests passed (758); after the final conservative
layout guard, all 81 CLI tests pass, including bordered browsing/query Unicode,
explicit TSV, atomic failures, final output ceilings and early layout rejection.
Warnings-denied all-feature/default workspace Clippy, rustdoc, the default release
build, formatting, twelve-package boundary checks and ten Python regressions pass.
The registered example retains its original SHA-256. Nothing was pushed.


## Existing-width Decimal write previews

The optional ACE 16 fixed/batch APIs now edit qualified Decimal payloads in
existing 17-byte variable slots, retaining exact field precision/scale, offsets,
record placement and file size. NULL clearing retains payload bytes; restoration
requires an existing full-width slot. Zero-width expansion, noncanonical negative-
zero edits, wrong precision/scale and unknown field layouts reject. Native word
encoding remains separate from sorted index-key encoding; no SQL arithmetic or
implicit rescaling is introduced.

Single Decimal and two-to-ten-component Decimal indexes reuse the checked
existing-page writer and complete final constraints. Decimal tuples select a wider
private atom; compact numeric/GUID tree entries retain their representations.
Original-association update cascades now include same-precision/scale Decimal
pairs. All changed masks/fields/indexes retain authorized-byte preservation proofs.
[Contract](testing/ACE16_WRITE_PREVIEWS.md#decimal-slot-and-index-write-previews)
keeps record resizing, other variable carriers, allocation/splits, production saves,
journaling and native Access/data-macro qualification separate.

Seven integration tests include both families/directions, exact coefficient/scale
extrema, NULL clear/restore and zero-slot refusal, stale/duplicate/unique conflicts,
Decimal in every position of two-to-ten-component GUID mixed keys, budget/cancel
failure and negative-zero refusal. A fixed-seed 128-case property checks full-width
coefficients; two literal unit goldens cover field/index words and signs. Fresh
independent V2016/V2019 originals match 22 complete candidates (four no-ops), 22
reverse-order controls and fourteen atomic rejections. Full rows/long values,
independent cursors, topology/reference sets and authorized byte spans agree.
The previous 21 numeric ACE/Jet 4, 24 GUID and 46 Boolean/IEEE/DATE candidate manifests
remain frozen. [Fixture identities](FIXTURES.md#decimal-slot-and-index-update-inputs)
record the observed inputs/manifests without claiming native Access conformance.

All 755 all-feature workspace tests/doc tests and 338 default ACE tests/doc tests
pass. Warnings-denied all-feature/default workspace Clippy, rustdoc, the default
release build, formatting, twelve-package dependency guard, ten Python regressions
and comparison-script compilation pass. The registered application sample hash is
unchanged. No dependency or package boundary changed; the immediate native
read/query priority and remaining backend acceptance work remain in force.

## Native storage and scalar-column support reporting

`NativeBrowser` now exposes captured table storage entries and inert local-table
`TableDescription` results. The engine's typed `NativeTableStorage` reuses its
existing catalog tags for local, non-ODBC linked and ODBC linked declarations;
local schema inspection uses the same classifier. It does not inspect connection
properties or infer a non-ODBC provider/source format. Column support delegates to
the engine's authoritative scalar-carrier classifier rather than a CLI type map.

`tables --details` adds storage and explicit row-inspection status, preserving the
default names-only output. `inspect --table NAME` now includes scalar support and
unsupported-column counts before its original raw metadata table. Supported
carriers do not imply verified row contents, historical layouts, long values,
constraints, resource requirements or full readability. Unsupported columns remain
visible; linked/malformed metadata, limits and cancellation retain typed failures.
[Browsing contracts](CLI_BROWSING.md) document the reusable API and limits.

Three new backend tests cover local/non-ODBC/ODBC metadata, linked refusals,
metadata-only page I/O despite corrupt records, unsupported columns, typed errors,
limits and final cancellation. A new CLI test covers details, summaries and
unsupported-column preservation; the existing exact schema-output expectation is
updated. Fresh original Jet 4 and both ACE 16-family databases match independently
read inventories/column metadata and linked refusal behavior, with unchanged
source bytes. [Fixture provenance](FIXTURES.md#native-browsing-metadata-inputs)
retains native Access/provider/Windows qualification as unfinished.

Workspace all-feature tests/doc tests, warnings-denied all-feature/default Clippy,
rustdoc, the default release build, formatting, twelve-package dependency guard,
ten Python regressions and comparison-script compilation pass in the current
worktree. No dependency or package boundary changed. This advances reliable native
browsing; it does not qualify a table merely because its metadata is supported.

## Explicit-mode LIKE retrieval

The expression library now owns a bounded iterative LIKE matcher with explicit
ANSI-89/ANSI-92 selection, host scalar collation, wildcard/list/range validation,
Unicode scalar units and three-valued NOT. The query adapter binds it through
projections, filtering, parameters, grouping/aggregate traversal and nested
queries. CLI `query run` exposes `--like-mode` alongside explicit `--collation`;
the optional SQLite worker forwards the same policy to TableCraft evaluation.
Native storage, CLI and SQLite do not implement separate matching engines.
[Pattern contracts](PATTERN_MATCHING.md) retain unsupported numeric/date formatting,
locale expansions/contractions and native Access conformance as explicit gaps.

Twelve focused tests cover literal and generated matching, policy/type/NULL/error
boundaries, allocation/work/cancellation, SQL evaluator eagerness, parameter reuse,
grouping and correlated scopes, atomic CLI failure and worker policy forwarding.
The fixed-seed property checks 256 short patterns against an independent exhaustive
model. Fresh original Jet 4 and both ACE 16-family inputs each contain 28 inert
text/NULL records. Independent stored-row reads plus explicit ordinal regex models
match 228 projection/filter/NULL/error controls overall, with unchanged input bytes.
[Fixture provenance](FIXTURES.md#native-like-query-inputs) distinguishes these from
native Access SQL/locale evidence. No native provider or active content executes.

Workspace all-feature tests/doc tests, standalone default expression tests,
warnings-denied all-feature/default Clippy, rustdoc, the default release build,
formatting, twelve-package dependency guard, ten Python regressions and comparison-
script compilation pass in the current worktree. Local link targets in the changed
documents exist. No dependency or package boundary changed. LIKE still performs
bounded residual evaluation rather than native index prefix pushdown; the broader
backend roadmap and native compatibility qualification remain unfinished.

## Reusable native browsing and bounded physical pages

`tablecraft-access::NativeBrowser` now owns table inventory and page orchestration
for CLI and future UI hosts. The CLI supplies explicit file ownership, arguments,
host budgets and formatting. Physical parsing stays in ACE; no application package
is a library dependency. `TablePageRequest` validates page size, and the distinct
`TablePage` result exposes selected rows plus whether root enumeration reached an
end/count check. This is separate from complete snapshot/audit contracts.

The page reader decodes only selected records and their long values/relocations;
skipped roots consume traversal work without constructing values or spending the
decoded-row allowance. It stops when full, permitting larger declared totals
within existing file/map/page and caller ceilings. All column layouts must still
be supported. Skipped/unvisited payloads are explicitly unchecked; full query
snapshots and integrity audits retain their existing behavior. [Browsing scope](CLI_BROWSING.md)
and [native page contract](NATIVE_TABLES.md#bounded-physical-row-pages) describe these limits.

Seven engine tests cover page-I/O/charge reductions, an 18,000-row table under a
small selected-row budget, deleted/relocated roots, empty/invalid ranges, corrupt
selected payloads/visited pages, count mismatches, column rejection, final
cancellation and exhausted budgets. A fixed-seed 128-case property compares pages
with independent sequences. Two backend tests cover table filtering, direct page
use, cumulative controls and cancellation without the CLI. Existing CLI tests
remain passing. Fresh original Jet 4 and both ACE 16-family files each contain
20,050 independently enumerated rows; thirty page controls compare 351 returned
rows, including deep offsets and long text. All source bytes remain unchanged.
[Provenance and observed hashes](FIXTURES.md#larger-native-browsing-page-inputs)
distinguish independent evidence from pending native Access/Windows qualification.

Workspace all-feature tests/doc tests, warnings-denied all-feature/default Clippy,
rustdoc, the default release build, formatting, twelve-package dependency guard,
ten Python regressions and comparison-script compilation pass in the current
worktree. All earlier small browsing inventory/page/SELECT controls pass unchanged.
No dependency or package boundary changed. The broader backend roadmap remains
unfinished; this advances the immediate ACE 16 and Jet 4 read/query priority.

## Direct native table browsing

The CLI now provides `tables PATH` for all catalog table names and
`rows PATH TABLE --limit N --offset N` for local scalar contents without SQL.
Both open only the explicit stable/offline source read-only and use existing
native catalog/snapshot readers. Rows retain the shared exact-value formatter
and buffered failure contract. Paging limits displayed rows after a complete
bounded snapshot; it does not bypass read/resource limits. Linked and unsupported
column layouts fail explicitly. [Scope and examples](CLI_BROWSING.md) distinguish
this from native shared-file acquisition and general Access compatibility.

Four new CLI tests cover Jet 4 discovery/pages, registered-sample output agreement,
source preservation, unknown/unusual names, unsupported columns, truncation and
invalid pagination. Fresh independent V2000/V2016/V2019 files each match complete
catalog table inventories, five scalar page controls and an ordered SELECT. Their
original bytes remain unchanged. [Fixture provenance](FIXTURES.md#native-cli-browsing-inputs)
records the original recipe, reviewed artifact and observed hashes; these are
independent comparisons, with native Access/Windows qualification still pending.

All 721 all-feature workspace tests/doc tests pass. Warnings-denied all-feature
and default workspace Clippy, the default release build, formatting, twelve-package
dependency guard, ten Python regressions, comparison-script compilation and 242
local link targets in the changed documentation pass. No dependency or library
boundary changed. Reliable ACE 16 and Jet 4 reads/queries remain the immediate priority; the broader backend roadmap remains
unfinished.

## Boolean IEEE and legacy DATE index previews

Opt-in ACE 16 fixed/batch previews now maintain Boolean, Single, Double and legacy
DateTime indexes and mixed integral/Currency/GUID tuples through the same bounded
tree writer. Boolean false becomes a real non-NULL atom from the clear mask bit;
explicit Boolean NULL remains invalid. IEEE keys preserve every bit, including
signed zeros/infinities/NaN payloads. Valid DATE keys preserve aliases and sub-
millisecond bits without calendar normalization. Single-key changes for these
carriers repack existing pages; numeric-only keys stay compact and integral-only
output behavior remains unchanged. Complete final constraints, byte preservation,
capacity checks and cancellation/budgets remain mandatory. Relationship audit/
action coverage for the new carriers, allocation/splits, Decimal/text/binary and
Extended DateTime mutation and ordinary ACE saves remain separate work.

Six authored integration tests cover both families/directions, Boolean unique
swaps and rejected NULLs, IEEE classes/NULL transitions, DATE aliases/precision,
a ten-component mixed key, shared masks, stale/duplicate requests, cancellation/
resources and a fixed-seed 128-case exact Double model. Literal unit goldens cover
Boolean keys, IEEE signs/payloads, DATE serials, widths and invalid DATE rejection.
Earlier blanket Single-target and composite-sibling rejection tests now verify
successful updates/no-ops and complete values while preserving unknown flag gates.
Independent comparisons match forty-six complete candidates, forty-six reversed
controls and eighteen rejections. All exact bits, complete tables, index cursors,
topology/reference sets and authorized byte spans agree. The earlier 21 single-
column ACE/Jet 4 and 24 GUID candidates retain frozen manifests. [Contract](testing/ACE16_WRITE_PREVIEWS.md#boolean-ieee-and-legacy-date-index-write-previews)
and [fixture manifests](FIXTURES.md#boolean-ieee-and-date-index-update-inputs)
retain native Access/data-macro/save qualification as unfinished.

All 717 all-feature workspace tests/doc tests and 329 default ACE tests/doc tests
pass. Warnings-denied all-feature/default workspace Clippy, rustdoc, the default
release build, twelve-package dependency guard, ten Python regressions,
formatting and 1,072 local documentation links pass. Default builds omit the
preview-only mask adapters. Registered sample/reference hashes are unchanged.
No dependency or package boundary changed; backend milestones 1–9 remain active.


## GUID index write previews

ACE 16 fixed/batch previews now maintain single GUID and two-to-ten-component
integral/Currency/GUID indexes, including qualified unique/primary/foreign aliases.
The shared tree writer repacks GUID pages within existing topology and capacity;
split/allocation requirements still reject without a candidate. Stored GUID UUID
byte order and tagged physical key segments remain separate. Numeric-only tree
profiles retain compact keys. Typed private update atoms replace the integer-only
mutation carrier; original expectations, exact row masks, final constraints and
byte-preservation checks remain mandatory. Original-association update cascades
now include GUID components and retain conflict/RESTRICT behavior.

Six authored integration tests cover both families/directions, NULL transitions,
unique swaps/duplicates, every GUID position in two-to-ten-component indexes with
numeric sibling updates and ten-GUID keys, stale/duplicate/no-op requests, cancellation/resources,
and a fixed-seed 128-case canonical-byte model. Literal unit goldens cover stored
endian order, both tagged segment encodings, truncation and cross-domain rejection.
Independent fresh 2016/2019 files match twenty-four candidates, twenty-four
reversed controls and ten atomic rejections, including a split-required change.
Complete tables/long values, index cursors, topology/reference sets and authorized
byte spans agree. The earlier 21 single-column ACE/Jet 4 candidates and 22 composite/mask
batches retain frozen manifest hashes. [Contract](testing/ACE16_WRITE_PREVIEWS.md#guid-index-write-previews)
and [immutable manifests](FIXTURES.md#guid-index-update-inputs) distinguish this
engineering evidence from native Access/data-macro qualification.

All 710 all-feature workspace tests/doc tests and 328 default ACE tests/doc tests
pass. Warnings-denied workspace Clippy/rustdoc, the default release build,
twelve-package dependency guard, ten Python regressions, formatting and 1,062
local documentation links pass. Registered sample/reference bytes are unchanged.
No dependency or package boundary changed. Normal ACE saves, Boolean/IEEE/date/Decimal/text indexed mutation,
variable records, allocation/splits, INSERT/DELETE and the rest of backend
milestones 1–9 remain unfinished.


## Declared update-cascade previews

The separate opt-in `preview_ace16_cascading_updates` API expands bounded
integral/Currency update actions from original-image parent/child matches.
Explicit final cells are pinned, identical generated demands coalesce, conflicting
demands fail, and a monotone fixed point covers multi-hop, overlapping and cyclic
paths before complete tuple validation. Declared RESTRICT blocks referenced key
changes even when a final permutation would preserve membership. All generated
cells retain original expectations and share batch caps/budgets; the existing
batch writer owns final constraints, index maintenance and preservation proof.
The engineering example opts in with `--cascade-updates`; normal ACE saves,
SQL action execution, delete actions and data macros remain unavailable.

Six authored tests include original-parent key-swap behavior, explicit/no-op
conflicts, stale/duplicate inputs, cancellation/resources, empty/no-op requests
and a fixed-seed 128-case original-association model. Independent fresh 2016/2019
recipes match fourteen complete candidates, fourteen reversed controls and
sixteen typed rejections without output. Complete related/untouched tables, long
values and index cursors agree; literal full batches yield identical bytes.
The new recipe also exposed a reader gap: a one-to-one foreign index may have a
separate enforcing unique index with different directions. Link checking now
admits the matching unique column set; complete physical/relationship checks
remain mandatory. A focused unit regression also covers different ordering,
nonunique/wrong/unknown declarations and cancellation/resource failure.
[Contract](testing/ACE16_WRITE_PREVIEWS.md#declared-update-cascade-batches) and
[immutable manifests](FIXTURES.md#declared-update-cascade-inputs) retain the
independent/native qualification distinction. Native Access comparisons,
non-integral indexed mutation, variable records, allocation/splits, INSERT/DELETE,
production saves and the rest of backend milestones 1–9 remain unfinished.

All 703 all-feature workspace tests/doc tests and 327 default-feature ACE tests/doc
tests pass. Warnings-denied workspace Clippy/rustdoc, the default release build,
twelve-package dependency guard, ten Python regressions, formatting and 1,052
local documentation links pass. Original application/reference hashes are
unchanged. No dependency or package boundary changed.


## Explicit multi-column and multi-table previews

The opt-in ACE 16 batch API accepts `FixedColumnUpdates` groups against one
original image. All requested tables pass original metadata/constraint gates;
fixed payloads and row masks are staged together. Disjoint mask bits merge, each
affected physical index receives every changed component once, and only the
complete candidate is checked against final indexes and field/table/relationship
rules. Changed-row counts deduplicate rows within their table; no-ops are exact.
This permits explicit parent/child and tuple changes without validating invalid
intermediate states. It does not discover native actions or commit files.

Nine authored tests cover transient duplicates, shared masks, multi-table/self
relationships, cross-field table rules, scoped row counts, stale/duplicate inputs,
empty/no-op requests, format/structural caps, resource limits/cancellation, order
independence and a deterministic 128-case tuple model. The clap-based batch
example shares exact carrier parsing with the single-column harness and creates
only fresh outputs after preparation completes. Independent comparisons match
22 complete batches (two exact no-ops) plus 22 reversed-order controls and fourteen
typed rejections. Complete related/untouched tables, long values, exact carriers,
index cursors, page/count/child/row-reference sets and authorized byte spans agree.
The earlier 84 single-column mask candidates keep their original row hashes.
[Contract](testing/ACE16_WRITE_PREVIEWS.md#explicit-fixed-field-batches) and
[fixture manifests](FIXTURES.md#explicit-fixed-field-batch-inputs) distinguish this
evidence from native Access/data-macro/transaction qualification.

All 696 all-feature workspace tests/doc tests and 26 default mutation/composite
tests pass, along with warnings-denied workspace Clippy/rustdoc, the default
release build, twelve-package dependency guard and ten Python regressions.
No dependency or package boundary changed. Original sample/reference bytes are
unchanged. Automatic cascades, non-integral indexed mutations, variable records,
allocation/splits, production ACE saves and the remaining backend roadmap stay
unfinished; final membership alone does not establish implicit-action semantics.

## Integral composite index previews

ACE 16 previews now maintain two-to-ten-column integral/Currency tuples through
the shared tree writer, retaining per-component widths, directions and NULLs.
Only the selected component changes; complete tuple/row pairs drive sorting and
node bounds. Composite changes repack existing pages even for non-NULL field
replacements, while single-column fixed-layout rules remain intact. Clearing a
tuple component also clears its private equality carrier. Unique/required and
before/after relationship checks still gate the complete candidate. Jet 4
composite mutation, non-integral components, multi-column assignments, cascades,
allocation/splits and ordinary ACE saves remain unsupported.

Seven authored tests cover every supported tuple length/component position,
all integral carriers, mixed directions, aliases, all/partial NULLs, unique swaps,
deep bounded/tail trees, no-ops, cancellation/resources and a deterministic
128-case model. A 600-row source proves a fitting growth with eight free bytes
and atomic refusal of a required split. Independent V2016/V2019 comparisons match
36 complete candidates (four exact no-ops) and fourteen typed rejections,
including composite foreign keys and ten-column indexes. Related rows, complete
Memo/OLE values, page/count/child/row-reference sets and permitted byte spans
agree. All 21 earlier single-column candidates retain their original ACE/Jet 4
manifest hashes. [Contract](testing/ACE16_WRITE_PREVIEWS.md#integral-composite-index-updates)
and [fixture hashes](FIXTURES.md#integral-composite-index-update-inputs) retain the
native Access/data-macro qualification gaps.

The new read-only `check_table` engineering example provides explicit bounded
library checks. The CLI inspector's 16 MiB budget cannot finish the 1,000-row
mixed fixture; the selected library harness completes full index and constraint
checks under its documented larger allowance. No CLI limit was silently raised.
All 687 all-feature workspace tests/doc tests and 26 default mutation/composite
tests pass, as do warnings-denied workspace Clippy/rustdoc, the default release
build, twelve-package dependency guard and ten Python regressions. Formatting
and local documentation links pass; registered sample/reference bytes are
unchanged. No dependency or package boundary changed. Backend work remains active.

## Checked reciprocal-index write previews

ACE 16 engineering previews now admit eligible fixed-field changes in tables
with completely checked foreign indexes. Single-column integral/Currency keys
use the existing physical writer, preserving ordinary and primary/foreign aliases,
NULL semantics and page topology. Complete before/after field and relationship
audits retain parent membership, requiredness and one-to-one uniqueness. An
explicit catalog-coverage check also rejects enforced foreign declarations that
were removed or disguised as ordinary indexes, even when physical reads pass.
Normal ACE saves, Jet 4 foreign writes, required cascades, composite/non-integral
target indexes and allocation remain blocked.

Five authored tests cover self/alias/direction/family profiles, exact values,
typed orphan/required/unique/conflict errors, unreferenced parent moves, disguised
graphs, resource/cancellation failures and a deterministic 128-case membership
model. Independent V2016/V2019 comparisons match 78 candidates (twelve exact
no-ops) and reject 54 requests with their expected constraint failures. Related
tables, complete Memo/OLE values, all selected index cursors, page/count/child/
row-reference sets and unrelated bytes remain intact. The earlier 84 Boolean/
mask candidates still match their original row hashes, with 22 current rejection
cases; two final ordinary-Boolean span checks also pass with unrelated enforced
relationships present. [The contract](testing/ACE16_WRITE_PREVIEWS.md#checked-foreign-index-updates)
and [fixture record](FIXTURES.md#checked-foreign-index-update-inputs) retain native
Access, data-macro, production-save and mixed-client qualification limits.

All 680 all-feature workspace tests/doc tests, nineteen default mutation tests,
warnings-denied workspace Clippy/rustdoc and the default release build pass.
The twelve-package dependency guard and ten Python regressions pass. No dependency
or package boundary changed; original sample/reference hashes remain unchanged.
The full backend roadmap remains in progress.

## Exact integral and Currency index previews

The fixed-scalar ACE 16 preview now maintains eligible ordinary/unique Byte,
Int16/32/64 and Currency indexes within existing trees. A typed private codec
retains exact widths, signed domains and scaled Currency units. Non-NULL changes
keep their original layout; NULL transitions repack admitted existing-page
regions. Node bounds retain full key/row maxima, including bounded final children.
Foreign/composite/non-integral targets, allocation/splits and normal ACE saves
remain blocked; the public Jet 4 planner stays Int32-only.

One literal codec test and five integration tests cover extremes, duplicates,
NULLs, compression, deep trees, both directions, uniqueness swaps, stale/wrong
carriers, cancellation/resource failures and a deterministic 128-case exact model.
Fresh independent V2016/V2019 comparisons match 56 full candidates (eight exact
no-ops), reject 32 requests without output, and retain page topology, row
references, all unrelated bytes and complete long values. [The contract](testing/ACE16_WRITE_PREVIEWS.md#integral-and-currency-index-updates)
and [fixture hashes](FIXTURES.md#integral-and-currency-index-update-inputs) separate
independent evidence from pending native Access/data-macro qualification.

All 675 all-feature workspace tests/doc tests, nineteen default native mutation
tests, warnings-denied workspace Clippy/rustdoc and the default release build
pass. The twelve-package dependency guard and ten Python regressions pass.
No dependency or package boundary changed. Registered original sample and
Jackcess artifact hashes remain unchanged. The backend roadmap remains active.

## Bounded final-child index previews

ACE 16 engineering previews now retain and rewrite trees whose nodes name every
child with an explicit upper-bound entry instead of an unbounded tail. Traversal
omits the absent tail; all separator key/row pairs follow updated subtree maxima.
Existing entry counts, page sets and links remain intact. Fixed and nullable
rewrites keep their separate layout rules. Jet 4 still rejects this topology for
mutation, and normal ACE saves remain blocked.

Authored shallow/deep fixtures, expanded direction/alias/uniqueness/NULL matrices,
the deterministic 128-case key model, late cancellation and resource failures
pass. All 354 ACE all-feature tests/doc tests and all-target/all-feature
warnings-denied ACE Clippy pass. Documentation checks validate 1,006 local
links/anchors across seventy-two public Markdown documents. No dependency changed.
[Evidence limits](testing/ACE16_WRITE_PREVIEWS.md#bounded-final-child-nodes) remain
explicit: no independent-reader or native Access qualification is claimed for
this topology extension.

## Indexed Int32 NULL previews and within-page repacking

The fixed-update engineering API now supports eligible indexed Int32 presence
changes. Explicit nullable expected/replacement keys retain stale-payload rules
and exact unique/required checks. Within existing trees, nullable updates can
repack shared prefixes, boundary bitmaps and free bytes while preserving entry
counts, ownership, page/child/tail/sibling sets, record placement and file length.
Required splits reject the entire candidate. Fixed non-NULL plans retain their
original byte-layout rules. Foreign/wider indexed targets and ordinary ACE saves
remain blocked; the preview is still non-journalable.

Four packet goldens/capacity tests and six integration tests cover aliases,
directions, leaf/node/deep trees, nullable batches, rules, resource limits and late
cancellation. A 600-row source proves fitting growth and whole-batch split refusal.
Fresh independent V2016/V2019 controls match thirty candidates (six exact no-ops),
fourteen rejected requests, exact geometry/topology, full rows and long values.
[Contract](testing/ACE16_WRITE_PREVIEWS.md#indexed-int32-null-transitions) and
[hashes](FIXTURES.md#indexed-int32-null-preview-inputs) retain native qualification gaps.

Verification passed all 667 all-feature workspace tests/doc tests, the default
eighteen-test mutation suite, warnings-denied workspace Clippy/rustdoc and the
default release build. All 21 earlier non-unique candidate hashes remain identical
across both ACE families and Jet 4. Formatting, the twelve-package dependency guard,
ten Python regressions and 1,004 local links/anchors across seventy-two public
Markdown documents pass. Registered sample/reference hashes are unchanged. No
dependency or package boundary changed; the full backend roadmap remains in progress.

## Non-unique existing-tree Int32 updates

The shared fixed-field writer now accepts ordinary non-unique single-Int32
indexes alongside unique/primary declarations. Key ties retain ascending physical
row-reference order in either key direction, including node separators and shared
aliases. Unique/required/rule checks and original topology/prefix/width/bitmap/
free-space preservation remain mandatory. No selected NULL transition, foreign
mutation, allocation or ordinary ACE save capability is enabled.

Independent ACE 2016/2019 and Jet 4 controls match 21 candidate copies, including
three exact no-ops, complete rows/Memo/OLE values, duplicate index order and byte
spans. Fifteen invalid requests publish no output; existing outputs and originals
remain unchanged. All seven Jet 4 CLI copies match engine output, while normal
ACE editing remains blocked. [Contract](NATIVE_WRITES.md#non-unique-int32-replacements)
and [hashes](FIXTURES.md#non-unique-index-update-inputs) retain native qualification gaps.

Verification passed all 657 all-feature workspace tests/doc tests, the final
nineteen-test mutation suite and eighteen-test default mutation suite,
warnings-denied workspace Clippy/rustdoc and the default release build. Formatting,
the twelve-package dependency guard, ten Python regressions and 994 local
links/anchors across seventy-two public Markdown documents pass. Registered sample
and reference hashes are unchanged. No dependency or package boundary changed;
the full backend roadmap remains in progress.

## Foreign-index physical reads and checks

Named foreign-index reads and typed numeric ranges now validate reciprocal links
and relationship-catalog agreement before using the existing physical key codecs.
Complete table checks traverse eligible foreign trees and validate exact unique/
required declarations. Unsupported metadata stays explicit; malformed metadata,
physical failures, resource limits and cancellation return no partial result.
Qualification is operation-local; fresh raw/link inspection remains unqualified.
An explicit foreign-index write guard preserves ConstraintCoverage for previews
and otherwise eligible journalable Jet 4 plans. No row-membership/cascade or native
Access qualification follows from these reads.

Independent comparisons match 38 foreign-index reads and 5,408 rows across fresh
v3 ACE families, the registered sample and existing nullable Large Number fixtures.
Duplicate child keys, aliases, composite/self relationships and node roots are
covered. Eight corrupted copies reject through both reads and complete checks;
all original hashes remain unchanged. [Contract](NATIVE_INDEXES.md#foreign-physical-traversal-and-checks)
and [fixture evidence](FIXTURES.md#foreign-physical-read-inputs) record current limits.

Verification passed all 653 all-feature workspace tests/doc tests, the final
fourteen-test foreign-index suite and thirteen-test default-feature suite,
warnings-denied workspace Clippy/rustdoc and the default release build. Formatting,
the twelve-package dependency guard, ten Python regressions and 985 local
links/anchors across seventy-two public Markdown documents pass. No dependency
or package boundary changed. Native Access comparison, foreign-index mutation,
data macros and broader backend roadmap work remain incomplete.

## Foreign-index relationship-catalog agreement

Reciprocal foreign-index inspection now verifies MSysRelationships endpoint and
column-pair mappings, action declarations, required parent/one-to-one uniqueness
and complete enforced endpoint coverage. Self-relations require both local ends;
non-enforced relationships need no foreign index. Declaration ordinal permutations
are accepted without accepting crossed key pairs. Missing/ambiguous/disagreeing
metadata and unknown rules fail without a report. System relationship metadata
is read, while user rows and physical key pages remain untouched. Ordinary
foreign-index constraints and all existing write gates remain unchanged.

Independent v2 controls match 28 directed links across both ACE families and two
registered-sample links. Sixty-two corrupt copies reject, and reordered composite
declarations agree with both readers. [Contract](NATIVE_INDEXES.md#reciprocal-foreign-index-links)
and [provenance](FIXTURES.md#relationship-catalog-agreement-inputs) keep metadata
evidence separate from row membership, physical reads/writes and native Access.

Verification passed the 647-test/doc-test all-feature workspace suite, followed by
the final eight-test foreign-index suite and warnings-denied workspace Clippy
after strengthening source-identity checks. The current default release build,
warnings-denied rustdoc, formatting, twelve-package dependency guard, ten Python
regressions and 974 local links/anchors across seventy-two public Markdown documents
pass. Independent comparisons cover both ends of every generated enforced edge,
including source-page aliases; original sample and reference hashes are unchanged.
The full backend roadmap remains in progress.

## Reciprocal foreign-index inspection

Native logical index metadata now retains foreign-reference pointers, role and
raw action bytes separately from physical flags. Explicit reciprocal inspection
checks unique local target identities, logical index numbers, opposite roles,
matching actions/key counts and back-pointers without recursion or user-row reads.
Self-references and shared physical primary aliases retain distinct logical links.
Unknown profiles and broken pointers return no partial report. No existing
foreign-index read/constraint/write profile or ordinary ACE gate is widened.

Fresh independent restrict, cascade, SET NULL, composite, self and alias inputs
match 24 directed links across V2016/V2019. Two registered-sample links also match;
32 malformed copies reject without output, and all originals remain unchanged.
[Inspection contract](NATIVE_INDEXES.md#reciprocal-foreign-index-links) and
[fixture evidence](FIXTURES.md#reciprocal-foreign-index-inputs) distinguish this from
MSysRelationships agreement, physical-tree qualification and native Access work.

Verification passes the 644-test/doc-test all-feature workspace suite, the final
six-test foreign-index suite (including an additional ambiguous-catalog regression),
warnings-denied workspace Clippy and the final ACE Clippy check, the default release
build and warnings-denied all-feature rustdoc. Formatting, the twelve-package
dependency guard, ten Python regressions and 970 local links/anchors across
seventy-two public Markdown documents pass. Registered sample/reference hashes
remain unchanged. The full backend roadmap remains in progress.

## ACE 16 Boolean and NULL-mask candidates

The optional fixed-scalar preview now handles Boolean mask values and non-indexed
NULL transitions for the nine fixed carriers. Only the selected mask bit and, for
NULL-to-value, its exact payload may change. Clearing NULL presence preserves
ignored bytes; no-op NULL/Boolean requests preserve the complete image. Changed
rows count logical updates, not field/mask patch spans. Normal ACE writes remain
blocked and no data macros execute or become implicitly qualified.

Shared row/field validation, typed expected-value checks and whole-image proofs
remain in force. Mask changes recheck all supported indexes after construction.
Required/rule failures and cancellation reject before publication. Fresh independent
fixtures retain enforced relationships and their automatic type-2 foreign-key
indexes; those unqualified index links still block child edits with ConstraintCoverage.
They were not removed to make the test pass. [Mask contract](testing/ACE16_WRITE_PREVIEWS.md#boolean-and-null-mask-transitions)
records the current profile, independent comparisons and remaining native/index work.

Verification passes all 639 all-feature workspace tests/doc tests, the final
five-test mask suite, warnings-denied workspace Clippy/rustdoc, the default release
build, formatting, the twelve-package dependency guard and ten Python regressions.
Independent comparisons pass 84 mask candidates (28 exact no-ops) and 24 rejected
requests across both families; all eighteen previous scalar candidates still match.
All 958 local links/anchors across seventy-two public Markdown documents pass.
Registered sample and reference artifact hashes remain unchanged. Native Access
comparison, data macros and ordinary ACE writes remain unqualified.

## Shared native row visitation and patch application

Native fixed-field mutation now shares bounded snapshot-row visitation, duplicate/
missing-row checks and byte-patch application/preservation helpers. This prepares
mask-aware updates without widening any public write profile. Fixed Int32 storage
and unique-index behavior retain their existing contract. All 634 all-feature
workspace tests/doc tests and warnings-denied workspace Clippy pass after extraction.
Native Access/data-macro qualification and ordinary ACE write blockers are unchanged.

## ACE 16 exact fixed scalar candidates

The opt-in engineering preview now handles exact Byte, Int16/32/64, Currency,
Single, Double, legacy DateTime and GUID replacements through FixedScalarUpdate.
Shared const-width patching preserves the original Int32 footprint and native
index behavior, with fallible charged buffers and exact carrier/byte checks.
Other indexed targets, NULL transitions, Boolean masks and variable/extended
carriers remain explicitly unsupported. The normal ACE save gate is unchanged.

Fresh independent V2016/V2019 fixtures match eighteen scalar candidate files,
including exact wide extrema, floating bits, native GUID order and complete
unrelated Memo/OLE contents. Only the selected field span changes; originals and
existing outputs remain intact. [Contract and evidence](testing/ACE16_WRITE_PREVIEWS.md#exact-fixed-scalar-candidates)
include six focused tests and a 128-case multirow Int64 model. No new dependency,
SQL coercion or native Access/data-macro qualification is implied.

Verification passed 634 all-feature workspace tests/doc tests and the final
six-test fixed-update suite, warnings-denied workspace Clippy/rustdoc, the default
release build, the twelve-package dependency guard/ten Python regressions,
formatting and 950 local links/anchors. Fresh independent comparisons pass for
all eighteen candidates; Jet 4 Decimal-adjacent and ACE 16 Int32 candidate
regressions also pass unchanged. The registered sample and reference artifact
hashes are unchanged. Actual Access, broader index mutation and production writes remain
unqualified; the backend roadmap is still in progress.

## ACE 16 physical write previews

The opt-in `ace16-write-preview` feature now exposes a distinct candidate type
for bounded fixed non-NULL Int32 replacements and qualified unique Int32 index
reordering on unencoded 2016/2019 layouts. It reuses existing constraints,
property/opaque-child blockers, expected-value checks and unchanged-byte proofs.
It has no native journal/commit operation and cannot convert to NativeEditPlan;
normal ACE writes remain blocked. CLI/Access consumers do not enable the feature.

The fresh-copy example completes validation before output creation, refuses
existing paths and identifies its engineering qualification. Fresh independent
V2016/V2019 fixtures match reopened rows, Memo/OLE lengths and index references;
every byte outside selected field/index-entry spans remains identical. Rules,
stale values and duplicate keys reject without files, existing candidate paths
are retained, and original images are unchanged. [Contract, reproduction and
Windows checklist](testing/ACE16_WRITE_PREVIEWS.md) separate these byte-level
results from missing macro semantics, actual Access comparisons and production
save/shared-file safety. No registry dependency or cross-crate boundary changed.

Final verification on macOS ARM passed all 628 all-feature workspace tests/doc tests,
all-target/all-feature workspace Clippy with warnings denied, the default release
workspace build and warnings-denied all-feature rustdoc. The twelve-package guard,
ten Python regressions, formatting, 945 local links/anchors across seventy-two
public Markdown documents and the original registered fixture hash also pass.
Windows/native Access comparisons remain pending. Disposable independent originals,
candidates and a hash manifest are prepared locally for that later work.

## Jet 4 current-profile extension

[Jet 4 qualification](NATIVE_JET4.md) now covers legacy Decimal single/composite
index order and exact relationship membership, independently compared against fresh
V2000 inputs. ACE 12 corrected Decimal encoding also has fresh V2007 comparisons.
Returned values retain exact coefficients/scales; the private legacy direction key
never supplies SQL comparison. Existing ACE 16 controls match unchanged.
Jet 4 parameter and compound saved-SELECT reconstruction matches the same owned
models, including buffered malformed-copy rejection. A fresh Jet 4 Int32 UPDATE
beside Decimal indexes changes only its exact field span and matches both readers.
No new dependencies, native engine/runtime boundaries or default ACE write gates
changed. Other legacy expansion stops here; ACE 16 remains the next focus.

Verification passed the all-feature workspace suite (622 tests/doc tests before
one additional preservation regression), the final eight-test Decimal suite,
all-target/all-feature workspace Clippy with warnings denied, independent Decimal/
saved-query/edit comparisons, formatting and unchanged registered fixture bytes.
No Windows/native Access, encryption, shared-file safety or full CRUD claim follows.

## Built-in backend and external-provider ownership

The twelve-package workspace now places shared DB contracts in
`crates/foundation/tablecraft-db` and built-in Access/SQLite integration in
`crates/backends/`. All 54 relocated files are byte-identical; root member/dependency
paths, public path references and boundary-test fixtures follow the move. Cargo.lock,
registry features and reviewed dependency edges are unchanged. No external adapter
packages or empty directories were created.

[External Data](features/EXTERNAL_DATA.md) records import/append/link/export,
saved tasks, connection management, command search, Office file interchange and
all twelve supplied product-feature mappings. Later notes supersede the earlier
SharePoint-clone/CSV-only and all-integrations-in-adapters proposals: SharePoint
means a live Lists connector, with internal backends separate from external
providers. Native SQL Server, ODBC and Graph are future selections; no connection,
interchange, task runner, UI or external qualification was implemented. ACE 16
priority, SQLite pause, VBA deferrals and ACE write blockers remain unchanged.

Verification on macOS ARM passed all 52 relocated-package tests/doctests with
all features in release mode, all-target/all-feature Clippy with warnings denied,
formatting, locked/offline metadata and the twelve-package dependency guard with
ten regression tests. The workspace release build and CLI version, inline query
and native saved-SELECT smoke checks also passed. All 924 local links/anchors across seventy public Markdown
documents resolve; the original fixture hash is unchanged. The initial development
profile test link failed with disk exhaustion; after clearing only generated dev
artifacts, serial release testing passed. No native Access or external-provider
compatibility evidence is implied by these checks.

## Data-macro evidence dependency

Inert reinspection of the unchanged registered example finds three ordinary catalog
Macro objects, zero declared MSysAccessXML rows and 328 declared MSysAccessStorage
rows. This supplies no positive ACE 16 table-event fixture or proof of macro absence.
The reviewed independent artifact/fixture workflow has no qualified creation/
detection path for that evidence. No macro, provider or application code was run.

[The data-macro plan](testing/DATA_MACROS.md) now records primary desktop event
references, XML/service versus native-storage boundaries, required original ACE 16
fixture cases, exact build/provenance capture and acceptance before writes. Desktop
example XML and published service schemas use different namespaces; neither is a
qualified native ACCDB locator. Catalog Macro kinds, empty XML tables and NULL
opaque hints cannot replace positive/absence qualification. Native Access/Windows
comparisons remain unavailable/unrun in this session, and environment availability
has been requested. All ACE writes remain blocked; roadmap 1–9 are unfinished.

This is a documentation/evidence update, not an implemented macro subsystem. No
Rust or dependency behavior changed; 592 workspace/304 ACE all-feature results
from the preceding verified chunks remain the current baseline. Local link/anchor
checks and the original example hash check pass.

## Compound conditions on native two-source joins

The saved SELECT adapter now combines up to sixteen same-kind/same-orientation
condition fragments on the same two declared sources. It preserves the reviewed
reference's reverse stored-fragment order and resulting owned AND association,
without predicate sorting or deduplication. Each expression passes independent
original/guarded validation before bounded conjunction/source parsing. Borrowed
single-condition text and fallible charged multi-part buffers retain cancellation
and cumulative limits. Join keys must be present/four-byte/increasing; mixed kinds,
unqualified endpoints, invalid metadata and larger source graphs fail explicitly.

Two new focused tests cover two/three-condition ordering for every kind, rejected
metadata, expression boundaries, the 16/17 policy ceiling, budgets and late
cancellation. Independent V2016/V2019 fixtures match twenty-eight models including
six compound joins per family through both hosts. Eight raw-inspectable mutations
each reject reconstruction without stdout. [Contract and evidence](NATIVE_SAVED_QUERIES.md#multiple-conditions-on-the-same-pair)
separate syntax/order results from actual Access and native execution/error order.
All ACE writes remain blocked and roadmap 1–9 remain unfinished.

All 592 workspace tests/doc tests, all-target workspace Clippy, release CLI/example
builds, warnings-denied Access docs and dependency/documentation checks pass. The
parameter comparison corpus also passes with unchanged hashes. No dependencies
were added. Generated development build caches were cleared after checks when
volume space became critically low; release binaries and fixture evidence remain.

## Native saved parameter declarations and complete query model

`SavedSelect::query()` now returns the complete owned Query, retaining ordered
declarations and the SELECT retrieval statement. Final sorting/owner-access use
the same outer ReadQuery fields as SQL parsing. The parser-created statement
carrier receives existing owned syntax by move; no new unchecked Box allocation
or AST clone is introduced. Original native identity/fragments remain separate.

Twelve qualified type flags, literal ASCII prompts/names and nullable/zero/positive
Text syntax lengths work. Missing/unknown types, unhandled fields, negative lengths,
duplicate names, unsupported name forms and missing/duplicate/reversed ordering
fail without defaults. The declaration ceiling is 255 under existing SQL policy;
explicit budgets and cancellation remain cumulative. Variant/Memo/newer type tags,
qualified/Unicode name matching and native runtime-value limits remain gaps.

Four new adapter tests cover mapping/model equality, explicit memory-only carrier
binding, unsupported metadata, the 255/256 boundary, budgets and late cancellation.
One CLI test prints declarations without values or prompts, rejects an unknown
type, verifies private diagnostics and preserves input bytes. An original recipe
matches twenty-one models/eight parameterized definitions per V2016/V2019 family,
with twelve type tags and eight inspectable-but-rejected mutations each. The
[contract and provenance](NATIVE_SAVED_QUERIES.md#native-parameter-declarations)
separate syntax evidence from native Access/runtime qualification. All ACE writes
stay blocked and roadmap 1–9 remain unfinished.

All 590 workspace tests/doc tests and 304 ACE all-feature tests pass, alongside
both all-target Clippy profiles, release CLI/example builds, warnings-denied SQL/
Access docs, dependency policy/tests and documentation checks. The earlier SELECT
and join comparison corpora also pass with unchanged canonical hashes after the
complete-model change. No registry dependencies or features were added.

## Access IEEE floating type spellings

The SQL parser now recognizes IEEESINGLE and IEEEDOUBLE in parameter/DDL syntax,
preserving their distinct uppercase spellings. Existing parameter binding already
mapped them to Single/Double, but those branches were unreachable from SQL text.
One syntax round-trip test and one execution test cover both names, exact carrier
bits and rejection of a Double supplied for Single. The Microsoft type comparison
linked in [SQL parser references](SQL_PARSER.md) supplies the synonym facts.

All 585 workspace tests/doc tests, all-target workspace Clippy, release CLI and
dependency/documentation checks pass. No dependencies or native storage behavior
changed; native saved-parameter reconstruction remains the next adapter task.

## Qualified native two-source saved joins

The Access saved SELECT adapter now accepts exactly two declared named sources
connected by one INNER/LEFT/RIGHT fragment. Alias/name endpoints determine left/
right orientation independently of table storage order. ASCII case-insensitive
labels, distinct self-join aliases and Unicode table names with ASCII aliases
work. Missing/ambiguous endpoints fail; compound/larger graphs, Cartesian source
lists and non-ASCII label comparison remain explicit reconstruction gaps.

Conditions parse independently before the bounded source parser constructs the
owned Join node. The single root, kind and condition are checked before moving
its source vector into the model. Shared expression adaptation now also requires
stored and guard-enclosed text to parse to the same owned expression: delimiters
cannot repair malformed stored parentheses. Both parses charge existing allowances;
row/work/allocation limits and cancellation remain cumulative. No AST is cloned
and no query/source/function is bound or executed.

Three additional focused tests exercise kinds/orientations, self/Unicode sources,
missing/ambiguous/unsupported metadata, expression boundaries, budgets and late
cancellation. An original fixture mode adds nine joins to thirteen baseline
SELECTs. Independent V2016/V2019 comparisons match all twenty-two models through
both hosts; eight mutations/family retain inert inspectability but fail reconstruction
without stdout. [Contract and evidence](NATIVE_SAVED_QUERIES.md#two-source-native-joins)
separate these results from native Access and execution qualification.

All 583 workspace tests/doc tests, all-target workspace Clippy, release CLI/example
builds, warnings-denied Access docs and dependency/documentation checks pass.
The original thirteen-SELECT comparison also passes after the shared expression
boundary change, with unchanged hashes. No dependencies were added. All ACE writes
remain blocked and roadmap 1–9 remain unfinished.

## Saved SELECT CLI

`query saved --database PATH NAME` now composes the inert native definition reader
and qualified Access SELECT adapter. It opens only the explicit stable/offline
copy read-only, shares the native budget across both operations and buffers
normalized SQL under the common 1 MiB output ceiling. It does not bind functions,
expand saved sources, activate providers or execute SQL. Query dispatch parses
SQL only for commands that accept SQL text. No CLI or registry dependency was added.

Four additional integration tests cover normalized syntax with unresolved calls,
required arguments and rejected execution/write options, private verbose
diagnostics, namespace/unsupported/late-corruption rejection, unchanged input and
the registered example's quoted leading-digit source. The independent runner now
checks both the engineering host and public command for thirteen original SELECTs
and eight rejected corruption copies per V2016/V2019 family, plus the separately
qualified example identity regression. Canonical hashes remain unchanged.

All 580 workspace tests/doc tests, all-target workspace Clippy, release CLI and
local dependency/documentation checks pass. ACE all-feature qualification remains
304 tests from the preceding adapter chunk; this chunk changes only the host and
comparison runner. Native joins/parameters/external fragments and broader saved
query families remain unsupported for reconstruction. All ACE writes stay blocked;
roadmap 1–9, native Access conformance and application save/reopen remain unfinished.

## Owned native saved SELECT syntax

`tablecraft-access::reconstruct_saved_select` consumes an explicitly read native
definition into an owned SELECT while retaining its original fragments/identity.
The qualified zero/one-source profile preserves projections/aliases, wildcards,
WHERE, GROUP BY/HAVING, sort keys, DISTINCT/DISTINCTROW/TOP/PERCENT and owner-access
syntax. It rejects unhandled native metadata, actions, joins, parameters, external
source fragments, ambiguous singleton clauses and unsupported ordering/identifier
forms. Expression parsing preserves unresolved calls/subqueries without activation.
Reconstruction does not bind, execute, authorize owner-access or expand saved sources.

Strings/vectors reserve fallibly after budget checks; parser AST allowances and
clamped byte/token/depth limits bound adaptation. Fragment visits, cumulative
work/allocation and cancellation precede publication. Parenthesized expression
parsing keeps clause/list/statement syntax out of fragment boundaries; aliases
are parsed separately into owned names. A consuming SQL syntax accessor avoids
AST cloning. An engineering example buffers normalized output before stdout.

Seven integration tests include full model equality, separate flag semantics,
unknown/conflicting metadata, injection/identifier/order boundaries, budgets,
late cancellation, original example identity and a 128-case Unicode alias/literal
model. Independent V2016/V2019 recipes match thirteen reconstructed SELECTs and
reject eight corruption copies each. The existing example's reference renderer
uses a bare leading-digit name outside the SQL parser's current spelling profile;
that exact observation and an explicitly quoted expected identity are checked
separately. [Contract and provenance](NATIVE_SAVED_QUERIES.md) distinguish these
results from native Access conformance or application save/reopen.

All 576 workspace tests/doc tests and 304 ACE all-feature tests pass, alongside
both all-target Clippy profiles, release workspace/example builds, warnings-denied
workspace all-feature docs, dependency policy/tests and local documentation checks. Access now
uses the existing workspace SQL package as a normal dependency and existing
Proptest for generated tests; no registry versions/features were added. All ACE
writes stay blocked, SQLite expansion is paused and roadmap 1–9 remain unfinished.

## Inert native saved-query fragments

`read_saved_query_definition` now binds one explicit query through the scoped
catalog and matches its signed native ID against MSysQueries.ObjectId without
page masking or scalar coercion. Owned rows retain all eight known field carriers,
NULL/empty distinctions, unknown attributes, raw flags and native storage/order
bytes. Payloads move out of validated snapshots without cloning. Fallible
allocation, shared whole-system-table budgets and cancellation precede publication.
This inspects metadata without SQL reconstruction, validation, provider access or
execution; empty/partial/broken-index fragments are not labeled valid queries.

CLI `inspect --query NAME` buffers escaped/quoted fields using borrowed output
helpers and the existing output limit. It explicitly conflicts with every other
inspection mode, including combinations where Clap would suppress required-table
arguments. Table names do not become query definitions; saved-query names do not
become retrieval FROM sources. Verbose diagnostics remain payload-free.

Five native tests cover negative/scoped IDs, namespace/empty-definition behavior,
unknown/nullable values, carrier/geometry failures, unchanged input, budgets/
cancellation and a 128-case representation model. A retained seed guards an empty
synthetic usage map; a unit sweep round-trips all 256 attribute bytes. Three CLI
tests cover exact inert output, quoted NULL/empty fields, every conflicting mode,
private diagnostics and buffered late failures. Independent V2016/V2019 originals
match 11 definitions/75 fragments per family, plus 59 queries/332 fragments in the
unchanged registered ACE-code-2 example. Four inspection mutations and two buffered
failures/profile also pass. [Contract and evidence](NATIVE_SAVED_QUERIES.md) keep
index/header/query integrity, SQL reconstruction/execution, actual Access and
application save/reopen unqualified. All ACE writes stay blocked; 1–9 remain unfinished.

All 569 workspace tests/doc tests and 304 ACE all-feature tests pass, along with
both all-target Clippy profiles, release CLI/all-example builds, warnings-denied
ACE docs, dependency policy/tests and local documentation checks. No dependency
was added; local test debug symbols remain disabled and release defaults unchanged.

## Stored Memo version histories

`read_version_history_column` now resolves a visible Memo name to its checked
hidden history storage without generated-name heuristics. The exact ACE 16
two-field template, observed hidden-key marker and flat schema/reference checks
must agree. Borrowed `StoredVersion` records preserve scoped identity, nullable
text, exact nullable DATE bits and storage order. No current value, timestamp,
newest-first order or latest-value policy is synthesized. Resolved metadata feeds
the shared link reader directly, avoiding duplicate schema resolution.

Five CI tests cover visible/hidden/source/profile binding, template reordering,
exact representation/storage order, NULL/empty distinctions, orphan rejection,
unchanged source bytes, resource/cancellation limits and a 128-case Unicode/DATE
sequence model. A retained seed guards valid synthetic memo encoding outside
the reserved compressed-text header. Original V2016/V2019 fixtures independently
match six parent collections/18 versions per family, with 13 literal timestamp
bit probes, different visible Memo names across two tables and nullable records.
Public reference API membership agrees; its equal-time tie ordering is observed
separately. Six corrupt copies fail without stdout, while a NULL-time mutation
still matches raw storage. Earlier link/multivalue/attachment comparisons pass
unchanged after the shared-reader extraction.

[Contract and evidence](features/ACCESS_ADVANCED_FIELDS.md#stored-version-history-collections)
keep current-value reconciliation, default ordering, history mutation, query
expansion, actual Access and application save/reopen unqualified. The reference
builder's one-append-only-Memo-per-table restriction is not an Access claim.
All ACE writes remain blocked; milestones 1–9 remain unfinished.

All 560 workspace tests/doc tests and 298 ACE all-feature tests pass, as do both
all-target Clippy profiles, release CLI/all-example builds, warnings-denied ACE
docs, dependency policy/tests and local documentation checks. No dependency was
added; local test debug symbols stay disabled and release defaults are unchanged.

## Typed stored attachment collections

`read_attachment_column` now returns checked `AttachmentColumn` collections with
borrowed typed `StoredAttachment` metadata and wrapped bytes. Nullable URLs/names/
type labels/data, exact DATE serial bits and raw flags stay distinct from empty
values; scoped child identities and original storage order remain available.
The six named conceptual/flat fields and observed template flags are qualified
before publication. Borrowed accessors return typed errors without unchecked
casts or extra payload copies; record checks charge work and cancellation.

Explicit `StoredAttachment::decode` delegates to the existing bounded decoder:
NULL payload stays None, while present invalid bytes fail. Stored field labels
do not replace wrapper labels, and metadata inspection does not inflate data.
The engineering attachment example now consumes this API, retaining its original
decoded output and adding an explicit encoded-metadata mode. No extraction,
URL resolution, active-content execution or write permission was introduced.

Four CI tests cover exact metadata, NULL/empty distinctions, template reordering/
guards, inert Unicode/NUL/path-like labels, negative DATE bits, independent zlib
bytes, resource/cancellation limits and unchanged source bytes. Independent
V2016/V2019 comparisons match all six stored records/profile. Six disposable
mutations/profile verify preserved NULL/corrupt-wrapper metadata and buffered
decode/template/reference failures, including a late final-file failure. The
65,810-byte original decode comparison and input hashes remain unchanged.
[Contract and evidence](features/ACCESS_ADVANCED_FIELDS.md#typed-stored-attachment-collections)
keep native Access, mutation, query expansion, streaming and application save/
reopen unqualified. All ACE writes stay blocked; milestones 1–9 remain unfinished.

All 555 workspace tests/doc tests and 293 ACE all-feature tests pass, with both
all-target Clippy profiles, release CLI/example builds, warnings-denied ACE docs,
dependency policy/tests and local documentation checks. No dependency was added;
local test debug symbols remain disabled and release defaults remain unchanged.

## Bounded stored multivalue collections

`read_multivalue_column` now returns an owned, validated `MultivalueColumn` with
borrowing scalar-value iterators and the existing raw schema/identity links.
Exact storage order, duplicates, NULL cells and empty/absent parent distinctions
are retained. Qualified scalar templates include exact Decimal metadata; unknown
flags/templates and more than 100 values per parent fail before publication.
No SQL expansion, lookup display translation, sorting/deduplication or mutation
is inferred. The existing fallible storage reads/budgets/cancellation remain shared.

An original fixture exposed the reviewed reference's empty Decimal conceptual
template: precision 18/scale 0 and declared width 9, paired with ordinary 17-byte
child values. A focused metadata exception now accepts that exact hint; no
nine-byte record decoder was added. Strict synthetic tests reject changed hints,
nonempty templates and incompatible children. Seven new tests also cover exact
Large Number values, 100/101 boundaries, reference/type/resource/cancellation
failures, unchanged source bytes and a 128-case nullable sequence model.

Independent V2016/V2019 comparisons match 32 collections/137 stored values per
family through the reviewed Jackcess 5.0.4 collection API. Twelve disposable corrupt
copies fail without output; NULL/empty/duplicates and exact IEEE/Decimal carriers
match, and the separate 101-item control remains readable through raw links.
Earlier complex-link comparisons still pass unchanged. [Contract and evidence](features/ACCESS_ADVANCED_FIELDS.md#bounded-stored-multivalue-collections)
keep SQL expansion, lookup display semantics, mutation, actual Access and
application save/reopen unqualified. All ACE writes stay blocked; 1–9 remain unfinished.

All 551 workspace tests/doc tests and 289 ACE all-feature tests pass, with both
all-target Clippy profiles, release CLI/example builds, warnings-denied ACE docs,
dependency policy/tests and local documentation checks. No dependency was added;
local test debug symbols remain disabled and release defaults remain unchanged.

## Exact ACE 16 Decimal foreign-key audits

Native constraint reports now check one-to-ten-column tuples containing Decimal
fields with matching paired precision/scale on ACE 16-family profiles. Separate
wider equality atoms retain exact i128 coefficients and scale; compact integer/
Currency/GUID tuple allocations remain unchanged. All-NULL exemption, exact
partial NULLs, parent/one-to-one uniqueness, fallible allocation and bounded
sorting/search/cancellation use the existing audit. Older Decimal profiles and
metadata mismatches remain Unsupported; physical index checks stay separate.

Six integration tests cover all 29 scales, exact extrema/adjacent coefficients,
mixed carriers, empty/NULL metadata/profile rejection, unchanged source bytes,
resource/cancellation limits and a 256-case independent tuple-set model. A focused
unit test retains scale/domain distinctions for zero. Original V2016/V2019
recipes independently match 116 exact rows and 12 table audits/profile; 17 corrupt
copies fail without stdout, two precision/scale mismatches decline, and all-NULL
exemption remains. Currency's 48-row comparison also passes on unchanged inputs.
[Contract and evidence](NATIVE_CONSTRAINTS.md#exact-decimal-relationship-membership)
keep native Access, cross-scale semantics, cascades and Decimal mutation
unqualified. All ACE writes remain blocked; milestones 1–9 are not complete.

All 544 workspace tests/doc tests and 282 ACE all-feature tests pass, as do both
all-target Clippy profiles, release CLI/example builds, warnings-denied ACE docs,
dependency policy/tests and local documentation links. Local test debug symbols
remain disabled; release defaults/runtime assertions are unchanged. No dependency
or SQL coercion policy changed.

## Separate compact and Decimal column metadata

Composite profiles now select compact numeric/GUID column sets or wider Decimal
column sets statically. Both use the same bounded tuple policy and key models.
The compact component codec no longer carries a Decimal branch, addressing the
3–5% instruction regression measured in the initial tuple implementation. Native
type detection chooses the set; the builders retain full format, flag, column
mapping and precision/scale qualification. The read/check API, entry-buffer caps,
resource/cancellation checks and write gates are unchanged. No dependency or
new compatibility profile was added.

All 537 workspace tests/doc tests and 275 ACE all-feature tests pass, as do both
all-target Clippy profiles, release CLI/examples/benchmark builds, warnings-denied
ACE docs and dependency/documentation checks. Independent numeric/GUID/Decimal
tuple runners match 12,537 ordered rows per family on unchanged inputs. Local
test debug symbols remain disabled; release defaults are unchanged.
[The follow-up benchmark](testing/NATIVE_BENCHMARKS.md#separate-column-set-follow-up)
retains 520 samples: five/ten numeric tuples improve 0.559/0.514 and 0.369/0.330 ms
before/after, with 2.8%/4.4% fewer instructions. Mixed GUID/Decimal wall results
vary; no uniform improvement is claimed. Both reports retain the original
regression and all 1,040 samples. Actual Access, application save/reopen and all
ACE writes remain unqualified; milestones 1–9 continue.

## Complete Decimal-bearing composite indexes

ACE 16-family two-to-ten-component Decimal tuples now retain each field's exact
i128 coefficient, precision and scale, including mixed GUID/numeric/Boolean/legacy
DateTime keys and reordered directions. A dedicated wider atom model is selected
only for Decimal-bearing tuples; compact numeric/GUID-only models stay separate.
The existing derived 198-byte buffer still bounds every supported combination.
Actual allocation sizes and per-component work remain charged; NULL/zero and
unique/primary/required semantics retain the common complete reader/checker.

Six new tests cover every other qualified numeric carrier, per-field scales,
nullable typed models, maximum compressed prefixes, exact constraints, corruption,
profile/resource/seek/cancellation boundaries and ACE edit rejection. Original
V2016/V2019 recipes independently match 3,512 ordered rows/profile, maximum-width
ten-Decimal node trees (188/184-byte entries), residual queries, complete checks
and buffered failure for corrupt roots/unqualified Text tuples. The earlier
6,116-row single-Decimal comparison now also checks its empty tuple; original
hashes remain unchanged. [Encoding/evidence](NATIVE_INDEXES.md#complete-decimal-bearing-composite-indexes)
keep Decimal FK/seeks/mutation, old variants, actual Access and application
save/reopen unqualified. No dependency or SQL coercion/ordering policy changed.
All ACE writes remain blocked; milestones 1–9 continue.

All 537 workspace tests/doc tests and 275 ACE all-feature tests pass, as do both
all-target Clippy profiles, release CLI/examples/benchmark builds, warnings-denied
ACE docs and dependency/documentation checks. The 4,512-row GUID tuple and 6,116-row
single-Decimal comparisons pass on both unchanged inputs. Local test debug
symbols remain disabled; runtime assertions/release defaults are unchanged.
[Initial benchmarks](testing/NATIVE_BENCHMARKS.md#decimal-tuple-measurements) retain
520 samples: existing numeric tuples cost 3–5% more instructions and about 10%
more user time/cycles, despite retaining their key models. Shared component
metadata dispatch is the next optimization target. New complete mixed 1,000-row
reads cost 0.929 ms indexed/0.454 ms sequential; ten Decimal fields over 500 rows
0.650/0.258 ms. No speed or parity claim is made.

## Reuse current root data pages during indexed reads

Complete and typed-range index readers now reuse one current data-page buffer
for consecutive references. Returning to another page fetches it again; each
native operation starts fresh. The existing borrowed-root accessor retains
owner/slot/deletion checks, row/key agreement and per-row work/cancellation.
Relocation targets still take physical reads, including same-page targets. No
global cache, dependency, format eligibility or write permission was added.

Four new tests trace hits/misses, fresh operations, short reads, empty selections,
physical relocation failures, late cached corruption and every cancellation point.
Range tests retain disjoint-child pruning and exact selected-row budget charging.
Access adapter tests now track required-index reads separately from data-page
reads, proving that partial snapshots stay separate from complete-table caches
and declined hints cannot silently select a sequential fallback.

Independent GUID tuple, IEEE, DATE and Decimal comparisons still match 25,696
ordered rows per V2016/V2019 family, with unchanged original hashes, exact bit/
coefficient checks, complete constraints and buffered corrupt-root failures.
Native Access, shared-file snapshots and application save/reopen remain unrun;
all ACE writes stay blocked and milestones 1–9 continue.

All 531 workspace tests/doc tests and 269 ACE all-feature tests pass, as do both
all-target Clippy profiles, release CLI/examples/benchmark builds, warnings-denied
ACE docs and dependency/documentation checks. Local test debug symbols remain
disabled; release flags/default features match the frozen baseline. [Recorded benchmarks](testing/NATIVE_BENCHMARKS.md#indexed-root-page-reuse)
retain all 800 samples: 1,000-row Int32 reads improve 0.538/0.361 ms before/current,
GUID 0.413/0.222 ms, Single/Double about 0.41/0.21 ms, DateTime 0.406/0.203 ms and
scale-four Decimal 0.476/0.284 ms. Scattered mixed GUID/DATE tuples gain little.
These are complete public API reads over memory-backed Cursors, not disk or
application timings. No additional I/O refactor or dependency was needed.

## Complete ACE 16 single-Decimal index reads/checks

ACE 16-family single-Decimal indexes now preserve exact i128 coefficients and
declared scale/precision through complete reads/checks. A separate corrected
fixed-point codec and 26-byte entry buffer avoid widening existing compact key
models. Every key, including internal bounds, validates sign and precision;
row agreement rejects different carriers/scales without coercion. NULL and zero
remain distinct. No dependency was added.

Original V2016/V2019 recipes independently match 6,116 ordered rows/profile,
precisions 4/28, scales 0/2/4/28, beyond-i64/2^53 neighbors, full extrema, complete
node trees and unique/primary/NULL checks. Required-index residual queries and
corrupt-root buffering pass with unchanged inputs. Seven new tests include an
independent i128 model, carrier/scale gates, sign/word-order goldens, compressed
prefixes, invalid parent bounds, metadata/profile/resource failures and ACE write
gates. [Encoding/evidence](NATIVE_INDEXES.md#complete-single-decimal-indexes) keep
Decimal composites, old numeric index variants, seeks/key mutation, foreign keys,
actual Access and application save/reopen explicitly unqualified. All ACE writes
remain blocked; milestones 1–9 continue beyond this slice.

All 527 workspace tests/doc tests and 265 ACE all-feature tests pass, as do both
all-target Clippy profiles, release CLI/examples/benchmark builds, warnings-denied
ACE docs and dependency/documentation checks. Local test debug symbols remain
disabled; runtime assertions/release defaults
are unchanged. [Recorded benchmarks](testing/NATIVE_BENCHMARKS.md#single-decimal-index-measurements)
retain 360 samples: complete 1,000-row Decimal reads take about 0.483–0.487 ms
indexed versus 0.229–0.233 ms sequential. Existing Int32 controls remain close in
this run; no speed improvement is claimed.

## Complete legacy DateTime index reads/checks

Legacy DATE now participates in complete single/mixed indexes using exact serial
bits, with focused finite/calendar-envelope validation for every key, including
internal separators. Signed zero, negative day-zero aliases and sub-millisecond
values remain unchanged. Physical serial order stays separate from SQL civil-time
equality/sorting. Existing entry buffers, limits and typed seek gates remain
unchanged; no dependency was added.

Original V2016/V2019 recipes independently match 4,528 ordered rows/profile,
exact serial bytes, sequential controls, complete node-tree/constraint checks and
buffered failure for corrupt roots. Residual SQL checks explicitly prove alias
equality and civil ordering over required physical indexes. Six new tests include
an arbitrary valid-bit model, wire goldens, a malformed parent separator with a
valid live child, mixed/ten-component keys, constraints, resource failures and
unrelated Jet 4 edit preservation. [Encoding/evidence](NATIVE_INDEXES.md#complete-legacy-datetime-indexes)
keep the distinction between artifact and actual Access behavior. DateTime key
mutation/seeks, foreign-key qualification, Extended DateTime indexes, all ACE
writes and application save/reopen remain unqualified; milestones 1–9 continue.

All 520 workspace tests/doc tests and 258 ACE all-feature tests pass, as do both
all-target Clippy profiles, release CLI/examples/benchmark builds, warnings-denied
ACE docs and dependency/documentation checks. The existing 10,540-row IEEE
comparison passes on both unchanged inputs after disabling oracle expression
evaluation explicitly. Local test debug symbols remain disabled; runtime
assertions/release flags are unchanged. [Recorded benchmarks](testing/NATIVE_BENCHMARKS.md#legacy-datetime-index-measurements)
retain 320 samples: 1,000-row DateTime reads take 0.412 ms indexed/0.157 ms
sequential, mixed rows 0.627/0.212 ms. Int32 controls remain close in this run.

## Exact DATE comparison oracle correction

The independent date-bit helper now serializes the reviewed Jackcess DATE carrier
to an in-memory buffer, validating ordinary widths or the known calculated
prefix/length/trailer. Its former `toDateDouble` path can normalize aliases and
round through milliseconds. Original [precision probes](testing/compare_date_bits.py)
now match 28 ordinary/cached cells per V2016/V2019 family against golden bits,
including signed zero, subnormals, negative aliases and sub-millisecond values.
Generation demonstrates the old conversion loses at least one probe's bits.
The earlier 30-cell stored-scalar comparison still passes with unchanged input
hashes. No production Rust, dependencies, database bytes or expression execution
changed; native Access remains unrun. DateTime index qualification follows this
stronger oracle rather than treating a calendar rendering as exact bit evidence.

## Complete Single/Double index reads/checks

Complete single and mixed indexes now retain exact Single/Double bits. A focused
integer-only transform preserves signed zeros, subnormals, infinities and all NaN
signs/payloads under private physical ordering, without new SQL coercion or float
equality. Typed integer seeks reject floating carriers and existing bounded
buffers/allocations remain unchanged. Six new tests include arbitrary IEEE bits,
exact constraints, compressed prefixes, mixed/ten-component keys, resource errors
and unrelated Jet 4 preservation. No dependency was added.

Original V2016/V2019 recipes independently match 10,540 ordered rows/profile,
exact tagged IEEE bytes, sequential controls and complete node-tree checks. Their
unique indexes distinguish signed zeros/quiet NaN payloads and reject identical
keys. Finite-only controls pass required-index CLI/residual queries; non-finite
tables retain query's explicit rejection, even for ID-only projections. New
engineering-only tagged index modes avoid changing the CLI output protocol.
[Encoding and evidence](NATIVE_INDEXES.md#complete-single-and-double-indexes) retain
the native Access/non-finite qualification limits. All ACE writes and floating
key mutation/seeks/foreign keys remain outside this slice; milestones 1–9 continue.

All 514 workspace tests/doc tests and 252 ACE all-feature tests pass, as do both
all-target Clippy profiles, release CLI/examples/benchmark builds, warnings-denied
ACE docs and dependency/documentation checks. Local test debug symbols remain
disabled; runtime assertions/release flags are unchanged. [Recorded benchmarks](testing/NATIVE_BENCHMARKS.md#boolean-and-ieee-index-measurements)
retain 600 samples: complete 1,000-row IEEE reads take about 0.415–0.420 ms indexed
versus 0.158–0.159 ms sequential; mixed rows 0.641/0.218 ms. Numeric tuple controls
cost about 1.6–2.1% more instructions; wall medians vary. No speed gain is claimed.
Actual Microsoft Access and application save/reopen remain unrun.

## Complete Boolean index reads/checks

Native Yes/No now participates in complete single and mixed numeric/GUID index
reads/checks. A focused strict two-value codec preserves true-before-false native
ascending order and descending complementation. False remains a value, never NULL;
malformed bytes/NULL markers fail and row agreement requires Boolean carriers.
Existing buffers, allocation charging, SQL semantics and typed seek boundaries
remain unchanged. No dependency was added. Six new tests cover arbitrary Boolean
sequences, compression, every value byte, exact constraints, mixed/ten-component
tuples, resource failures and unrelated Jet 4 edit preservation.

Original V2016/V2019 recipes independently match 4,509 ordered rows/profile,
actual node trees, required-index residual queries and complete checks. Generation
also asserts stored NULL-to-false conversion and duplicate NULL-input rejection
on unique/primary indexes. Input hashes remain unchanged. [Encoding and evidence](NATIVE_INDEXES.md#complete-boolean-indexes)
retain the native stored-field scope. All ACE writes, Boolean key mutation/seeks,
foreign-key qualification and actual Microsoft Access evidence remain unqualified;
milestones 1–9 continue beyond this slice.

All 508 workspace tests/doc tests and 246 ACE all-feature tests pass, as do both
all-target Clippy profiles, release CLI, formatting and dependency/documentation
checks. The earlier 4,512-row GUID tuple comparison passes on both families.
Test debug symbols remain disabled for local disk space; runtime assertions and
release flags are unchanged. Native Access and application save/reopen remain unrun.

## Complete GUID-bearing composite indexes

Complete index reads/checks now support two-to-ten mixed numeric/GUID components,
including reordered directions, exact NULL shapes and unique/primary/required
tuples. Composite models, column codecs and profiles live in focused modules.
Static profiles keep compact numeric tuples and their earlier buffer limits;
only GUID-bearing tuples use wider atoms and a derived 198-byte entry buffer.
Single/compound GUIDs share strict segment validation. Actual allocation sizes
and bounded component work remain charged, with no new SQL coercion/ordering.

Original V2016/V2019 recipes match 4,512 ordered rows/profile, required-index NULL
filters, complete checks and immutable input hashes. Ten-GUID trees reach depth
two, with maximum expanded entries 198/194 bytes. The separately committed
explicit-final-child fix resolved their valid zero-tail roots. Six new tests
cover mixed carriers, a generated tuple model, ten-GUID/193-byte prefix goldens,
NULL/constraint/type/corruption/resource boundaries and unrelated Jet 4 edit proofs.
Affected composite key edits, integer seeks over composites and all ACE writes
remain blocked. The earlier empty GUID/Int32 corpus now qualifies too.

All 502 workspace tests/doc tests and 240 ACE all-feature tests, both Clippy profiles,
release CLI/examples/bench builds, warnings-denied ACE docs and dependency checks
pass. Test debug symbols were disabled after local disk exhaustion; release flags
are unchanged. [Two benchmark reports](testing/NATIVE_BENCHMARKS.md#guid-bearing-tuple-measurements)
retain all 1,040 samples: numeric tuple instructions cost about 1.7–2.1% more;
wall medians are noisy. An inconclusive inlining contender was removed. Final
source fingerprints match the original tested implementation. No dependency was
added. Composite seeks/mutation, other index encodings, native Access, application
save/reopen and milestones 1–9 remain ongoing.

## Explicit final-child index nodes

Complete reads, checks and typed single-key seeks now accept nonempty internal
nodes whose children all have explicit upper-bound entries and whose separate
tail pointer is zero. The walker no longer follows an absent pointer into page
zero. Childless internal nodes, nonzero leaf tails, duplicate child traversal,
cycles and invalid bounds still fail. The existing key writer explicitly rejects
this valid read topology until its mutation is qualified.

Three independent synthetic tests cover full results, check coverage, a pruned
seek, corruption and mutation rejection. The fix was also applied to a clean
archive of its parent commit and validated separately from the in-progress wider
GUID-tuple feature. This changes structural read coverage without enabling ACE
writes or claiming Microsoft Access conformance.
All 234 ACE all-feature tests/doc tests and all-targets/all-feature Clippy pass
in that isolated parent-plus-fix checkout. Test debug symbols were disabled to
fit local disk space; runtime/debug assertions and the release profile are unchanged.

## Complete single-GUID index reads/checks

Single-GUID indexes now retain canonical 128-bit keys through the shared complete
tree/row reader and explicit unique/primary/required checks. A dedicated bounded
segment codec validates the fixed intermediate/final tags, descending rules and
byte-order boundary, with a separate 27-byte entry buffer. NULL and zero GUIDs
remain distinct. Core Guid and SQL comparison semantics do not gain general
ordering; integer seeks and GUID composites remain unsupported.

Fresh V2016/V2019 recipes independently match 2,034 ordered rows/profile, actual
node trees, complete checks, required-index NULL filters and buffered failure for
corrupt selected roots/unqualified GUID composites. Original hashes stay unchanged.
Five new tests include arbitrary-byte ordering, compressed segment/row prefixes,
exact constraints, tag/width/key/profile/resource failures and existing write gates.
Unrelated qualified Jet 4 Int32 edits preserve the full GUID index and every byte
outside their field; GUID targets and all ACE writes remain blocked.

All 493 workspace tests/doc tests and 231 ACE all-feature tests pass, as do both
Clippy profiles, release CLI, warnings-denied ACE docs and dependency/documentation/
fixture checks. [Encoding/evidence](NATIVE_INDEXES.md#complete-single-guid-indexes)
records the reviewed reference artifact, original recipes and limits. No dependency
was added. GUID seeks/mutation/composites, other index carriers, native Access,
application save/reopen and milestones 1–9 remain ongoing.

## Complete numeric composite indexes

Complete indexed reads/checks now support two-to-ten Byte/Int16/Int32/Int64/Currency
components with independent directions, native key order, exact NULL shapes and
row agreement. Shared bounded page/tree validation uses private static key
policies, retaining the 17-byte single-key buffer and a separately bounded
98-byte tuple buffer. Actual allocation sizes and component work remain charged.
Unique tuples exempt only entirely NULL keys; primary/required indexes reject
any NULL component. Typed integer seeks still reject composite indexes.

Original V2016/V2019 fixtures match 4,513 ordered rows/profile, actual mixed-width/
maximum-width node trees, complete checks and residual queries. The existing
12-file ACE16 runner additionally matches 1,000 original two-Int32 composite rows
per family. Seven new tests include exact tuple properties, 93-byte prefix
goldens, all I/O/cancellation failures and write boundaries. Composite key edits
remain blocked; unrelated qualified Jet 4 fields have an unchanged-index proof.
The CLI now labels index constraints exact unique/required across key types.

All 488 workspace tests/doc tests and 226 ACE all-feature tests pass, along with
both Clippy profiles, release CLI/examples/bench builds, warnings-denied ACE docs
and dependency checks. Existing Int64 range/query and Currency comparisons pass.
[Recorded benchmarks](testing/NATIVE_BENCHMARKS.md#composite-numeric-index-follow-up)
retain all samples: single-index controls stay within observed noise; current full
compound reads are slower than sequential scans (1,000 mixed rows: 0.583/0.212 ms).
No dependency was added. Composite seeks/mutation, other index carriers, native
Access qualification, all ACE writes and milestones 1–9 remain ongoing.

## Exact Currency foreign-key audits

Currency now participates in one-to-ten-column exact relationship tuples,
including mixed integer/GUID components. A distinct private scaled-integer atom
retains the Currency domain; paired field types must match even for equal-width
Currency/Int64. Parent/one-to-one uniqueness, exact partial NULLs and all-NULL
exemption reuse the bounded shared audit without floating conversion.

Fresh V2016/V2019 recipes independently match 48 exact rows, six table audits and
eight corrupt disposable-copy failures per family. One-quantum/partial-NULL
mismatches and duplicate complete/partial parent or child tuples fail without
partial output; original hashes remain unchanged. Five new CI tests include an
independent 256-case tuple-set model, mixed carriers and same-width type rejection.
No dependency was added. Physical composite indexes, broader keys, cascades,
native Access evidence, ACE writes and milestones 1–9 remain ongoing.

All 481 workspace tests/doc tests and 219 ACE all-feature tests pass, as do both
Clippy profiles, release CLI/example builds, warnings-denied ACE docs and
dependency/documentation/fixture checks. The existing 32-row GUID and 59-row
narrow-key comparisons also pass on both families. [Feature evidence](NATIVE_CONSTRAINTS.md#exact-currency-relationship-membership)
records exact scope, original recipes and comparison outcomes.

## Exact complete Currency indexes

Single Currency indexes now share bounded complete traversal, decoded row/key
agreement and explicit unique/required checks. Private key types distinguish
Currency from Int64 despite identical widths; Int32/Int64 range APIs reject
Currency even for All/Null. Exact signed scaled integers retain every value and
the result remains Currency. Shared carrier extraction also removes duplicated
integer checking without changing coercion policies.

Fresh V2016/V2019 recipes match 1,426 independently ordered rows/profile, real
node trees, complete table checks, NULL/sign SQL filters and original file hashes.
Six CI tests include an arbitrary exact-key model, compressed duplicates, type/
constraint/corruption/resource failures and existing write gates. A synthetic
Jet 4 unchanged-byte regression verifies unrelated qualified Int32 updates leave
Currency indexes untouched; Currency targets and all ACE writes remain rejected.
No dependency was added. Currency ranges/mutation, composite/text indexes,
Microsoft Access qualification and milestones 1–9 remain ongoing.

All 476 workspace tests/doc tests, 214 ACE all-feature tests, both Clippy profiles,
release CLI/examples, warnings-denied ACE docs and dependency/documentation/fixture
checks pass. [The feature evidence](NATIVE_INDEXES.md#complete-exact-currency-indexes)
records the independent artifact, exact rows, original recipes and limitations.

## Current-page reuse during native scans

Catalog and sequential table scans now borrow their already-fetched data page
for live roots. The shared physical-row decoder retains signature/owner/slot/
range validation, relocation reads and cycle/hop checks. Nothing is cached across
pages or operations, no extra allocation is introduced, and stable/offline sources
remain required. Avoided physical reads no longer consume read/byte allowance;
row/work/copy allocations still use the shared budget.

Read-count regressions confirm one root-page fetch and separate relocation reads;
corrupt target owners/signatures/ranges/cycles fail without partial snapshots.
All 470 workspace tests/doc tests, 208 ACE all-feature tests, both Clippy profiles
and the independent 12-file ACE16 fixture comparisons pass. The
[recorded release benchmark](testing/NATIVE_BENCHMARKS.md#reusing-the-current-scan-page)
measures 1,000-row sequential reads at 0.504 → 0.279 milliseconds over a
memory-backed Cursor; the 1,000-row indexed control remains within noise.
All timing samples are retained. No new dependency, write permission, native
shared-file behavior, Microsoft Access conformance or milestone completion follows.

## Bounded opaque catalog payload inspection

`NativeImage::read_object_payloads` now returns exact bounded assembled
Lv/LvModule/LvExtra bytes with explicit Present/Absent/Uninspected states and a
captured object identity. Empty inline bytes remain distinct from NULL. Strict
native OLE/long-value geometry, existing byte/chunk caps and complete-result
failure behavior apply. A present empty definition is rejected without invented
NULL/empty output. Duplicate payload-column names now fail presence inspection and
the existing write gate. Payload models and reads live in focused feature modules;
the existing projected row decoder supplies the shared native geometry/codec.

Original V2016/V2019 fixtures independently match all bounded catalog payloads
(37/41 objects), with explicit rejection of one 65,537-byte object/profile. The
registered Access example matches all 146 catalog records/24,693 bytes. Twelve
module declarations have NULL local payloads, reinforcing that those fields alone
do not establish VBA absence. Input hashes remain unchanged. Synthetic inline/
external/chained, metadata, empty/NULL, identity, late failure/cancellation and
budget tests pass. No dependency was added and no application content executes.
This is opaque storage inspection, without VBA extraction, native Access
save/reopen, ACE write permission or completion of milestones 1–9.

All 468 workspace tests/doc tests and workspace Clippy pass, as do ACE
all-feature Clippy and 206 tests/doc tests, release CLI/engineering builds,
warnings-denied ACE docs, dependency-boundary and documentation/fixture checks.

## Exact GUID relationship membership

GUID fields now join bounded mixed integer/GUID tuple membership. Private typed
atoms retain canonical GUID bytes, integer values and NULL separately; parent/
child storage types must match per component. The all-zero GUID stays a value,
partial NULLs stay exact tuples and all-NULL keys remain exempt. Private set order
does not define SQL GUID ordering or physical indexes. Carrier extraction and
generic membership validation now live in focused feature modules with actual
allocation/work/cancellation accounting.

Original V2016/V2019 fixtures match 32 exact rows, six table audits and six
corrupted-copy failures/profile with no partial output; input hashes remain
unchanged. Arbitrary GUID byte/tuple models, zero-GUID/NULL distinctions, mixed
pair-type gates and exact orphan checks pass. A read-only probe records a
Jackcess default-cursor case limitation; canonical fixture input avoids importing
that limitation into TableCraft. The CLI now labels successful coverage exact key
membership; existing integer comparisons retain their results. No dependency was
added. GUID/composite indexes, ordering, cascades, native Access and milestones
1–9 remain ongoing.

All 463 workspace tests/doc tests and workspace Clippy pass, as do ACE
all-feature Clippy and 201 tests/doc tests, release CLI/example builds,
warnings-denied ACE docs, dependency-boundary and documentation/fixture checks.
Existing 59-row narrow and 35-row Int64 relationship comparisons also pass on
both families with the new CLI coverage label.

## Exact Byte and Int16 relationship audits

One-to-ten-column relationship keys now include unsigned Byte and signed Int16
alongside Int32/Int64. Private exact integer tuples retain matching paired storage
types, lossless values, all-NULL exemption and exact partial-NULL matching. No SQL
coercion, collation or new generic key model is introduced.

Original V2016/V2019 recipes independently assert insert rejections, match all 59
rows and nine table audits/profile, and exercise six corrupt disposable copies.
Orphans, cross-matched components, partial-NULL mismatches and duplicate parent/
one-to-one child keys fail without partial output; entirely NULL tuples remain
exempt. Original hashes stay unchanged. Synthetic CI covers both widths, mixed
tuples, storage-type mismatches and same-type orphans. No dependency was added.
Composite physical trees, cascades, native Access and milestones 1–9 remain ongoing.

All 458 workspace tests/doc tests and workspace Clippy pass, as do ACE
all-feature Clippy and 196 tests/doc tests, release CLI/example builds,
dependency-boundary and documentation/fixture checks.

## Complete Byte and Int16 native indexes

Complete native integer index reads/checks now include unsigned Byte and signed
Int16 keys with width-qualified decoding, compressed prefixes and exact row/key
validation. Existing unique/required rules use the same exact integer carrier.
Typed Int32/Int64 range APIs still reject narrow widths; no implicit widening or
narrow-key mutation is enabled. Query sources retain complete selected-index
reads for narrow fields and existing exact numeric filtering semantics.

Original V2016/V2019 fixtures independently match 3,067 ordered rows/profile across
both directions, actual node trees, unique/nullable/empty tables and SQL filters.
The cursor oracle normalizes Java's Byte into Access's unsigned value. Complete
checks and unchanged input hashes pass. Synthetic tests cover signed/unsigned
boundaries, compressed duplicates, row agreement, constraint/corruption/resource
failures and a 256-case independent key model; CLI coverage preserves carriers.
No dependency was added. Narrow relationships/ranges/writes, native Access
comparison and the remaining milestones 1–9 remain ongoing.

All 456 workspace tests/doc tests and workspace Clippy pass, as do ACE
all-feature Clippy and 194 tests/doc tests, release CLI, warnings-denied ACE docs,
dependency-boundary and documentation/fixture checks.

## Explicit stored scalar calculations

`read_stored_calculation` now inspects Boolean, Int32, Double, Currency, legacy
DateTime and Short Text caches through an owned exact-value snapshot. The strict
Int32 API remains. Matching declarations and known variable capacities/flags/
wrappers are required even on empty tables. Variable calculated Boolean retains
its declared capacity and uses a wrapped value; ordinary fixed Boolean mask
semantics remain unchanged. NULL/false and NULL/empty Text stay distinct. Formula
execution and ordinary scalar query eligibility remain gated.

Original V2016/V2019 fixtures independently match 30 cached cells each, including
compressed/uncompressed Unicode, signed zero/subnormal Double, full-range scaled
Currency and exact date serial bits. An original read-only oracle retrieves the
date bits from Jackcess's Date carrier to avoid its calendar precision loss.
The observed 65 ns rendering difference is documented explicitly. Empty tables
and corrupted disposable caches preserve output atomicity and original hashes.
Synthetic exact bit/UTF-16 models, strict profile/capacity/encoding failures and
late cancellation/budget checks pass. CLI inspection uses the same bounded output
formatter as queries; its Int32 output stays unchanged. No dependency was added.
Native Access conformance, remaining result types/recalculation and milestones
1–9 remain ongoing.

All 452 workspace tests/doc tests and workspace Clippy pass, as do ACE
all-feature Clippy and 191 tests/doc tests, release CLI, warnings-denied ACE docs,
dependency-boundary and documentation/fixture checks. Original stored Int32
comparisons still match seven rows plus corruption/empty cases per family.
The Unicode model caught an invalid raw compression-marker prefix; the corrected
generator and explicit U+FEFF escape regression now pass without weakening decoding.

## Safe exact Int64 query seeks

Backend-neutral `ScanConstraint::Int64` now carries full-width integer bounds.
The planner admits wholly safe same-column comparisons, NULL tests and AND
intersections over Int32/Int64 fields with exact integer literals/plain parameters.
Every original WHERE remains residual. Mixed floats, arithmetic, unary parameter
expressions and partial unsafe conjunctions retain complete scans and eager errors.
Bounds are neither saturated nor converted to floats; Int32 bounds still require
an exact narrowing conversion.

The Access adapter selects the matching typed seek only for an explicitly required
index on that field. Missing/corrupt indexes still fail; wrong-width host hints
retain the required complete path. Partial snapshots remain ephemeral. Counters
prove an equality above 2^53 fetches one matching record, NULL fetches its two
records and a later full scan/cache remains complete. A 256-case independent
Int64 filter model, parameter reuse and eager failure tests pass. CLI tests retain
exact extrema/parameter output and buffered failures. Original V2016/V2019 files
match 20 additional query intervals/profile against independent ordered cursors;
hashes remain unchanged. No dependency was added. Native Access comparison,
broader actions/collations and milestones 1–9 remain ongoing.

All 447 workspace tests/doc tests and workspace Clippy pass, as do standalone
no-default-feature query/Access checks, release CLI, warnings-denied query/Access
docs, dependency-boundary and documentation/fixture checks.

## Exact native Int64 index seeks

`Int64IndexSelection` and `read_table_int64_index_range` now select exact Int64
intervals/NULL keys from qualified ACE 16-family indexes. A shared integer bounds
implementation retains Int32 behavior, direction-independent numeric endpoints,
duplicate separator handling and strict width gates. Full reads retain complete
count checks; partial reads qualify only visited pages/returned rows. Int64
extrema and adjacent values beyond double precision never pass through floats.

Two-level synthetic trees and a 256-case independent numeric model cover both
directions, both profiles, duplicate boundaries, NULLs, empty/reversed/extreme
ranges, corruption, limits and cancellation. Traced I/O proves a disjoint corrupt
leaf is skipped, while full/visited reads fail. Original V2016/V2019 fixtures
independently match 24 direct selections per format plus the prior 1,212 ordered
rows and complete index checks; original hashes stay unchanged. The engineering
example uses clap Parser and explicit width/table selectors. No dependency was
added. This initial seek slice left query pushdown Int32-only; the extension above
now consumes the typed Int64 API. Native Access qualification and the
remaining milestones 1–9 remain ongoing.

All 441 workspace tests/doc tests and workspace Clippy pass, as do ACE
all-feature Clippy and 187 tests/doc tests, release CLI/range-example builds,
warnings-denied ACE docs, dependency-boundary and documentation/fixture checks.

## Qualified existing-tree unique index maintenance

Offline Jet 4 Int32 edits now retain existing multi-page unique index trees.
Sorted key/row references are repartitioned using existing leaf counts and node
separators are rebuilt from exact subtree maxima. The writer requires balanced
nonempty child subtrees with matching recorded levels and zero write stamps;
source checks establish ownership and row coverage. Only entry-data spans change:
page counts, allocation, links, levels, prefixes, bitmaps, free space, counters and
opaque padding stay identical. Prefix/width changes, allocation, nonunique indexes
and all ACE writes remain blocked.

A fresh original 1,000-row Jet 4 fixture has ascending and descending three-leaf
trees. Independent Jackcess comparisons match three cross-leaf moves and a full
1,000-key swap, exact row references, memo/binary digests and unchanged bytes
outside original field/entry spans. Native and CLI copies agree; cross-leaf
duplicates publish no output. A 256-case independent model exercises NULL,
arbitrary Int32 values, both directions and two tree heights, including a
zero-separator tail subtree. Journal faults before the root and between rewritten
index pages restore the exact complete original image. No dependency was added.
Actual Access writes, shared-file coordination and milestones 1–9 remain ongoing.

All 438 workspace tests/doc tests and workspace Clippy pass, as do ACE
all-feature Clippy and 184 tests/doc tests, release CLI/example builds,
warnings-denied ACE docs, dependency-boundary and documentation/fixture checks.
Late cancellation and a one-byte-short allocation budget publish no plan.

## Qualified unique leaf-index maintenance

Offline Jet 4 fixed Int32 plans now update affected unique/primary indexes when
their complete tree fits one root leaf and existing prefixes/entry widths remain
usable. The writer binds changes to original root references/keys, reorders
entries, checks uniqueness and emits one patch per physical index, including
logical aliases. Nullable ordinary unique indexes retain existing NULL keys.
Nonunique/node/recompression/unknown-header profiles remain unsupported. All ACE
writes, AutoNumber changes, allocation and row resizing remain blocked.

Exact field/index-entry spans now feed a shared independent unchanged-byte proof;
index headers, bitmaps, allocation, counters and opaque padding remain identical.
Candidates recheck complete index/row consistency plus field/rule/relationship
audits. Duplicate keys and later field-rule failures publish no plan. CLI UPDATE
copies consume the same native implementation.

The original Jet 4 recipe independently reopens Id 2-to-27 edits with index order
1/3/27 and unchanged Unicode memo/binary records. Native/CLI images match; exact
byte comparisons admit only the selected field and original entry-data span.
Synthetic tests cover directions, aliases, NULL, no-op, node/prefix/header/profile
rejection, rule failures, CLI atomicity and 256 generated row/key models. Optional
journal integration commits/reopens indexed images and rolls back a deliberately
inconsistent data-before-index partial write; no-op multi-page replay also passes.
[Native write scope](NATIVE_WRITES.md#existing-tree-int32-index-maintenance)
records current qualification. No dependency was added. Native Access and broader
B-tree mutation remain unqualified; milestones 1–9 remain ongoing.

All 436 workspace tests/doc tests and workspace Clippy pass, as do ACE
all-feature Clippy and 182 tests/doc tests, warnings-denied ACE documentation,
release CLI/example builds, dependency-boundary and documentation/fixture checks.

## Bounded original attachment decoding

`decode_attachment` now returns exact original file bytes/type labels from
qualified direct and zlib FileData wrappers, without opening files or choosing
paths. Exact body/header/type lengths, UTF-16 terminators, checksums, stream end
and input consumption are validated. One extra output byte detects expansion
beyond advertised sizes. Unknown tags/dictionaries and malformed/trailing/oversized
inputs fail without partial data. Normal scalar/complex views still preserve raw
encoded Binary; decoding is explicit. The current 64 KiB input/file, 68 KiB body
and bounded native text/header limits leave large streaming attachments separate.

Flate2 1.1.10 uses only the pure Rust backend; five reviewed registry packages
were added through workspace declarations. Project output buffers reserve
fallibly; codec blocks check cancellation/resources. A conservative logical codec
workspace charge does not intercept upstream allocations. [Dependency records](DEPENDENCIES.md#native-attachment-compression)
own versions/features/licenses and allocation limits.

Original V2016/V2019 fixtures each independently match six files/type labels
(65,810 bytes), including both wrapper modes, empty/Unicode/binary files, inert
PNG/ZIP signatures and a 64 KiB file. Input hashes remain unchanged. An independent
Python-zlib golden, 256 generated files and corruption, size, cancellation and
budget tests cover the codec. All 430 workspace tests/doc tests and workspace
Clippy pass, as do ACE all-feature Clippy and 176 tests/doc tests, release CLI,
warnings-denied ACE docs, dependency-boundary checks and local docs/fixture checks.
Full validation passed after clearing generated artifacts exhausted during an
earlier link attempt. No native Access comparison, attachment mutation, host file
publication or general streaming qualification follows; milestones 1–9 continue.

Twenty-run/three-warmup Hyperfine measurements over 20,000 synthetic 60,000-byte
payloads/process report about 1 microsecond for direct wrappers, 22 microseconds
for compressible zlib and 167 microseconds for noisy zlib. A large noisy-case
outlier is retained. [Absolute benchmark evidence](testing/NATIVE_BENCHMARKS.md#bounded-attachment-decoding)
includes raw samples and source/binary hashes; this is not a native Access or
whole-query performance comparison.

## Scoped native complex parent and child storage

`read_complex_column_links` now resolves a selected ACE 16-family complex column,
projects its qualified stored parent keys and returns checked raw child memberships.
Parent keys and child identities remain unique in their own scopes; orphan,
ambiguous and NULL child references return typed failures without a partial view.
Positive assigned keys, exact flags/widths and qualified child schemas are required.
NULL parent storage and empty membership retain distinct snapshot shapes, and
child iterators retain flat snapshot order. Encoded attachment Binary stays raw.

CLI `inspect --table NAME --complex-links COLUMN` reports buffered parent ordinals,
keys and raw counts. Ordinary scalar/collection query eligibility is unchanged.
Shared fallible projected-carrier construction and native row traversal retain
budgets/cancellation; feature modules separate encoding, reads, metadata and errors.
Independent original fixtures match nine parent links per family across multivalue,
attachment and history columns. Twelve corrupted disposable copies/profile fail
with no partial stdout; hashes remain unchanged. Synthetic tests cover exact child
rows/order, empty/NULL shape, ambiguity/orphans/identity/NULL failures, profile and
resource gates, and CLI atomicity. [Advanced field scope](features/ACCESS_ADVANCED_FIELDS.md#scoped-complex-parent-and-child-links)
owns the implemented profile. Native Access, query expansion, attachment extraction,
history semantics and ACE writes remain unqualified; milestones 1–9 are ongoing.

All 425 workspace tests/doc tests and workspace Clippy pass, as do ACE all-feature
Clippy and 171 tests/doc tests, release CLI, warnings-denied ACE documentation,
dependency-boundary and local documentation/fixture checks. Explicit stored Int32
calculation comparisons also pass after sharing projected-carrier construction.
No dependency was added.

## Explicit stored Int32 calculation inspection

`read_stored_int32_calculation` and CLI `inspect --table NAME --stored-calculation
COLUMN` now inspect strictly qualified 2016/2019-family calculated Int32 caches.
Matching typed declarations, variable storage, exact flags/capacity and known
wrapper prefixes/lengths/trailers are required even on empty tables. NULL and
Int32 extremes are retained. Unknown/malformed states fail without a partial
result. Output labels stored values and zero-based snapshot-local ordinals;
ordinary scalar queries still reject calculated fields. No expression executes
and no freshness/recalculation guarantee is implied.

The feature reuses bounded native record traversal/count/ownership checks through
a factored private helper; ordinary scalar eligibility remains unchanged.
Independent fixtures deliberately disable expression evaluation and match seven
known caches plus an empty table per family, including a stale result that differs
from the expression. Three corrupted disposable copies/profile fail atomically.
Synthetic CI tests cover profile/type/declaration gates, NULL/extremes, late
failures, ordinary-query rejection and resource/cancellation limits. Original
hashes remain unchanged. [Advanced field scope](features/ACCESS_ADVANCED_FIELDS.md#explicit-stored-int32-calculations)
records the implementation and limitations. All 419 workspace tests/doc tests and
workspace Clippy pass, as do ACE all-feature Clippy and 166 tests/doc tests,
release CLI, warnings-denied ACE documentation, dependency-boundary checks and
local documentation/fixture checks. The complete twelve-file ACE 16 regression
comparison also passes with current binaries. No dependency was added.
Native Access, other result/error
variants, recalculation and ACE writes remain unqualified; the broader pre-UI
milestones remain ongoing.

## Exact Int64 and mixed relationship audits

The bounded one-to-ten-column relationship audit now accepts Int32/Int64 tuples
when each parent/child storage-type pair matches. Keys retain exact full-range
integers without floating conversion or implicit cross-type coercion. Partial
NULLs require identical parent tuples; wholly NULL keys remain exempt. Existing
unique-parent, one-to-one, ordered-index eligibility, cancellation and resource
checks remain in force. CLI checked coverage now says integer key membership.

Original V2016/V2019 fixtures independently match 35 records and six audit
reports each. Generation checks invalid inserts; four corrupted disposable
copies per family fail with typed missing-parent/duplicate errors and no partial
stdout. Existing composite fixtures still match all fifteen records/audits and
corrupt-copy cases. Input hashes remain unchanged. Synthetic mixed self relations,
mismatched types, exact uniqueness, large adjacent values and fixed-seed tuple-set
checks cover the implementation. All 413 workspace tests/doc tests and workspace
Clippy pass, as do ACE all-feature Clippy and 161 tests/doc tests, release CLI,
warnings-denied ACE docs and local documentation/fixture checks. No dependency was
added. Native Access, cascades and physical composite indexes remain unqualified;
the remaining pre-UI milestones are still ongoing.

## Exact ACE 16 Int64 index reads and checks

Complete named index reads now support qualified single-Int64 keys on the
2016/2019 profiles. Width-specific decoding preserves the full signed range,
NULL/duplicate ordering and compressed prefixes in either direction. Existing
ownership, separator, sibling, live-root, count and exact row-key checks still
apply. Explicit table checks also validate unique/required Int64 declarations.
The Int32 range API remains Int32-only; the Access source retains the complete
selected Int64 index for other filters, including NULL. Missing/corrupt indexes
fail without a sequential fallback or partial CLI output.

Original multi-page fixtures independently match 1,212 ordered rows per family,
with real Int64 node roots, complete checks and precision/NULL filters. Earlier
modern-scalar fixtures also pass with BigInt indexes now checked. Input hashes
remain unchanged. Synthetic tests cover exact extremes, adjacent large keys,
duplicate/NULL constraints, shared descending prefixes, strict typed selection,
corruption and budgets/cancellation. [Index scope](NATIVE_INDEXES.md#ace-16-int64-index-qualification)
records the implemented profile. All 410 workspace tests/doc tests and workspace
Clippy pass, as do ACE all-feature Clippy and 158 tests/doc tests, release CLI,
warnings-denied ACE documentation, dependency-boundary checks and local
documentation/fixture checks. No dependency was added; native Access remains
unrun and index mutation remains separate work.

## Owned-object preservation gate

`NativeImage::inspect_object_children` now reports direct catalog containment for
an exact captured object identity, with bounded allocation/work and cancellation.
Unknown or foreign roots fail rather than appearing dependency-free. No object
body or application content executes. This is containment metadata, not a complete
dependency graph or proof of absent table data macros.

Offline edit planning now rejects separately owned catalog objects even when the
table's own Lv/LvModule/LvExtra fields are NULL. Synthetic tests cover known and
opaque child kinds, exact scope, unchanged input, limits and cancellation. All
405 workspace tests/doc tests and workspace Clippy pass, as do ACE all-feature
Clippy and 154 tests/doc tests, release CLI, warnings-denied ACE docs and local
documentation/fixture checks. No dependency was added. ACE writes remain blocked
and the remaining pre-UI milestones remain ongoing.

## Shared range/membership kernels and native rule audits

SQL-independent predicate kernels now serve bound query execution and source-free
scalar evaluation. BETWEEN supports either written bound order; IN retains exact
numeric and NULL/NOT truth without materializing candidate lists. Hosts retain
comparison/collation, work and cancellation policy. Eager evaluation still exposes
late errors after earlier true matches. Empty syntactic lists remain invalid.
The CLI can evaluate these predicates without a database.

Native pure-rule preflight now admits numeric range/membership forms, with shared
limits and field/table scope. Text, calls, subqueries and unknown NULL validation
results remain unqualified, including on empty tables. Existing fixed Int32 edit
plans recheck range/list predicates before/after candidates; rejected values
publish no plan. ACE writes are still blocked. [Expressions](EXPRESSIONS.md) and
[native constraints](NATIVE_CONSTRAINTS.md) own the profiles.

Two original newer-family fixtures independently match six rule reports and
thirteen exact rows each; generation asserts forward/reversed-range and list
write acceptance/rejection. Input hashes remain unchanged. Source-free CLI,
fixed-seed full-i64 intervals, eager failures, precision/NULL, budget/cancellation
and native pre/post-edit tests cover the shared implementation.
All 402 workspace tests/doc tests passed; seven mutation tests including the new
rule-gating case passed separately afterward. Expression minimal/all-feature
Clippy and 19/33 tests/doc tests pass, as do ACE all-feature Clippy and 152 tests/
doc tests, workspace Clippy, release build, warnings-denied workspace docs and
formatting/local documentation/fixture checks. No dependency was added. Native
Access remains unrun; the broader pre-UI milestones remain ongoing.

## Composite Int32 referential-integrity audits

The constraint milestone now checks ordered one-to-ten-column Int32 tuples
against qualified unique/primary parent declarations. Entirely NULL keys remain
exempt; partially NULL keys require an exact parent tuple and participate in
uniqueness. One-to-one child uniqueness follows the same rule. Fixed-width key
buffers, fallible allocation, bounded comparison work and cancellation retain
operation limits. Physical composite index validation/cascades and all existing
native write guards remain separate. [Constraint scope](NATIVE_CONSTRAINTS.md)
records implementation versus native Access qualification.

New original V2016/V2019 fixtures each independently match fifteen exact records
and assert matched/unmatched partial-NULL, all-NULL and one-to-one cases. Four
corrupt disposable copies/profile produce expected typed missing-parent/duplicate
errors without partial stdout. All original hashes remain unchanged. Native
synthetic tests cover eligibility/order, self relations and duplicate tuples;
256-case generated tuple-set checks supplement existing scalar-key tests.

All 393 workspace tests/doc tests and workspace Clippy pass, as do ACE all-feature
Clippy and 150 tests/doc tests, release CLI, warnings-denied ACE docs, formatting,
whitespace and local documentation/fixture checks. The full twelve-fixture ACE 16
regression comparison also passes with composite coverage now checked. No new
dependency was added. Native Access remains unrun; broader constraints and the
remaining pre-UI milestones are ongoing, not completed by these audits.

## ACE 16 raw complex-child storage

Qualified fixed Int32 child-FK fields with exactly extended flag 0x08 now decode
on the 2016/2019 profiles. Resolved multivalue, attachment-storage and history flat
tables feed the existing scalar snapshot/query adapters. Parent 0x12 collections
remain rejected, including empty tables. Wrong width/type/extra bits and older
unqualified profiles stay unsupported. Common native feature flag constants now
have one owner in layout. [Advanced field scope](features/ACCESS_ADVANCED_FIELDS.md#raw-complex-child-storage-reads)
separates raw storage from collection semantics and attachment extraction.

Original populated fixtures match nine exact raw child rows each with the
independent reader, including Unicode, empty values, opaque encoded attachments
and fixed history timestamps. CLI queries return the expected counts, parent
scalar queries fail without partial output, and input hashes remain unchanged.
New synthetic checks cover exact carriers and strict profile/type/width/flag gates.
All 388 workspace tests/doc tests pass, plus workspace Clippy, release CLI build,
warnings-denied ACE documentation, formatting, whitespace and documentation/fixture
checks. No dependency was added. Native Access remains unrun and milestones 1–9
remain in progress; full parent collection/extraction/calculation is not completed.

## ACE 16 complex-schema reference resolution

Continuing milestone 1, `inspect_complex_schema` now joins native complex IDs,
MSysComplexColumns declarations, catalog references and value/flat table schemas.
It validates owner/name/identity, duplicate/dangling/self/alias references and the
qualified fixed-Int32 child FK/identity shape. Native schemas stay separate from
scalar collections and mutation authorization. CLI `inspect --table NAME --complex`
reports resolved names/keys/count declarations with no partial stdout on failure.
[Advanced fields](features/ACCESS_ADVANCED_FIELDS.md#resolved-complex-field-schemas)
own current scope; parent keys/child values, decompression and calculation remain next.

Both original advanced-family inputs match three independently resolved complex
rows each, including raw type IDs, type/flat names, FK/identity names and declared
counts. Hashes remain unchanged. Four new synthetic tests cover resolution,
corrupt/missing/duplicate mappings, geometry, ordinary tables and cancellation/
limits; a CLI integration test covers successful output and corrupt-map atomicity.
All 144 ACE all-feature tests/doc tests and three feature-mode CLI tests pass,
as do workspace Clippy, ACE all-feature Clippy, warnings-denied ACE docs,
formatting/whitespace and local docs/fixture checks. No dependency was added.
Native Access remains unrun; milestones 1–9 are ongoing, not complete.

## ACE 16 inert advanced-field declarations

The first implementation slice of the authorized pre-UI milestones adds
`inspect_table_features`, borrowed column-feature views and CLI
`inspect PATH --table NAME --features`. It exposes calculation flags/expression/
result tags, raw ACE complex type IDs, child-FK/AutoNumber/Hyperlink markers and
explicit AppendOnly/TextFormat declarations. Unknown/absent properties remain
retained/unknown; wrong typed/width values fail rather than becoming defaults.
Schema and original properties remain available without executing active content.
[Advanced fields](features/ACCESS_ADVANCED_FIELDS.md#working-inert-metadata-slice)
own the precise read-only scope. Structured children/recalculation and ACE writes
remain unimplemented; this does not complete milestone 1 or the nine-milestone path.

Original V2016/V2019 fixtures independently match ten field declarations each,
including generated attachment/multivalue and append-only/history metadata. Raw
complex IDs have independent synthetic golden checks; the oracle checks presence,
not numeric ID values. Scalar queries on their empty complex/calculated tables
fail with no stdout. Input hashes remain unchanged; native Access remains unrun.

All 380 workspace tests/doc tests and workspace Clippy passed. ACE all-feature
Clippy and 140 tests/doc tests passed, including two additional strict byte-property
tests added after the workspace run. Release CLI build, warnings-denied ACE API
documentation, formatting, doc links and fixture hash pass. No dependency was added.
Next in milestone 1: resolve complex child-table references and qualify their
structured data paths, with unsupported behaviors still blocking writes.

## Structured editing and agent requirements

The [shared editing decision](decisions/0002_SHARED_APPLICATION_EDITING.md) records
common discover/design/preview/validate/apply services for human and agent hosts.
Focused contracts specify typed changesets, stable project identities and revision
preconditions, semantic diffs, digest-bound applicable approval, idempotency,
revalidation and recovery. They explicitly preserve native write/format gates,
backend-specific transaction scope and the no-locks-during-review rule.

New [preview](features/HEADLESS_PREVIEWS.md),
[application scenario](testing/APPLICATION_SCENARIOS.md) and
[MCP adapter](interfaces/MCP.md) requirements cover semantic plus visual artifacts,
reproducible workflows, capabilities, bounded/redacted data, untrusted project
content, job ownership, executable trust and protocol negotiation. Verified the
MCP 2026-07-28 baseline and current SDK documentation against primary sources;
Tasks cancellation remains cooperative and durable state is a host responsibility.
The candidate review records rmcp, Ruff, type checking and pytest dispositions.

`tablecraft-edit` is a prospective/module-first boundary. The target map now has
38 prospective libraries across 15 groups, still twelve actual packages. The
`tablecraft-builders` name is retained. No editing/MCP/preview/scenario runtime,
new manifest/dependency/lockfile change or broader native write support was added.
ACE 16 advanced-field/write-safety implementation remains next; this requirements
slice does not resume SQLite expansion or native VBA execution.

Verification covers local documentation links/anchors, target-map uniqueness,
original fixture hash, formatting/whitespace and the existing package-boundary
policy guard with its ten regression tests. Those tests exercise dependency
policy, not agent capabilities or changeset behavior. Rust runtime tests were not
rerun for this documentation-only change; examples/tool names and acceptance
scenarios are not working commands or observed application test results.

## Native project decisions and dependency enforcement

Reconciled the seven supplied proposals into an
[accepted decision](decisions/0001_PROJECT_FORMAT_AND_EXECUTION.md) and focused
format/versioning, lifecycle, execution, transaction, Python, conversion and peer
schema-evolution contracts. Python supersedes the earlier JS/TypeScript default;
SQLite-backed `.tcraft` and equivalent `.tcraftproj/` representations are the
native project direction. MDB/ACCDB remain first-class, with ACE 16 still the
immediate implementation priority and existing VBA/Windows deferrals unchanged.

The target ownership map adds project/Python/codegen responsibilities only in
documentation (37 prospective libraries / 15 groups; still twelve actual Cargo
packages). No new manifest, registry dependency, lockfile change, project format,
Python host, migration implementation or generic transaction API was introduced.
The review records all supplied runtime/tool alternatives and optional ecosystem
packages separately from adopted dependencies. Internal container migrations,
developer data migrations and application releases have distinct contracts.

The working [dependency guard](architecture/DEPENDENCY_BOUNDARIES.md) checks all
declared local-package edges, including optional, renamed, platform, build and
development dependencies. New/unreviewed local packages fail. CI runs the guard
and its ten regression tests using an engineering Python installation; this is
not the application scripting host. All twelve packages pass, as does metadata
inspection with a fresh Cargo cache. Guard tests cover engine/adapter/DB separation,
query independence, host isolation, path/alias bypasses and library placement/kinds.
External third-party transitive/feature closures remain a separate review concern.

Formatting, whitespace, public local links/anchors, target-map uniqueness and the
original registered fixture hash pass checks. Rust production sources and Cargo
manifests are unchanged, so the Rust runtime suite was not rerun for this slice;
[project qualification](testing/PROJECT_COMPATIBILITY.md) is an acceptance plan,
not passing behavioral evidence. CI's Linux/Windows runs remain unrun locally.
Next implementation remains ACE 16 advanced fields/write safety; the native
project versioning/migration proof follows the current native priority.

## ACE 16 exact modern scalars

Native Large Number (0x13) now yields exact Int64 on codes 5/6; Date/Time Extended
(0x14) yields validated civil dates with 100ns precision on code 6. Width, fixed
storage and profile gates apply even to empty tables. Jiff performs Gregorian
calendar conversion for the validated 42-byte extended encoding. Calculated
columns remain explicitly unsupported; complete Int64 indexes are covered by the
later qualification above. All ACE writes stay blocked.
[NATIVE_ACE16.md](NATIVE_ACE16.md#modern-scalar-reads) owns details.

Same-kind extended dates support exact expression/query comparison and sorting.
The CLI preserves all seven fractional digits; mixed legacy/extended comparison
and implicit conversion remain unsupported. New tests cover full signed integer
extremes, values beyond double precision, exact typed parameter filters, NULL,
years 1–9999, malformed rows with no partial output, profile/width rejection,
and fixed-seed generated Gregorian dates and arbitrary native byte payloads.

All 374 workspace tests/doc tests pass, as do workspace Clippy, ACE journal-enabled
tests/Clippy, standalone expression minimal/all-feature tests/Clippy, the release
workspace build, formatting and warnings-denied all-feature API documentation.
Both original modern scalar fixtures match the independent reader exactly
(7/13 rows), query ordering/filtering and unsupported-feature checks pass, and
the earlier twelve-fixture ACE 16 comparison still passes. Inputs remain unchanged.
The dependency record now includes ACE's working use of the existing Jiff
workspace dependency; no registry package was added. Documentation links and the
original registered fixture hash pass verification. A full-disk build failure was
resolved by clearing generated incremental build cache and rerunning checks.
Native Microsoft Access qualification remains unrun. Next: ACE 16 advanced-field
metadata/safety gates, then broader index and native write coverage.

## ACE 16-family bounded readers

ReadProfile now selects unencoded subversion-zero 2016/2019-family layouts
(codes 5/6). Existing catalog/schema/scalar/property/relationship/audit and Int32
index/range algorithms work through the Access adapter and CLI. All ACE writes,
encoded/unknown-subversion inputs and unsupported field/index semantics remain
blocked. [NATIVE_ACE16.md](NATIVE_ACE16.md) records scope and reference review.

Twelve original generated-family fixtures match Jackcess across 480 catalog
records, mixed scalars/long values, properties, complete relationship inventories,
constraints, ranges and 200-column chained definitions. All input hashes remain
unchanged. The oracle now includes built-in navigation relationships in V2019;
chained-definition comparison selects native metadata fields from the current CLI
header rather than confusing newly added derived constraint columns with them.
All 364 workspace tests/doc tests pass, including three engine regressions
and one CLI regression using clearly identified
retagged disposable copies for code-path coverage only. Workspace Clippy passes.
No registry dependency was added and no Microsoft Access oracle ran. Modern
scalar qualification is recorded in the subsequent slice above.

## ACE 16 priority decision

ACE 16 is now the primary implementation and compatibility-testing focus, with
100% compatibility as the long-term target. Other Jet/ACE generations are
secondary, and further SQLite expansion remains paused. Existing working profiles
and regression checks are retained. [ROADMAP.md](ROADMAP.md) records the scope,
evidence requirements and unchanged explicit VBA/Windows deferrals.

The priority decision itself did not expand format support. Subsequent ACE 16
reader qualification is recorded above; native edits remain restricted to the
documented Jet 4 profile.

Documentation checks passed 490 local links/anchors across 49 public Markdown
files, whitespace checks and the unchanged original fixture hash. Runtime tests
were not rerun for this documentation-only decision.

## Bounded query UPDATE and fresh native copies

`plan_update` now binds one local table/one Int32 assignment through DB snapshot
contracts, evaluating supported pure expressions and WHERE before one backend
call. It reuses retrieval scalar/Between/IN/NULL behavior and explicit collation,
with checked integral assignment narrowing. Calls, parameters, joins, subqueries
and broader actions remain unsupported. A late error produces no backend plan.

CLI `query update --database INPUT --output NEW_COPY` composes this with the Access
adapter and engine gates. It opens the offline input read-only under an OS lock,
creates a fresh output after validation, syncs it and reports matched/changed rows.
Existing paths are never overwritten. Copy I/O failure may leave incomplete output;
this is separate from the optional library in-place journal boundary.

[QUERY_EDITING.md](QUERY_EDITING.md) records the profile, primary reference and
verification. All 360 minimal workspace tests/doc tests and 166 optional-feature
ACE/I/O/SQLite tests pass locally, along with workspace all-target Clippy, release
build and warnings-denied all-feature API docs. Additional targeted assertions
check cancellation after staged rows, row-budget failure, CLI lock contention and
aliased table rejection. The independent Jet 4 recipe confirms CLI and engine
copies are identical: Score 20 becomes 27, exactly one byte changes, and Jackcess
plus native primary-index reopen pass. Original fixtures remain unchanged.

Only existing local dependencies were added. SQLite production code/capabilities
are unchanged; optional tests cover the shared query runtime change. Further
SQLite expansion stops at the requested boundary. The roadmap explicitly retains
ACE writes/data macros, allocation/index mutation, INSERT/DELETE, full application
preservation/dependency discovery, broader query updateability and native Access
qualification as unfinished work. The entire engine is not declared complete.

## Working database snapshot and edit contracts

The twelfth package, `tablecraft-db`, owns exact validated owned snapshots,
process-local row handles and consuming batch edit contracts. `NativeEditor`
implements them over an immutable native image, preserving standalone engine
independence. Cross-snapshot handles and wrong carriers fail before planning;
ACE reports updates unavailable and Jet 4 retains all native gates.

[DB.md](DB.md) records responsibilities and limits. Two contract tests and two
adapter integration tests pass, including a generic native planning consumer;
standalone checks and all-target Clippy pass. No registry dependency or SQLite
behavior changes. CI now includes standalone DB and optional native journal
checks; configuring CI is not evidence those remote jobs ran. Next is bounded
query editing over these working contracts.

## Conservative native payload gates

Native image inspection now reports Lv/LvModule/LvExtra null-mask presence without
following object streams. Missing columns remain Uninspected, distinct from NULL.
The Jet 4 writer additionally requires explicit absence on its selected table.
Two additional focused tests cover opaque-content rejection, missing metadata,
foreign identities and cancellation; existing mutation/preservation/journal tests
and all-target all-feature ACE Clippy pass. The independent Jet 4 update recipe
still succeeds. [Coverage](NATIVE_PRESERVATION.md#inert-payload-presence) is physical
evidence, not complete VBA/data-macro discovery or ACE write qualification.

## Journaled offline native edits

The optional ACE journal feature connects range-proven edit plans to the existing
generic image protocol. `prepare_expected` checks the complete validated preimage
before journal writes. Native prepare/commit/rollback and explicit recovery retain
journals and require exclusive offline resources with host-owned durable namespace
setup/cleanup. Recovery captures the resulting native image.

Three native integration tests and the additional generic preimage test pass,
including a hybrid payload after an injected write failure, repeatable rollback,
stale targets, retained-journal rejection and locked file close/reopen recovery.
All-target all-feature ACE/I/O Clippy passes. The feature adds no registry package
and stays off for normal readers. [Scope](NATIVE_WRITES.md#optional-journaled-persistence)
excludes native Access coordination, actual power-loss and network-filesystem
qualification. Next is working backend-neutral database contracts.

## Fixed Int32 native edit plans

The bounded Jet 4 engineering profile in [NATIVE_WRITES.md](NATIVE_WRITES.md)
plans existing non-indexed fixed Int32 replacements with before/after constraints
and unchanged-byte preservation proofs. AutoNumber, unsupported metadata/rules,
relocated rows, indexed fields and ACE writes are blocked. Source images remain
immutable; the fresh-copy laboratory harness never modifies its input.

All 344 workspace tests/doc tests and all-target Clippy pass, including five
focused mutation tests and a 256-case independent row model. Jackcess
reopens an original generated Jet 4 copy with Score 20 changed to 27; exactly one
byte differs and unchanged primary-index traversal remains valid. Rule/stale/index
rejections publish no output. Unicode memo/binary payloads remain byte-identical.
No new registry dependency or tracked binary fixture was added. Native Access
remains unrun. Next is journaled native persistence and recovery.

## Complete native image capture

`NativeImage` now retains complete immutable Jet 4/ACE 2007 bytes and exposes
borrowed readers plus conservative catalog dependency coverage. Opaque VBA,
unknown objects, unused pages and padding survive capture exactly. The original
example retains all bytes and 146 objects; its module/application declarations
report unavailable execution without decoding bodies. Three focused tests pass,
including unknown bytes, budgets and midstream cancellation.

[Preservation scope](NATIVE_PRESERVATION.md) distinguishes capture evidence from
unimplemented edit/save/reopen guarantees and complete dependency discovery. No
new dependency or fixture was added. Next is range-proven native physical edits.

## Bounded field and table validation rules

Native constraint audits now evaluate the pure profile in
[NATIVE_CONSTRAINTS.md](NATIVE_CONSTRAINTS.md), using owned standalone expression
parsing and token-based implied field references. Field rules bind only their own
field; table rules bind their own table. False results fail with typed violations;
NULL results remain Unsupported. Calls, text comparisons and unimplemented forms
are rejected before row evaluation, including empty tables. No defaults or active
content execute and normal reads are unchanged.

All 336 workspace tests/doc tests and all-target Clippy passed locally. Original
Jet 4/ACE fixtures match six row-count/coverage reports per format against the
independent reader; input bytes remain unchanged. New focused tests cover parser
limits/trailing SQL, rule rejection/failure, budgets and cancellation. Existing
SQL/expression packages add no registry dependencies. Native Access remains unrun.
The bounded constraint slice is ready for preservation work; broader constraints
remain explicit gaps and cannot authorize writes.

## Explicit field and foreign-key record audits

`check_table_constraints` and `inspect --table NAME --constraints` now check
explicit Required/AllowZeroLength declarations and qualified single-Int32 foreign
keys touching the selected table. Checks verify parent uniqueness, child membership
and declared one-to-one child uniqueness; NULL children and explicitly unenforced
relationships retain separate behavior. Composite/unknown/unbound profiles and
validation expressions report Unsupported. Normal reads and structural/index
checks remain separate. CLI dispatch now uses typed inspection modes.

[Constraint coverage and primary references](NATIVE_CONSTRAINTS.md) document remaining
default/expression/index/macro/write gaps. Shared schema owns allocation-free scalar
field policies and typed violations; native audit errors omit payloads. Original
Jet 4/ACE relationship fixtures match row counts and coverage across all ten child
checks. In-memory mutation tests detect Required, empty Text, orphan-child and
duplicate-parent failures; a 256-case independent set model checks key semantics.
All 331 workspace tests/doc tests and workspace all-target Clippy passed locally.
Registered fixture bytes remain unchanged; native Access remains unrun. Step 2
continues with bounded field/table validation expressions before preservation work.

## Native relationship declarations

Shared schema now owns validated ordered parent/dependent relationship shapes and
qualified rule/action metadata. ACE retains native flags and bounded grouping of
MSysRelationships rows; unknown/inherited/conflicting profiles remain unqualified.
`inspect --relationships` reports inert declarations without opening providers,
checking user records or executing cascades.

[Scope and primary references](NATIVE_RELATIONSHIPS.md) record shape validation,
resource charging and unresolved endpoint policy. Original Jet 4/ACE recipes match
five relationships/six pairs per format against Jackcess, including composite,
one-to-one, unenforced and cascade/set-null declarations. Partially NULL composite
behavior remains explicitly unqualified pending native comparison. All 322 workspace
tests/doc tests and all-target Clippy passed locally, including constructor,
malformed metadata, cancellation/budgets, a fixed-seed ordinal property and CLI
checks. Registered fixture bytes remain unchanged. Step 1's bounded metadata slice
is complete; next is step 2, field/relationship record validation. Newer formats,
native Access comparisons and relationship writes remain unimplemented.

## Native field/table property declarations

`inspect_table_properties` and CLI `inspect --table NAME --properties` now decode
bounded MR2 table/column/index property maps in the qualified Jet 4/ACE profile.
Boolean/text declarations remain inert; unknown values/flags and the full original
LvProp stream remain available. Scoped groups distinguish identically named columns
and indexes. Internal metadata projection skips unrelated module/object bodies.
Ordinary schema/row APIs keep their conservative nullability contract.

[Property scope and references](NATIVE_PROPERTIES.md) record framing, limits,
cancellation and typed access. Original generated Jet 4/ACE fixtures match all 22
property declarations against Jackcess, including quoted defaults and Unicode.
All 312 workspace tests/doc tests and workspace all-target Clippy passed locally;
new tests cover malformed streams, budgets/cancellation, a fixed-seed property,
real fixture declarations, CLI options/output and unchanged input bytes. Native
Access remains unrun. Rule enforcement/default evaluation and native property
mutation are not implemented. Next in step 1: relationship declarations.

## Optional tokio-rusqlite reader

The `tablecraft-sqlite/async` feature supplies worker-owned Access query retrieval
through tokio-rusqlite 0.8.0 and Tokio 1.53.3 sync admission. Each request owns its
snapshot, TableCraft binding/evaluation and rollback. One job is admitted per reader;
additional requests fail immediately. Dropping its future sets cancellation while
the actual worker retains admission until cleanup. Cleanup failure prevents reuse.
No raw SQLite handles, arbitrary SQL execution or cross-request transactions escape.

[Async scope and cancellation](SQLITE.md#optional-async-worker) document callback/I/O
limits and host ownership. [All eighteen added registry packages](DEPENDENCIES.md#optional-async-sqlite-worker)
are recorded, including upstream cache and Wasm-only defaults. Native CLI dependency
inspection excludes Tokio/tokio-rusqlite and their cache/worker closure. CI includes
standalone all-feature SQLite tests and Clippy using existing version-tagged actions.

All 17 SQLite tests plus the compiled async API example and all-feature Clippy passed
locally. Four worker tests exercise successful requests, cancellation/exhaustion,
cleanup and a deliberately blocked host callback to verify abandoned requests cannot
release admission early. Workspace release build and warning-denied all-feature API
docs passed. Windows/Linux/browser execution and native Access remain unrun.

## Read-only SQLite source and CLI

`tablecraft-sqlite` now supplies bounded ordinary-table schemas and scans to the
existing query evaluator. `query run --sqlite PATH` opens an existing file read-only,
retains a SQLite read transaction and buffers results before output. Access SQL
is evaluated by TableCraft; only generated metadata/table reads reach SQLite.
Carrier mismatches, unknown encodings and unsupported types fail explicitly;
views/virtual tables are unavailable and generated columns are rejected.

[SQLite scope](SQLITE.md) records snapshots, limits, immediate busy handling,
shared cancellation, VM interruption and internal-allocation/I/O limitations.
The bundled rusqlite dependency is [reviewed separately](DEPENDENCIES.md#sqlite-reader).
All 304 workspace tests/doc tests and targeted adapter/CLI all-target Clippy passed
locally on macOS ARM, including 13 SQLite tests (one 256-case property) and two CLI
integration tests. Original native fixtures and disposable CLI SQLite database
bytes remain unchanged. Windows/Linux, browser storage, async hosts and native
Access comparisons remain unrun. Native writes/field properties/relationships and
expanded format qualification remain separate unfinished work.

## Explicit native read profiles

`Header::read_profile()` and catalog decoding now share one qualified read-profile
selector. Header recognition remains broader than decoding: only unencoded Jet 4
and ACE 2007 with subversion zero select working profiles. Unsupported families,
subversions and codecs still fail before catalog reads. Structural ceilings moved
from `profile.rs` to `limits.rs`; shared feature implementations remain unchanged.

All native package tests/doc tests and all-target Clippy passed locally, including
an independent header/profile matrix. No new format support, writes, dependency
or native Access comparison is claimed. See [selection](NATIVE_READING.md#read-profile-selection).

## Optional image commit and recovery foundations

The tablecraft-io/journal feature implements bounded fixed-length image prepare,
commit, rollback and restartable recovery over explicit DurableStorage resources.
The prepared journal is synced before target changes; the target is synced before
its digest-bound commit marker. Any commit error requires explicit recovery, and
existing journals are never silently overwritten/deleted/reused. Framing, SHA-256
and complete paired-target checks precede recovery writes. Interrupted rollback
leaves its journal unchanged so partial markers cannot become mixed by overwrites.
Bounded short-write/Interrupted handling is also available independently.

[Protocol ownership, resource costs and durability preconditions](IMAGE_JOURNAL.md)
are explicit. Hosts own exclusive access, correct resource identity, durable
journal namespace setup/cleanup and timeouts. Only equal nonempty images up to
64 MiB are supported. This is not the Jet/ACE recovery format, an integrated native
writer or native shared-file coordination. Existing native writes remain unavailable.

Optional sha2 0.11.1 supplies SHA-256 integrity; [exact dependency records](DEPENDENCIES.md#optional-image-journal-integrity)
cover all six new lockfile packages and feature/license/MSRV boundaries. Defaults
stay empty, and the observed normal CLI/native-reader closure excludes sha2/digest.
The standard lock wrappers implement DurableStorage without exposing raw handles.
No security/database facade crate, async runtime or unused scaffold is added.

Tests interrupt every commit/recovery callback before and after side effects,
force short writes and partial markers, and model survival/loss of unsynced bytes.
They verify barrier order, repeatable recovery, every truncation/byte corruption,
independent framing/digest goldens, cancellation/limits, mismatched targets and
existing-journal refusal. Two fixed-seed properties cover mixed old/new target
bytes and bounded arbitrary journals; disposable locked files reopen and recover
both outcomes. No registered fixture bytes change.

Local Rust 1.99.0 checks passed all 288 workspace tests/doc tests, 23 optional-I/O
tests in debug and release, workspace/all-feature-I/O Clippy, standalone package
checks, minimal/all-feature expressions, all-feature core, release build and
warning-denied API docs. CI now includes the optional I/O profile using existing
version-tagged actions. Physical power-loss, native Access, other target execution,
network filesystems and remote CI remain unrun. Next: native field properties and
relationship constraints, then preservation-aware native mutation on disposable
offline copies; OS locks and generic journaling alone do not qualify those writes.

## Offline read transactions and OS lock guards

OfflineReadTransaction captures a bounded immutable image under an owned shared
OS file lock and implements Read + Seek for existing engine/query consumers.
SharedFile/ExclusiveFile provide nonblocking OS lock guards, typed contention,
reported explicit close and best-effort drop release; they never create/truncate
files. Explicit drop unlock prevents transiently inherited child descriptors from
extending the guard's lifetime. No native write API or Access lock protocol is
introduced, and CLI defaults remain unchanged.

[Transaction scope](NATIVE_TRANSACTIONS.md) documents offline capture, cumulative
memory/read/work charges, callback timeout ownership and remaining write gates.
Original tests verify cooperating handle/process exclusion, repeatable image reads,
existing table checks through transactions, cancellation/limits and lock release.
A compile-fail test excludes Write. Workspace Clippy and all 285 workspace tests/
doc tests passed locally on macOS ARM. Native Access, Windows/Linux and network
filesystems remain unrun. Generic journal/recovery ordering is the next slice;
native commit, preservation, required macros and mixed-client coordination remain
unimplemented.

## Qualified native index constraints

Native index inspection decodes known primary, unique, required-key and ignore-NULL
declarations without changing field nullability or guessing unknown/foreign
profiles. Primary semantics propagate across logical aliases of physical backing;
multiple logical primary declarations are rejected. Explicit check_table operations
now validate unique/non-NULL rules over complete qualified Int32 traversals and
return sanitized typed constraint failures. CLI inspection exposes declarations
and checked constraint coverage. Normal reads retain their structural contract.

[Constraint scope and primary references](NATIVE_CHECKS.md#qualified-index-constraints)
record the distinction between index declarations and field properties. Independent
Jackcess-generated Jet 4/ACE fixtures match all six index declarations/checks and
all 18 tagged rows, including duplicate NULLs in a unique index. Mutation tests
and a 256-case set-model property cover violations and aliases. Workspace Clippy
and all 278 workspace tests/doc tests passed locally; original fixture bytes are
unchanged. Native Access remains unrun. Composite/Text/foreign-key checks, field
validation and write-time enforcement remain unimplemented. Transaction/locking
and recovery foundations are the next active slice; native writes stay unavailable.

## Optimized Jiff calendar adapters

Legacy DATE calendar conversion retains Jiff's checked civil types while avoiding
redundant date arithmetic and an i128 time-duration conversion. Rounded midnight
carry is folded into one civil-day addition; exact stored/Serde bits, negative
DATE rules, field bounds and checked errors are unchanged. No dependency or
feature change was needed. Stored-date comparison does not construct calendars.

The [20-run follow-up](testing/NATIVE_BENCHMARKS.md#jiff-conversion-follow-up)
records every sample, including scheduling outliers. Five-million round-trip
medians improve from 39.50 to 27.15 ns per operation (about 31%); the time baseline
is 23.20 ns, leaving a roughly 17% median gap on this macOS ARM host. Mean
improvement is about 28%. These are calendar conversion results, not overall
engine/query speed claims. The original migration measurements remain recorded.

Five new tests include independent golden boundary/rounding cases and three
fixed-seed properties against Jiff's general arithmetic. Local Rust 1.99.0 checks
passed 273 workspace tests/doc tests, all-target Clippy, minimal package checks,
minimal/all-feature expression and all-feature core tests/Clippy, release and
benchmark builds, and warning-denied API docs. Native Access, other targets and
remote CI remain unrun. Next: qualify native constraint metadata and semantics
before considering native writes.

## Native schema allocation audit

Native column/table schema names now use the same fallible reserve/copy path
instead of two remaining infallible clones. Caller byte charges remain explicit.
Column-number sorting uses the in-place unstable sort: valid layouts already
reject duplicate numbers, so this preserves ordering without an uncharged stable-
sort scratch allocation. Schema constructors/validation and supported profiles
remain unchanged; no dependency was added.

Existing schema, budget, mutation, row, query and consistency suites cover the
unchanged behavior. This is allocation-failure hardening, without a new performance
claim. Local Rust 1.99.0 verification passed 268 workspace tests/doc tests,
all-target Clippy, minimal package checks, release build and warning-denied API
docs. All 32 independent query comparisons still match; documentation links and
the original fixture SHA-256 pass. Native Access, remote CI and other target
execution remain unrun.

## Explicit native table consistency reports

`check_table` now combines a complete sequential scalar scan with every eligible
Int32 index, using one cumulative budget. It verifies matching schema/identity,
live-root counts and full tree/key/root agreement. Known unsupported profiles are
reported per index without claiming validation; metadata or selected traversal
failures return no report. Index eligibility has one owner shared with normal
indexed reads. No native constraints, orphan discovery, write safety or whole-file
integrity certificate is implied.

CLI `inspect PATH --table NAME --check` buffers explicit checked/unverified
coverage, preserving inertness, limits, redaction and no partial failure output.
Tests prove detection of an extra live root missed by an otherwise count-correct
index, skipped corrupt Text-index coverage, cumulative allowances/cancellation and
every I/O callback failure. Generated Indexed tables report 1,000 rows with 2/3
checked/unverified indexes; Wide/EmptyWide report 7/0 rows with both indexes checked
in both Jet 4 and ACE. [Check scope](NATIVE_CHECKS.md) and
[Hyperfine evidence](testing/NATIVE_BENCHMARKS.md#table-consistency-checks) record
the current limitations and all timing samples.

Local Rust 1.99.0 checks passed 268 workspace tests/doc tests, all-target Clippy,
minimal package checks, release/benchmark builds and warning-denied API docs.
Dependencies and registered fixture bytes are unchanged. Native Access, other
targets and remote CI remain unrun. Next: audit remaining native allocation failure
paths before expanding qualified constraint semantics.

## Linear usage-map traversal

Owned-page bitmap traversal now verifies strict order/uniqueness against the last
emitted page, replacing repeated searches through prior pages. Inline bit order and
disjoint indirect ranges establish the sorted contract required by native index
ownership lookups. Existing reference, allocation/work, file/page and cancellation
limits remain. Four focused tests include a 256-case independent bit model.

The original 2,000-page recipe and standalone clap-based table_data example match
all 4,000 tagged Jet 4/ACE rows, including every Binary byte, against Jackcess.
The engineering consumer supplies 256 MiB explicitly; CLI defaults remain 16 MiB.
[Usage-map limits](NATIVE_READING.md#usage-map-ordering-and-ownership-bounds),
[fixture identities](FIXTURES.md#original-generated-usage-map-inputs) and
[Hyperfine samples](testing/NATIVE_BENCHMARKS.md#usage-map-traversal) record the
evidence. Timings are noisy, including a worse ACE sequential mean; no broad
whole-reader speedup is claimed for this complexity/invariant change.

Local Rust 1.99.0 checks passed 261 workspace tests/doc tests, all-target Clippy,
release/benchmark/example builds, minimal package checks and warning-denied API
docs. No dependency or original fixture bytes changed. Native Access, other
targets and remote CI remain unrun. Next: read-only consistency checks across
sequential rows and qualified indexes, with skipped validation reported explicitly.

## Distinct Binary read and creation limits

The native Binary read ceiling remains 510 declared bytes, separately named from
Text's UTF-16 width limit. A temporary restriction to the writer's 255-byte creation
limit broke valid system-table queries in the complete workspace suite and is
corrected: MSysObjects/MSysACEs have independently read 510-byte Binary metadata.
Authoring limits do not establish limits for reading existing native files.
Boundary tests now retain these actual schemas and 255/256/510/511-byte cases.
Zero-length declarations remain outside the qualified reader profile.

Targeted schema tests and all-target ACE Clippy passed on Rust 1.99.0; registered
fixture bytes and dependencies remain unchanged. This corrects reader qualification,
not an observed Microsoft Access comparison. Usage-map work continues separately.

## Bounded chained table definitions

The native shared definition reader now joins at most 64 pages/261,640 logical
bytes for catalog, schema and index consumers. It validates declared length,
signatures, termination, cycles and references, with caller allocation/work/I/O
budgets and cancellation. One-page definitions avoid join allocation. Columns,
Unicode names and index blocks can cross continuation headers; malformed chains
return no partial metadata. Unknown properties/write semantics remain unqualified.

An original 200-column recipe generates two seven-page definitions in each format.
Jackcess comparisons match 800 column descriptions, eight index definitions and
12 sequential/bounded CLI outputs across Jet 4/ACE, including empty tables.
Independent golden assembly, malformed chains, fixed-seed mutation, cancellation,
budgets and every I/O callback failure are covered. [Definition limits](NATIVE_TABLES.md#bounded-chained-table-definitions),
[fixture provenance](FIXTURES.md#original-generated-chained-definition-inputs)
and [Hyperfine evidence](testing/NATIVE_BENCHMARKS.md#chained-table-definitions)
record the accepted profile. Ordinary-reader means changed about -0.1% to +3.3%;
the new capability is not presented as a speedup.

Local Rust 1.99.0 checks passed 256 workspace tests/doc tests, all-target Clippy,
minimal package checks, release/benchmark builds and warning-denied API docs.
Dependencies and original fixture bytes remain unchanged. Native Access, other
targets and remote CI remain unrun. Next: harden usage-map traversal and its
ownership/order evidence before broadening native operations.

## Schema discovery and safe native filter pushdown

NativeSource binding now reads bounded scalar metadata without user records;
complete snapshots remain cached and selected snapshots remain ephemeral. ACE
exposes budgeted scalar schema construction with fallible owned-name allocation.
Query's optional backend-neutral Source constraint defaults to a full scan. Safe
single-Int32 comparisons/NULL tests and same-column conjunctions can select native
ranges through an explicitly required matching index; the original WHERE remains.
Arithmetic, OR, coercions, mixed-column predicates and outer join/subquery
conditions keep full scans, preserving eager failures. Index choices never fall
back silently. A one-table plan avoids a redundant unit cross-product.

Tests cover exact selected root counts, later whole-table cache behavior, schema
discovery with corrupt user data, matching/ignored constraints, parameter reuse,
NULL, eager errors, callback/late source failures and 256 fixed-seed intervals.
The release CLI matched 32 independently filtered/sorted Jackcess outputs across
generated Jet 4/ACE files. [Query contracts](QUERY_EXECUTION.md#safe-source-constraints),
[native source limits](NATIVE_QUERIES.md) and
[Hyperfine evidence](testing/NATIVE_BENCHMARKS.md#query-filter-pushdown) record the scope.

Local Rust 1.99.0 checks passed 249 workspace tests/doc tests, all-target Clippy,
minimal package checks, release/benchmark builds and warning-denied API docs.
The only lockfile edge adds existing clap to the adapter's development dependencies
for its original benchmark harness; normal library closures retain no clap.
Fixture bytes remain unchanged. Native Access, other targets and remote CI are
unrun. Next: qualify bounded chained table definitions.

## Bounded native Int32 seeks

`read_table_index_range` selects NULL or numeric open/closed/unbounded intervals in
either physical index direction. Separator pruning skips disjoint children and
only selected live roots are fetched. Duplicate boundary keys, selected key/row
agreement, visited sibling links, ownership, cancellation and caller budgets remain
checked. Full reads retain complete traversal/count validation; seeks validate
only visited pages and selected records, not skipped tree integrity.

The reproducible example and independent Jackcess comparison match 28 selections
across generated Jet 4/ACE ascending/nullable-descending indexes. Golden and
fixed-seed range tests, exact I/O pruning assertions, visited/skipped corruption
and every-callback read/seek failures cover the new API. Hyperfine's four
500-read workloads measure roughly 6–13× faster selected reads than full snapshots
plus filtering on this host, with all samples retained, including outliers.
See [native index limits](NATIVE_INDEXES.md#native-int32-range-selections) and
[benchmark evidence](testing/NATIVE_BENCHMARKS.md#native-index-seeks).

Local Rust 1.99.0 checks passed 239 workspace tests/doc tests, all-target Clippy,
release/benchmark/example builds and warning-denied API docs. No dependency or
fixture bytes changed. Query source discovery/predicate pushdown is next; CLI
queries still load full snapshots. Native Access, other targets and remote CI
remain unrun.

## Measured civil/calendar and native byte libraries

Jiff replaces time for checked civil calendars and bounded ISO date input while
retaining exact date carriers/Serde, negative Automation DATE rules and field
year/precision bounds. Endpoint and literal tests guard the adapter: Jiff's broader
parser cannot silently accept zones/leap seconds, and civil day spans cover the
full year-9999 endpoint. The unpublished calendar APIs now expose Jiff civil types.

ACE uses zerocopy for explicitly little-endian unaligned index headers/long-value
descriptors. Wide/bytemuck classify private raw-byte ASCII text, avoiding an
intermediate UTF-16 vector while preserving strict Unicode fallback. Bitmap work
uses precharged eight-byte word blocks with bounded cancellation polling.
No semantic Value/Decimal/date receives POD or unchecked construction.

[Hyperfine samples and reproduction](testing/NATIVE_BENCHMARKS.md) record 20 runs
after three warmups per command. Catalog/table workloads improve about 13–22% on
this macOS ARM host; the initial Jiff calendar round trip was about 65% slower.
The [adapter follow-up](testing/NATIVE_BENCHMARKS.md#jiff-conversion-follow-up)
records the subsequent improvement and remaining conversion cost.
Bitmap microbenchmarks vary by density, and a whole-catalog comparison established
no reliable SIMD advantage over the simpler word iterator. These are measured
tradeoffs, not general or cross-platform speed guarantees.

Local Rust 1.99.0 verification passed 231 workspace tests/doc tests, workspace
Clippy, minimal package checks, minimal/all-feature expression and all-feature core
tests/Clippy, release build and warning-denied docs. Independent Jackcess index
comparisons retain all six previous exact row hashes. The fixture SHA-256 is
unchanged; native Access, remote CI and other targets remain unrun. Dependency
records distinguish actual host dependencies from inactive lockfile packages.

## Complete dependency/reference review and purpose grouping

[DEPENDENCY_REVIEW.md](DEPENDENCY_REVIEW.md) individually assesses 103 crates/tools,
reference projects/specifications and all ten proposed standalone RustVBA components.
It covers every candidate in the supplied library, Wine and RustVBA proposals with
source/license metadata, disposition, owner and adoption conditions. The library
policy and technology/reference docs link that registry; installed dependencies
remain separately recorded. Candidates, prototypes and deferred providers were not
added as unused dependencies.

Root workspace dependency declarations are grouped by purpose. Parsed Cargo
manifest semantics, complete locked/offline metadata (including resolved features)
and Cargo.lock match the pre-grouping state exactly; ten packages remain. Local
Rust 1.99.0 formatting, locked/offline workspace check, documentation links/anchors,
candidate/component completeness, target-map counts and diff whitespace checks
passed. The original fixture SHA-256 is unchanged. Runtime suites/builds were not
rerun for this docs/comment/reordering change; no benchmark, advisory scan, license
gate, fuzz run or native Access comparison was performed by the review.

## VBA scope revision

The 18 October 2026 priority decision supersedes the historical VBA/Windows plans
below. VBA execution, authoring, editor/debugger, VBA-specific DAO/ADO, Windows API
emulation, COM/Wine and RustVBA integration are deferred. Existing-code preservation
through unrelated saves and inert dependency detection remain requirements, both
unimplemented. Access table data macros remain in database compatibility scope:
required semantics must run or affected writes must fail.

The active target map removes `tablecraft-vba` and `tablecraft-wincompat`; no
`tablecraft-wine` is added. `tablecraft-vba-host` is reserved in docs only for a
future standalone RustVBA release. Macro/automation ownership remains. Current
working packages and read-only CLI behavior are unchanged. See [VBA scope](VBA_COMPATIBILITY.md)
and the revised [priority order](ROADMAP.md).

Local documentation checks verified 301 links/anchors across 37 public Markdown
files, the revised 34-library/13-domain active target map and the unchanged fixture
SHA-256. These are documentation checks, not VBA preservation or execution tests.

## Explicit bounded native indexed reads

ACE read_table_index traverses qualified single-Int32 index trees and fetches live
referenced rows, checking prefix/boundary encodings, ordering, separator bounds,
ownership, sibling links, cycles/depth, counts and decoded key agreement. The Access
adapter can require a named index and cache the complete result; CLI exposes
`--database PATH --index TABLE INDEX`. Unsupported or corrupt indexes never trigger
sequential fallback. Query clauses still run after full snapshot loading; no range
seek, predicate/TOP pushdown or automatic optimizer selection is claimed.
[NATIVE_INDEXES.md](NATIVE_INDEXES.md) records exact limits, tests and primary references.

Independent Jackcess cursor comparisons matched 1,000-row ascending/nullable-descending
indexes in both generated Jet 4 and ACE databases, plus two 38-row candidate indexes.
Byte goldens, two-level/corrupt trees, mutation, cache/no-fallback, every-callback I/O
failure, mid-read cancellation and CLI semantic tests cover the implementation.
Local macOS / Rust 1.99.0 checks passed 224 workspace tests/doc tests, workspace
Clippy, minimal package and scalar feature checks, release build and warning-denied
API docs. No registry dependency was added and original fixture bytes remain unchanged.
Native Access, other platforms and remote CI remain unrun.

## Native index metadata

ACE now returns explicit logical/physical native index metadata through
inspect_table_indexes; CLI inspection uses `--table NAME --indexes`. Native
numbers/references, raw flags/types, ordered key columns and directions are retained
with strict bounds and caller budgets. [NATIVE_INDEXES.md](NATIVE_INDEXES.md) records
pinned research references, unsupported layouts, original recipes and independent
metadata matches for the registered candidate plus generated Jet 4 / ACE samples.
No index traversal, optimizer path, writing or native constraint validation is
claimed by this inspection slice.

Local macOS / Rust 1.99.0 checks passed 211 workspace tests/doc tests, workspace
Clippy, release build and warning-denied API docs. Static/malformed/cancellation
and 256-case mutation tests plus CLI coverage were added. Native Access, remote
CI and other targets were not run; original fixture bytes remain unchanged.

## Opt-in structured diagnostics

CLI `-v` enables debug operation spans; `-vv` adds low-level I/O detail. Output
is stderr-only with monotonic timing, explicit counts/cache/outcome fields and
no automatic function arguments. Libraries emit through tracing and never install
subscribers. Query stdout and error/exit contracts are unchanged; no environment
filter, network exporter or async runtime is introduced. [TRACING.md](TRACING.md)
and dependency records document the implementation.

Local macOS / Rust 1.99.0 checks passed 207 workspace tests/doc tests, workspace
Clippy, release build and warning-denied API docs. New CLI tests verify quiet
behavior, unchanged stdout, level filtering and omission of SQL/value/name/path
payloads on success/failure. Remote CI, other targets and native Access were not run.

## Workspace-owned dependencies

Registry requirements, local package paths and shared dependency defaults now live
in root `[workspace.dependencies]`; every member normal/development dependency
inherits them. Consumer-specific features and optional flags stay local. The
resolved graph, standalone minimal/all-feature expression and all-feature core
feature trees, and Cargo.lock are unchanged. No registry dependency was added.
`sys-locale` is recorded as a future host-localization candidate in the library
policy; current query/output locale rules remain explicit and unchanged.

Local macOS / Rust 1.99.0 checks passed 205 workspace tests/doc tests, standalone
package checks, workspace Clippy, minimal/all-feature expression and all-feature
core lint/tests, release build and warning-denied API docs. Other targets, remote
CI and native Access were not run.

## Platform I/O extraction

`tablecraft-io` now owns working format-independent synchronous exact reads and
read-at-offset helpers under `crates/platform/`. It consumes core budgets and
thiserror; no registry package or runtime was added. Generic read retry/cancellation
tests moved out of ACE. New focused cases cover nonzero-offset short reads,
byte-charge ownership, EOF, invalid reader counts, sanitized read/seek failures
and pre-seek cancellation/exhaustion. [IO.md](IO.md) documents the API and limits.

ACE header/page reads now consume this crate and translate its errors to the
existing native error contract. Page geometry, allocation, row interpretation and
native locking ownership remain in ACE. Absolute seeks now explicitly charge a
step; byte charges remain at the native caller. Locks, durability and async I/O
were not added. CI includes the new standalone check and uses existing action tags.

Local macOS / Rust 1.99.0 checks passed 205 workspace tests/doc tests, standalone
I/O and engine/adapter checks, workspace Clippy, release build and warning-denied
API docs. An external Rust consumer built and exercised read-at-offset/EOF with
no engine, query, adapter or application dependency. Original fixture bytes remain
unchanged. Other platforms, remote CI and native Access were not run.

## Dense appended-column row reading

The native row reader now derives older-row geometry under a qualified dense
append-only column profile. Missing fields follow independently observed native
storage semantics (NULL, or FALSE for Boolean), while current defaults remain
inert. It handles rows written before any variable field existed and retains
strict checks on present payloads. Gapped/deleted histories and type changes
remain unqualified. [NATIVE_TABLES.md](NATIVE_TABLES.md#dense-appended-column-histories)
records the three-stage independent fixture comparison and limits.

Local macOS / Rust 1.99.0 checks passed 200 workspace tests/doc tests, minimal
engine/adapter/query checks, workspace Clippy, release build and warning-denied
API docs. Scalar feature checks remain as recorded at their unchanged milestone.
The original native fixture remains unchanged. Native Access, other targets and
remote CI remain unrun; no writing/schema mutation was added.

## Mixed-scalar Jet 4 / ACE independent comparison

An original Java recipe now generates fresh V2000 MDB and V2007 ACCDB samples
with three rows/fourteen columns spanning the entire qualified scalar profile.
The external Rust consumer and Jackcess agreed on every tagged cell, including
IEEE signed zero, Currency extremes, Unicode, MEMO/OLE chains, GUID, Decimal and
a negative Automation date. Both formats produced identical normalized output.
[NATIVE_TABLES.md](NATIVE_TABLES.md#mixed-scalar-jet-4-and-ace-comparison) records
inputs, reproduction commands, hash and limitations. The Java comparison helper
now tags GUIDs canonically. Generated binaries and the JAR remain outside Git/CI.

The current Rust baseline remains 197 passing workspace tests/doc tests plus the
recorded lint/release/doc checks; no Rust code changed in this comparison milestone.
Java helpers ran locally on Temurin 21.0.8. This adds independent-writer Jet 4
evidence, not native Access/provider or other-platform qualification.

## Retrieval completion and allocation hardening

A new regression reproduced cancellation inside the final filter comparison
returning a successful empty result. Execution now checks cancellation/work before
publishing the completed result. Parameter-reference/output-column containers,
the implicit source row and ungrouped aggregate container now consume the shared
byte allowance with fallible reservation. Outer joins build NULL padding directly
inside the charged output row instead of allocating unchecked temporary vectors.
Default query results are unchanged; small budgets may now reject work previously
omitted from accounting. Sort comparisons and aggregate errors remain fallible.

Local macOS / Rust 1.99.0 checks passed 197 workspace tests/doc tests, minimal query
check, workspace Clippy, release build and warning-denied API docs. Standalone
expression/core feature checks passed at the preceding unchanged-scalar milestone.
Native Access, other targets and remote CI remain unrun.

## Opaque OLE and shared long-value storage

Native OLE fields now expose bounded opaque Binary through the same checked
inline/single/chained long-value storage as MEMO. Text decoding and its decoded
UTF-8 ceiling remain separate. No content is activated. Synthetic row tests cover
raw/empty/NULL bytes and a decoded-text expansion regression protects the existing
MEMO limit. [NATIVE_TABLES.md](NATIVE_TABLES.md#opaque-long-binary--ole) records the
four-row independent comparison, including a 12,000-byte chain, and the complete
146-row MSysObjects comparison: non-date cells match exactly; fractional calendar
conversion differs by at most 313 ns. CLI Count over MSysObjects now returns 146.
No dependency or generated binary fixture was added.

Local macOS / Rust 1.99.0 checks passed 195 workspace tests/doc tests, minimal
package checks, workspace Clippy, standalone minimal/all-feature expression and
all-feature core lint/tests, release build and warning-denied API docs. Original
fixture SHA-256 is unchanged. Native Access, other targets and remote CI were not run.

## Bounded I/O failure handling

Header/page reads now charge work and check cancellation for every read attempt,
including interrupted retries and short reads. This closes an unbounded retry path
inside the standard exact-read helper. Host callbacks still own individual I/O
timeouts. EOF and other failures remain typed and omit source messages.
[NATIVE_READING.md](NATIVE_READING.md#io-failure-and-retry-boundaries) records the
boundary and tests. Fault injection covers every read/seek in scalar and Long Text
snapshots; adapter tests cover no partial publication, explicit recovery and
cumulative limits on repeated failures. No dependency was added.

Local macOS / Rust 1.99.0 checks passed 192 workspace tests/doc tests, minimal
package checks, workspace Clippy, release build and warning-denied API docs.
Other platforms, remote CI and Microsoft Access were not run.

## Exact native Decimal reading

Qualified 17-byte native NUMERIC fields now decode into validated exact Decimal
values, preserving declared scale and checking precision before conversion. Native
precision/scale metadata is exposed separately from shared scalar carriers. Golden
byte-order and malformed-input tests plus actual row tests cover the codec. The
[Decimal profile](NATIVE_TABLES.md#exact-native-decimal) records unsupported layouts,
negative-zero semantics and the independent four-row Jackcess comparison, which
matched coefficients/scales exactly. CLI projection/filtering/ordering was checked.
No new dependency, arithmetic implementation or generated binary fixture was added.

Local macOS / Rust 1.99.0 checks passed 186 workspace tests/doc tests, workspace
Clippy, release build and warning-denied API docs. Original fixture bytes remain
unchanged. Microsoft Access, other targets and remote CI were not run.

## GUID decoding and complete scalar output

Native GUID fields now decode through `uuid`'s mixed-endian conversion and support
projection, equality/inequality, literals and explicit GUID parameters. Nil/NULL
stay distinct; native GUID ordering remains unqualified and unsupported when
required. Core semantic IDs remain unrelated to UUID generation. Binary values
already supported by the reader now have bounded hexadecimal CLI output.
[NATIVE_TABLES.md](NATIVE_TABLES.md#guid-and-binary-output) and dependency records
capture accepted behavior and the minimal dependency selection.

A temporary independent Jackcess-generated three-row table verified known/nil/NULL
GUID and nonempty/empty/NULL Binary output. A static native byte-order golden test
and CLI parser/equality/ordering/parameter/source tests cover the integration.
No Microsoft provider, RNG, GUI/network runtime or active content is introduced.

Local macOS / Rust 1.99.0 checks passed 182 workspace tests/doc tests, workspace
Clippy, standalone minimal/all-feature expression lint/tests, release build and
warning-denied API docs. Original fixture bytes remain unchanged. Other targets,
remote CI and Microsoft Access execution were not run.

## Bounded Long Text reading

The native MEMO decoder now follows inline, single-row and chained LVAL storage
with explicit lengths, signatures, row flags, cycles, termination, cancellation
and allocation/work ceilings. NULL and empty text retain distinct native meanings.
Scalar-cell work now consumes the shared step budget. [NATIVE_TABLES.md](NATIVE_TABLES.md#bounded-long-text)
records precise accepted layouts, limits and independent evidence.

CLI queries read CMD_LINE_TB descriptions and USysRibbons XML as inert text. The
independent comparison expanded to 1,622 rows across eleven tables. Three native
codec tests include a new 256-case mutation property, and CLI tests verify the
registered copy's counts/XML without activating it. Unsupported-type tests now
use complex/GUID fields because Date/Time and Long Text are implemented. No new
dependency was introduced, and original fixture bytes remain unchanged.

Local macOS / Rust 1.99.0 checks passed 178 workspace tests/doc tests, workspace
Clippy, release build and warning-denied API docs. Scalar/parser feature checks
from the preceding date milestone remain applicable; no scalar code changed here.
Native Access, remote CI and other-platform runs were not performed.

## Legacy Date/Time end to end

The library-policy user edit was committed before continuing engine work. Native
Date/Time fields now retain validated raw DATE bits and participate in temporal
query comparisons, ordering, grouping and DISTINCT. ISO date literals/parameters
and explicit rounded calendar CLI output are implemented. [NATIVE_TABLES.md](NATIVE_TABLES.md#legacy-datetime)
records precise semantics, unsupported coercions and comparison evidence.

Core calendar validation/conversion uses the reviewed `time` library; SQL-enabled
expressions use its static parsing formats. Manual extended-calendar validation
was replaced with checked library APIs. [Dependency records](DEPENDENCIES.md)
record the six added packages/features/licenses/MSRVs and unsafe-code boundary.
Independent Jackcess agreement now covers 1,606 rows across nine tables. The CLI
returns dated CUM_VAL_TB rows (starting 1986-06-01) and accepts typed date parameters.
Native Access and other-platform execution remain unobserved.

Local macOS / Rust 1.99.0 validation passed 174 workspace tests/doc tests,
workspace Clippy, standalone expression minimal checks, expression/core all-feature
lint/tests, release build and warning-denied API docs. Tests include invalid native
NaN dates, equivalent/negative DATE representations, calendar endpoints, literals,
parameters, output and failures on empty sources. The fixture hash is unchanged.

## Numeric-contract cleanup

A workspace source audit replaced unexplained native offsets/masks/tags/page sizes,
query/binding/evaluation limits, Pratt binding powers, CLI budgets and scalar
scale/calendar/identity widths with names or type-derived widths. Format facts and
structural policies remain separately owned; constants follow all imports.
CLI native inspection/retrieval share one explicit host-budget/scope policy.
Public limit/carrier constants expose existing contracts; their values, validation,
accepted SQL/native profiles and output remain unchanged. Independent fixture
bytes and ordinary arithmetic/test inputs remain explicit regression evidence.
No dependency or workspace-package change was introduced.

Local macOS / Rust 1.99.0 checks passed workspace Clippy and 169 tests/doc tests,
standalone schema/query/model/ACE/adapter checks, expression minimal/all-feature
and core all-feature lint/tests, release build, warning-denied rustdoc, formatting
and whitespace. Import ordering was checked across changed Rust modules and the
original fixture hash is unchanged. Native Access comparison, other targets and
remote CI were not rerun for this refactor.

## Native scalar retrieval CLI

`tablecraft-access` is a working storage adapter over ACE snapshots and the existing
query Source interface; the workspace now has nine packages. ACE and query retain
their independent dependency boundaries. The CLI's `query run --database PATH`
opens only that explicit stable/offline source read-only and uses the existing
retrieval/expression engine. [NATIVE_QUERIES.md](NATIVE_QUERIES.md) records supported
formats/carriers, eager table loading, caching, budgets and failure behavior.
Without --database, inline retrieval is unchanged. No active content or writes run.

Three adapter tests cover real query execution, caching/read counts, immediate
visitor rejection, unsupported/missing and empty tables, limits and cached-scan
cancellation. Three new CLI tests cover native TOP/filter/parameter/aggregate,
self-join/empty/text-collation and payload-free error results with unchanged copies.
The existing help test now describes explicit --database capability.

Local macOS / Rust 1.99.0 verification passed all standalone package checks,
workspace Clippy and 169 tests/doc tests, expression minimal/all-feature and core
all-feature lint/tests, release build, warning-denied rustdoc, formatting and
whitespace. The release binary's documented native TOP example matched exactly.
Dependency metadata verified nine packages, independent engine/query closures and
no GUI/network/provider runtime in the CLI. Public links across 33 Markdown files
and unchanged original/comparison-copy fixture SHA-256 passed. Eighteen ACE tests
include three 256-case fixed-seed properties. Native Access, remote CI and
Windows/Linux execution were not observed.

## Native scalar row snapshots

ACE's `read_table` now returns a bounded complete snapshot over the supported
scalar schema profile. [NATIVE_TABLES.md](NATIVE_TABLES.md) describes exact
carriers, NULL/empty/Boolean handling, physical order, structural rejection and
shared budget/cancellation behavior. No registry dependency was added.

Seventeen ACE tests passed locally, including new fixed/variable row fixtures,
exact representation/NULL/empty tests, unsupported/calculated/corrupt inputs,
mid-operation cancellation and a 256-case mutation property. An external Rust
consumer matched Jackcess 5.0.4 for 1,488 rows across seven explicitly named
candidate tables; format-specific source review and normalized result hash are
recorded in NATIVE_TABLES.md. Original fixture bytes remain unchanged. The initial
row API initially had no native query adapter; the adapter milestone above adds
that consumer. No write path or Microsoft Access qualification was added.
ACE Clippy/tests passed on macOS / Rust 1.99.0; other targets were not observed.

## Native table schemas

ACE now exposes bounded local-table schema inspection through `inspect_table_schema`
and CLI `inspect --table NAME`. [NATIVE_TABLES.md](NATIVE_TABLES.md) records the
implemented native metadata and shared Column mapping, unsupported types and
conservative nullability policy. Shared page/layout/Unicode code remains internal;
catalog-specific validation is separate. No registry dependency was added.

All 36 local-table schemas / 187 columns matched Jackcess 5.0.4 for name, native
type, normalized byte length and order on a disposable candidate copy. No active
content or table rows were evaluated. Original fixture bytes are unchanged.
Focused ACE/CLI Clippy and tests passed locally on macOS / Rust 1.99.0.
Native Access comparison and remote CI were not run.

## Shared object model and native catalog inspection

`tablecraft-model` now provides scoped object descriptors and validated read-only
catalog lookup, with real consumers in `tablecraft-ace` and the CLI. [MODEL.md](MODEL.md)
records its identity/name/category/parent contracts and ownership boundaries.
The workspace has eight packages. No placeholder application models were added.

`inspect --catalog` reads five selected MSysObjects fields through dynamic column
metadata, owned-page allocation maps and bounded relocation following. It preserves
unknown native types and unresolved parent references, rejects unsupported/malformed
structures explicitly, and buffers escaped output before stdout. Native origins
stay in ACE rather than entering the shared model. [NATIVE_READING.md](NATIVE_READING.md)
records format profiles, limits, pinned research and the manual comparison recipe.

A disposable copy of the registered candidate yielded 146 metadata records; all
146 `(Id, ParentId, Type, Flags, Name)` tuples matched a read-only Jackcess 5.0.4
scan exactly. The original SHA-256 remains unchanged. No active content, saved
query, linked source or object body was executed/interpreted. This is independent
reader evidence, not a Microsoft Access oracle result. General table data, object
contents, edits, locking and native application execution remain unavailable.

Local verification on aarch64-apple-darwin / Rust 1.99.0 passed standalone
model/ACE/schema/query checks, minimal workspace Clippy and 154 tests/doc tests,
expression minimal/all-feature and core all-feature lint/tests, release build,
warning-denied rustdoc, formatting and whitespace checks. ACE has ten tests,
including two 256-case fixed-seed properties; model has two focused tests and a
runnable public doctest; the CLI has 22 integration tests. The release catalog
command produced the expected 146 rows. Public links across 31 Markdown files,
eight-package metadata and the fixture hashes were checked.

An unrelated temporary Rust package outside the workspace also consumed ACE/core/
model, decoded all 146 objects and resolved AutoKeys as a Macro. Its normal
closure contained only these foundation/engine libraries and thiserror's macro
closure, without CLI, SQL, GUI, network, Java or provider dependencies. No new
registry dependency was added to TableCraft. Native Access, remote CI, Windows
and Linux execution were not observed.

## Initial native header inspection

`tablecraft-ace` now performs bounded read-only header inspection independently
of application packages. The CLI exposes it through `tablecraft inspect <path>`.
[NATIVE_READING.md](NATIVE_READING.md) records exact supported fields, limits and
pinned MDB Tools/Jackcess research references. A copy of the registered example
reports Ace2007/subversion 0, 4 KiB pages and 827 pages with a zero encoding-key
field. No active content ran and the original fixture remains unchanged. Native
catalog/records, writes, locking, runtime execution and Access oracle evidence
were unimplemented at that header-only milestone. The workspace then had seven packages.

Local macOS/Rust 1.99.0 verification passed workspace Clippy, native/CLI tests,
formatting, whitespace and public local links. Three engine tests include a
256-case fixed-seed arbitrary-prefix property; two new CLI tests cover read-only
output and payload-free failures. The copy's observed header was checked and the
original SHA-256 is unchanged. Native Access comparison was not run.

## Shared schema ownership

[SCHEMA.md](SCHEMA.md) records the extracted SchemaName, TypeDescriptor and explicit
SchemaPolicy/NameComparison contracts. Columns, memory table identities and bound
parameters now share validated names. Fields and parameters share declared Text
constraints and exact carrier checks; nullability and runtime resource/finiteness
checks remain separate. SQL type-spelling conversion stays in the query adapter.

Generic schema construction no longer imposes the query profile's 255-byte names
and 256-column ceilings. Query binding and MemoryTable still enforce those same
limits through an explicit query-owned policy. The standalone validate_columns
API now requires an explicit comparison policy. Column::new retains its carrier
constructor, while Column::with_type supports reusable declared constraints.
No new crate, dependency, Serde support, storage behavior or native claim was added.

Verification on aarch64-apple-darwin with Rust 1.99.0 on 18 October 2026 passed
workspace minimal Clippy/tests, standalone schema/query checks, the release build,
rustdoc with warnings denied, formatting and whitespace checks. Five schema tests,
the schema API doctest and three new query/schema integration tests cover the
extracted contracts; existing query/property/CLI tests also passed. The Unicode
Text parameter CLI example returned its expected result. Local links across 29
public Markdown files, unchanged six-package metadata/schema dependency boundary
and the original fixture hash passed. Other targets and native Access comparison
were not run for this refactor.

## Current retrieval milestone

`tablecraft-schema` and `tablecraft-query` now provide validated scalar relation
contracts, explicit read-only sources, prepared slot binding and bounded retrieval
execution. [QUERY_EXECUTION.md](QUERY_EXECUTION.md) defines the implemented profile:
projection/filter/order, joins, grouping/aggregates, DISTINCT/TOP/UNION, parameters
and derived/correlated retrieval subqueries. The workspace has six packages. No
native file source, write execution, full Access expression profile or native
comparison is implemented. Later sections retain evidence from earlier milestones.

Local verification on aarch64-apple-darwin with Rust 1.99.0 on 18 October 2026
passed workspace minimal Clippy/tests, standalone expression minimal/all-feature
Clippy/tests, serde-enabled core Clippy/tests, release build and rustdoc with
warnings denied. The new suites have 14 retrieval acceptance tests, three
fixed-seed properties (256 cases each), one schema test and a runnable public
API doctest. Public local links, formatting, diff checks and the original fixture
hash passed. No Windows/Linux, file-backed source or native Access comparison
was run. CLI retrieval integration is recorded separately below.


## Inline retrieval CLI

`tablecraft query run` now consumes prepared retrieval over an empty host catalog,
with inline SELECT/UNION/derived sources, filtering/sorting, joins, grouping and
subqueries. Repeated typed `--param` arguments validate exact declared carriers;
`--collation ordinal` selects the explicit non-locale text profile. The shared
formatter emits one header and zero or more rows under its 1 MiB logical ceiling.
Execution and formatting finish before stdout; errors exit 2 without partial
results or query/value payloads. Parse/eval behavior is unchanged.

No native file/source adapter, VBA, GUI/network runtime or new registry dependency
was added. CI checks standalone scalar-schema and query builds using the existing
version-tagged actions. The CLI suites now have 18 tests, including seven new
retrieval tests for queries, bindings/collation, failures and output exhaustion.

Local verification on aarch64-apple-darwin on 18 October 2026 passed standalone
schema/query checks, minimal workspace Clippy/tests, the release build, rustdoc
with warnings denied, formatting and whitespace checks. Both documented examples
ran against the release executable with exact expected output. Public local links
across 28 Markdown files, six-package metadata, the normal CLI dependency closure
and the original ACCDB hash were checked. No new runtime dependency enters the
CLI. Native Access comparison, remote CI and other targets were not observed.

## Domain-oriented workspace

The workspace now groups `tablecraft-core` under `crates/foundation/` and the
SQL parser under `crates/query/tablecraft-sql`. The parser was renamed from
`tablecraft-query` to `tablecraft-sql`, including Rust imports, its CLI dependency,
lockfile and CI package selection. Parsing APIs and CLI behavior are unchanged.
The existing feature modules, tests and proptest regression seed moved with their
owning packages. No unused packages or dependencies were added.

[ARCHITECTURE.md](ARCHITECTURE.md) records the implemented package layout,
the target domain map and boundaries for progressive extraction. The future
`tablecraft-query` package is reserved for binding, planning and execution;
`tablecraft-expressions` now implements the initial scalar profile described
below. The planner was absent at that relocation milestone; the bounded retrieval
implementation is now described above. The private architecture
pack remains ignored.

Local verification on aarch64-apple-darwin with Rust 1.99.0 on 17 October 2026
passed locked/offline standalone core and SQL checks, minimal workspace
Clippy/tests, serde-enabled core Clippy/tests, the release build and rustdoc with
warnings denied. Formatting, CLI version/parsing smoke checks and local public
documentation links passed. The suites retain 45 SQL tests plus its doctest,
five CLI tests, and the existing core contracts and optional serde tests. All 71
relocated files retain their contents except for the intended parser manifest
and doctest import rename. Remote CI and other native targets were not observed.

## Headless build foundation

Implemented the minimal Cargo workspace, pinned Rust toolchain, clap-based
headless CLI, build/feature/licensing policy and three-platform CI configuration.
The MIT license is preserved.

The CLI implements help/version, syntax-only `query parse` and source-free scalar
`query eval`. Integration tests launch it
without display environment variables and check output streams, version identity
and rejection of unsupported commands/extra arguments.

Local verification passed on aarch64-apple-darwin with Rust 1.99.0 on 17 October 2026:

- rustfmt check and Clippy with warnings denied.
- All three CLI integration tests (help, version and invalid arguments).
- Locked, offline release build and executable version smoke check.
- Dependency tree contains clap and its derive dependencies; the CLI remains
  free of graphics, network and database runtimes. Versions, licenses, features
  and minimum toolchains are recorded in DEPENDENCIES.md.
- Git whitespace check.

Remote CI and native Windows/Linux execution have not yet been observed.
Database inspection/editing commands and a desktop host do not exist yet.

## Shared core types

`crates/foundation/tablecraft-core` implements distinct object/table/operation/cursor IDs,
table- and generation-scoped record locators, cursor bookmarks, scalar values,
typed validation failures and checked resource budgets. Errors use `thiserror`;
optional `serde` support is limited to scalar value exchange. The CLI consumes
values and resource budgets for scalar evaluation.

Exact Currency, 28-digit field Decimal, width-preserving integers, bit-preserving
floats, GUIDs and distinct legacy/extended timezone-free dates are implemented.
NULL stays separate from empty text, and conversion rejects expression-only
Empty/Missing/Error states. Value and locator diagnostic output redacts payloads.
[CORE_TYPES.md](CORE_TYPES.md) maps the implemented contracts to Microsoft primary
references and records source inconsistencies and remaining boundaries.

Local core verification on aarch64-apple-darwin with Rust 1.99.0 on 17 October
2026 includes 17 feature-local unit/property tests, three public-API contract
tests, four compile-fail doctests and the three existing CLI tests. With the
optional serde feature, seven additional serialization tests check typed round
trips and rejection of invalid constructor inputs. Eight proptest properties
across both configurations use 256 cases each with a fixed seed and regression
persistence. The exhaustive small-coefficient decimal sweep remains in place.
Formatting, Clippy in both feature configurations, minimal and serde-enabled
release builds, and rustdoc with warnings denied also passed locally.

Serde preserves scalar integer/bit representations without JSON-specific string
policies. JSON is a test-only format, with direct typed decoding for wide numeric
values. IDs, generations, expression states, locators, budgets and errors have
no serde implementations. Native codecs, production serialization protocols,
Access oracle comparison and native database query execution remain unimplemented.

The user-requested future library choices and decimal candidate assessment are
recorded in [LIBRARY_POLICY.md](LIBRARY_POLICY.md); those future libraries have
not been added as unused dependencies.

## Access data-query parser and owned model

`tablecraft-sql` implements `parse_query` for retrieval, parameter declarations,
unions, crosstabs, append/update/delete, make-table and schema queries. `parse_select`
remains a single-SELECT convenience entry point. Expressions, clauses, relations,
parameter/type syntax and statement variants are TableCraft-owned; upstream
parser/token types do not escape the adapter. Source is retained independently
from normalized SQL. See [SQL_PARSER.md](SQL_PARSER.md) for precise grammar,
limits, reference mapping and unsupported forms.

The Access Pratt grammar implements operator precedence explicitly, including
ampersand, integer division, Mod, Eqv and Imp. Numeric/date literals retain syntax
without floating conversion or calendar/locale interpretation. Function calls
preserve qualified names and wildcard/omitted arguments; GUID literals and
@@IDENTITY have distinct syntax nodes; LIKE keeps pattern syntax without assuming
wildcard mode. Joins, ordered UNION ALL modifiers, subqueries, fixed pivot headings
and external source/destination metadata survive round trips. Make-table queries
are rejected inside retrieval operands and distinguished as root action syntax.
CREATE/ALTER TABLE, indexes and named/inline constraints retain syntax without
applying schema changes. Saved views/procedures retain ordered parameter headers
and one parsed body; procedure recursion shares the parser budget. Identifier
classification uses Access SQL reserved words instead of upstream multi-dialect
keyword status, including ordinary ID/Ref names. No SQL is executed, and external
source strings are never opened.

Local parser verification on aarch64-apple-darwin with Rust 1.99.0 on 17 October
2026 passed formatting, Clippy with warnings denied, 45 query tests and the API
doctest. Tests cover each query family, structure/precedence, original source,
normalization, malformed/foreign rejection and bounded recursion across nested
queries. Six fixed-seed properties use 256 cases each with regression persistence.
The grammar uses SQLParser std only; recursive-protection and native stack helpers
were removed after moving expression grammar behind an explicit shared budget.

The CLI integrates the parser through clap Parser/Subcommand derives and
`tablecraft query parse "<SQL>"`. Integration tests check normalized output,
parameter/action syntax without execution, malformed/multiple-query rejection,
byte limits, help/version and invalid arguments. All five parse/help/version CLI tests pass locally.
The CLI dependency closure remains free of GUI, network and database runtimes.

The final local workspace checks also passed minimal workspace tests/Clippy,
serde-enabled core tests/Clippy (including seven serde round trips), the release
build, rustdoc with warnings denied, CLI version output, local public-document
links and diff whitespace checks. These checks cover this checkout on macOS.

Binding, schema/type validation, function evaluation, collation, wildcard matching,
native query execution and ACCDB query extraction remain unimplemented. No native Access
comparison or Windows/Linux execution evidence is available for this slice.

## Initial scalar expression semantics

`tablecraft-expressions` implements Boolean/NULL logic, exact numeric comparison,
explicit half-even integral coercion, checked Int64/Double arithmetic, exact
Currency addition/subtraction and multiplication by integers, and bounded
Text/NULL concatenation. Its direct scalar functions consume core values;
they do not use representational equality as SQL equality. Text comparisons
require an explicit collation. The supported carrier profile and deferred
Variant/locale/Decimal behavior are in [EXPRESSIONS.md](EXPRESSIONS.md).

Local formatting, Clippy with warnings denied and 12 scalar tests pass on
aarch64-apple-darwin with Rust 1.99.0 on 17 October 2026. Four properties use 256
fixed-seed cases each. No native Access comparison or query execution is claimed.
The package reuses core, thiserror and development-only proptest; no new registry
dependencies, serialization or runtime libraries were introduced.

The optional `sql` feature now evaluates supported owned expression trees over
host-supplied borrowed values. It supports arithmetic/comparison/logical/text
operators and IS NULL, with explicit unsupported errors for calls, subqueries and
other deferred forms. Binary operands evaluate eagerly in written order. The
evaluator checks cancellation per node, charges core step/Text-allocation budgets
before work, and enforces nesting and per-value byte limits. Its numeric literal
carrier choices are explicit, without claiming full native type inference.

With `sql`, nine more tests (one generated property) and an API doctest pass;
default and SQL-enabled Clippy checks pass with warnings denied. See
[EXPRESSIONS.md](EXPRESSIONS.md) for reference mapping and current limitations.

Final local checks passed locked/offline minimal workspace Clippy/tests,
SQL-enabled expression Clippy/tests, serde-enabled core Clippy/tests, the minimal
release workspace build, rustdoc with warnings denied, CLI version output,
formatting and public-document link/whitespace checks. Dependency-tree inspection
confirms that the default expression package excludes SQLParser/log. The CLI's
subsequent evaluator integration is described below. Other platforms and native
Access evaluation remain unverified.

## CLI scalar evaluation

`tablecraft query eval "<SELECT>"` now invokes the existing evaluator for one
source-free SELECT's scalar projections. Optional aliases label a two-line,
tab-separated result table; Text is quoted/escaped and NULL remains distinct
from empty text. The command rejects database sources, query clauses, parameters,
UNION, action/definition statements and unsupported expressions. It supplies no
field bindings or text collation. `query parse` retains its syntax-only behavior.

One shared budget covers all columns, with 1,024 steps and 65,536 cumulative
Text bytes; depth, per-value limits and the parser input ceilings still apply.
Output is buffered with a 1 MiB logical cap and fallible allocation. Any parse,
validation, evaluation or formatting failure exits 2 without partial stdout.
Projection errors identify the column index without exposing values or SQL.
No query clause is ignored or external content opened.

The CLI now consumes core values/budgets and the expression crate's `sql` feature.
No new registry dependencies were introduced. CI checks both standalone minimal
expressions and the SQL-enabled profile explicitly, since the CLI enables SQL
evaluation in the workspace's unified feature set. CLI command handling and output
policy live in feature modules; library semantics remain in the expression crate.

All 11 CLI integration tests pass locally: the five existing tests and six eval
tests covering results, NULL/text formatting, controls, unsupported query clauses,
eager errors, limits, diagnostics, help and absence of partial output.
The documented arithmetic/concatenation/NULL example was also run successfully.

Local verification on aarch64-apple-darwin with Rust 1.99.0 on 17 October 2026
also passed formatting, standalone minimal and SQL-enabled expression
Clippy/tests, workspace Clippy/tests, serde-enabled core Clippy/tests, release
build, rustdoc with warnings denied, CLI version output and public-document
links/whitespace checks. Dependency inspection confirms the CLI adds only the
existing local core/expression packages, with no Serde, GUI, async/network or
database runtime. Remote CI, other targets and native Access comparison were
not observed.

## Standalone native engine architecture

[NATIVE_ENGINE.md](NATIVE_ENGINE.md) now makes standalone Rust engine usability
an architectural requirement. The target map adds `engines/tablecraft-ace` and
a later `tablecraft-ace-ffi`, while `tablecraft-access` becomes the TableCraft
adapter. Reusable query components and backend-neutral contracts must not import
that adapter, application services or runtime; hosts compose concrete adapters.
The document separates Rust-first implementation, later C ABI/ODBC, engine scope,
publication requirements and native compatibility evidence.

This is a documentation/guidance change. The current four-package workspace is
unchanged. No engine, adapter, FFI, driver or public transaction API has been
implemented or published. The proposal's positional-parameter example is not a
working API; the current SQL parser still rejects positional `?` placeholders.
Private plans and the original ACCDB fixture remain untouched.

Documentation verification passed local links across 14 public documents, target
map counts (33 distinct package names across 13 domains), acyclic proposed
dependency arrows, rustfmt and whitespace checks. Locked/offline Cargo metadata
still lists the same four implemented packages. Rust runtime tests/builds were
not rerun for this documentation-only change; it introduces no native execution
or compatibility evidence.

## VBA and macro architecture

[VBA_COMPATIBILITY.md](VBA_COMPATIBILITY.md) adopts preservation, documented
execution and native authoring as separate compatibility tracks. The target map
adds `tablecraft-vba` and `tablecraft-macros`, with `tablecraft-automation` narrowed
to reusable host contracts. It records interpreter-first language work, project
metadata/unknown-stream preservation, module absence, DAO/DoCmd and form/report
events, cancellation/transaction boundaries, SQL callable injection and debugger
ownership. Required native data macros must work headlessly or affected writes
must fail explicitly. Execution authorization remains separate from preservation.

Microsoft primary references were reviewed on 18 October 2026 and linked in the
owning document. These are requirements and research inputs, not implemented
VBA, macros, module discovery/writing, DAO objects, callable registry or execution
evidence. The current four-package Cargo workspace, runtime behavior and original
fixture are unchanged; no interpreter dependencies or empty packages were added.

## Windows compatibility architecture

[WINDOWS_COMPATIBILITY.md](WINDOWS_COMPATIBILITY.md) adds the planned
`compatibility/tablecraft-wincompat` product boundary, distinct from engineering
oracle tooling. It records per-API Rust/emulated/native/service providers through
shared automation contracts, with VBA independent of concrete providers. Mechanism,
coverage and authorization are separate facts; partial, blocked and unsupported
calls cannot return fabricated success or retry around a denial.

The document covers inert dependency discovery, object dispatch, explicit virtual
paths, filesystem/environment/registry boundaries, qualified Declare signatures,
optional bridges, compatibility reporting and WIN-0 through WIN-5 acceptance.
It separates replicated data validation from replayed UI events/external side
effects, without allowing required data macros to be skipped. Microsoft primary
references were reviewed on 18 October 2026 and linked in the owning document.

No provider, COM/Win32 bridge, DLL loader, isolation helper or new runtime/library
dependency was implemented. The four working packages, CLI behavior, fixtures
and private planning material are unchanged. VBA and Windows tracks are design
requirements, not passing native or cross-platform compatibility evidence.

Documentation verification on 18 October 2026 passed local links across 16 public
documents, the target map (36 distinct package names in 14 domains), acyclic
arrows in all three engine/VBA/provider dependency diagrams, rustfmt and whitespace
checks. Locked/offline Cargo metadata still lists the same four working packages.
Rust runtime tests/builds and native comparisons were not rerun for these
documentation-only changes.

## Application compatibility contracts and corpus policy

The application review now has explicit future contracts for startup/session
state, imported Ribbon XML, DAO/ADO recordsets, advanced field behavior and native
distribution/signatures, linked from [ARCHITECTURE.md](ARCHITECTURE.md). Microsoft
primary references were reviewed on 18 October 2026, including build-qualified
form/report limits and the distinction between code and package signatures.

[ACCESS_APPLICATIONS.md](testing/ACCESS_APPLICATIONS.md) defines separate
open/preserve/execute/edit/save-and-reopen results, provenance/privacy handling,
oracle profiles and representative application selection. The existing MIT
example is its sole candidate, with every native operation unrun. No application
harness, native format support, new fixture, crate or dependency was added. Private
planning and the original fixture bytes remain unchanged.

Documentation verification passed local links across 22 public Markdown files,
the unchanged 36-package/14-domain target map, locked/offline metadata for the
four actual packages, original fixture SHA-256, rustfmt and whitespace checks.
Rust runtime suites and native Access comparisons were not rerun for this
documentation-only change.

## Application workflow priorities

[ROADMAP.md](ROADMAP.md) records P0–P4 priorities and bounded native inspection as
the next native implementation slice. Feature contracts cover reusable expression
editing, saved import/export versus linked tasks, dependency/refactoring analysis,
Documenter/Compare/search/application tests and templates/wizards. The roadmap
also deepens forms/reports, linked-source, native shared-file laboratory, P2P UX
and browser requirements, with explicit scope limits. Existing engine/VBA/Windows
documents remain canonical; the engine document adds public API and ODBC
qualification requirements.

This is documentation only. The parser and initial scalar evaluator already work;
visual editing, application tools and native database features remain future work.
There are no new crates, dependencies, application test runs or fixture changes.

Verification passed links across 27 public Markdown files, three acyclic proposed
dependency diagrams, the unchanged target map and four-package Cargo metadata,
fixture SHA-256, rustfmt and diff whitespace checks. Runtime suites/builds,
diagram rendering and native Access comparisons were not run for this doc-only
revision.

## Access example

The supplied [Access example](examples/Access_Example_VBA.accdb) is registered in
[FIXTURES.md](FIXTURES.md) with its
size and SHA-256. It matches upstream Access-projects/Access-examples commit
`d0ffd681d431f38d91c138559763c30df0699c55` byte for byte, and its original MIT
notice is retained. It has not been parsed, executed or qualified against Access.

Real Access comparison still requires a separately provisioned Windows oracle.
The fixture has not established native format or application compatibility.
