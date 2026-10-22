# Contextual string concatenation

Access `&` now concatenates supported scalar operands through the shared evaluator.
Text is retained exactly; Boolean uses `True`/`False`; numeric and legacy/extended
date operands reuse [CStr](CSTR.md). Numeric output requires an explicit
`NumberFormat`, including integer operands. Dates require `DateTextFormat`.
No ambient locale or CLI cell renderer supplies expression text.

```sh
tablecraft query eval "SELECT 'ID=' & 9007199254740993" --number-format invariant
tablecraft query eval "SELECT #2024-01-02# & '!'" --date-format iso
tablecraft query run "SELECT 'ID=' & ID FROM Contacts" \
  --database example.accdb --number-format invariant
```

One NULL operand contributes empty text; two NULLs return NULL. Empty Text remains
Text. The separate `+` path still concatenates Text/Text, propagates a participating
NULL and uses its existing contextual numeric rules for mixed numeric operands.
Both operands evaluate in written order before scalar formatting: a later division
by zero is not hidden by NULL or a prior operand needing an output policy.
Unselected conditional branches remain eager.

Binary and GUID implicit formatting remain unsupported; callers use explicit
qualified conversions where available. Expression-only Error, Empty, Missing and
byte-string/array concatenation remain outside this scalar extension. In particular,
Error is not rendered as CLI `#ERROR(...)` text inside an expression. Full native
Variant, byte-array and regional behavior remain compatibility gaps.

## Shared execution and metadata

Source-free evaluation, bound queries, named/derived consumers, grouping, nested
subqueries and native sequential/indexed scalar sources share the same evaluator.
Prepared plans retain no output locale; each execution uses its own number/date
policies. No database bytes, stored schema or parameter carrier changes.

For known supported scalar operands, `&` output metadata is unconstrained Text.
NULL is allowed only when both operands can be NULL; two null-only operands
establish null-only output. Unknown/unsupported families stay Dynamic. Inference
never formats values, reads rows or activates a provider. Operand Text constraints
are not reused as concatenated output length limits.

Metadata-only named crosstab binding still permits only its qualified Text/NULL
constant concatenations. A heading needing scalar formatting is Unsupported there,
so explicit execution-stage discovery resolves it under the current context and
shared budgets. Missing execution policies remain visible errors; binding does
not infer settings or scan data to invent headings.

## Resource and evidence limits

Already materialized Text/NULL operands are borrowed during the join. Scalar-to-
Text conversions charge their emitted intermediate bytes through CStr; the final
joined Text is charged separately before fallible allocation. Shared cancellation,
work and the existing 65,536-byte per-value ceiling remain active. Failed work is
not refunded. Queries and ordinary CLI output remain buffered, with no success
stdout on a later conversion, quota or formatting failure.

Focused tests cover exact LongLong/fixed-point digits, date and numeric profiles,
Boolean/NULL behavior, policy reuse, metadata-only sources, derived/grouped/nested
queries, named crosstab discovery, all observed conversion cancellation checkpoints,
intermediate/final allocation caps and unchanged input values. The original
[Python runner](testing/compare_concatenation.py) passes 190 controls through eval
and run, including unchanged native inputs and both directions of indexed seeks.
Python integer/Decimal/calendar models and read-only Jackcess getters provide
expectations. Exact Decimal tests use lexical CDec input to isolate them from the
separate floating SQL literal policy. [Provenance](FIXTURES.md#original-contextual-concatenation-controls)
records the existing input sets. No benchmark or speed claim is introduced.
Native Windows Access SQL, formatting, coercion and error-order comparisons remain
unrun; these controls establish the explicit implementation profile.

Microsoft's [Access expression examples](https://support.microsoft.com/en-us/access/examples-of-expressions)
establish `&` use in calculated query fields. The
[operator reference](https://learn.microsoft.com/en-us/office/vba/language/reference/user-interface-help/ampersand-operator)
and [MS-VBAL operator contract](https://learn.microsoft.com/en-us/openspecs/microsoft_general_purpose_programming_languages/ms-vbal/9f072ffc-e943-4fcc-a4d0-f3c7db96abd9)
describe string coercion and NULL handling. They are implementation references,
not native qualification of our explicit rendering policies or deferred operands.
