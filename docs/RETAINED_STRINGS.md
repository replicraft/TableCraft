# Lossless Strings in query cells

Query projections and intermediate rows now retain lossless String bytes through
`ExpressionValue::StringBytes(ExpressionString)`. This closes premature Unicode
publication in derived tables, UNION ALL, correlated scalar subqueries and First/
Last results. Byte/unit observations, String inspection, NULL tests, selectors,
concatenation and binary StrComp can consume those cells through their existing
lossless services. For example:

```sh
tablecraft query run "SELECT LenB(S),VarType(S) FROM (SELECT ChrB(65) AS S) AS Q"
tablecraft query run "SELECT Len(S) FROM (SELECT CStr(ChrW(55296)) AS S) AS Q"
```

The first query returns 1 and 8; the second returns 1. The intermediate String
does not have to become ordinary Text before its length or type is observed.
Metadata-only preparation still performs no row scans or publication.

Stored `Value`, source snapshots, parameters and write contracts remain unchanged.
Core conversion to Value explicitly rejects StringBytes with its own non-storable
category. Representational equality compares active bytes, without implying SQL
equality or collation. Core Clone copies active bytes only; callers own allocation
budgets. Query copies instead charge before fallible allocation and check shared
work/cancellation while copying bounded chunks. Policy and declaration metadata
travel with the retained cell. No Serde/wire format is added.

Ordinary scalar consumers request explicit Unicode publication. Valid ranges
become Text under the existing text/byte/work limits; odd byte counts and unpaired
surrogates fail UnrepresentableText. DISTINCT/UNION duplicate removal and scalar
sort/group keys retain their existing ordinary-Text comparison profile rather
than silently ordering unsupported raw data. Broader raw predicate/key semantics
and native Access qualification remain unfinished.

Both public query result modes retain strict final String publication. A raw
String may survive intermediate rows, but a directly selected unrepresentable
String still fails before buffered results reach the caller/CLI. Explicit Error
cells remain available through execute_expression. The schema-discovery entry
point shares the same final publisher, text limit and operation budget.

Expression hosts can explicitly use Evaluator::publish_expression_value or its
borrowed String publication service before materialized scalar APIs. Raw-aware
consumers can move StringBytes through ExpressionByteOperand::try_from without
decoding. Other scalar APIs retain explicit refusal of unpublished carriers.

Focused controls cover retained odd/surrogate cells, UNION ALL, correlations,
First/Last, valid scalar/set boundaries, strict final output, parameter rebinding,
byte quotas and every observed cancellation checkpoint. The original independent
[cell runner](testing/compare_retained_string_cells.py) passes 1,053 source-free
controls. Existing byte/selection/comparison/concatenation and native-policy
recipes also pass. These are implementation/model comparisons; native Windows
Access execution remains unrun.
