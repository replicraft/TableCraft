# Explicit Decimal parameter constraints

The owned query binder accepts `DECIMAL(precision,scale)` and its `NUMERIC` synonym
in explicit PARAMETERS declarations. The parser already retains these two unsigned
arguments; binding now attaches the existing checked schema DecimalConstraints.
Precision is 1 through 28 and scale is 0 through precision. Invalid or oversized
arguments fail during metadata preparation without scanning source records.

```sh
tablecraft query run \
  'PARAMETERS P DECIMAL(4,2); SELECT P AS Exact UNION ALL SELECT P' \
  --param P=decimal:1.20
```

The host supplies a Decimal carrier with the exact retained scale and a coefficient
whose decimal digits fit the declared precision. `1.20` fits DECIMAL(4,2); `1.2`,
`1.200`, `100.00`, Double and integer carriers do not. SQL NULL remains permitted.
This is the existing strict parameter boundary: no rounding, rescaling, truncation
or implicit numeric conversion occurs while accepting host values. Query operators
retain their separate expression conversion and arithmetic policies.

Parameter descriptors survive named and derived sources, scalar subqueries and
qualified UNION composition. Named definitions may share an ASCII-insensitive
parameter name only when their complete validated descriptors agree. Matching
DECIMAL/NUMERIC aliases share one supplied value; differing scale, precision or a
bare Decimal declaration conflict. Named definitions retain isolated expression
scopes, and binding never reads rows to infer constraints from supplied values.
Execution checks every supplied parameter before publishing results; CLI failures
remain buffered and emit no partial rows.

Bare DECIMAL and NUMERIC retain their existing unconstrained scalar-carrier
contract, including the full 96-bit expression range and retained scales. They do
not silently become DECIMAL(18,0). This addition requires both explicit arguments;
precision-only/default-scale syntax remains unqualified. Native saved-query
parameter flag mappings are unchanged, so parsing explicit SQL does not authorize
reconstructing an unknown native parameter type.

Microsoft's [PARAMETERS contract](https://support.microsoft.com/en-us/access/parameters-declaration)
accepts primary Access SQL data types and synonyms. Its [SQL data-type reference](https://learn.microsoft.com/en-us/office/client-developer/access/desktop-database-reference/sql-data-types)
specifies Decimal precision 1–28 and scale 0–precision. These support the declared
constraints; they do not qualify TableCraft's strict host ABI, native SQL parameter
rescaling, provider-specific syntax or error ordering. Microsoft Access execution
and native saved/reopen comparisons remain unrun.

Focused query tests cover every declared precision/scale pair for both aliases,
positive/negative coefficient boundaries, NULL, wrong scales and carriers, bare
range preservation, named-scope agreement/conflicts and metadata-only refusal.
CLI tests exercise subquery/UNION composition, exact printed scales and buffered
failures. No engine, storage, dependency or write behavior changes.
