# Predicates over retained String data

The six relational operators now retain String operands until comparison.
String/NULL pairs return Unknown without Unicode publication, numeric formatting
or locale lookup. This includes odd byte ranges and unpaired UTF-16 units: they
are String data, not operational failures. Both operands still evaluate eagerly
in written order. Invalid constructors and expression failures remain failures.

IN/NOT IN lists, BETWEEN/NOT BETWEEN and query membership/ANY/ALL share this
comparison boundary. They preserve existing three-valued accumulation and
candidate/range evaluation. Empty subqueries retain ANY=False and ALL=True without
decoding an otherwise unused String operand. Query projections, WHERE/HAVING,
groups and correlated/derived rows use the same retained-cell path.

```sh
tablecraft query eval 'SELECT ChrW(55296)=NULL,ChrB(65) IN(NULL)'
tablecraft query run 'SELECT S=ANY(SELECT NULL) FROM (SELECT ChrW(55296) AS S) AS Q'
```

These return NULL. `ChrW(55296) IN(NULL,1/0)` still fails division by zero. Error
paired with String remains TypeMismatch; Missing/Empty retain their unsupported
comparison-state contract. NULL propagation does not fabricate empty Text or
store an expression-only state as NULL.

Under the explicit declared-arithmetic profile, two caller-established Variant
operands retain the reviewed String/numeric category rule: numeric values precede
Strings independently of contents. StringBytes now supplies the same String
identity without decoding. The shared variant_expression_text_numeric service
never infers declarations from returned values, rejects non-finite numeric inputs
and leaves Boolean/Date outside this exception. Unknown/Scalar categories cannot
activate it. Default materialized comparison behavior remains unchanged.

Non-NULL comparisons outside that exception still publish raw Strings strictly
before existing numeric/date coercion and policy-aware Text comparison. Runtime
and static text policies, declarations, effective numeric selection, limits and
cancellation stay in their existing services. This adds no Unicode replacement,
implicit binary comparison, locale provider or raw set/sort key profile.

LIKE/NOT LIKE also retain both operands until matching. Raw String/NULL pairs
produce NULL without decoding or consulting a collation, including odd ranges
and unpaired units. The host must still explicitly select ANSI-89 or ANSI-92;
missing mode remains LikeModeRequired. Both argument expressions evaluate before
the NULL gate. Non-NULL raw matching still requires strict Unicode publication
before the existing pattern/policy service. Broader raw matching, native locale
providers and key semantics remain unfinished.

Focused tests cover all six comparisons, state/eager failures, lists/ranges,
group/filter/subquery consumers, empty sets, issued declarations, zero decoded
payload budget and cancellation at every observed query checkpoint. The original
[independent runner](testing/compare_raw_string_predicates.py) passes 7,113
source-free NULL/category/refusal controls. Retained-cell and native-policy recipes
also pass; only TableCraft evaluates SQL. Native Windows Access qualification and
benchmarks were not run.
