# UNION output types and exact widening

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
| Text with different length ceilings | Greater ceiling, or unconstrained if any arm is unconstrained | Original text retained |
| Decimal with different precision/scale declarations | Unconstrained Decimal carrier | Original coefficients/scales retained; no rescaling |
| Dynamic or other mixed families | Dynamic | Existing per-arm carriers and comparison refusals retained |

Text length uses the existing [schema profile](SCHEMA.md), not newly inferred native
UTF-16 units. Boolean/integer, integer/floating, Currency/integer, Currency/Decimal,
Decimal/floating, text/numeric and legacy/extended DateTime combinations have no new
common conversion. Unknown inference does not invent a NULL, parse numeric text,
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

Rows widen in place before each UNION duplicate-removal stage and final ordering.
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
