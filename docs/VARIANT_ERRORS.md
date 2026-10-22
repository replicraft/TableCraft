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

Evaluator::evaluate_expression_value returns expression data deliberately; normal
Evaluator::evaluate and scalar QueryResult rows retain their stored-scalar contract.
A raw CVErr projection therefore fails NonScalarExpression in both CLI commands.
An explicit consumer can produce an ordinary result cell. No Error is converted
to NULL, an implicit integer, Binary or a display string to fit the existing row
type. Value::try_from(Error) still rejects storage, with the existing typed core
ExpressionNotStorable error. Arithmetic and ordinary predicates cannot implicitly
consume an Error code.

Unconsumed Error result cells, expression-valued arrays and derived/source fields,
full operator/aggregate propagation, CStr/date conversion and broader Variant
coercion remain unfinished. A future expression-valued query result contract must
carry these states explicitly; scalar snapshots and native writes must continue
to reject them. This is a remaining read/query acceptance item, not a reason to
waive Variant compatibility or move Error into stored Value.

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

The original [independent runner](testing/compare_cverr.py) passes 3120 standard-code
ranges, Decimal half-even input rounding, selection/inspection and refusal cases.
Reviewed getters supply native ID fields from four original ScalarFixture files
with expression evaluation disabled; source hashes remain unchanged. The initial
CLI/model expectations used lowercase Boolean spellings and were corrected to
the existing uppercase TRUE/FALSE transport. No product output policy changed.
These models/getters do not execute CVErr in Access or qualify native coercion,
HRESULT mapping, query error cells or error ordering. CVErr remains Partial.
No dependencies, core stored carrier or native format changed.
