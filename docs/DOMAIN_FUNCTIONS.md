# Read-only domain functions

Bound `query run` retrieval implements DAvg, DCount, DFirst, DLast, DLookup,
DMin, DMax, DSum, DVar, DVarP, DStDev and DStDevP. Each accepts two or three
expression arguments: expression text, domain name, and optional criteria text.
This is an implemented partial profile, not complete Access qualification.
Source-free `query eval` and UPDATE do not gain domain access.

```sh
tablecraft query run --database example.accdb \
  "SELECT DCount('*','Orders','Amount>100') AS OrdersOver100"
```

## Source and evaluation

The domain resolves only through the caller-selected `Source`, as a local table
or a parameterless named SELECT/UNION/fixed-schema crosstab. Transitive declared
query parameters are rejected even if the outer query has matching bindings.
No outer parameters or fields are implicitly captured inside the strings. Outer
argument expressions can explicitly construct the strings using their own bound
fields and parameters. Preparation captures those dependencies without resolving
dynamic domain names or scanning rows.

All supplied arguments evaluate in written order before string validation. This
profile requires Text carriers; explicit NULL is InvalidNullConversion and other
carriers are TypeMismatch. Omitted or whitespace-only criteria select the whole
domain. Criteria contain a scalar expression without WHERE. A missing criteria
field returns NULL; missing expression fields, source failures and unsupported
syntax remain errors. Subqueries and object/member access inside strings are
rejected before binding, so their lookup failures cannot masquerade as missing
criteria fields. SQL and domain aggregates inside expression text are rejected.
Criteria may contain supported domain calls under the same recursion limits.

Runtime parsing, binding, scans, aggregate buffers and results share the query's
cumulative allocation/work/row limits and cancellation callback. Parsing has the
existing syntax ceilings, binding retains its per-plan ceilings, and nested
domain execution is limited to the named-query depth ceiling. Calls do not create
fresh public executions or reset quotas. No result cache is introduced. Payload-free
operation spans contain no expression, domain, criteria, parameter or row data.

## Results and text policies

The implementation reuses ordinary aggregate kernels. DCount('*',...) counts
rows; other DCount expressions omit NULL and currently return Int64. Empty counts
return zero. Other empty aggregates return NULL. DFirst/DLookup and DLast use
first/last source encounter, including NULL, and evaluate the expression over all
selected rows. Encounter order is not an Access ordering guarantee. A unique
DLookup criterion avoids depending on it. Numeric and statistical results follow
the existing [aggregate profile](STATISTICAL_AGGREGATES.md), including DVarP's
fewer-than-two-values NULL boundary. DStDevP also requires two values and returns
NULL for a singleton, separately from SQL StDevP's singleton-zero profile.

Criteria and DMin/DMax use captured domain field text policies through the supplied
resolver. Returned domain Text has default static provenance in the enclosing
plan: dynamic string input does not establish a statically bound field lineage.
Unknown required policies fail without fallback. Native reads retain the existing
stable/offline input, source schema validation and explicit collection opt-ins.
Ordinary scalar subsets of mixed tables work without expanding unused children.
Direct collection/member references inside domain strings remain unsupported.

## References and remaining work

Microsoft's [DCount](https://learn.microsoft.com/en-us/office/vba/api/access.application.dcount)
and [DLookup](https://learn.microsoft.com/en-us/office/vba/api/access.application.dlookup)
references define parameterless domains, criteria, NULL/count and lookup behavior.
Its [DVarP](https://learn.microsoft.com/en-us/office/vba/api/access.application.dvarp)
and [DStDevP](https://learn.microsoft.com/en-us/office/vba/api/access.application.dstdevp)
references define the small-population boundaries. They also describe broader form,
control and supplied-function contexts that this read-only implementation does
not supply. External providers are not opened merely by naming a domain.

Remaining work includes broader criteria/subquery and collection semantics,
applicable explicit callable/host contexts, native Variant coercion and subtype
qualification, optional NULL/empty criteria behavior, error/evaluation order,
encounter order and mixed-result text precedence. Windows comparisons remain
pending at the user's direction. Current Rust/CLI controls establish the stated
profile; they do not establish native Access execution equivalence.
