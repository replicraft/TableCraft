# Numeric relational keys

Relational keys compare mixed Single/Double values through exact binary64 views.
A finite Single widens exactly; the Double retains every stored bit. This gives
successful binary key comparisons one consistent numeric order and equivalence
relation. It applies to GROUP BY, DISTINCT, UNION duplicate removal, ORDER BY,
TOP ties, extrema and crosstab heading/group comparisons. It does not convert
result cells or change their retained carriers, declarations or text policies.

Scalar predicates retain their separately documented
[effective numeric comparison rules](NUMERIC_PREDICATES.md). In particular,
Single/Double scalar comparison can round Double to Single. A scalar comparison
can therefore report equality where strict relational keys are distinct. This
separation is intentional: pair-dependent rounding does not form a transitive
equality relation over a complete heterogeneous key domain.

## Why the distinction matters

Consider the runtime values Double `1`, Single `1` and Double `1.00000001`. The old key
comparison considered the Single equal to either Double, while the two Doubles
were distinct. Depending on encounter order, GROUP BY and DISTINCT produced one
or two groups; Min/Max could miss the distinct larger Double; TOP 1 with descending
ordering could retain one, two or three rows. Changing only the first retained
representative could therefore change cardinality and aggregate membership.

Exact binary key views put Double `1` and Single `1` in one class, and
Double `1.00000001` in another. The first representative and output row order retain
their existing rules, but the equality classes no longer change with encounter
order. Both signed zero representations remain numerically equal. Subnormal
values keep their exact magnitude. A finite Double beyond Single's range no
longer fails merely because an unrelated Single key would force lossy narrowing.

This is an explicit TableCraft engineering key profile. Microsoft's
[relational effective-type table](https://learn.microsoft.com/en-us/openspecs/microsoft_general_purpose_programming_languages/ms-vbal/f8acd631-55c1-4199-bc1e-022aaab6d9c8)
remains the scalar reference. Its pairwise rules do not establish native
heterogeneous Variant grouping. The
[numeric coercion contract](https://learn.microsoft.com/en-us/openspecs/microsoft_general_purpose_programming_languages/ms-vbal/56fb78c3-a0ea-4bf0-8ee4-eb5f89bda4f7)
and exact binary32-to-binary64 representation support lossless widening; the
[Access GROUP BY contract](https://support.microsoft.com/en-us/access/group-by-clause)
describes grouping identical key values. Native Access qualification of this
heterogeneous key profile remains unrun.

## Preserved family boundaries

| Participating values | Key behavior |
| --- | --- |
| Boolean/integral/Currency/Decimal, without binary floats | Existing exact coefficient/scale comparison |
| Same Single or same Double | Existing finite numeric comparison |
| Mixed Single/Double | Exact Single-to-Double key view; Double unchanged |
| Integral with binary float | Existing exact integer-to-Double admission; an inexact integer conversion still fails PrecisionLoss |
| Currency/Decimal with binary float | Existing unsupported numeric combination |
| Legacy Date with legacy Date | Existing temporal order, including equivalent negative-fraction aliases |
| Extended Date with Extended Date | Existing date/time tuple order |
| Date with numeric/Text, or numeric with Text | Existing strict type refusal; no implicit parsing |
| Text/raw String | Existing explicit provider and raw-unit contracts |
| NULL/Error/expression-only states | Existing owning admission, equality and ordering rules |

Boolean TRUE remains numerically -1 for keys; Byte 255 does not become equivalent
through CByte's explicit Boolean exception. No automatic NumberFormat, locale or
date policy enters a key comparison. Non-finite values still fail the existing
finite checks; producing comparison views is not an admission or validation gate.
The existing integer/float guards apply when those values are actually compared,
not as a newly introduced global preflight of every possible key pair.

Static UNION common-type promotion retains its existing authority. Statically
known Single/Double arms already widen to Double before duplicate removal,
including when one arm has no rows. Dynamic expressions retain their actual
carriers; their comparison views do not invent a static type or rewrite them.
Prepared and completed output descriptors retain their selected profile contracts.

## Implementation and verification

`execution/keys/numeric.rs` owns the narrow mixed-float view helper. The shared
runtime relational comparison boundary uses those views with the existing strict
comparer. Retained key comparisons and scalar crosstab key helpers both use this
boundary; ordinary predicates keep their original service. No payload allocation,
new dependency or source scan is introduced. Existing cancellation, work checks,
provider routing, eager failures and buffered result publication remain intact.

The focused unit tests reconstruct IEEE binary64 fields from binary32 sign,
exponent and fraction fields without a floating-point cast. Explicit independent
bit goldens cover both zero signs, minimum/maximum subnormals, minimum normal and
signed maximum finite Single values. Five generated properties use 512 cases each:
exact widening, equality/order agreement and symmetry, transitivity, adjacent
Double distinctions around a Single value, and mixed binary pairs with an exact
Int32 third value in every order. An independent integer rank over IEEE fields
supplies expected order. Carrier restrictions and mixed/same-family NaN/Infinity
controls preserve the existing strict admission boundaries. Scalar narrowing is
pinned separately from the new key view.

Query integration controls exercise consumers, encounter orders, policies,
metadata and eager/resource failures in their owning query test target. These
controls and the independent engineering models do not substitute for Windows
Access comparison evidence.
