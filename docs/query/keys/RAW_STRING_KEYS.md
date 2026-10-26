# Retained String keys

Grouping, DISTINCT, active DISTINCTROW, UNION duplicate removal, ORDER BY and TOP
ties now retain expression String data until an actual comparison or final
publication needs it. They use the same separately selected raw-unit comparison
providers as [relational String predicates](../../expressions/text/RAW_STRING_COMPARISON.md).

Raw Strings stay String data, including odd byte ranges and isolated surrogates.
A singleton key does not invent a comparison, Unicode decoding or locale lookup.
For example, both queries return one without a comparison provider:

```sh
tablecraft query run "SELECT Count(*) AS N FROM (SELECT DISTINCT ChrW(55296) AS S) AS Q"
tablecraft query run "SELECT Count(*) AS N FROM (SELECT DISTINCT ChrB(65) AS S) AS Q"
```

An actual String/String comparison requires the selected provider's input
contract. UTF-16 providers accept complete units and refuse odd byte counts;
Unicode-only providers use strict publication. Required unknown/conflicting
field policies and callback failures propagate without fallback. A duplicate odd
String therefore still fails when its bytes must be compared. NULL key identity
and ordering need no String decoding: NULL equals NULL for grouping/set keys and
sorts before non-NULL values in ascending order.

Key comparison retains strict type rules, independently of predicate numeric/date
coercion and the explicit Variant String/numeric category exception. The
[numeric key contract](NUMERIC_KEYS.md) separately gives mixed Single/Double keys
exact widened views. Raw String/numeric keys fail TypeMismatch.
[Standard Error keys](../../expressions/VARIANT_ERRORS.md) compare their explicit codes without
becoming stored numeric data; mixed Error/scalar comparisons remain refused.
Empty/Missing and unsupported expression states remain non-scalar key failures.

Query runtime and static field policies travel with the retained cells. Existing
policy unification, encounter representatives, stable ordering, TOP ties and
UNION stage boundaries are preserved. Active DISTINCTROW compares the selected
sources' complete records; the single-source/all-source ignored cases are unchanged.
Operational failures in later expressions/rows remain eager, including rows that
TOP would omit. Copies, sort indices and callback unit buffers retain their
cumulative byte/work/cancellation controls; these are buffered query operations.

Crosstab row groups and sorting share this key service. Fixed-heading named
crosstabs now also retain raw row/cell Strings for outer raw-aware consumers.
Preparation remains metadata-only. Pivot/heading names retain their separate
strict scalar/Unicode contract, and inline TRANSFORM sources remain unsupported.
Context-dependent named schema discovery retains its existing publication limits.

Final String publication remains strict. `SELECT DISTINCT ChrW(55296)` still
fails before buffered results reach the CLI, even with General selected. An outer
LenB/AscW observer or numeric projection can consume raw intermediate keys without
publishing their contents. Stored/native Text, source/parameter admission, write
gates and index pushdown are unchanged.

The original [key runner](../../testing/compare_raw_string_keys.py) passes 406 independent
controls over all registered short General vectors in each family, plus NULLs.
Python supplies grouping, sorting, stable ties and first representative lengths
from the reviewed key bytes. It also checks duplicate removal, active DISTINCTROW,
singleton data, eager failures and strict final output. These are explicit model
controls, not Microsoft Access SQL execution evidence. The shared vector reader
uses balanced constructors and bounded projection batches; no engine/parser limit
was raised and no vector was dropped.

Focused tests additionally cover field policy resolution, every observed query
cancellation checkpoint, raw crosstab row sorting and metadata-only preparation
of fixed named crosstabs with raw cells. Existing comparison/LIKE/predicate,
retained-cell and native-policy recipes pass with registered native inputs
unchanged. [Raw Min/Max](../aggregates/EXPRESSION_AGGREGATES.md#retained-string-extrema) now reuse
this strict comparison boundary. Wider Variant/coercion and other built-in consumers,
additional locales and native Access qualification remain acceptance work.
