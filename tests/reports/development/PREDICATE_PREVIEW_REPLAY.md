# Independent row-local predicate previews

The explicit calculation preview admits pure same-row comparison, logical and
NULL-test syntax. This record checks recomputed output from ordinary native
getters independently of stored calculated Boolean caches. It does not qualify
Microsoft Access formula execution, freshness, cache replacement, writes or
broader formula admission.

## Original inert corpus

[PredicatePreviewFixture.java](PredicatePreviewFixture.java) uses reviewed
external Jackcess 5.0.4 public builders with expression evaluation disabled. Fresh
ACE 2016/2019 files contain `Predicates` and its empty counterpart. The populated
table has ordinary nullable Int32 inputs `-2, -1, 0, 1, 2, NULL`, ordinary row IDs
and 29 calculated Boolean declarations. Public getters report
capacity-39 calculation metadata with typed BYTE ResultType 1 and exact stored
Expression strings.

The 26 admitted formulas cover all six comparisons, Not/And/Or/Xor/Eqv/Imp,
IS NULL/IS NOT NULL and mixed constant/NULL truth boundaries. Those boundaries
include false AND unknown, true OR unknown, false IMP unknown, unknown IMP true
and NOT unknown. Every counted preview cell deliberately differs from its stored
cache: the producer stores the inverse of a non-NULL modeled Boolean and TRUE for
a modeled NULL. This cache-authoring choice is separate from the independent
Python model used to verify previews.

Two additional declarations use a function call and a calculated-field dependency;
both must remain refused before reading rows, including empty tables. The final
declaration is `([Input]<>1) Or (1/([Input]-2)>0)`. The fifth input makes its eager
right operand divide by zero even though the left operand is true. Populated
preview must fail without publishing earlier results; the empty counterpart
publishes only its `Recomputed` heading. No formula executes inside the producer
or oracle.

[PredicatePreviewOracle.java](PredicatePreviewOracle.java) reopens originals
read-only with expression evaluation disabled. It emits exact typed declaration
properties, ordinary Int32/NULL getters and stored Boolean cache getters.
[The runner](compare_predicate_previews.py) verifies exact formula identity before
using original Python three-valued truth functions and integer comparisons.
Stored formula text is never evaluated by Python. Only the CLI's explicit
restricted preview evaluates a selected admitted expression.

## Results and boundaries

On 24 October 2026, the [accepted report](results/predicate-previews-2026-10-24.json)
records **248 passing operation controls** and **312 independently modeled
recomputed cells**, all 312 distinct from their stored caches. The files contain
116 calculated declarations including 58 empty-table declarations and 348 stored
cache cells. The counted recomputation cells belong to the 26 admitted formulas;
the refused/late-error declarations are not counted as successful recomputations.

Native byte, query byte, query step and output limits are checked separately on
both populated and empty tables, with their actual error categories asserted.
All failures must leave stdout empty. The empty admitted declarations check
metadata-only admission without inventing input rows. Calls and calculated
dependencies remain refused even when no rows exist.

All twelve pinned recipe/oracle/runner/helper, input, reference JAR, calculation
service/syntax, CLI consumer and immutable executable hashes remain unchanged
after replay. The immutable CLI SHA-256 is
`4a6cb30c06f086d3b3416b0c5bbcb98d47c4056934bb3deff2a7dee7ac2589d8`.
No production source was changed by this evidence slice, and no CLI was relinked
by the replay.

## Provenance and reproduction

The producer, getter oracle and Python runner are original repository code under
MIT. No third-party implementation or fixture is copied. The external Jackcess
5.0.4 JAR retains its existing Apache-2.0 review and SHA-256
`19a7494a56f439a57fda85c059b019b665802085cec5ae8186d1e028cc1bdfbc`.
Generated databases, classes and executables remain ignored.

Original inputs under
`.cache/tablecraft-fixtures/predicate-previews-final-20261024/`:

| File | Bytes | SHA-256 |
| --- | ---: | --- |
| V2016.accdb | 401,408 | `72d99e718ae6b9f2bf54dcfb7c09743995aa454c5079515555bb44c8554dba1b` |
| V2019.accdb | 475,136 | `e0073ed7ab96a13c1eeca51fd85d6053a08535f04c15f802724e4315910ba6e2` |

Producer SHA-256:
`6a69199d152e02917f351be55e54773316350e2ff8453214aa648dc64b0133b2`.
Oracle SHA-256:
`793e64d0febc13c1f028c79e0f22c8237cc84f158d0a47f5a5b81d16806a1dc1`.
Runner SHA-256:
`93c4a35cb7630bd7c583167a329d0ea3e1c70e6e79fc90cd4891ed0bf111161b`.

The earlier files under `predicate-previews-20261024/` are excluded capability
inputs created before adding the constant/NULL truth-boundary family. Accepted
results use only the final originals above. No original is mutated and no active
database content is executed implicitly.

```sh
python3 docs/testing/compare_predicate_previews.py \
  --cli .cache/tablecraft-bin/predicate-preview-20261024 \
  --jar .cache/tablecraft-fixtures/jackcess-5.0.4.jar \
  --directory .cache/tablecraft-fixtures/predicate-previews-final-20261024 \
  --report /tmp/predicate-previews.json
```
