# Exact mixed floating relational keys

Relational keys now compare a mixed Single/Double pair through exact binary64
views. Every finite binary32 value widens exactly; the view changes neither
stored cells nor selected result carriers. Grouping, DISTINCT, dynamic UNION,
ordering/TOP, extrema, domain extrema and crosstab keys share this boundary.
Scalar predicates and prepared static UNION promotion retain their separate
contracts.

Previously, scalar narrowing could make Double 1 equal Single 1, and Single 1
equal Double 1.00000001, while the two Doubles differed. Such pairwise results
cannot define a consistent partition. Encounter order could change groups,
duplicates, extrema and TOP ties. A finite large Double could also overflow when
compared with Single. Exact key widening admits that finite mixed pair without
changing the scalar narrowing/refusal rule.

Microsoft's [GROUP BY contract](https://support.microsoft.com/en-us/access/group-by-clause)
describes grouping identical values. Its
[scalar relational table](https://learn.microsoft.com/en-us/openspecs/microsoft_general_purpose_programming_languages/ms-vbal/f8acd631-55c1-4199-bc1e-022aaab6d9c8)
selects Single for Single/Double scalar comparisons; that behavior remains.
[Numeric Let-coercion](https://learn.microsoft.com/en-us/openspecs/microsoft_general_purpose_programming_languages/ms-vbal/56fb78c3-a0ea-4bf0-8ee4-eb5f89bda4f7)
describes representable conversion and rounding. These references distinguish
the scalar contract and justify exact widening; they do not establish how native
ACE infers a common grouping type for heterogeneous Variant expressions. This
is an explicit coherent engineering key profile pending Access comparison.

## Independent model and retained boundaries

[The original Python runner](compare_numeric_keys.py) decomposes finite IEEE
bit patterns into exact `Fraction` values. Fixed-point inputs use exact signed
coefficients/scales. It compares complete partitions, exact key values, ordering,
maximum tie membership and extrema across all six encounter permutations and
both default/declared arithmetic profiles. Nine independent goldens check IEEE
normal/subnormal boundaries, signed zero and decimal ties-to-even.

The fifteen source-free families include normal/subnormal/large mixed floats,
signed zero, fixed-point aliases, adjacent Currency values, adjacent large
integers, Boolean/Byte distinctions, integral/floating exact inputs, Date aliases,
raw/ordinary Text and strict cross-family refusals. Boolean TRUE retains numeric
-1; it does not become Byte 255. Numeric Text remains Text despite an explicit
number-format policy. Currency/Decimal with floating keys remains unsupported;
large integer/floating comparisons retain their exact-conversion guard.

The observer projects `VarType(K)` and `CDbl(K)` outside floating key selection,
including outside DISTINCT/UNION. VarType never becomes part of the duplicate
key. This recovers exact widened floating bits without inferring keys from
ambiguous shortest Single strings. Fixed-point observers retain exact decimal
output. A representative may preserve any original carrier belonging to its
class; ORDER/TOP observers also verify each original row's carrier and zero bits.

Source-free runtime selectors use `IIf(CBool(True),...)`. The cast condition is
not a literal selector refinement, so mixed runtime carriers remain dynamic.
Plain `IIf(True,...)` would establish a static selected type and could promote
UNION before testing the intended key path. Explicit separate static UNION
controls verify existing widening to Double. Scalar controls still make
Single 1 equal Double 1.00000001 and still refuse scalar Single/large-Double
narrowing with Overflow.

## Original native fields

[NumericKeyFixture.java](NumericKeyFixture.java) authors twelve inert rows per
Jet 4/V2000, ACE 2007/V2007 and ACE 16-family V2016/V2019 file through public
table/column builders. Int32 Family/Ordinal fields and Boolean Kind select
nullable SingleValue/DoubleValue columns. The unused floating column is NULL.
The four families are normal aliases, subnormal aliases, large finite distinct
values and signed zeros.

[NumericKeyOracle.java](NumericKeyOracle.java) opens each original read-only,
disables expression evaluation and refuses link resolution. It records public
schema getters and raw IEEE cell bits, with zero resolver calls. The native model
uses those actual getter bits, not cast-expression guesses. Single 3e38 differs
from Double 3e38 even when their short labels coincide; Double 1e300 supplies a
third large key. Original selected carriers and signed-zero bits remain visible.

Nested integer-ordinal ORDER BY controls the encounter order before native
grouping, sorting, TOP, extrema and crosstabs. Typed dynamic UNION observers use
three ordered one-row native SELECT arms and an empty DISTINCT arm to stay
within the existing binding-depth ceiling. Native DMin/DMax, scalar predicates
and static UNION run directly against the same original fields. No reference
SQL, imported query or active database content executes.

Numeric pivot labels have their own retained invariant-format contract: Single
uses seven significant digits and Double fifteen. Independent exact rational
rounding accepts the label of any original representative in that key class;
native carrier tags come from actual getters and typed observers. Labels alone
never establish exact key bits. The three
large native keys partition correctly, but the first two format to the same
heading name. All 48 such dynamic column-pivot controls retain AmbiguousName;
row-key crosstabs still produce three groups. A label collision never merges keys
or removes a case from the report.

## Accepted replay and earlier audit

The [2026-10-24 final report](results/numeric-keys-2026-10-24.json) retains
2,910 candidate controls: 1,326 source-free and 1,584 native controls, plus
144 baseline observations and nine model goldens. The native controls are 396
per original file. Candidate outcomes comprise 2,524 ordinary exact-model
successes, eight correct per-comparison-domain successes, 330 retained comparison
or scalar refusals and 48 heading-name refusals. All candidate outcomes agree
with their explicit model; all 25 selected hashes remain unchanged.

The candidate is `.cache/tablecraft-bin/numeric-keys-cli-final-v2-20261024`,
SHA-256 `e36fd5c168fb48e2b6eed849a97d29d44e7bdb46da2ba55372106a51bdc4ad02`.
The comparison baseline is `.cache/tablecraft-bin/external-native-cli-final-20261024`,
SHA-256 `2fea30893b358d283b895049abc47d484d946e8232e8359dc6eed175fdcf1a83`.
The log is `.cache/numeric-keys-final-replay-20261024.log`. Exact commands,
getters, source/input hashes and runner arguments are in the report; choose a
fresh report path to replay because the runner refuses evidence overwrite.

The report also preserves the original 720-command audit unchanged, including
its older executable hash. Its 70 binary-family disagreements remain distinct
from two classification caveats: 48 fixed/float cases refused correctly with
different diagnostic wording, and four TOP cases correctly returned a maximum
without comparing the unsupported integer/floating pair. Those are not 52
additional demonstrated key defects.

The new 144 baseline observations use stronger typed/value observers. They find
78 disagreements: 22 normal, 22 subnormal and 34 large floating cases. The eight
additional findings distinguish incorrect membership from merely matching
sorted group-size counts. This is a different observation set, not a rewritten
count for the original audit. Correct large-integer/floating ORDER/TOP successes
remain explicitly classified: guards apply to actual comparisons rather than
an unconditional whole-domain preflight.

Earlier observer probes stopped at the existing binding-depth ceiling or tried
to decode shortened heading labels as exact keys. They are excluded; the final
runner preserves typed observers and separately models naming. The successful
2,856/144-control probe precedes the final added domain/scalar/static controls
and is also excluded from accepted counts. No production behavior was weakened
to satisfy those observer corrections.

Original recipes/oracle/model are repository MIT code. The reviewed Jackcess
5.0.4 JAR is Apache-2.0, SHA-256
`19a7494a56f439a57fda85c059b019b665802085cec5ae8186d1e028cc1bdfbc`.
Original databases, getter records, JAR, executables and local probes remain
ignored. No third-party implementation or fixture bytes are copied or tracked.
There are no writes to originals. Native Access grouping, broader numeric
coercion, NULL/Error qualification and application save/reopen are not established
by this independent finite-key corpus.
