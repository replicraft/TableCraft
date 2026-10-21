# Value inspection and expression-only arrays

IsArray, IsEmpty, IsError, TypeName, VarType, LBound and UBound now work in the
explicit materialized expression profile. Scalar `query eval` and bound read-only
`query run` accept the supported inputs, including qualified native saved/named
retrieval. Public kernels also inspect host-issued Empty and Error states.
No function activates VBA, COM, object properties or a provider.

```sh
tablecraft query eval 'SELECT TypeName(CInt(1)),VarType(CInt(1))'
tablecraft query run 'SELECT IsArray(Array(1,Null)),LBound(Array()),UBound(Array())'
tablecraft query run 'SELECT TypeName(ID),VarType(ID) FROM Contacts' --database example.accdb
```

## Type and state semantics

TypeName reports the materialized carrier, not a field declaration or a type
inferred from the payload. VarType returns an Integer (`Value::Int16`) subtype
code. Integer widths remain distinct; the existing evaluator's integer literals
are Int64, so `TypeName(1)` currently returns LongLong. Use explicit conversions
when requesting a particular carrier. This records TableCraft's current promotion
profile rather than claiming native Access literal/promotion behavior.

| Carrier/input | TypeName | VarType |
| --- | --- | ---: |
| Empty supplied by a host | Empty | 0 |
| NULL | Null | 1 |
| Int16 | Integer | 2 |
| Int32 | Long | 3 |
| Single | Single | 4 |
| Double | Double | 5 |
| Currency | Currency | 6 |
| Legacy DateTime | Date | 7 |
| Text | String | 8 |
| Error supplied by a host | Error | 10 |
| Boolean | Boolean | 11 |
| Decimal | Decimal | 14 |
| Byte | Byte | 17 |
| Int64 | LongLong | 20 |
| Explicit Variant-element array | Variant() | 8204 |
| Materialized Binary/borrowed Byte array | Byte() | 8209 |

The array flag is added to its declared element subtype; a homogeneous Array(1,2)
remains a Variant-element array. Binary is inspected as a zero-based Byte array
without interpreting an OLE, attachment, encoding or application payload. This is
the explicit materialized-binary mapping; its native field/Variant behavior remains
unqualified. Direct scalar host-bound Binary inspection borrows the bytes and resolves the
field once, without a payload copy or element scan. Ordinary scalar Binary
expression operations retain their existing restrictions. Public kernels and
bound queries also accept Binary; native row materialization remains budgeted.

GUID and extended DateTime have no guessed VBA subtype: TypeName/VarType reject
them with TypeMismatch. Array/Empty/Error predicates can still report their known
non-array/non-language-state status. LongLong remains available independently of
host pointer width, like the existing CLngLng profile; 32-bit VBA compatibility is
not established. Inspection metadata does not coerce numeric payloads, parse Text
or validate a Double's arithmetic finiteness. Normal expression evaluation still
rejects failing/non-finite computations before inspection can consume their result.

IsEmpty distinguishes explicit Empty from NULL, empty Text, zero and empty arrays.
IsError recognizes an opaque `ExpressionValue::Error` supplied by a host. It does
not catch an EvalError, cancellation, failed allocation, missing policy or a query
failure; `IsError(1/0)` fails. SQL CVErr construction and uninitialized language
variables remain unimplemented. Missing remains an optional-argument marker,
rejected as a first-class inspection input rather than reinterpreted as an Error.
The core's expression-state-to-storage rejection remains unchanged.

## Array values and bounds

`tablecraft-expressions::arrays::ArrayValue` owns an immutable one-dimensional
sequence of stored scalar elements and explicit Long lower/upper bounds. It is
an expression intermediate, not a new stored Value, field type or parameter type.
`from_owned` adopts a caller-allocated and caller-charged Vec without copying;
it validates at most 65,536 elements and checked Long extents. Empty upper bounds
are lower minus one. Empty arrays based at Long::MIN and overflowing extents fail.
Debug exposes bounds/length only. CashFlows shares this element ceiling while
retaining its separate finite-Double conversion and financial period rules.

Array(...) and VBA.Array(...) can supply direct scalar-element intermediates to
these inspection/bound functions as well as [cashflow functions](CASHFLOW_FUNCTIONS.md).
Every supplied expression evaluates in written order, including NULL and unused
values; an element failure is never replaced by a successful type test. Missing
elements and wildcards fail shape validation before element evaluation. Nested
arrays, general array-returning expressions, array-valued parameters, indexing,
multidimensional/typed Variant arrays, Split/Filter/Join and ReDim remain open.
SELECT Array(...) is rejected rather than serialized into a scalar cell.

Query-created arrays have lower bound zero; there is no VBA module Option Base
context. The public owned input can carry a different explicit lower bound.
Microsoft's Array and LBound pages conflict on Option Base's effect on unqualified
Array; the current query profile assumes no module context, and no broader
module/library-qualified native equivalence is claimed.

LBound/UBound return Long (`Value::Int32`). Omitted or explicitly missing dimension
defaults to one. Supplied numeric/Boolean dimensions use existing checked CLng
conversion; only resulting dimension one is supported. NULL, nonnumeric,
out-of-range and other dimensions fail. A dimension expression is evaluated even
when the first input will later fail array conversion. Byte inputs use bounds
zero through length minus one, with an empty upper bound of -1. Empty bounds,
dimension coercion/error precedence and native limits await Access comparison.

## Budget, binding and evidence

Pure metadata/bound kernels use constant work and borrowed inputs. The evaluator
meters inspection/publication, fallibly materializes type-name Text under the
existing output limits, and charges explicit element buffers before allocation.
Each array element uses the ordinary evaluator's limits and cancellation checks;
array positions are not charged as table rows. Bound queries use the same shared
budget. Binding visits scalar metadata only, never scans records to infer types,
and retains normal pruning, grouping identity, aggregate/domain and correlated
operand walks. Text grouping/comparison still requires an explicit collation.
UPDATE's separate pure profile remains unchanged.

Independent named subtype goldens, state/array/Byte controls, a fixed-seed
256-case wide-bound property and scalar/bound tests cover carrier widths, limits,
evaluation order, metadata-only binding, grouping, parameter reuse, name allocation,
every observed cancellation checkpoint and late failures. The original
[fixture recipe](testing/SavedInspectionFixture.java) creates twelve inert
inspection definitions alongside the original forest's fourteen, without executing
SQL, formulas or VBA. Its [runner](testing/compare_value_inspection.py) passes
284 scalar/bounds/getter/rendering/direct/saved/named/refusal controls across four
original Jet 4/ACE 2007/2016/2019 files with unchanged bytes. Type-name display
quoting is decoded separately from TSV transport. See [provenance](FIXTURES.md#value-inspection-query-inputs).

These controls establish TableCraft behavior and independent metadata/row agreement;
Jackcess is not a query/VBA oracle. Native SQL admissibility of Array/inspection
calls, carrier promotion, empty bounds, native binary/extended fields, objects,
default properties and exact VBA errors remain unqualified. No Windows Access
comparison was run, and these functions remain Partial in the built-in tracker.

Primary references:

- Microsoft [TypeName](https://learn.microsoft.com/en-us/office/vba/language/reference/user-interface-help/typename-function) and [VarType](https://learn.microsoft.com/en-us/office/vba/language/reference/user-interface-help/vartype-function): names, result widths and named subtype/array codes.
- Microsoft [IsArray](https://learn.microsoft.com/en-us/office/vba/language/reference/user-interface-help/isarray-function), [IsEmpty](https://learn.microsoft.com/en-us/office/vba/language/reference/user-interface-help/isempty-function) and [IsError](https://learn.microsoft.com/en-us/office/vba/language/reference/user-interface-help/iserror-function): array and language-state distinctions.
- Microsoft [Array](https://learn.microsoft.com/en-us/office/vba/language/reference/user-interface-help/array-function), [LBound](https://learn.microsoft.com/en-us/office/vba/language/reference/user-interface-help/lbound-function) and [UBound](https://learn.microsoft.com/en-us/office/vba/language/reference/user-interface-help/ubound-function): constructor, dimensional bounds and optional dimension contracts.
