# Numeric aggregate output descriptions

Metadata-only preparation describes the successful result carrier of the existing
SUM and AVG accumulators. It now follows bounded IIf, Choose and Switch alternatives,
including transparent CVar wrappers, before merging their aggregate result types.
This avoids losing a stable aggregate schema merely because the selected input
carriers differ.

For example, `Sum(IIf(Flag,CInt(1),CLng(2)))` has a nullable Int64 output description:
although its input can be Int16 or Int32, both accumulator paths return Int64.
`Avg(IIf(Flag,CLng(1),CCur(2)))` similarly has a nullable Double description.
Single/Double SUM alternatives share Double; Decimal alternatives retain Decimal
without carrying an input precision/scale constraint onto the total.

These are guarantees of TableCraft's implemented accumulators, not inferred native
Access subtype rules. Microsoft's [SUM query contract](https://learn.microsoft.com/en-us/office/vba/access/concepts/criteria-expressions/sum-function-microsoft-access-sql)
and [AVG query contract](https://learn.microsoft.com/en-us/office/vba/access/concepts/criteria-expressions/avg-function-microsoft-access-sql)
establish numeric query expressions and NULL omission; they do not qualify the
accumulator representation or mixed-type promotion used here. The existing
[mean](AVERAGES.md), [Decimal SUM](DECIMAL_SUMS.md) and
[selector](../../expressions/CONDITIONAL_FUNCTIONS.md) engineering profiles remain authoritative.

## Conservative boundaries

Different possible aggregate result carriers remain Dynamic: Currency/integer SUM
and Decimal/Double AVG are examples. Unknown fields and unsupported or Error-valued
alternatives do not acquire numeric descriptions. Closed literal selector controls
use the existing successful-branch facts; data, parameters and general expressions
are not evaluated to discover a branch. Choose no-match and all-NULL alternatives
retain NULL. Every non-NULL numeric description allows NULL for an empty or all-NULL
group.

The aggregate output description can flow through derived tables and UNION inputs.
An already heterogeneous derived field remains Dynamic when used by an outer
aggregate: preparation does not scan it or invent a retained set of input carriers.
Min/Max/First/Last and declaration-category provenance are unchanged.

All expressions still bind and execute under their existing eager rules. A precise
successful output description does not promise that execution succeeds: division
by zero in an unused IIf or Switch choice still fails. No conversion is inserted,
no source is scanned during preparation, and no provider is called to establish
metadata. The traversal uses existing binding depth, node, byte, step and cancellation
checks; it retains no vector of alternatives and visits each selected-expression
subtree once per ordinary result-description traversal.

## Verification

`aggregate_selector_types` enumerates ten independent input families, both
aggregates and four selector forms: 800 metadata cases and 1,600 executions across
both arithmetic modes. Every claimed descriptor accepts the successful runtime
carrier. Further controls cover inert preparation, downstream derived/UNION
schemas, Dynamic derived inputs, empty/NULL groups, literal selection and eager
operational failures. Native Access comparison remains deferred.
