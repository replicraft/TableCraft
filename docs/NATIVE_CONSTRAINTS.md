# Explicit native record constraints

`check_table_constraints` and CLI `inspect PATH --table NAME --constraints` audit
field declarations and qualified foreign keys over stable/offline sources. Results
are buffered; typed failures publish no partial report. No defaults, macros,
providers or cascades execute. Ordinary reads remain unchanged; structural/index
checks remain a [separate API](NATIVE_CHECKS.md).

## Field constraints

Shared `FieldConstraints` applies explicit Required/AllowZeroLength policies
without coercion. Required rejects NULL when true. AllowZeroLength applies only
to Text and rejects empty strings when false; NULL and empty Binary differ.
ACE obtains declarations from [property maps](NATIVE_PROPERTIES.md). Each field
reports Checked, NotDeclared or Unsupported independently for both policies and
ValidationRule. Missing declarations remain absent; wrong carriers/unqualified
semantics are never marked checked. Empty/missing validation expressions are
NotDeclared; the bounded expression profile below reports coverage separately.
Defaults remain metadata and are not applied to existing records.

## Foreign keys

Incoming and outgoing relationships touching the table are inspected. Qualified
unenforced declarations report NotEnforced. Unknown/inherited/conflicting flags,
unresolved endpoints, composites, unsupported carriers/tables and missing qualified
single-column parent unique declarations report Unsupported.

The checked profile has one Int32 field per side and a declared unique/primary
parent index. Complete sequential snapshots verify non-NULL parent uniqueness,
non-NULL child membership and, for one-to-one relationships, non-NULL child
uniqueness. NULL children have no reference. Empty parents accept no non-NULL
children. Exact i32 comparisons are used, not general Value representational equality.

Fallibly reserved vectors and in-place sorting spend shared allocation/work
budgets; reads/field/key visits accumulate and cancellation is checked between
bounded units and before/after sorts. This does not check physical indexes,
parent primary-key NULL rules, cascading actions, macros, preservation or writes.
Composite NULL behavior remains [unqualified](NATIVE_RELATIONSHIPS.md#references-and-evidence).
Unsupported coverage cannot authorize later writes.

## References and evidence

Microsoft documents [Required](https://learn.microsoft.com/en-us/office/client-developer/access/desktop-database-reference/field-required-property-dao),
[AllowZeroLength](https://learn.microsoft.com/en-us/office/client-developer/access/desktop-database-reference/field-allowzerolength-property-dao)
and [referential integrity](https://support.microsoft.com/en-us/access/guide-to-table-relationships).
Independent comparisons are not Microsoft Access runtime conformance.

The original [relationship recipe](testing/RelationshipFixture.java) supplies valid
duplicate/NULL children, one-to-one/set-null declarations, an Unsupported composite
and a NotEnforced orphan. [Comparison runner](testing/compare_record_constraints.py)
verifies all five child-table row counts and coverage in Jet 4/ACE without changing
files. Candidate tests mutate only in-memory copies to create Required, empty Text,
orphan-child and duplicate-parent violations. A 256-case fixed-seed BTreeSet model
checks key outcomes; budget/cancellation and CLI tests complement it.

Defaults, broader expression/collation and composite-key profiles, native writes
and Microsoft Access comparison remain separate work. Unsupported coverage blocks
write qualification; this is a bounded step-2 profile, not universal validation.

## Bounded validation expressions

Field rules supply their implied field through a quoted lexer token, for example
`>=0 And <=100`; explicit references must name that same field. Table rules may
reference fields of their own table, for example `[Minimum] <= [Maximum]`. Pure
scalar arithmetic, comparisons, Boolean logic and IS NULL use the existing
[scalar evaluator](EXPRESSIONS.md), with no providers or callbacks. Rules are
preflighted even on empty tables. Calls, subqueries, host members, LIKE/BETWEEN/IN
and text comparisons are Unsupported. No implicit collation is selected.

False results produce typed field/table-rule violations. NULL results report
Unsupported: native Access NULL-validation behavior is not qualified. Evaluation
errors fail the audit; no rejected row is silently skipped. The evaluator's eager
expression ordering is retained. This is an explicit diagnostic, not implicit
execution of database application content or a write authorization.

Rules are capped at 4,096 bytes, 128 tokens and depth 16. Expansion of implied
fields shares byte/token limits. AST/name allocation allowances and evaluation
spend the caller budget, with cancellation between bounded units. Parser temporary
buffers have separate structural ceilings rather than a strict allocator quota.

Microsoft describes [field rules](https://learn.microsoft.com/en-us/office/client-developer/access/desktop-database-reference/field-validationrule-property-dao)
and [table rules](https://learn.microsoft.com/en-us/office/client-developer/access/desktop-database-reference/tabledef-validationrule-property-dao).
The original [validation recipe](testing/ValidationFixture.java) and
[comparison runner](testing/compare_validation.py) verify six tables per format:
field ranges, cross-field bounds, explicit NULL handling, unsupported calls on
populated/empty tables and an intentionally unresolved NULL result. Jackcess
rejects the generated out-of-range/cross-field invalid writes. These comparisons
do not establish native Access NULL behavior or general expression equivalence.
