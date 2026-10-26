# Explicit native record constraints

`check_table_constraints` and CLI `inspect PATH --table NAME --constraints` audit
field declarations and qualified foreign keys over stable/offline sources. Results
are buffered; typed failures publish no partial report. No defaults, macros,
providers or cascades execute. Ordinary reads remain unchanged; structural/index
checks remain a [separate API](../NATIVE_CHECKS.md).

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

The checked profile has one to ten Boolean/Byte/Int16/Int32/Int64/Currency/GUID fields per side,
plus qualified ACE 16 Decimal fields, and a declared
unique/primary parent index with the same ordered native columns. Each paired
parent/child field must have the same storage type; mixed-type tuples are allowed
when corresponding pairs agree. Decimal pairs additionally require identical
native precision/scale and ACE 16-family profiles. Native scalar/profile gates still apply.
Complete sequential
snapshots check parent tuple uniqueness, child membership and one-to-one child
uniqueness. Entirely NULL tuples are exempt. Partially NULL tuples remain exact
keys: they require an identical parent tuple, not componentwise matches or an
implicit exemption. Duplicate partial tuples fail uniqueness. This follows the
observed independent-reader profile; Microsoft Access comparisons remain unrun.
Exact nullable Boolean/integer/Currency/GUID/Decimal tuples are compared without general Value/SQL coercion
or floating conversion. The CLI labels successful coverage exact key membership.

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

The original [relationship recipe](../../testing/RelationshipFixture.java) supplies valid
duplicate/NULL children, one-to-one/set-null declarations, a checked composite
and a NotEnforced orphan. [Comparison runner](../../testing/compare_record_constraints.py)
verifies all five child-table row counts and coverage in Jet 4/ACE without changing
files. Candidate tests mutate only in-memory copies to create Required, empty Text,
orphan-child and duplicate-parent violations. A 256-case fixed-seed BTreeSet model
checks key outcomes; budget/cancellation and CLI tests complement it.

Defaults, broader expression/collation and other key profiles, native writes
and Microsoft Access comparison remain separate work. Unsupported coverage blocks
write qualification; this is a bounded step-2 profile, not universal validation.

Text FK expansion has a concrete [reference comparison prerequisite](../../testing/TEXT_RELATIONSHIP_RESEARCH.md):
observed reference foreign-key matching differs from its physical unique-index
key equivalence for trailing/ignored units, combining text and expansions.
Existing text-index support therefore does not establish Text FK membership.
Text relationships remain Unsupported, with existing mutation guards unchanged.

## Bounded validation expressions

Field rules supply their implied field through a quoted lexer token, for example
`>=0 And <=100`; explicit references must name that same field. Table rules may
reference fields of their own table, for example `[Minimum] <= [Maximum]`. Pure
scalar arithmetic, comparisons, Boolean logic and IS NULL use the existing
[scalar evaluator](../../expressions/EXPRESSIONS.md), with no providers or callbacks. Numeric BETWEEN/NOT BETWEEN and IN/NOT IN lists
reuse shared scalar predicate kernels. Rules are preflighted even on empty tables.
Only two pure calls are admitted: unqualified case-insensitive `Len(text)` and
`StrComp(left, right, 0)`, with exact arity and a literal integer zero comparison
mode. Their operands must be statically Text or NULL: string/NULL literals,
declared Text fields, and recursive `+`/`&` compositions of those operands. Field
rules still restrict references inside calls to their own field; table rules may
reference their own table's Text fields. Numeric/date operands and mixed
concatenations are refused before evaluation rather than implicitly formatted.
`Len` measures UTF-16 units; binary `StrComp` distinguishes case and trailing
spaces. Both use the existing evaluator without a locale provider.

Unknown or qualified calls, `UCase`, omitted/nonbinary/computed comparison modes,
subqueries, host members, LIKE and ordinary text comparisons remain Unsupported.
No implicit collation is selected. Nested calls are not admitted as Text operands;
these two numeric-returning calls may participate in existing numeric comparisons,
BETWEEN and IN predicates.

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
The original [validation recipe](../../testing/ValidationFixture.java) and
[comparison runner](../../testing/compare_validation.py) verify six tables per format:
field ranges, cross-field bounds, explicit NULL handling, admitted Len calls on
populated/empty tables and an intentionally unresolved NULL result. Jackcess
rejects the generated out-of-range/cross-field invalid writes. These comparisons
do not establish native Access NULL behavior or general expression equivalence.

Microsoft also provides [validation examples using Len and binary StrComp](https://support.microsoft.com/en-us/access/restrict-data-input-by-using-validation-rules).
Its StrComp example includes `UCase`; that locale-dependent part remains
unqualified. Original synthetic native records independently cover supplementary
UTF-16 characters, embedded NUL, binary case/trailing spaces, NULL outcomes and
explicit guards, pure text addition/concatenation, eager division failures,
rejected signatures/scopes even when empty, limits and prefix cancellation. This
expansion does not qualify all validation functions or native Access saves.
The [independent text-call replay](../../testing/VALIDATION_TEXT_CALLS.md) records
204 controls and 618 getter cells across original ACE 2016/2019 inputs, with
all 19 pinned hashes unchanged. The existing ACE/Jet 4 recipe also passes its
12 coverage checks on fresh originals.

## Composite independent evidence

[CompositeConstraintFixture.java](../../testing/CompositeConstraintFixture.java) creates
fresh V2016/V2019 inputs with matching partial/all-NULL tuples, duplicate all-NULL
parents/children and an enforced one-to-one relationship. Generation asserts
rejection of unmatched partial tuples, componentwise-only matches and duplicate
partial unique keys. [The runner](../../testing/compare_composite_constraints.py)
compares fifteen exact rows/profile, checks coverage and mutates only disposable
copies to verify missing-parent, duplicate-parent and duplicate-child errors.

Three native synthetic integration tests cover ordered unique-index eligibility,
self relations, componentwise mismatch and duplicate complete tuples. Fixed-seed
256-case tuple-set comparisons supplement the original scalar-key tests. Full
native Access NULL behavior remains unqualified. Physical numeric composite
indexes now have a separate [structural check](../indexes/NATIVE_INDEXES.md#complete-numeric-composite-indexes);
this membership audit does not perform it or authorize writes/cascades.

## Numeric range and membership rule evidence

Original [predicate-rule fixtures](../../testing/PredicateRuleFixture.java) contain
forward/reversed ranges, allowed/excluded integer sets and explicit unsupported
text/unknown-NULL cases. Generation checks rejected out-of-range/nonmember rows
with the independent reader. [Comparison](../../testing/compare_predicate_rules.py)
matches six coverage reports and thirteen exact records per V2016/V2019 family,
without changing originals. Range endpoints may appear in either written order;
NULL membership results still report Unsupported in the native audit.

Rule evaluation, source-free CLI evaluation and query execution share eager
predicate kernels. Existing qualified fixed Int32 plans recheck the new rules
before/after edits; rejected candidates publish no plan. This does not enable
ACE writes, native callbacks, physical index changes or implicit collation.

## Int64 and mixed tuple evidence

[Int64RelationshipFixture.java](../../testing/Int64RelationshipFixture.java) generates
fresh V2016/V2019 inputs with full-range Int64 keys, adjacent values beyond double
precision, mixed Int32/Int64 composite keys, matching partial-NULL keys and
one-to-one declarations. Generation asserts rejection of missing parents,
precision-sensitive mismatches and duplicate dependent unique keys.
[The comparison runner](../../testing/compare_int64_relationships.py) compares 35 exact
rows/profile, audits all six tables and corrupts only disposable copies to verify
missing-parent, duplicate-parent and duplicate-child errors with no partial CLI
output. Original input hashes are recorded in
[fixture provenance](../../testing/FIXTURES.md#original-ace-16-int64-relationship-fixtures).

Synthetic CI tests cover mixed self relations, mismatched paired types, exact
parent uniqueness and adjacent large values; full-range tuple-set property checks
retain fixed seeds. Physical tree validation uses the separate index checker;
cascades and native Microsoft Access comparison remain unqualified. This audit
does not authorize ACE writes.

## Byte and Int16 relationship evidence

Unsigned Byte and signed Int16 now join the same exact nullable tuple profile.
Values are widened losslessly inside the private key audit; paired storage types
must still match, so Byte/Int16 pairs are not coerced. Width/profile/type gates,
parent unique declarations, all-NULL exemption and exact partial-NULL membership
remain unchanged.

The original [narrow relationship recipe](../../testing/NarrowRelationshipFixture.java)
generates separate Byte/Int16 single-key and mixed-tuple relationships, including
one-to-one declarations and unsigned/signed boundaries. Jackcess independently
rejects orphan and duplicate inserts. [The runner](../../testing/compare_narrow_relationships.py)
matches all 59 exact rows and nine table audits per family. Six corrupted disposable
copies/profile fail for missing/duplicate parents, componentwise/partial-NULL
mismatches and duplicate one-to-one children; a complete NULL tuple remains exempt.
Input hashes remain unchanged. Synthetic CI adds exact narrow self-tuples, crossed
pair-type rejection, parent duplicates and same-type orphan checks for both widths.
This evidence qualifies bounded membership only; physical index validation uses
the separate checker. Cascades, ACE writes and Microsoft Access remain unqualified.

## Exact GUID relationship membership

GUID fields now participate in the same bounded tuple audit, including mixed
integer/GUID tuples with matching paired storage types. Private atoms retain
canonical 16-byte GUIDs separately from integers and NULL. Every bit pattern,
including the all-zero GUID, remains a value. Sorting supplies a private set/search
order; it does not define SQL GUID ordering or physical index encoding. Text GUID
spellings are not coerced into stored keys. Key allocation charges use the actual
private tuple size and retain existing limits/cancellation.

The original [GUID recipe](../../testing/GuidRelationshipFixture.java) generates 32 rows
in six tables/profile, including nearby GUIDs, zero/high patterns, exact Int64
extremes and partial/all-NULL tuples. It independently rejects duplicate/orphan
inserts. [The comparison](../../testing/compare_guid_relationships.py) matches all rows
and audits, rejects six corrupt disposable copies/profile without partial stdout,
and verifies all-NULL exemption and immutable inputs. Synthetic exact-byte/tuple
models cover arbitrary GUID bits, mixed pair-type rejection and exact orphans.

The reviewed Jackcess 5.0.4 default GUID cursor returns false for one lowercase
lookup and true for its canonical uppercase spelling; its FK insertion path also
rejected that existing lowercase key. The recipe uses canonical spelling and
retains a read-only `--probe-case` observation. This independent-engine limitation
does not become a TableCraft key rule: native bytes alone determine membership.
An initial partial generation attempt remains unqualified; the final fresh
fixtures are registered in [provenance](../../testing/FIXTURES.md#original-ace-16-guid-relationship-fixtures).
GUID and GUID-bearing composite physical indexes now have separate
[complete checks](../indexes/NATIVE_INDEXES.md#complete-guid-bearing-composite-indexes).
SQL GUID ordering, cascades, native Access comparison
and ACE writes remain unqualified. CLI checked text changed from integer key
membership to exact key membership; comparison runners/tests use the new label.

## Exact Currency relationship membership

Currency keys retain their signed scale-four integer in a distinct private atom,
separate from integer keys, GUIDs and NULL. Corresponding parent/child field types
must match, including Currency versus equal-width Int64. One-to-ten-column mixed
tuples reuse the existing bounded exact-set audit: parent uniqueness, complete
child membership, one-to-one uniqueness, exact partial NULLs and all-NULL exemption.
There is no floating conversion or general SQL/Value equality. Private tuple
allocation/work charging and cancellation remain shared.

The original [Currency relationship recipe](../../testing/CurrencyRelationshipFixture.java)
generates six inert tables/profile, with scalar and mixed Currency/Int32 keys,
signed extrema, adjacent ten-thousandths beyond floating-point integer precision,
NULL shapes and one-to-one declarations. Generation independently rejects orphans,
componentwise-only matches, unmatched partial NULLs and duplicate children.
[The runner](../../testing/compare_currency_relationships.py) matches all 48 exact rows
and six table audits per V2016/V2019 family. Eight corrupt disposable copies/profile
fail atomically for exact orphans, one-unit mismatches, duplicate parents (including
partial NULLs) and duplicate one-to-one children. An all-NULL tuple remains exempt;
original file hashes are unchanged and recorded in
[provenance](../../testing/FIXTURES.md#original-ace-16-currency-relationship-inputs).

Synthetic tests add exact Money/Money self relations, mixed Money/Int64/GUID
carriers, equal-width type rejection, NULL/zero distinctions and a fixed-seed
256-case independent tuple-set model. This qualifies bounded membership, without
physical composite index validation, Currency mutation, cascades, ACE writes or
native Microsoft Access conformance.

```sh
java -cp path/to/jackcess-5.0.4.jar docs/testing/CurrencyRelationshipFixture.java new-2016.accdb V2016
java -cp path/to/jackcess-5.0.4.jar docs/testing/CurrencyRelationshipFixture.java new-2019.accdb V2019
python3 docs/testing/compare_currency_relationships.py --jar path/to/jackcess-5.0.4.jar \
  --cli target/release/tablecraft --rows target/release/examples/table_data \
  --ace2016 new-2016.accdb --ace2019 new-2019.accdb
```

## Exact Decimal relationship membership

Jet 4 and ACE 12/16-family Decimal keys retain exact i128 coefficients and field scale in a
separate private equality atom. Mixed integer/Currency/GUID components retain
their domains. Decimal-bearing tuples select wider, fallibly reserved vectors;
existing compact tuple allocations stay unchanged. Sorting is a private set/search
order, without SQL coercion or a physical-index ordering claim.

Corresponding Decimal fields must have identical native precision and scale,
including empty or all-NULL tables. Mismatches and older native profiles report
Unsupported; no rescaling or rounding supplies an implicit match. The existing
one-to-ten-column audit checks parent uniqueness, exact membership and one-to-one
child uniqueness. All-NULL tuples remain exempt; partial NULLs stay exact.

The original [recipe](../../testing/DecimalRelationshipFixture.java) generates 12 inert
tables/profile: scales 0/4/28, mixed Decimal/Int32/Decimal tuples with different
per-field scales, adjacent coefficients beyond f64/i64, extrema, NULLs and
one-to-one declarations. Generation independently rejects orphans, componentwise
matches, unmatched partial NULLs and duplicate children using reviewed Jackcess
5.0.4. Its relationship creator checks matching native types; TableCraft's exact
precision/scale gate deliberately qualifies a narrower profile.

[The comparison](../../testing/compare_decimal_relationships.py) matches 116 exact rows
and 12 table audits per V2016/V2019 family. Seventeen corrupt disposable copies
fail without stdout for exact orphans and duplicate complete/partial parent and
one-to-one child tuples. Two precision/scale mutations with all-NULL children
report Unsupported; another all-NULL child remains exempt. Original hashes remain
unchanged and recorded in [provenance](../../testing/FIXTURES.md#original-ace-16-decimal-relationship-inputs).

Six integration tests include every scale, extreme/adjacent coefficients, mixed
integer/Currency/GUID carriers, metadata/profile rejection, cancellation/budgets,
unchanged source bytes and a 256-case independent nullable tuple-set model. A
focused unit test retains scale/domain distinctions even for zero. Physical
index integrity remains a separate check. Native Access, cascades, general
cross-scale semantics and production ACE writes remain unqualified. Separate
[ACE 16 Decimal key previews](../../testing/ACE16_WRITE_PREVIEWS.md#decimal-slot-and-index-write-previews)
now cover existing-width slots, index maintenance and declared same-precision/scale
update cascades; they do not establish native Access write conformance.

```sh
java -cp path/to/jackcess-5.0.4.jar docs/testing/DecimalRelationshipFixture.java new-2016.accdb V2016
java -cp path/to/jackcess-5.0.4.jar docs/testing/DecimalRelationshipFixture.java new-2019.accdb V2019
python3 docs/testing/compare_decimal_relationships.py --jar path/to/jackcess-5.0.4.jar \
  --cli target/release/tablecraft --rows target/release/examples/table_data \
  --ace2016 new-2016.accdb --ace2019 new-2019.accdb
```

Fresh Jet 4/ACE 12 Decimal relationship fixtures now match the same exact membership
and malformed-copy checks; see [Jet 4 qualification](../profiles/NATIVE_JET4.md). Physical legacy
index ordering never enters foreign-key equality.

## Stored Boolean relationship membership

Boolean fields participate in the same exact stored-key audit, including mixed
Boolean/nullable integer and Decimal tuples when each paired field has the same
native type. Private Boolean atoms remain distinct from integers, including numeric
zero and minus one. A cleared native Boolean mask
bit represents **false, not NULL**; false therefore participates in parent and
one-to-one child uniqueness and always requires a matching parent. A tuple such
as `(false, NULL)` remains a partial key, not an exempt all-NULL tuple.

This is a read-only expansion. An internal explicit allowlist retains the prior
mutation key-carrier boundary even when a Boolean relationship now reports
`Checked`. Boolean-bearing relationships, including tuples that also contain
Decimal, do not thereby authorize fixed or batch edits or cascades. Unenforced
relationships retain their previous treatment. Every other mutation prerequisite
still applies to the older carrier profile.

Original synthetic native records cover true/false membership, duplicate false,
one-to-one children, exact composite tuples, mixed nullable integer and Decimal
components, paired-type and parent-index rejection, independent set-model cases,
allocation/row/work limits and cancellation after successful read prefixes. Header
variants preserve the existing Jet 4/ACE 12/16-family read gates. These controls do
not establish physical tree validity or Microsoft Access qualification.

The original [Boolean fixture recipe](../../testing/BooleanRelationshipFixture.java)
and [read-only getter oracle](../../testing/BooleanRelationshipOracle.java) exercise
Jackcess Boolean and composite relationships. A capability probe found that
Jackcess 5.0.4 rejects an explicit false child without a parent, but accepts NULL
input that subsequently reads back as stored false, even with an empty parent.
That reference insertion anomaly does not redefine stored-key membership:
TableCraft reports the resulting orphan rather than silently exempting false.
The clean corpus uses explicit Boolean values and valid parent keys; separate
negative inputs preserve the anomalous stored orphan. This is independent-reader
evidence, not a claim about Microsoft Access insertion behavior.

Microsoft's [relationship requirements](https://support.microsoft.com/en-us/access/create-edit-or-delete-a-relationship)
and [DAO index contract](https://learn.microsoft.com/en-us/office/client-developer/access/desktop-database-reference/index-object-dao),
and Jackcess's [relationship builder contract](https://jackcess.sourceforge.io/apidocs/com/healthmarketscience/jackcess/RelationshipBuilder.html)
support matching paired types and unique parent keys. They do not establish the
NULL-input anomaly as native Access semantics or qualify Boolean SQL coercion.

The [independent Boolean relationship replay](../../testing/BOOLEAN_RELATIONSHIPS.md)
passes 78 controls and 67,340 getter/index cells across ACE 2016/2019 inputs,
including real node trees, partial NULL tuples and refused orphan/uniqueness
copies. All 27 pinned hashes remain unchanged. Native Access comparison and
Boolean-key mutation qualification remain separate.
