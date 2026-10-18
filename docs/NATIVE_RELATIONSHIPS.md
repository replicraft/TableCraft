# Native relationship declarations

`inspect_relationships` reads bounded MSysRelationships declarations in the
qualified unencoded Jet 4/ACE 2007 profile. CLI: `tablecraft inspect PATH
--relationships`, conflicting with other inspection modes. All output is buffered
and escaped; errors/traces omit identities and data. No providers, rules or
cascades execute, and table records are not checked by metadata inspection.

`tablecraft-schema` owns validated Relationship/RelationshipColumn declarations
and backend-neutral RelationshipRules/ReferentialAction. They preserve unresolved
names and ordered referenced-parent/dependent-child field pairs. Empty/repeated
field mappings fail constructor validation; consumers supply name comparison and
bound widths before duplicate checking. Shared contracts import no native engine.

ACE owns native flags and system-table decoding. It groups records by relationship
name, accepts unordered composite ordinals and requires every ordinal exactly once
with consistent parent/child/count/flags. It rejects NULL/wrong carriers, invalid
names, incomplete mappings and repeated fields. Limits are 256 relationships and
256 columns per mapping, alongside existing catalog/row/page/operation limits.
Allocations and grouping/validation work are charged; cancellation is checked.

Known declarations expose integrity enforcement, one-to-one cardinality and
restrict/cascade/set-null actions. Unknown bits, inherited-provider declarations
and conflicting cascade/join combinations retain raw flags with `rules() == None`.
None means unqualified semantics, never a disabled constraint. Join-design flags
do not execute or rewrite query joins. Endpoint existence, key eligibility and
row integrity remain separate validation work.

## References and evidence

[Microsoft RelationAttributeEnum](https://learn.microsoft.com/en-us/office/client-developer/access/desktop-database-reference/relationattributeenum-enumeration-dao)
documents one-to-one, unenforced/inherited, update/delete cascade and join-design
flags. Reviewed [Jackcess DatabaseImpl](https://github.com/jahlborn/jackcess/blob/be0a2139b087a227d14cafd4ea231306a5a3a862/src/main/java/com/healthmarketscience/jackcess/impl/DatabaseImpl.java)
for MSysRelationships names, endpoint direction and composite grouping. The
previously reviewed Jackcess 5.0.4 JAR exposes cascade-null bit 8192; that action
has independent declaration evidence, not native Microsoft Access execution proof.
No third-party implementation or generated binary was copied into the repository.

[RelationshipFixture.java](testing/RelationshipFixture.java) is an original inert
recipe, refusing existing outputs, with enforced, unenforced/left-join, one-to-one,
cascade-update/delete, set-null and two-column declarations. Both V2000 and V2007
match the independent oracle across five relationships/six ordered field pairs.
The canonical sorted declarations have SHA-256
`4633c2d319c2116dae514416d0a35b4c3fcede7722134953c48e53efac05920d`.

```sh
java -Dstdout.encoding=UTF-8 -cp path/to/jackcess-5.0.4.jar docs/testing/RelationshipFixture.java new.accdb V2007
java -Dstdout.encoding=UTF-8 -cp path/to/jackcess-5.0.4.jar docs/testing/RelationshipFixture.java new.mdb V2000
python3 docs/testing/compare_relationships.py --cli target/debug/tablecraft --jar path/to/jackcess-5.0.4.jar --ace new.accdb --jet new.mdb
```

Jackcess rejects the recipe's unmatched partially NULL composite child keys while
accepting all-NULL keys. Additional original fixtures show matching partial-NULL
parents are accepted and duplicate partial unique keys are rejected. The bounded
[record audit](NATIVE_CONSTRAINTS.md#composite-independent-evidence) now implements
that exact integer tuple profile, including qualified Int64 and mixed tuples
with matching paired types; native Access comparison remains unrun. Metadata
inspection alone does not enforce it. Generated files/JARs stay local;
[fixture records](FIXTURES.md) identify inputs. Original bytes remain unchanged.

Tests include real candidate declarations, malformed/missing/repeated ordinals,
conflicting metadata, unknown flags, shared budgets/cancellation, constructor
invariants and a 256-case fixed-seed ordinal-order property. CLI tests verify
buffering, argument conflicts, inert output and diagnostic redaction. Native
Access, relationship mutation and write-time/cascade enforcement remain unrun.

[ACE 16-family comparisons](NATIVE_ACE16.md) now exercise these bounded reads on
original V2016/V2019 fixtures. Native Access comparison and ACE writes remain unqualified.
