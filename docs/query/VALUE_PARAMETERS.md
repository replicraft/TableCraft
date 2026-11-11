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
mapping; its existing provenance/hash remains in [FIXTURES.md](../../tests/reports/reading/FIXTURES.md).
No upstream implementation or fixture bytes were copied.

The original [registered-definition comparison](../../tests/access/queries/saved/compare_example_saved_definitions.py)
uses read-only Jackcess query/parameter getters with expression evaluation
disabled. All 51 read definitions in the registered ACE 2007 application's
59-query inventory reconstruct and round-trip through TableCraft. Their parameter
declarations match the independent getters, including twelve VALUE declarations;
the eight DDL/pass-through definitions remain refused by read reconstruction.
Original database/JAR hashes remain unchanged. This does not mean those reads
all execute: missing application sources, VBA UDFs and execution policies retain
their own failures. Neither getter nor comparison executes SQL or application code.

```sh
python3 tests/run.py tests/access/queries/saved/compare_example_saved_definitions.py \
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

## Explicit unresolved-name discovery

`prepare_with_parameter_discovery` is a separate metadata-only query service.
Ordinary `prepare` and `prepare_with_options` remain strict: an unknown field does
not silently become a parameter. Runtime `InspectQuery` still parses syntax and
explicit declarations without native source callbacks. Normal desktop Run uses
an explicit `DiscoverQueryParameters` request before opening its existing prompt.

The discovery binder resolves real fields, inner/outer scopes, declared inputs
and eligible projection aliases first. Only a remaining unqualified scalar name
becomes an implicit VALUE input. Repeated names share one input under the existing
identifier policy, retaining the first encountered spelling. A bracketed literal
such as `[Enter minimum:]` or `[x.y]` remains one name; it is not a qualified path
or an expression to execute. Unknown tables, unavailable or ambiguous fields,
qualified misses and source/policy failures remain typed failures, not prompts.

`parameter_discovery_mode` chooses the eligible local read profile before binding.
Named definitions, domains, external sources, crosstabs and actions retain the
existing strict route. There is no failed-discovery fallback. Classification and
binding use caller work/cancellation allowances; expression traversal does not
introduce a second depth ceiling. Actual preparation retains its existing binding
scope, structural and node checks. Neither classification nor discovery samples
user rows or query functions to infer names or types.

`PreparedParameterQuery` owns the actual bound parameter nodes and captured source
schemas, with borrowed prompt metadata. Runtime issues that owner beside the
original SQL/document witness; it retains no native reader, provider or resource
borrow. `RunDiscoveredQuery` rechecks the original witness before input parsing
and uses existing schema validation, literal scalar input parsing, buffered
execution and result-local collection issuers. SQL and the original AST are not
rewritten or copied into synthetic declarations. The desktop preserves its
logical inspection identity across the separate metadata worker generation;
cancellation and stale replies cannot replace the accepted prompt or prior result.

Implicit prompts use the existing explicit VALUE carrier picker. This is not a
claim about native inferred parameter types, prompt enumeration order, implicit
coercions or application-reference resolution. Saved-query and mutation parameter
discovery are outside this first profile; their declared input services remain
unchanged. The authored query/runtime/headless acceptance cases require execution
in the coordinated qualification batch before reporting passing evidence.

Microsoft documents that [identifiers Access cannot interpret can request a
parameter value](https://support.microsoft.com/en-us/access/why-does-access-want-me-to-enter-a-parameter-value),
including misspellings. Its [parameter-query guidance](https://support.microsoft.com/en-us/access/use-parameters-to-ask-for-input-when-running-a-query)
describes bracketed criteria prompts and recommends explicit types for numeric,
currency and date inputs. Those contracts establish the prompt behavior; they do
not establish complete native type inference for this service.
