# UNION output types and numeric conversion

UNION and UNION ALL now establish common result descriptors without scanning rows.
The binder considers every declared arm, including empty and NULL-only arms. It
keeps first-arm output names and ordinary width checks. Named/derived consumers
receive the common descriptors before execution.

This is the current TableCraft result profile, not a qualified native Access type
precedence table. Microsoft's [UNION reference](https://learn.microsoft.com/en-us/office/client-developer/access/desktop-database-reference/union-operation-microsoft-access-sql)
permits differing field sizes/types but does not specify the full coercion matrix.
Native first-arm effects, mixed-family conversion/rounding and provider/version
behavior still need native Access evidence. Windows comparisons remain unrun.

## Working combinations

| Declared arms | Common result | Value handling |
| --- | --- | --- |
| Same descriptor | That descriptor | Existing values retained |
| NULL-only and a known descriptor | Known descriptor, nullable | NULL stays NULL |
| Byte / Int16 / Int32 / Int64 | Widest contributing integer carrier | Exact widening; no float conversion |
| Single / Double | Double | Exact binary32-to-binary64 widening |
| Boolean with integral carriers | At least Int16, then widest integral | True becomes -1 and False becomes 0; no Byte True-to-255 conversion |
| Currency with Boolean/Byte/Int16/Int32 | Currency | Exact four-place conversion |
| Currency with Int64 | Unconstrained Decimal | Retains the entire Int64 and Currency ranges exactly |
| Decimal with integral/Boolean/Currency | Unconstrained Decimal | Exact coefficient/scale conversion; existing Decimal cells retain their representation |
| Single/Double with any other numeric family | Double | Existing CDbl conversion, including binary64 rounding of large integers and fixed point |
| Text with different length ceilings | Greater ceiling, or unconstrained if any arm is unconstrained | Original text retained |
| Decimal with different precision/scale declarations | Unconstrained Decimal carrier | Original coefficients/scales retained; no rescaling |
| Dynamic or other mixed families | Dynamic | Existing per-arm carriers and comparison refusals retained |

Text length uses the existing [schema profile](SCHEMA.md), not newly inferred native
UTF-16 units. Same-family Boolean and Single retain their own carrier. Other
non-numeric combinations, including text/numeric and legacy/extended DateTime,
have no new common conversion. Unknown inference does not invent a NULL, parse numeric text,
round a Decimal, or convert expression-only Error/Empty states into stored values.

For example, this returns Long values 1 and 2:

```sql
SELECT CByte(1) AS N UNION ALL SELECT CLng(2);
```

An empty wider arm still supplies its declared result type:

```sql
SELECT CByte(1) AS N UNION ALL SELECT CLngLng(9) WHERE False;
```

The returned value is LongLong 1. Successful Byte/Int16/Int32 values widen exactly
even at signed endpoints and beyond binary64's exact integer range. Single widening
retains signed zero and subnormals; non-finite source/expression checks remain.

## Execution and boundaries

Rows convert in place before each UNION duplicate-removal stage and final ordering.
UNION/ALL stages retain written order; equality, NULL grouping, text policy routing
and first-representative retention use the existing executor rules. No row scan
or value sample supplies a binding type, and no extra result copy is needed.
Per-cell/row widening checks cancellation and shared work limits. All arm expression
and provider failures remain observable before buffered publication.

Conditional functions keep their existing branch inference/materialized carriers.
UNION inference is a separate operation; it does not silently widen IIf/Choose/Nz,
change stored values or authorize writing query results to a native field.

## Evidence

The full all-feature query suite passes 548 tests including doc tests; all-target/
all-feature query Clippy, the minimal query build and CLI build pass. Focused
controls cover arm order, mixed UNION/ALL duplicate removal, first-arm names,
parameters/NULL/empty arms, outer TypeName/VarType consumers, named descriptors,
exact Decimal constraints, signed zero/subnormal widening, late failures, resource
caps, every observed cancellation checkpoint and generated exact integer sets.

[compare_union_types.py](testing/compare_union_types.py) passes 85 CLI controls with
independent Python integer/set and IEEE models. Four registered immutable
[Unicode saved-source inputs](FIXTURES.md#unicode-saved-source-inputs) additionally
supply native Int32 table rows through read-only Jackcess getters for inline UNION
and outer carrier checks across Jet 4, ACE 2007 and ACE 16-family files. The runner
checks reviewed JAR/input hashes and unchanged originals. These checks validate
TableCraft's explicit profile, not native Access result-type inference.

```sh
python3 docs/testing/compare_union_types.py --cli target/debug/tablecraft \
  --jar path/to/reviewed/jackcess-5.0.4.jar \
  --native-directory path/to/registered-unicode-sources
```


Stable [arithmetic result descriptors](QUERY_RESULT_TYPES.md#stable-binary-arithmetic-metadata)
now participate in common-carrier selection. The independent runner passes 109
controls with arithmetic arms in both orders and additional native stored-field
expressions; the original 85 controls remain included. Value-dependent arithmetic
stays Dynamic, and conditional promotion/native SQL precedence remain separate.


## Provisional mixed numeric profile

The numeric table above is a complete current matrix for the nine supported numeric
families, including Boolean. It is selected using declarations across all arms,
without record scans, and is independent of arm order. Empty and NULL-only arms
still affect metadata and conversion. No numeric Text parsing, date serial
conversion, implicit expression Error conversion or provider activation is added.

All-fixed combinations retain values exactly: LongLong with Currency uses Decimal
rather than overflowing Currency or losing integer digits in Double. A mixed
floating result deliberately selects Double and reuses the existing explicit CDbl
kernel. This can round LongLong/Currency/Decimal values before duplicate removal.
For example, LongLong 9007199254740993 and Double 9007199254740992 become one Double
row under UNION and two equal Double rows under UNION ALL. An empty Double arm can
also cause this conversion. This is result-column conversion, not a change to
source/database values, arithmetic precision guards or core representational equality.

Consumers see the converted column. Avg over a Decimal/Double UNION therefore
receives Double values. A Dynamic IIf producing actual Decimal and Double values
still follows the existing accumulator's Decimal precedence. Conditional result
selection and standalone conversion semantics are unchanged. Per-row/cell checks
retain shared cancellation/work limits, with no extra intermediate row vector.

This matrix is provisional implementation policy, not qualified native Jet/ACE
precedence. Microsoft's UNION documentation allows differing field types but does
not define this numeric matrix; SQL Server precedence is not substituted as native
Access evidence. Native first-arm, fixed/floating rounding and version behavior
remain explicit Windows comparison requirements.

The full query suite passes 565 tests including doc tests. Focused controls cover
all 81 pairs, three-arm combinations, metadata-only binding, exact endpoints,
post-conversion duplicates, empty/NULL arms, downstream aggregates, cancellation
and generated independent Decimal/IEEE expectations. Query Clippy and the minimal
query build pass; the CLI builds. The original
[compare_numeric_unions.py](testing/compare_numeric_unions.py) passes 180 controls
using Python Decimal/IEEE models and read-only native IDs across four unchanged
priority-family inputs. The prior 109 UNION controls also pass.

```sh
python3 docs/testing/compare_numeric_unions.py --cli target/debug/tablecraft \
  --jar path/to/reviewed/jackcess-5.0.4.jar \
  --native-directory path/to/registered-unicode-sources
```
