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
paired with String remains TypeMismatch; Missing retains its unsupported
comparison-state contract. Host Empty has the explicit admission below. NULL
propagation does not fabricate empty Text or store an expression-only state as NULL.

Under the explicit declared-arithmetic profile, two caller-established Variant
operands retain the reviewed String/numeric category rule: numeric values precede
Strings independently of contents. StringBytes now supplies the same String
identity without decoding. The shared variant_expression_text_numeric service
never infers declarations from returned values, rejects non-finite numeric inputs
and leaves Boolean/Date outside this exception. Unknown/Scalar categories cannot
activate it. Default materialized comparison behavior remains unchanged.

Non-NULL String/String comparisons now use complete raw units with explicitly
selected [UTF-16 comparison providers](RAW_STRING_COMPARISON.md). Scalar-only
providers and mixed numeric/date coercion retain strict Unicode publication. Runtime
and static text policies, declarations, effective numeric selection, limits and
cancellation stay in their existing services. This adds no Unicode replacement,
implicit binary comparison, locale provider or raw set/sort key profile.

LIKE/NOT LIKE also retain both operands until matching. Raw String/NULL pairs
produce NULL without decoding or consulting a collation, including odd ranges
and unpaired units. The host must still explicitly select ANSI-89 or ANSI-92;
missing mode remains LikeModeRequired. Both argument expressions evaluate before
the NULL gate. Non-NULL raw matching now uses the existing UTF-16 literal/class
callbacks when the chosen provider explicitly supports that profile. Complete
units, including isolated surrogates and unaligned ranges, retain their contents.
Odd byte counts remain refused. Scalar-only providers still require strict
Unicode publication. See [pattern contracts](PATTERN_MATCHING.md). Native locale
providers, wider native matching qualification and raw key semantics remain
unfinished.

Focused tests cover all six comparisons, state/eager failures, lists/ranges,
group/filter/subquery consumers, empty sets, issued declarations, zero decoded
payload budget and cancellation at every observed query checkpoint. The original
[independent runner](../../testing/compare_raw_string_predicates.py) passes 7,113
source-free NULL/category/refusal controls. Retained-cell and native-policy recipes
also pass; only TableCraft evaluates SQL. Native Windows Access qualification and
benchmarks were not run.

The independent [raw LIKE runner](../../testing/compare_raw_like.py) additionally passes
2,745 unit-count, exact byte-slice, NULL/refusal and existing General expansion
controls under both explicit General families and ANSI modes. A generated
exhaustive unit-glob model checks the pure service independently of its state
machine; focused tests cover raw classes, limits and every observed cancellation
checkpoint. Neither model is native Access execution evidence.

## Host Empty comparisons

The expression API now admits caller-supplied Empty at the relational boundary.
Empty beside a numeric/Boolean or legacy Date operand selects zero before the
existing checked effective-type service. Empty/Empty uses Integer zero. Empty
beside Text or retained String selects empty Text and keeps the selected comparison
provider; it does not parse a String as numeric zero. Both operand orders and all
six operators use this admission. The [MS-VBAL relational table](https://learn.microsoft.com/en-us/openspecs/microsoft_general_purpose_programming_languages/ms-vbal/f8acd631-55c1-4199-bc1e-022aaab6d9c8)
supplies the effective-type rules; this is not native Access execution evidence.

Empty/NULL returns Unknown. Empty/Error remains TypeMismatch, Missing remains
refused, and unsupported stored carriers do not become zero. Raw complete units
can reach an opted-in UTF-16 provider without Unicode publication; odd ranges
remain refused. Missing comparers, nonfinite values, byte/work limits and
cancellation propagate through their owning services. No stored Empty carrier,
parameter, SQL constructor or implicit binary/text provider is introduced.
Query Nz(NULL) retains its existing empty-Text result.

Six focused tests cover the scalar families, raw and decoded Strings, isolated
units, state boundaries, quotas and every observed raw-comparison cancellation
checkpoint. A generated independent zero/sign model checks all six integral
predicates in both orders. Stored/query Empty provenance and broader Variant,
native comparison and result-type qualification remain separate acceptance work.
