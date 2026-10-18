# Shared scalar schema contracts

`crates/foundation/tablecraft-schema` owns reusable schema identities, declared
scalar types, field nullability and explicit consumer policies. It depends on
core values and thiserror, with no SQL parser, query executor, backend or
application dependency. These are implemented scalar contracts; tables, keys,
indexes, relationships and schema differences remain future work. Native field
metadata stays in ACE, with a working mapping to supported shared scalar Columns;
see [NATIVE_TABLES.md](NATIVE_TABLES.md).

## Names and profile policies

`SchemaName::new` validates nonempty, control-free names and preserves spelling.
It does not impose a universal byte limit. Its representational equality stays
case-sensitive; `NameComparison` explicitly selects exact or ASCII-insensitive
lookup/duplicate checks. Neither policy implements Access locale identification.
Names omit Debug/Serde; callers access original spelling through `as_str()`.
Columns, in-memory table identities and bound parameter declarations use this
same validated name contract.

`SchemaPolicy` supplies maximum name bytes, maximum input columns and the name
comparison. There is no implicit default. Its `validate_columns` checks limits,
nonempty schemas and duplicate identities. The free `validate_columns` function
checks nonempty/duplicate invariants with an explicit `NameComparison`, leaving
size ceilings to the caller's policy. Schemas wider than 256 columns and names
longer than 255 bytes are valid generic constructions if the chosen profile
permits them; these numbers are not universal database-format limits.

The query crate's `SOURCE_SCHEMA_POLICY` retains its 255 UTF-8 byte name limit,
256-column input/output limits and ASCII-insensitive matching. `MemoryTable`
and query binding apply this profile, including to custom host schemas. Existing
query rejection boundaries are preserved. Expression depth, work counters,
parameter/list counts and allocation limits remain query/application concerns.

## Declared types and row validation

`ScalarType` classifies exact core carriers. `TypeDescriptor` adds supported
constraints shared by fields and parameters: an unconstrained carrier or Text
with a positive maximum Unicode-scalar count. Zero Text limits are rejected at
construction. There is no coercion, truncation, Decimal rounding or implicit
character-unit conversion. Unicode-scalar counting preserves the initial query
parameter behavior; native Access length-unit fidelity remains unqualified.

`TypeDescriptor::validate_value` checks the exact non-NULL carrier and declared
Text limit. NULL passes this type check; field nullability is enforced by
`validate_row`, while current query parameters permit NULL. Floating finiteness,
the evaluator's 65,536-byte Text ceiling and execution allocation budgets remain
execution policies rather than universal schema rules.

`Column::new` retains its carrier-only constructor. `Column::with_type` accepts
a validated descriptor; `type_descriptor` exposes it and `data_type` continues
returning its scalar carrier. Row validation now applies the same declared Text
constraint used by parameters. Source schema comparison includes these constraints,
so changing a field's declared Text limit invalidates an existing prepared plan.
The crate's public doctest demonstrates a constrained field and an explicit profile.

## Syntax and consumer boundaries

SQL type spellings and syntactic arguments stay in `tablecraft-sql`. Query binding
lowers those spellings to schema descriptors in its `binding/parameters` adapter;
SQL INTEGER/SHORT mappings and unsupported argument rejection stay there.
Schema does not import parser ASTs. Bound parameter identity/scope, plans, source
scanning, runtime checks and CLI argument decoding remain with their consumers.
Core Value carriers and representational equality remain unchanged.

The generic `validate_columns` API now requires an explicit name-comparison
argument; callers wanting both size and comparison checks use SchemaPolicy.
The workspace's source and binder consumers have been migrated. No Serde feature,
registry dependency, native codec or new crate was introduced by this extraction.

Focused tests exercise generic versus query-profile limits, comparison selection,
Unicode Text constraints, nullability, exact carrier rejection, provider-row
validation and prepared-plan invalidation after a constraint change. See
[IMPLEMENTATION.md](IMPLEMENTATION.md) for observed checks; these tests do not
establish native Access compatibility.
