# Boolean relationship read audits

The original [producer](BooleanRelationshipFixture.java) and
[read-only oracle](BooleanRelationshipOracle.java) exercise stored Boolean
foreign-key memberships in ACE 2016/2019 files. This is independent offline
engineering evidence, not native Access write, cascade or SQL qualification.

## Observed reference capability

The reviewed Jackcess 5.0.4 public RelationshipBuilder creates enforced
relationships with Boolean primary/foreign keys and Boolean-bearing composite
unique keys. The fresh getter records confirm actual BOOLEAN declarations,
stored RI flags and ordered field pairs. Primary Boolean indexes are unique and
required; mixed unique indexes can retain nullable Int32 components. Missing
Required *column properties* do not imply a nullable Boolean storage carrier.
All observed Boolean stored values are true or false; NULL inputs become false.

An important reference behavior is retained explicitly: a NULL Boolean child
input bypasses its writer's RI input check and becomes a stored false even with
an empty parent table. Explicit false is rejected in the same situation. Likewise,
all-NULL composite inputs become false/NULL/false, which are not all-NULL stored
keys. The separate `--null-orphan` producer mode preserves this inconsistent
stored input as a negative read-audit case. It is not evidence for a valid NULL
Boolean representation or an exemption from stored membership checks.

Earlier capability-only probes also observed that an explicit partially NULL
Boolean/Int32 tuple requires an exact parent, and that inserting NULL into a
Boolean primary key duplicates an existing false value. These probes are
excluded from accepted replay counts; the committed producer and oracle own the
reproducible accepted workload.

## Original workload

Each clean file contains 10 tables, 6 enforced relationships and 5,714 rows:
Boolean parent/child, one-to-one Boolean children, a true-only parent with many
children, empty parent/child tables, and mixed Boolean/Int32/Boolean parent,
child and one-to-one child tables. Both Boolean states and all four Boolean
combinations with a NULL Int32 component have matching stored parents. NULL
child inputs are admitted only where their resulting false values have parents.

The mixed parent unique key is descending Boolean, ascending Int32 and descending
Boolean; generated foreign indexes and relationship field pairs retain their
actual recorded order. Four large indexes per profile have real node/leaf trees,
including repeated single-Boolean keys and composite parents/children. The oracle
records index flags, direction, RI metadata, exact ordinary getters and public
row locations without executing expressions or application content.

## Independent replay

The [runner](compare_boolean_relationships.py) independently checks exact stored
tuple memberships, parent uniqueness and one-to-one child uniqueness. It compares
ordinary CLI query results with tagged getters, index-ordered query results with
independent index cursors, and complete read-audit coverage for both endpoints.
It also verifies actual node-page roots rather than assuming row count establishes
a multi-page index.

Disposable copies alter late Boolean children, nullable composite keys and
parent/one-to-one uniqueness. These failures must publish no stdout. A separate
original negative file remains browsable as stored false, while explicit audit
must reject its missing parent. This distinguishes ordinary stored reading from
asserting relationship consistency. Originals, reference artifact, implementation,
helpers, producer/oracle/runner and immutable executable hashes are checked before
and after replay. Generated databases and compiled classes stay ignored.

## Accepted results

On 24 October 2026, the [accepted report](results/boolean-relationships-2026-10-24.json)
records 78 passing controls and 67,340 getter/index result cells across both
profiles. Each clean input retains 5,714 rows and four confirmed node/leaf roots.
Fourteen disposable corrupt copies are refused without stdout, and both original
NULL-input orphan files remain browsable while correctly failing stored membership
audit. All 27 pinned source, helper, reference JAR, input and executable hashes
remain unchanged after replay.

The immutable CLI SHA-256 is
`945882fb9dfc1c67b953458923e4a9a39c09f9bd21cbec954f342b9f051ad245`.
These checks broaden read-audit coverage only. They do not authorize native
Boolean-key edits or qualify cascades, shared-file behavior or Access execution.
[Fixture provenance](../FIXTURES.md#original-boolean-relationship-inputs) records
all original file hashes, ownership and excluded capability probes.

```sh
python3 docs/testing/compare_boolean_relationships.py \
  --cli .cache/tablecraft-bin/boolean-fk-audits-20261024 \
  --jar .cache/tablecraft-fixtures/jackcess-5.0.4.jar \
  --directory .cache/tablecraft-fixtures/boolean-relationships-20261024 \
  --report /tmp/boolean-relationships.json
```
