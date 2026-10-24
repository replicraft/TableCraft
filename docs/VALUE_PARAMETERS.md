# VALUE query parameters

`PARAMETERS P VALUE; SELECT P` now retains an explicitly supplied materialized
scalar without choosing a fixed carrier. The native saved-query parameter code
zero reconstructs as `VALUE`; it is not an unknown code or a default to Text.
This is a query parameter contract, independent of the deferred VBA runtime.

## References and evidence

Microsoft's [data-type comparison](https://learn.microsoft.com/en-us/office/vba/access/concepts/error-codes/comparison-of-data-types)
lists Value for query parameters, Variant for Visual Basic and VALUE for SQL,
while distinguishing this from a stored table-field type. The
[PARAMETERS contract](https://learn.microsoft.com/en-us/office/client-developer/access/desktop-database-reference/parameters-declaration-microsoft-access-sql)
requires named declarations and explicitly supplied values.
Jackcess's [parameter map](https://github.com/jahlborn/jackcess/blob/master/src/main/java/com/healthmarketscience/jackcess/impl/query/QueryFormat.java)
maps code zero to Value. The reviewed local 5.0.4 JAR's bytecode confirms the same
mapping; its existing provenance/hash remains in [FIXTURES.md](FIXTURES.md).
No upstream implementation or fixture bytes were copied.

The original [registered-definition comparison](testing/compare_example_saved_definitions.py)
uses read-only Jackcess query/parameter getters with expression evaluation
disabled. All 51 read definitions in the registered ACE 2007 application's
59-query inventory reconstruct and round-trip through TableCraft. Their parameter
declarations match the independent getters, including twelve VALUE declarations;
the eight DDL/pass-through definitions remain refused by read reconstruction.
Original database/JAR hashes remain unchanged. This does not mean those reads
all execute: missing application sources, VBA UDFs and execution policies retain
their own failures. Neither getter nor comparison executes SQL or application code.

```sh
python3 docs/testing/compare_example_saved_definitions.py \
  --cli target/debug/tablecraft \
  --jar .cache/tablecraft-fixtures/jackcess-5.0.4.jar
```

## Preparation and execution

- The syntax adapter admits VALUE for query parameters, including procedure
  declarations, with no length/precision arguments. It does not admit VALUE as
  a stored column type or add a VARIANT SQL synonym.
- Metadata-only preparation retains `OutputType::Dynamic` for a bare VALUE
  parameter. Direct and derived outputs can rebind to different carriers or NULL
  without freezing a type from the first execution. Consumers may independently
  impose the existing composition/type requirements.
- Execution accepts existing core `Value` scalars and NULL, preserving exact
  integer width, Decimal scale, IEEE bits, Text, Binary, GUID and date values.
  It does not add arrays, live objects, Error, Empty or Missing parameter carriers.
  Existing function/operator admission still applies to each supplied carrier.
- Scalar declarations retain exact carrier/constraint checks. VALUE and scalar
  declarations with the same name in different named definitions remain a
  parameter conflict; the binder does not infer a shared fixed type from a value.
- VALUE retains Variant declaration provenance through execution cells and
  prepared result metadata. The explicit declared-arithmetic mode therefore
  retains its existing Variant widening behavior. Default arithmetic and its
  documented materialized-value profile remain unchanged.
- Names remain explicit, case matched under the existing schema policy, with
  missing, duplicate and unknown assignments refused. Inspection/reconstruction
  never prompts. Non-finite Single/Double and oversized Text remain refused.
  Runtime parameter records and payload copies consume the existing cumulative
  allowance; cancellation and failures return no result.

The CLI uses its existing typed input syntax:

```sh
tablecraft query run 'PARAMETERS P VALUE; SELECT P AS Exact' --param P=decimal:1.2300
tablecraft query run 'PARAMETERS P VALUE; SELECT P+CInt(1) AS N' \
  --param P=short:32767 --declared-arithmetic
```

Library controls cover all fifteen current scalar/NULL carriers, rebinding,
direct/derived/scalar-subquery/grouped declaration retention, named scopes, payload
budgets and every observed cancellation checkpoint. Authored native fixture
controls cover Jet 4 and ACE header codes 2/5/6 without changing source bytes.
CLI checks cover exact output and buffered late failures. Native Access inference,
coercion, DAO metadata/error order and modern Variant carrier compatibility remain
unqualified. This does not complete the wider Variant semantics roadmap.
