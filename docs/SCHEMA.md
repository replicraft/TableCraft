# Shared scalar schema contracts

`crates/foundation/tablecraft-schema` owns reusable schema identities, declared
scalar types, field nullability and explicit consumer policies. It depends on
core values and thiserror, with no SQL parser, query executor, backend or
application dependency. Implemented contracts cover scalars and relationship
declaration shapes; table/key/index models and schema differences remain future work. Native field
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
constraints: an unconstrained carrier, Text with a positive maximum Unicode-scalar
count, or Decimal with declared precision and exact retained scale. SQL parameters
currently use the unconstrained/Text forms; native fields also use constrained
Decimal. Zero Text limits are rejected at
construction. There is no coercion, truncation, Decimal rounding or implicit
character-unit conversion. Unicode-scalar counting preserves the initial query
parameter behavior; native Access length-unit fidelity remains unqualified.

`TypeDescriptor::validate_value` checks the exact non-NULL carrier and declared
Text/Decimal constraints. NULL passes this type check; field nullability is enforced by
`validate_row`, while current query parameters permit NULL. Floating finiteness,
the evaluator's 65,536-byte Text ceiling and execution allocation budgets remain
execution policies rather than universal schema rules.

`Column::new` retains its carrier-only constructor. `Column::with_type` accepts
a validated descriptor; `type_descriptor` exposes it and `data_type` continues
returning its scalar carrier. Row validation now applies declared field constraints. Source schema comparison includes these
constraints, so changing a field's Text limit or Decimal precision/scale invalidates
an existing prepared plan.
The crate's public doctest demonstrates a constrained field and an explicit profile.

## Field constraint checks

`FieldConstraints::new` requires explicit Required/AllowZeroLength policies.
It checks exact Value carriers without allocation/coercion and distinguishes NULL
from empty Text; empty Binary is unaffected. There are no backend defaults or
expression evaluation. ACE's [record audit](NATIVE_CONSTRAINTS.md) is a working
consumer; declaration loading and coverage stay in the adapter.

## Syntax and consumer boundaries

Relationship/RelationshipColumn validate nonempty ordered field mappings and reject
repeated fields under explicit NameComparison. Referenced/dependent identities may
remain unresolved until catalog binding. RelationshipRules/ReferentialAction carry
qualified declaration semantics; absent rules mean unknown, never disabled
integrity. These contracts neither inspect records nor execute cascades. ACE is a
working consumer and separately retains [native flags and grouping](NATIVE_RELATIONSHIPS.md).

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

## Declared Decimal field constraints

DecimalConstraints owns Access field precision 1..=28 and retained scale
0..=precision, independently of numeric expression range. TypeDescriptor::decimal
attaches these checked parameters to a Decimal column; decimal_constraints exposes
them without native storage metadata. Validation rejects coefficients outside the
signed declared precision, mismatched carriers and different retained scales.
Even an exactly rescalable value or differently scaled zero fails: this is an
exact-carrier schema check, not assignment coercion. NULL passes type validation;
column nullability remains separate. Bare TypeDescriptor::new(Decimal) retains
its unconstrained-carrier meaning.

Qualified native Decimal columns now retain their declaration in shared inspection,
sequential, indexed, paged and projected schemas. Query source validation therefore
checks these parameters, and reusable plans reject precision/scale schema drift.
Cached calculated result carriers do not acquire guessed ordinary-field parameters.
No field bytes, codecs, expression carrier range, Serde or query arithmetic changed.
This field boundary is required before wider CDec expression values are introduced.
