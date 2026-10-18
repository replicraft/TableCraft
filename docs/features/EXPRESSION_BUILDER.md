# Context-aware Expression Builder

Status: future editor requirements, 18 October 2026. Access SQL expression parsing
and initial bounded scalar evaluation already work; see [SQL_PARSER.md](../SQL_PARSER.md)
and [EXPRESSIONS.md](../EXPRESSIONS.md). There is no visual editor, field binder,
function registry or complete Access expression runtime yet.

The planned design/presentation layer should reuse one editor across calculated
fields, default values, validation rules, query criteria, control sources and
report calculations. Each caller supplies visible fields/controls, parameters,
allowed functions/operators, expected type, nullability and Access version. A
function valid in a control need not be valid in a table field.

Provide syntax highlighting, bracket-aware completion, argument help, source
diagnostics and navigation to recognized references. Preserve entered source
separately from normalized SQL. Recovery for incomplete editor text must not
weaken the executable parser or admit invalid expressions into saved objects.
Keep parser/semantic diagnostics available headlessly through narrow contracts.

Sample evaluation uses explicit data bindings and bounded, cancellable execution.
Default previews cannot invoke arbitrary VBA, database actions, filesystem/network
providers or property getters with effects. Future UDF evaluation requires a
qualified effect contract and separate authorization; a function name alone is
not proof of purity. Unsupported evaluation remains visibly unsupported. Do not
present the current scalar profile as complete Variant/locale/date semantics.

Acceptance requires context-specific completion/rejection, source diagnostics,
sample results/errors compared with the native context, and proof that preview
cannot cause external effects. Test accessibility, keyboard operation and large/
incomplete input with a runnable UI. No editor/native comparison exists today.

Microsoft's [Expression Builder guide](https://support.microsoft.com/en-gb/access/use-the-expression-builder)
documents the workflow and context-sensitive identifiers/help. Existing parser
and evaluator tests establish only their separately documented implementation.
