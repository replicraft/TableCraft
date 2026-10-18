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
unresolved endpoints, unsupported carriers/tables, more than ten columns and
missing qualified parent unique declarations report Unsupported.

The checked profile has one to ten Int32 fields per side and a declared unique/
primary parent index with the same ordered native columns. Complete sequential
snapshots check parent tuple uniqueness, child membership and one-to-one child
uniqueness. Entirely NULL tuples are exempt. Partially NULL tuples remain exact
keys: they require an identical parent tuple, not componentwise matches or an
implicit exemption. Duplicate partial tuples fail uniqueness. This follows the
observed independent-reader profile; Microsoft Access comparisons remain unrun.
Exact nullable i32 tuples are compared without general Value/SQL coercion.

Fallibly reserved fixed-width key vectors and in-place sorting spend shared
allocation/work budgets, including bounded tuple comparison work. Reads/field/key
visits accumulate and cancellation is checked between bounded units and before/
after sorts. This does not check physical composite index pages, parent primary-
key NULL rules, cascading actions, macros, preservation or writes. Unsupported
coverage cannot authorize later writes; native write gates still require their
separate physical index checks and all other documented prerequisites.

## References and evidence

Microsoft documents [Required](https://learn.microsoft.com/en-us/office/client-developer/access/desktop-database-reference/field-required-property-dao),
[AllowZeroLength](https://learn.microsoft.com/en-us/office/client-developer/access/desktop-database-reference/field-allowzerolength-property-dao)
and [referential integrity](https://support.microsoft.com/en-us/access/guide-to-table-relationships).
Independent comparisons are not Microsoft Access runtime conformance.

The original [relationship recipe](testing/RelationshipFixture.java) supplies valid
duplicate/NULL children, one-to-one/set-null declarations, a checked composite
and a NotEnforced orphan. [Comparison runner](testing/compare_record_constraints.py)
verifies all five child-table row counts and coverage in Jet 4/ACE without changing
files. Candidate tests mutate only in-memory copies to create Required, empty Text,
orphan-child and duplicate-parent violations. A 256-case fixed-seed BTreeSet model
checks key outcomes; budget/cancellation and CLI tests complement it.

Defaults, broader expression/collation and non-Int32 key profiles, native writes
and Microsoft Access comparison remain separate work. Unsupported coverage blocks
write qualification; this is a bounded step-2 profile, not universal validation.

## Bounded validation expressions

Field rules supply their implied field through a quoted lexer token, for example
`>=0 And <=100`; explicit references must name that same field. Table rules may
reference fields of their own table, for example `[Minimum] <= [Maximum]`. Pure
scalar arithmetic, comparisons, Boolean logic and IS NULL use the existing
[scalar evaluator](EXPRESSIONS.md), with no providers or callbacks. Numeric BETWEEN/NOT BETWEEN and IN/NOT IN lists
reuse shared scalar predicate kernels. Rules are preflighted even on empty tables.
Calls, subqueries, host members, LIKE and text comparisons are Unsupported. No implicit collation is selected.

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

## Composite independent evidence

[CompositeConstraintFixture.java](testing/CompositeConstraintFixture.java) creates
fresh V2016/V2019 inputs with matching partial/all-NULL tuples, duplicate all-NULL
parents/children and an enforced one-to-one relationship. Generation asserts
rejection of unmatched partial tuples, componentwise-only matches and duplicate
partial unique keys. [The runner](testing/compare_composite_constraints.py)
compares fifteen exact rows/profile, checks coverage and mutates only disposable
copies to verify missing-parent, duplicate-parent and duplicate-child errors.

Three native synthetic integration tests cover ordered unique-index eligibility,
self relations, componentwise mismatch and duplicate complete tuples. Fixed-seed
256-case tuple-set comparisons supplement the original scalar-key tests. Full
native Access NULL behavior and physical composite-index conformance remain
unqualified; no automatic write/cascade support follows from this audit.

## Numeric range and membership rule evidence

Original [predicate-rule fixtures](testing/PredicateRuleFixture.java) contain
forward/reversed ranges, allowed/excluded integer sets and explicit unsupported
text/unknown-NULL cases. Generation checks rejected out-of-range/nonmember rows
with the independent reader. [Comparison](testing/compare_predicate_rules.py)
matches six coverage reports and thirteen exact records per V2016/V2019 family,
without changing originals. Range endpoints may appear in either written order;
NULL membership results still report Unsupported in the native audit.

Rule evaluation, source-free CLI evaluation and query execution share eager
predicate kernels. Existing qualified fixed Int32 plans recheck the new rules
before/after edits; rejected candidates publish no plan. This does not enable
ACE writes, native callbacks, physical index changes or implicit collation.
