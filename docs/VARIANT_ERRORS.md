# CVErr and expression Error data

CVErr now constructs the existing core ExpressionValue::Error carrier through
shared scalar/bound query kernels. Error data is distinct from SQL NULL, a stored
scalar and an EvalError. IsError recognizes the data; it does not catch division
by zero, cancellation, allocation failure or invalid constructor input.

The [Access query sandbox list](https://support.microsoft.com/en-us/access/functions-and-properties-in-access-blocked-by-sandbox-mode)
establishes direct query availability. The
[CVErr specification](https://learn.microsoft.com/en-us/openspecs/microsoft_general_purpose_programming_languages/ms-vbal/7a7facef-2f52-4d08-918e-63bf6a9261ed)
defines existing-Error identity, checked Long coercion and standard codes 0..65535.
The [Access function reference](https://support.microsoft.com/en-us/access/functions/cverr-function)
distinguishes explicit numeric conversion from forbidden implicit conversion.
These contracts are implementation references, not native Access test evidence.

## Construction and consumption

Numeric input is converted to Long with the existing checked/half-even policy,
then range-checked. Negative or greater-than-65535 converted codes fail Domain;
failure to fit Long remains Overflow. NULL fails InvalidNullConversion. Existing
opaque Error data is preserved without guessing a mapping for a host HRESULT.
Text input in evaluator/query contexts uses the explicit numeric Text profile,
shared budgets and normalization. The standalone constructor kernel accepts
materialized numeric carriers without ambient regional parsing.

CVar, eager IIf/Nz/Switch/Choose selection and the public expression-result API
retain Error data. Every supplied expression is still evaluated in written order:
an unselected CVErr(2001) is data and can be discarded, while an unselected 1/0
remains a failed evaluation. Switch/Choose condition/index conversion errors keep
the existing deferred-failure ordering. Selection conditions do not implicitly
unwrap Error codes. SQL array/binary admission is not widened by these changes.

TypeName returns `Error`, VarType returns 10, IsError returns True and IsArray/
IsEmpty return False. IsNull and IS NULL distinguish Error from SQL NULL, including
code zero. Explicit numeric conversions consume a standard code through the
existing destination constructor: CInt(CVErr(65535)) overflows, for example.
Opaque implementation-defined codes above 65535 require a supplied mapping and
are rejected by numeric conversion. They are never reinterpreted as signed values.

```sh
tablecraft query eval "SELECT IsError(CVErr(2001)),TypeName(CVErr(2001)),CLng(CVErr(65535))"
tablecraft query run "SELECT ID,IsError(CVErr(ID)),CLng(CVErr(ID)) FROM ScalarSamples" --database example.accdb
tablecraft query eval "SELECT IsError(IIf(True,CVErr(2001),0)),IIf(False,CVErr(2001),7)"
```

## Result and storage boundaries

Evaluator::evaluate_expression_value and PreparedQuery::execute_expression return
expression data deliberately. ExpressionQueryResult is QueryResult<ExpressionValue>;
its buffered rows retain Error codes alongside ordinary Value cells. Projection,
derived and named rows, UNION ALL, joins and correlated scalar subqueries carry
these cells without translating them into source/database values. Explicit
inspection, selectors and numeric consumers can use derived Error fields.

Evaluator::evaluate and PreparedQuery::execute retain the scalar-only Value
contract. The latter performs a checked, budgeted final publication pass and
refuses any retained non-scalar cell. Operational failures encountered while
executing still fail the entire call. Source schemas, source scans, parameters,
snapshots and native write contracts remain scalar. Value::try_from(Error) rejects
storage with the existing typed core ExpressionNotStorable error. There is no
parallel scalar row with a hidden Error side table.

Both CLI query commands use the expression-result path. Their buffered text
format renders Error cells as `#ERROR(2001)`, for example; ordinary Text is quoted,
so `'#ERROR(2001)'` renders as `"#ERROR(2001)"`. This is CLI display policy, not an
Access error-message catalog or a new persisted type. Existing scalar spellings
are unchanged. Empty/Missing publication remains unsupported. The CLI exposes no
partial stdout if a later expression or output limit fails.

```sh
tablecraft query eval "SELECT CVErr(2001),Null,2001,'#ERROR(2001)'"
tablecraft query run "SELECT Problem,IsError(Problem) FROM (SELECT CVErr(2001) AS Problem) AS Q"
```

Explicit [CStr conversion](CSTR.md#explicit-expression-error-and-empty-conversion)
now formats standard Error data and consumes Empty. Expression-valued arrays, full
operator/aggregate behavior, date conversion and broader Variant
coercion remain unfinished. Arithmetic, sort/group keys and set comparisons do
not implicitly unwrap Error codes. Qualified Error/Error predicates now compare
standard codes through the explicit rule below. Required
scalar consumers reject Error explicitly; DISTINCT/UNION comparisons involving
Error are not qualified. An operation needing no comparison does not invent one.
Crosstab row/cell publication uses the same carrier. First/Last now retain
expression Error data, including in pivot cells; heading discovery and numeric
aggregation retain their scalar consumer limits. No Error is silently replaced
with NULL, an implicit integer, Binary or display Text inside the query engine.

Data-row copies, source wrapping and scalar-only final publication charge their
allocations and work to the same operation budget. The scalar-only API needs an
additional row container for checked publication; the CLI expression API moves
its result rows directly. Cancellation checks also cover those conversion/copy
boundaries. Plans retain no expression state between executions.

The query model owns a CVErr child node and tracks its ordinary field, parameter,
group/aggregate and correlated dependencies. Native sources remain scalar and
binding does not scan rows. Constructors/selectors/numeric conversion live in the
expression library; CLI and native adapters add no duplicate rules. Work/depth,
fallible Text conversion allocation, text/output limits and cancellation remain
active. Stored VBA and exception handlers are not executed.

## Evidence and qualification

Three kernel controls cover checked construction, opaque identity, destination
limits and storage refusal, including a fixed-seed 256-case standard-code model.
Two scalar and three bound controls cover construction/inspection/selection,
explicit conversion, parameters, grouping, aggregates, correlation, eager errors,
source-predicate safety, budgets and observed scalar cancellation checkpoints.
Focused array-view regression checks preserve Binary and NULL selector inputs
and the existing CVar-array refusal. CLI checks exercise both query commands and
empty stdout on refused output.

Five additional query-result controls cover explicit/scalar APIs, derived and
correlated rows, joins, named queries, crosstabs, eager failures, shared quotas
and every observed cancellation checkpoint. Editing cancellation tests now cover
every observed checkpoint and verify that no backend candidate is requested.

The original [independent runner](testing/compare_cverr.py) passes 4522 controls,
including raw Error cells alongside standard-code
ranges, Decimal half-even input rounding, selection/inspection and refusal cases.
Reviewed getters supply native ID fields from four original ScalarFixture files
with expression evaluation disabled; source hashes remain unchanged. The initial
CLI/model expectations used lowercase Boolean spellings and were corrected to
the existing uppercase TRUE/FALSE transport. No product output policy changed.
These models/getters do not execute CVErr in Access or qualify native coercion,
HRESULT mapping, query error cells or error ordering. CVErr remains Partial.
No dependencies, core stored carrier or native format changed.

## Encounter-order aggregates and domains

First/Last now retain Error data in the first/last encountered operand, alongside
ordinary scalar and NULL values. Empty groups return NULL. Every operand still
evaluates: an unselected Error value can be discarded, while a later division by
zero, invalid constructor, cancellation or source failure fails the whole query.
The same endpoint selector serves DFirst, DLast and DLookup under the existing
[domain parsing/source/budget contract](DOMAIN_FUNCTIONS.md). No error code gains
implicit numeric coercion and no VBA exception handler is executed.

Explicit inspection/conversion can consume these results, grouped HAVING can test
them, and derived/named/crosstab publication retains their carrier. The scalar-only
execute API still rejects a selected Error cell on publication. Other aggregate
consumers follow the separate [expression aggregate profile](EXPRESSION_AGGREGATES.md).
Encounter selection does not qualify set comparisons or Error sort/group keys.
Final ORDER BY does not choose an endpoint inside each group; an explicitly
ordered derived input supplies its own
encounter order. Native Access optimizer/index/Error-order fidelity remains open.

Four bound controls include a fixed-seed 256-case independent endpoint model over
empty/scalar/NULL/Error sequences. They cover grouped/derived/crosstab/domain
results, explicit consumers, late operational failures, scalar publication refusal,
shared quotas and every observed query/domain cancellation checkpoint. CLI checks
verify Error display, late-failure buffering and source-free eval refusal. The
[original runner](testing/compare_variant_aggregates.py) passes 1058 controls
using independent list endpoints and native getter IDs from four unchanged ScalarFixture files. It does
not execute First/Last or CVErr in Microsoft Access.

## Contextual selectors

[Conditional Date/Text/state conversion](CONDITIONAL_FUNCTIONS.md#date-text-and-explicit-expression-states)
retains Error data only in selected payload positions; a condition/index Error
still fails TypeMismatch even when its code is zero. Typed Empty can act as false
or a zero/out-of-range index without becoming stored NULL; selected Empty still
fails ordinary scalar publication. Missing remains invalid. No new SQL state
constructor or VBA runtime is introduced.


CStr now deliberately consumes standard Error data as ordinary `Error <code>`
Text, including values from encounter/domain endpoints and derived/named rows.
It also converts expression Empty to empty Text. This does not make implicit
concatenation, arithmetic, grouping/comparison or stored-value publication admit
Error, nor catch failed expression evaluation. Opaque host codes still need a
mapping. See the [explicit conversion contract](CSTR.md#explicit-expression-error-and-empty-conversion).

## Counting expression Error values

Count(expr) now consumes expression nullness instead of requiring a stored scalar.
Standard and opaque Error data are non-NULL and each contributes one, without
reading or converting its code. Ordinary scalar values retain their existing
count behavior; NULL contributes zero. Empty/Missing query cells remain refused.
Count(*) does not evaluate an argument, and empty groups return zero.

The shared kernel serves grouped/HAVING queries, derived/named inputs, correlated
queries, crosstab cells/totals and DCount. It evaluates every selected argument;
invalid CVErr constructors and operational failures still fail the whole query.
Missing crosstab intersections remain NULL, distinct from a populated all-NULL
cell whose count is zero. This change does not widen numeric/statistical/min/max
aggregate consumers or permit Error storage, comparison or implicit conversion.

Microsoft's [Count reference](https://learn.microsoft.com/en-us/office/vba/access/concepts/criteria-expressions/count-function-microsoft-access-sql)
defines counting non-NULL expressions without depending on their stored values.
That motivates this explicit profile; Error-valued native query cells and actual
Access aggregate error ordering remain unqualified. The original
[comparison runner](testing/compare_error_counts.py) passes 98 independent controls:
seeded Python nullness models and read-only stored ID getters across four unchanged
20,000-row native inputs, sequential and indexed reads, small selected ranges,
DCount and late-error publication refusals. No Access SQL is executed by the oracle.

## Arithmetic Error admission and eager operands

Unary plus/minus and binary addition, subtraction, multiplication, division,
integer division, Mod and power now reject Error operands with TypeMismatch,
including Error/Error and Error/NULL pairs. They do not unwrap codes or return
an Error data cell. Explicit numeric conversion remains available before arithmetic.

Both binary operands evaluate in written order before arithmetic admission. Thus
`CVErr(0)+(1/0)` fails DivisionByZero, while `CVErr(-1)+(1/0)` fails the left
constructor's Domain check. IsError does not catch either failure. Shared resource
and cancellation checks remain active during operand evaluation. Scalar eval and
bound queries share the admission rule, including derived/correlated operands,
encounter-aggregate results and crosstab expressions. Scalar-only publication
still rejects a raw Error result with NonScalarExpression.

The [MS-VBAL arithmetic runtime rules](https://learn.microsoft.com/en-us/openspecs/microsoft_general_purpose_programming_languages/ms-vbal/e070115f-8d40-40cf-ac6d-ab18b9c6c906)
require type mismatch for Error operands. These rules guide this explicit profile;
Access SQL error ordering still needs native comparison. The reference's effective
type table also contains an Error/Error entry; the explicit runtime rejection is
applied rather than inventing an arithmetic operation on two error codes. Logical,
comparison, concatenation, Empty/Missing and numeric-aggregate behavior are separate
acceptance work, not silently generalized from this arithmetic rule.

The original [arithmetic Error runner](testing/compare_error_arithmetic.py) passes
1,190 CLI controls across both eval/run, seeded standard codes, unary/binary forms,
NULL/Error pairs and eager failures. Native checks reuse read-only ID getters from
four unchanged 20,000-row inputs through sequential and both indexed directions;
they compare TableCraft diagnostics and empty stdout, not Access SQL execution.

## Error predicates and logical operators

The six relational predicates compare two standard Error values by their codes
(0 through 65535), producing Boolean. This is an explicit Error/Error operation;
it does not make Error equivalent to a stored number. Mixing Error with any other
carrier, including NULL, fails TypeMismatch. An opaque implementation-defined code
in an Error/Error pair fails UnsupportedExpression rather than guessing an order.

IN/NOT IN lists, reversed-bound BETWEEN/NOT BETWEEN and bound IN-subquery and
ANY/ALL comparisons share this rule. Empty subqueries retain the established
vacuous truth results without inventing a comparison. All selected candidates
and range comparisons retain existing eager/error-order rules; an earlier match
cannot hide a later operational failure. Derived/named Error fields, correlated
operands, grouped/HAVING and crosstab expressions use the same service. Binding
remains metadata-only. Ordinary Text/Text predicates retain their explicit
per-field collation routing and contextual numeric/date comparison policies.

Not, And, Or, Xor, Eqv and Imp reject Error with TypeMismatch, even when ordinary
Boolean/NULL truth tables could otherwise decide a result. Binary operands finish
evaluation in written order before logical admission. IsError does not catch these
failures. Arithmetic shares the scalar-operator admission helper; explicit numeric
conversion, raw publication and stored-value validation retain separate boundaries.

[MS-VBAL relational rules](https://learn.microsoft.com/en-us/openspecs/microsoft_general_purpose_programming_languages/ms-vbal/f8acd631-55c1-4199-bc1e-022aaab6d9c8)
define standard Error-code comparisons and leave opaque-code results undefined.
The [logical rules](https://learn.microsoft.com/en-us/openspecs/microsoft_general_purpose_programming_languages/ms-vbal/2d70780d-6e99-47d6-9759-2b3a46b8e862)
require Error rejection. These references guide this explicit materialized-value
profile; they do not qualify native Access SQL, declared/Variant coercion or error
ordering. Sorting, grouping, DISTINCT/UNION set keys, concatenation, Empty/Missing
and numeric-aggregate behavior remain separate acceptance work. Predicate support
does not silently widen those consumers or stored sources.

The original [predicate runner](testing/compare_error_predicates.py) passes 838
independent CLI/model/native-source controls. Seeded Python unsigned-code relations,
lists and ranges check both eval/run. Read-only getters from four unchanged
20,000-row inputs support native sequential/indexed filters, derived rows, DCount,
IN and quantified subqueries. Mixed-type/logical/eager failures require typed
diagnostics and empty stdout. Rust controls separately cover opaque codes, a
256-case relation model, shared work, metadata-only named preparation, composition
and every observed query cancellation checkpoint. No native Access SQL is executed.

## Aggregate and concatenation consumers

[Expression aggregates](EXPRESSION_AGGREGATES.md) now include provisional
standard Error-only Min/Max and DMin/DMax, retaining Error data while ignoring NULL.
Mixed non-NULL kinds fail TypeMismatch and opaque codes are refused. Numeric and
statistical aggregates require explicit Error-code conversion. Ampersand now
rejects Error with TypeMismatch after both operands evaluate; explicit CStr remains
available. These changes do not widen stored values or sort/group/set keys, and
do not establish native Access aggregate behavior.
