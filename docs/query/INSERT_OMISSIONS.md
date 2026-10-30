# Presence-bearing INSERT planning

`plan_insert_with_omissions` is a separate opt-in planner over
`OmittingInsertableTable`. The existing `InsertableTable::plan_insert` and query
`plan_insert` still require a complete row; named subsets remain refused there.
Both planners use one shared metadata binder and written-order evaluator.
Neither scans records to invent schema or reads native default declarations.

The new consuming finish receives a complete-width `Vec<InsertValue>` in exposed
backend schema order. `Supplied(Value::Null)` is an explicit NULL;
`Omitted` means no value was supplied. Only an explicit field list may leave
positions omitted. Positional VALUES must supply every exposed column.
Unknown and duplicate resolved names refuse before any expression evaluates.
Expressions then evaluate in written VALUES order, use the existing stored
assignment law, and finish once after all values succeed. A late error produces
no backend candidate or partial finish.

A backend can expose an explicitly selected generated-column companion schema.
The vector then covers that schema, excluding only the backend-bound generated
field. The planner never invents an omitted identity slot or chooses a generation
mode. Companion mode still requires explicit field names.

An omitted nonnullable column remains omission intent: its backend may have a
qualifying default, or may reject it. The query layer does not substitute NULL,
parse a stored expression or apply a Required exception. Supplied carriers still
obey the existing checked integral-to-Int32 assignment law, nullable NULL law and
exact remaining carrier descriptors. No additional coercions, ambient clock,
DEFAULT VALUES, INSERT SELECT, parameter, UDF or external-source capability is
introduced.

Six new neutral controls passed on their first actual run alongside all nine
existing complete-INSERT controls. They cover NULL versus omission, positional
and complete-planner limits, name binding before expressions, written-order and
late-carrier failures, generated companion width, all three exact resource pools,
one-below limits and final cancellation before the sole backend finish.
Standalone neutral strict and minimal DB/query checks also passed. Native default resolution,
Access adapter routing, CLI publication and native Access qualification are
separate; these neutral tests do not establish those capabilities.
