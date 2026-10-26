# Numeric relational keys

Relational keys compare Boolean, integral, Currency, Decimal, Single and Double
values by exact numeric magnitude. Mixed Single/Double values use exact binary64
views; fixed/binary pairs use bounded integer comparison without rounding either
operand. This gives numeric key comparisons one consistent order and equivalence
relation. It applies to GROUP BY, DISTINCT, UNION duplicate removal, ORDER BY,
TOP ties, extrema and crosstab heading/group comparisons. It does not convert
result cells or change their retained carriers, declarations or text policies.

Scalar predicates retain their separately documented
[effective numeric comparison rules](../../expressions/numeric/NUMERIC_PREDICATES.md). In particular,
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
| Integral with binary float | Exact signed integer/binary magnitude, including values beyond 2^53 |
| Currency/Decimal with binary float | Exact coefficient/scale versus binary significand/exponent comparison |
| Legacy Date with legacy Date | Existing temporal order, including equivalent negative-fraction aliases |
| Extended Date with Extended Date | Existing date/time tuple order |
| Date with numeric/Text, or numeric with Text | Existing strict type refusal; no implicit parsing |
| Text/raw String | Existing explicit provider and raw-unit contracts |
| NULL/Error/expression-only states | Existing owning admission, equality and ordering rules |

Boolean TRUE remains numerically -1 for keys; Byte 255 does not become equivalent
through CByte's explicit Boolean exception. No automatic NumberFormat, locale or
date policy enters a key comparison. Non-finite values still fail the existing
finite checks; producing comparison views is not an admission or validation gate.
Finite checks apply when values are actually compared; no new global preflight
of every possible pair is introduced. Scalar predicates retain their separate
precision and effective-type rules.

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

## Exact fixed/binary numeric cohorts

The strict key service now admits fixed/binary pairs that previously failed
UnsupportedNumericCombination or PrecisionLoss. Exact Decimal/Currency `0.1`
forms one class; the slightly larger stored Double `0.1` forms a separate class.
LongLong `9007199254740993` sorts above Double `9007199254740992`, retaining its
integer value. Sorting, grouping, DISTINCT, dynamic UNION, TOP ties and aggregate/
domain extrema use this same boundary. Result cells and their first encountered
representatives retain their actual carriers. Scalar predicates and static UNION
common-type conversion retain their existing authority.

The implementation compares `c / 10^s` with `m * 2^e` by comparing `c` with
`m * 5^s * 2^(e+s)`. Binary64's 53-bit significand times `5^28` needs at most
119 bits, so the current Decimal/Currency envelope fits in u128. Compile-time
bounds protect that assumption. Leading bit lengths decide unequal magnitudes
before shifting; equal-length shifts remain representable. Subnormal exponents
and maximum finite binary values therefore need neither oversized integers nor
payload allocation. Signs and both binary zero signs have explicit handling.
Existing resource/cancellation checks bracket each comparison; non-finite values
still fail, and unsupported Date/Text/Binary pairs retain their existing gates.

This is a strict relational comparison, not a new Decimal arithmetic engine.
Core validated carriers and rust_decimal arithmetic are unchanged. Independent
tests use the already adopted num-bigint/num-rational libraries to compare full
arbitrary fractions through a different algorithm. Three fixed-seed properties
run 512 cases each: full-range ordering/symmetry, transitivity through a binary
middle and exact dyadic Decimal equality with adjacent Double distinctions.
Literal goldens include decimal/binary rounding boundaries, 96-bit maxima,
subnormals, signed zeros, Boolean TRUE and Int64 limits.

Query tests cover all six encounter orders, both execution profiles, retained
carriers, groups, sets, sorting/TOP, extrema/domains, eager errors, shared limits
and every observed cancellation checkpoint. CLI checks cover visible group
carriers, exact large-integer output and empty stdout after a later failure.
This closes an engineering key-admission gap; native Access heterogeneous Variant
coercion/grouping and full P0 result promotion remain unqualified. See the
[scoped verification](../../testing/results/fixed-binary-keys-2026-10-25.json).


The [independent mixed-key replay](../../testing/FIXED_BINARY_KEYS.md) now passes 3,956
candidate queries and 333 earlier-executable observations across fifteen parameter
cohorts and four unchanged native scalar files. Exact Fraction/IEEE expectations
agree on 8,648 returned rows and 25,112 cells. All twelve immutable pins and
seventeen versioned/live source snapshots were separately rehashed. The report
keeps the existing depth-16 binder boundary and native Access qualification gaps
explicit; no input, runtime policy or expected numeric relation changed.
