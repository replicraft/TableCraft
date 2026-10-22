# Materialized CVar values

`CVar(expression)` now runs as an unqualified one-argument query expression.
Microsoft's [sandbox query list](https://support.microsoft.com/en-us/access/functions-and-properties-in-access-blocked-by-sandbox-mode)
explicitly admits CVar in Access database-engine queries. The
[MS-VBAL specification](https://download.microsoft.com/download/0/a/6/0a6f7755-9af5-448b-907d-13985accf53e/%5Bms-vbal%5D.pdf),
section 6.1.2.3.1.13, defines its result as the argument data value. A separate
VBA variable or Variant container is not needed to preserve a materialized query
value's existing carrier.

The owned kernel returns Value unchanged. NULL remains NULL, Text is not parsed,
and numeric widths, Decimal scale, Currency, date bits and Boolean values are
retained. Text/Binary allocations move into the result without a second copy.
The pure library kernel preserves all IEEE bits; normal query source/literal
validation still rejects non-finite inputs before function execution.

```sh
tablecraft query eval "SELECT CVar('001') AS TextValue,VarType(CVar(CByte(255))) AS Subtype"
```

The values are Text `001` and Integer 17. CVar does not convert numeric-looking
Text to a number or promote a Byte to Double. Bound Binary, GUID and extended
DateTime values retain their existing TableCraft carriers; this does not invent
VBA subtype codes or establish native Access mapping for those extensions.

## Execution and scope

Both scalar and bound evaluation execute the operand once and move its result.
Bound plans retain an explicit CVar node, including normal work/cancellation
checks. Binding never evaluates values. Field dependencies, correlations,
aggregates/domains, grouping identity and collection-item walks traverse the
operand. Text provenance passes through unchanged, preserving explicit field
policies and policy conflicts. Clocks remain volatile; CVar cannot make a
volatile operand into a constant or swallow an operational error.

Wrong arity, wildcards, missing argument slots and qualified names fail before
operand evaluation. Eager callers retain errors from unselected expressions;
late failures produce no successful result/stdout. No new stored type, schema,
dependency, object activation or VBA runtime was introduced.

Expression-only arrays and Empty/Missing states are not admitted by this
scalar Value path. [CVErr Error data](VARIANT_ERRORS.md) now passes through the
explicit expression-data path and its consumers; unconsumed Error result cells
remain outside scalar Value publication. General VBA array/variable/object/default-property behavior
remains outside it. Existing scalar versus bound Binary operand-admission rules
remain unchanged. CVar stays **Partial** in the [tracker](ACCESS_BUILTINS.md);
the implemented identity kernel does not qualify full Variant/native behavior.

## Evidence

Kernel tests check carrier/representation preservation and unchanged owned
payload addresses. Query controls exercise correlations, aggregates/domains,
grouping, Decimal parameter reuse, native extension/Binary carriers, missing
clock context, cancellation and eager errors. Existing field-policy tests now
also wrap operands with CVar, including conflict rejection. CLI controls cover
subtype codes, NULL/Text, invalid calls and empty output on failures.

The original [independent runner](testing/compare_cvar.py) uses explicit subtype
goldens and integer models, including values beyond binary64 exact precision.
Optional original SavedForest-derived AData files provide native Int32 inputs;
their hashes must remain unchanged. This is TableCraft execution over original
files, not Microsoft Access evaluation, and it adds no saved CVar query fixtures.
