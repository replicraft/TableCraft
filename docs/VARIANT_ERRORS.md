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

Expression-valued arrays, full operator/aggregate propagation, CStr/date conversion
and broader Variant coercion remain unfinished. Arithmetic, ordinary predicates,
sort/group keys and set comparisons do not implicitly unwrap Error codes. Required
scalar consumers reject Error explicitly; DISTINCT/UNION comparisons involving
Error are not qualified. An operation needing no comparison does not invent one.
Crosstab row/cell publication uses the same carrier, but heading discovery and
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
