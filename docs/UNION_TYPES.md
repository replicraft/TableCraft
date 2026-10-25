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
| Text with any supported numeric family | Unconstrained Text | Existing CStr output profile at execution; original Text and NULL retained |
| Text with different length ceilings | Greater ceiling, or unconstrained if any arm is unconstrained | Original text retained |
| Decimal with different precision/scale declarations | Unconstrained Decimal carrier | Original coefficients/scales retained; no rescaling |
| Dynamic or other mixed families | Dynamic | Existing per-arm carriers and comparison refusals retained |

Text length uses the existing [schema profile](SCHEMA.md), not newly inferred native
UTF-16 units. Same-family Boolean and Single retain their own carrier. Other
non-numeric combinations, including Text with legacy/extended DateTime, Binary
or GUID, have no new common conversion. Unknown inference does not invent a NULL, parse numeric text,
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


## Text with numeric outputs

A Text arm combined with any of the nine numeric families now establishes an
unconstrained Text result. Binding reads declarations only, drops any original
Text length ceiling and combines NULL allowances. It never formats a value,
requests a number policy or scans records to choose the output type. Named,
derived, grouped and fixed-crosstab consumers retain this result metadata.

At execution, numeric cells reuse [CStr](CSTR.md) under the current number format.
Numeric rendering requires explicit NumberFormat, including integers. Boolean
uses its existing `True`/`False` spelling without that policy. Original Text stays
exact, including numeric-looking strings and their separators/leading zeroes.
NULL stays NULL rather than entering CStr's invalid-NULL conversion. Unknown or
expression-only result types remain Dynamic; no inferred conversion unwraps Error
or formats Binary/GUID/DATE cells through this extension.

Conversion precedes each set stage and final ordering, using the complete flat
UNION result descriptor. Thus `SELECT 1 UNION SELECT '1'` returns one Text row
under explicit invariant formatting and ordinal comparison; UNION ALL retains two.
An empty Text arm still supplies its descriptor and can require numeric formatting
for other rows. An empty numeric arm performs no conversion and requires no policy.
Preparing once does not capture NumberFormat; each execution can render different
separators and may produce different duplicate sets.

When the complete result is Text, individual numeric cells are rendered directly.
For example, a flat LongLong/Double/Text UNION retains all LongLong digits instead
of first rounding it to the intermediate two-arm Double descriptor. Already
materialized nested queries retain their own prior result conversion. Numeric
arithmetic and the all-numeric UNION matrix are otherwise unchanged.

CStr allocations charge the shared query budget before fallible allocation; already
owned Text is retained in place. No parallel converted snapshot or second row
vector is introduced. Resource limits, cancellation, eager failures and buffered
publication remain active. Existing contributing-field text identities and prefix
set-stage policy routing are retained; final sort/group consumers use their normal
output provenance. Required unknown/conflicting policies still fail without a
fallback. UNION ALL without comparisons does not activate a comparer unnecessarily.

Successful numeric/Boolean-to-Text rendering now supplies CStr's fresh Default
policy and complete runtime provenance. A numeric expression's latent policy from
an unselected String arm no longer conflicts with the actual Text arm. An arm
selecting `IIf(True,2,B)` can sort with actual Text from A under A's policy even
when B has an incompatible identity. Explicit CStr of that number already had this behavior. Actual Text,
NULL and empty-arm fallback retain their previous authority, and prefix DISTINCT
stages remain written-order operations. See [runtime policy provenance](RUNTIME_TEXT_POLICIES.md#numeric-union-rendering).

Microsoft's [Access SQL overview](https://support.microsoft.com/en-us/access/access-sql-basic-concepts-vocabulary-and-syntax)
explicitly permits Number/Text compatibility in UNION. It does not establish our
Text dominance, rendering/precision, complete first-arm effects or policy precedence.
These remain provisional implementation choices pending Windows Access comparison.
Broader non-numeric and Variant promotion remain open.

The full all-feature query suite passes, together with updated eight-test Text
conversion and nineteen-test collation targets. Controls cover all numeric families
in both arm orders, explicit formats, exact full-width integer property tests,
empty/NULL/parameter/named/derived/group/crosstab consumers, exact allocation-byte
accounting, every observed cancellation checkpoint and refusal boundaries.
All-target/all-feature query Clippy, the minimal query build and CLI build pass.
The original [compare_text_unions.py](testing/compare_text_unions.py) passes 216
controls against Python exact/IEEE/rendering models and read-only native getters.
Four priority header families have unchanged sequential/derived/indexed inputs;
qualified stored General field routing is included. Existing 180 numeric and 109
prior UNION controls also pass. These are independent implementation checks,
not native Access SQL, regional or save/reopen qualification.

```sh
python3 docs/testing/compare_text_unions.py --cli target/debug/tablecraft \
  --jar path/to/reviewed/jackcess-5.0.4.jar \
  --native-directory path/to/registered-unicode-sources \
  --indexed-directory path/to/registered-large-index-inputs
```


Known [query Nz output types](QUERY_EXECUTION.md#query-nz-and-checked-conversion-output-metadata)
now participate in the same common-carrier selection. An omitted fallback over
NULL/Text is known Text; nullable numeric inputs with an omitted fallback remain
Dynamic. Nz(NULL) UNION ALL 7 therefore renders 7 through the explicit CStr policy,
without changing the source value or adding a new UNION promotion rule. Checked
numeric conversions contribute non-null successful descriptors; outer NULL arms
and other NULL-producing operators still relax the common result.
