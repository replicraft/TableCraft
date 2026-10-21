# HyperlinkPart query components

`HyperlinkPart(value,part)` now extracts Text from stored hyperlink strings in
scalar and bound queries. Microsoft's
[query example and method contract](https://learn.microsoft.com/en-us/office/vba/api/access.application.hyperlinkpart)
require both arguments in queries, even though the VBA method permits an omitted
selector. SQL takes numeric selectors, not implicit application constants.

| Selector | Result |
| --- | --- |
| 0 | Displayed value: first nonempty display text, address or subaddress |
| 1 | Explicit display text |
| 2 | Address |
| 3 | Subaddress |
| 4 | Screen tip |
| 5 | Full address, including a hash and nonempty subaddress |

The selector values follow Microsoft's
[AcHyperlinkPart enumeration](https://learn.microsoft.com/en-us/office/vba/api/access.achyperlinkpart).
Missing components yield empty Text. An internal-only full address retains its
leading hash. Components retain their original spaces, case and Unicode.

```sh
tablecraft query eval "SELECT HyperlinkPart('Example#https://example.invalid#anchor#Tip',5)"
tablecraft query run 'SELECT HyperlinkPart(Link,2) FROM Hyperlinks' --database example.accdb
```

The first query returns Text `https://example.invalid#anchor`. This is component
extraction; no URI interpretation, normalization or resource access occurs.

## Current policies and limits

The kernel borrows slices delimited by the first three hashes. Remaining hashes
belong to screen-tip text in this profile. Bare Text occupies the display-text
component; incomplete strings retain absent empty components. These edge policies,
including extra delimiters, are not native Access qualification.

The selector uses existing checked Long/half-even conversion. Values outside
0..5 return Domain; NULL or unsupported selector carriers retain conversion
errors. A valid selector is required before a NULL hyperlink propagates NULL.
Other hyperlink carriers fail TypeMismatch. Full Variant coercion and native
NULL/error precedence remain open.

Every supplied operand evaluates eagerly in written order. One argument, missing
slots, wildcard arguments, extra arguments and qualified names fail before operand
execution. Binding reads metadata only. Ordinary dependency, correlation,
aggregate/domain and grouping walks apply; Text comparisons retain the existing
explicit collation requirements and fresh-text-result policy.

The `functions::text::hyperlink` slice owns the pure borrowed kernel and selector
type. It checks cancellation/work while finding delimiters, and the evaluator
charges output before fallible copying. Source/output ceilings and buffered
failure behavior remain unchanged. No new carrier, dependency, runtime or provider
was added. HyperlinkPart remains **Partial** in the [tracker](ACCESS_BUILTINS.md).

## Evidence

Kernel tests cover all selectors, display fallback, empty/internal-only addresses,
Unicode and retained tip hashes, numeric selectors, NULL, output limits and every
observed scan-cancellation checkpoint. A fixed-seed 256-case property checks
generated components. Query/CLI tests cover grouping, correlations, filtering,
aggregates, plan reuse, query-specific arity, eager failures and buffered stdout.

The original [fixture recipe](testing/HyperlinkFixture.java) writes actual
Hyperlink-marked Memo fields. Its [independent runner](testing/compare_hyperlink.py)
compares Jackcess-decoded fields with TableCraft's six component projections for
Jet 4/ACE 2007/2016/2019 files, including NULL/empty/Unicode values and unchanged
source hashes. Jackcess does not execute the function. Native Access evaluation,
field creation/save interoperability and broader malformed-field behavior remain
unqualified. [Fixture provenance](FIXTURES.md#original-hyperlink-field-values)
records original hashes; generated databases stay untracked.
