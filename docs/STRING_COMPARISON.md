# StrComp and explicit comparison policies

`StrComp(left, right[, compare])` shares a closed text kernel between scalar
SELECT evaluation and bound retrieval, including grouping, correlations, reusable
parameters and qualified saved SELECTs. It returns Int16 -1/0/1 or NULL. Every
supplied argument evaluates in written order; NULL strings do not hide errors in
another argument. No VBA or external function is invoked.

## Modes and host context

| Compare argument | Current implementation |
| --- | --- |
| 0 | Allocation-free lexicographic comparison of UTF-16 code units; no host required. |
| 1 | Explicit host `Collation::compare_text` callback. No default locale is inferred. |
| 2 | Explicit host `Collation::compare` database-policy callback. |
| Omitted or -1 | Requires the host's declared `Collation::option_compare` policy for non-NULL strings. A supplied database comparer alone does not imply a module setting. |

An omitted trailing SQL argument slot means omission, not stored NULL. Explicit
NULL comparison mode is InvalidNullConversion. Other supplied modes undergo
checked half-even numeric-to-Int32 conversion; only -1, 0, 1 and 2 are accepted.
Unsupported mode codes return Domain; conversion overflow/type errors remain
visible. String operands currently accept Text/NULL only, without implicit number,
date, Empty or Variant conversion. After mode validation, either NULL string
returns NULL without requiring a comparison provider.

Mode 0 preserves embedded NUL, trailing spaces and canonically distinct Unicode
spellings. It compares UTF-16 units, so supplementary characters can order
differently from Rust Unicode scalar ordering. It does not slice strings or
construct unpaired surrogates. This is an explicit current binary profile,
**not qualification of every native Windows code-page/binary sort behavior**.

Library callers use `functions::text::apply_with_collation`; the existing `apply`
API remains available without a host. Binary scans charge work per compared pair.
Host modes scan/charge both inputs before calling the provider; the provider owns
its callback work/cancellation bounds. Evaluator inputs retain normal text-size,
step-budget and cancellation limits. Results allocate no Text payload.

The CLI's explicitly selected `--collation ordinal` supplies the database policy
and declares that policy for omitted/-1 modes. It compares Unicode scalar order,
which is intentionally distinct from mode 0's UTF-16 profile. It does not supply a
locale-aware mode 1. The explicit `general-legacy`/`general-2010` CLI choices
likewise supply database/default comparison through the provisional
[full-string weight model](NATIVE_TEXT.md#general-query-policy), under a separate
cumulative budget; they do not supply mode 1. Without an explicit policy,
non-NULL omitted/host modes fail rather than guessing a locale or native database
sort order.

```sh
tablecraft query eval "SELECT StrComp('ABCD','abcd',0) AS Comparison"
tablecraft query run "SELECT StrComp('ABCD','abcd',2) AS Comparison" --collation ordinal
```

Both return -1 in these ASCII examples. CLI results remain buffered; unsupported
modes/providers and late evaluation failures publish no partial table.

## Evidence and remaining compatibility work

Microsoft's [StrComp reference](https://learn.microsoft.com/en-us/office/vba/language/reference/user-interface-help/strcomp-function)
defines result codes, NULL behavior and comparison modes. Its
[Option Compare reference](https://learn.microsoft.com/en-us/office/vba/language/reference/user-interface-help/option-compare-statement)
distinguishes module defaults, system text locale and database locale, and notes
Windows code-page effects. TableCraft does not infer a VBA module or database
locale from an opened file. Native code-page binary behavior, full Variant
coercion, locale-aware text comparison and database collation remain incomplete.

Focused value/evaluator/query/CLI tests cover separate provider callbacks, omitted
slots versus NULL, exact result carriers, UTF-16 ordering, grouping, correlations,
parameter reuse, eager errors, budgets and cancellation. A generated property
compares the binary kernel to independently constructed big-endian UTF-16 buffers.
The original [reference recipe](testing/compare_strcomp.py) passes 85 controls
against Python UTF-16/ordinal models and read-only independently decoded native
rows, including sequential/indexed reads. The registered Access example's source
hash remains unchanged. These are independent model/read checks; Microsoft Access
execution on Windows remains unrun. See the [coverage tracker](ACCESS_BUILTINS.md).

[Substring search](STRING_SEARCH.md) reuses mode selection but has its own
`Collation::find` provider contract; supplying compare/compare_text alone does not
implement locale substring matching.


## Database and system locale are separate

Microsoft's Option Compare reference distinguishes Text (system locale) from
Database (database locale). `StrComp(...,1)` must therefore not inherit a database
General descriptor merely because one was selected for SQL predicates. The
[DAO CollatingOrder reference](https://learn.microsoft.com/en-us/office/client-developer/access/desktop-database-reference/database-collatingorder-property-dao)
also permits a field to have a different order from its containing database.
TableCraft preserves these separate inputs; it does not use the current machine's
preferred UI language as a database comparison setting. The current binary
function profile uses UTF-16 units rather than treating strings as ASCII.
