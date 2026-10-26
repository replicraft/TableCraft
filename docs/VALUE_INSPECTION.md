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
| Stored Guid through the explicit fixed Byte-array view | Byte() | 8209 |

The array flag is added to its declared element subtype; a homogeneous Array(1,2)
remains a Variant-element array. Binary is inspected as a zero-based Byte array
without interpreting an OLE, attachment, encoding or application payload. This is
the explicit materialized-binary mapping; its native field/Variant behavior remains
unqualified. Direct scalar host-bound Binary inspection borrows the bytes and resolves the
field once, without a payload copy or element scan. Ordinary scalar Binary
expression operations retain their existing restrictions. Public kernels and
bound queries also accept Binary; native row materialization remains budgeted.

Stored GUIDs now have the explicit fixed Byte-array view described below.
Extended DateTime still has no qualified VBA subtype: TypeName/VarType reject
it with TypeMismatch. Its Array/Empty/Error predicates retain their known
non-array/non-language-state status. LongLong remains available independently of
host pointer width, like the existing CLngLng profile; 32-bit VBA compatibility is
not established. Inspection metadata does not coerce numeric payloads, parse Text
or validate a Double's arithmetic finiteness. Normal expression evaluation still
rejects failing/non-finite computations before inspection can consume their result.

IsEmpty distinguishes explicit Empty from NULL, empty Text, zero and empty arrays.
IsError recognizes an opaque `ExpressionValue::Error` supplied by a host. It does
not catch an EvalError, cancellation, failed allocation, missing policy or a query
failure; `IsError(1/0)` fails. [CVErr construction and explicit consumers](VARIANT_ERRORS.md)
now work in queries; uninitialized language
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

## Bound result composition

LBound/UBound now expose their existing nonnullable Long/Int32 success guarantee
in query output metadata. Binding does not inspect arrays, read table rows or
evaluate elements/dimensions. Invalid inputs still fail, including NULL and an
out-of-range dimension; a later dimension error retains its existing precedence.
No new array inputs or query functions are admitted.

This fixes `SELECT LBound(Array(1,2)) UNION SELECT '0'`: omitted metadata formerly
left Dynamic carriers and refused mixed integer/Text deduplication. The existing
[UNION conversion](UNION_TYPES.md) now converts the bound result to Text using an
explicit number format before deduplication. Wider integral arms receive their
common integer carrier; named/derived and scalar-subquery consumers retain the
Int32 descriptor, and aggregate/empty results retain their existing NULL rules.
These are guarantees of the current materialized query profile, not native Access
query-availability or array-typing qualification.

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


## Bound-result declaration provenance

The explicit declared-arithmetic query profile now retains the existing
LBound/UBound Scalar Long signature through direct calls, derived/named reads
and correlated scalar subqueries. For example,
`UBound(Array(1,2))+CLng(2147483647)` reports overflow rather than refusing an
unknown declaration. An explicit CVar wrapper retains Variant widening, and
default materialized arithmetic stays unchanged. UNION and aggregate outputs
keep their separate, still-unqualified declaration categories.

This uses the fixed Long return contracts in Microsoft's
[LBound](https://learn.microsoft.com/en-us/office/vba/language/reference/user-interface-help/lbound-function)
and [UBound](https://learn.microsoft.com/en-us/office/vba/language/reference/user-interface-help/ubound-function)
references. Existing direct-query admission remains the scope recorded above;
the signature references alone do not establish query availability. Array input,
dimension, eager error, empty-result and resource/cancellation rules are unchanged.
Native Access declaration and overflow behavior remain unqualified.

## Stored GUID inspection and fixed bounds

The current profile exposes stored Guid values to TypeName, VarType, IsArray,
LBound and UBound as fixed zero-based Byte arrays. Results are `Byte()`, 8209,
True, 0 and 15 respectively. IsEmpty and IsError remain False. NULL fields keep
their separate Null/1/non-array inspection results, and requesting NULL bounds
still fails InvalidNullConversion. Unsupported dimensions keep ArrayBounds;
written dimension expressions evaluate eagerly before bound admission.

Microsoft's [GUID conversion contract](https://support.microsoft.com/en-us/access/functions/guidfromstring-function)
defines stored GUIDs and GUIDFromString results as Byte arrays. This mapping
extends TableCraft's existing Byte-array inspection profile to its canonical
stored Guid carrier. Exact native subtype names, lower bounds, Variant promotion
and error precedence remain Windows qualification work. It does not change the
stored Guid type, canonical bytes, query result descriptor or strict equality.

`ArrayView::Guid(&Guid)` borrows the existing value for inspection/bounds only.
`ArrayView::from_scalar` centralizes Binary/Guid/NULL admission for scalar and
query evaluators. No Byte vector, element scan, GUID parsing or byte-order
conversion is needed to inspect fixed width. The view exposes no element-reading
or mutation API and cannot outlive its input. General arrays retain their current
separate limits. TypeName's Text output remains fallibly allocated and metered;
other inspection/bound results reuse their existing work/cancellation checks.

Native fields, scalar subqueries, typed GUID parameters, CVar/selectors, grouped
aggregates, domains and progressive queries use the same view. Source payloads
and returned Guid cells retain their original carrier. Unsupported Extended-date
inspection and Error/Missing/non-array bound admission remain unchanged. No VBA
runtime, object activation, host provider or new dependency is added.

Two focused expression tests, four query tests and a CLI test cover fixed extents,
wrapper normalization, source carrier preservation, parameter reuse, compound
consumers, streaming parity, eager failures and every observed query cancellation
point. Existing Binary/Variant array tests remain passing. See the
[verification record](testing/results/guid-inspection-2026-10-25.json).
This closes an implementation gap without completing native qualification or P0.

## Native GUID inspection replay — 25 October 2026

The original [inspection runner](testing/compare_guid_inspection.py) passes 84 CLI
controls over four unchanged Jet 4/ACE 2007/ACE 2016/ACE 2019 scalar originals.
Four independent read-only getters supply GUID presence and identities. Only
TableCraft executes SQL; expected observer values follow the explicit fixed
Byte-array profile, not an Access runtime oracle.

The 68 successful controls compare 96 rows and 432 cells across both arithmetic
profiles, buffered/progressive observers, NULL inspection, groups, domains,
subqueries and typed GUID parameters. Sixteen refusals preserve empty stdout:
the earlier executable's four type-name refusals and twelve current NULL-bound,
dimension and eager-division failures. Four earlier IsArray controls retain their
previous False results, documenting the changed observer behavior separately.

The [audited report](testing/results/guid-inspection-replay-2026-10-25.json) verifies
the complete unique command grid, fourteen immutable artifacts, five selected
committed/live source snapshots, matching getter transcripts and identical primary
observer output hashes across buffered/streaming and arithmetic profiles. Row/cell
counts are recomputed from original presence. Originals, tools and frozen binaries
are unchanged. Exact native subtype names, lower bounds and error precedence
remain unqualified; no fixture or dependency is added.

The candidate was built at `56c5617a`, SHA-256
`eb8ea6717440d41b9e4a4e159425c71d2d15b5cd2e48ad5c417d5d85654049f4`.
The baseline was built at `819980ce`, SHA-256
`2be6ed81c1839ce7235bf29ce0b6d6e8f5d6aea70da5ac4e20109ff62bf5b9ed`.
The runner downloads/builds nothing, verifies reviewed fingerprints, caps CLI
observations at 128 and requires a fresh report path. Commands have individual
timeouts and getters have a 256 MiB Java heap ceiling.

```sh
python3 -B docs/testing/compare_guid_inspection.py \
  --cli .cache/tablecraft-bin/guid-inspection-cli-20261025 \
  --baseline .cache/tablecraft-bin/guid-predicates-cli-20261025 \
  --jar .cache/tablecraft-fixtures/jackcess-5.0.4.jar \
  --directory .cache/tablecraft-fixtures/scalar-seek-20261023 \
  --revision 56c5617a \
  --report FRESH_REPORT.json
```
