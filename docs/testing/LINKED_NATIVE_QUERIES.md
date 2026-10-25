# Explicit one-hop linked native query evidence

Reviewed 24 October 2026. Independent original getter records now exercise the
Access adapter's explicit already-open source facade and the actual CLI
`query run --link ALIAS PATH` consumer. This is bounded engineering read/query
evidence, not native Access linked SQL, live cross-file snapshot or save evidence.
The [inert metadata contract](../NATIVE_LINKED_TABLES.md) remains separate from
authorization to open targets.

## Original corpus and oracle

The original [producer](LinkedQueryFixture.java) creates separate frontend/backend
pairs for Jet 4, ACE 2007, ACE 2016 and ACE 2019, plus a mixed Jet 4 frontend/ACE
2016 backend pair. Public table builders create nullable ordinary data: six people,
six orders, an empty remote table and three frontend rates. Public
`createLinkedTable` creates four mappable root aliases, including two aliases for
one physical People table. An unrelated unmapped root link and a backend link
chain point to a nonexistent target. Neither is opened by the positive queries.

The frontend additionally contains one inert saved SELECT root. Its START, SELECT,
TABLE, COLUMN, WHERE, ORDER and END records are authored through lower-level
`QueryImpl.Row`/MSysQueries metadata calls under the existing reconstruction
profile. This is not an ordinary saved-query writer API, reference SQL execution
or new native availability evidence. Expression evaluation is disabled throughout.

The [oracle](LinkedQueryOracle.java) reopens both explicitly named originals
read-only, installs throwing link resolvers and reads local tables separately.
It records raw scalar values, frontend link metadata, signed catalog IDs, inert
saved SQL reconstruction and exact column text-order identities. Ordinary values
come from public getters; text-order metadata uses pinned implementation getters.
All reference resolver-call counts remain zero. No reference engine executes SQL,
macro, VBA, expression, provider or imported application content.

The Python model derives joins, nullable/unmatched rows, filters, grouping, counts,
sums and correlated counts from those ordinary getter records. Unicode labels
are preserved verbatim. The tested stored-General case comparisons use only the
ASCII alpha/ALPHA and beta/BETA pairs, not an inferred general linguistic oracle.
The example's custom ordinal provider is explicit engineering Unicode-scalar
lexicographic `str::cmp`, matching the CLI's ordinal profile. It does not claim
native binary or UTF-16 `StrComp` semantics.

| Profile pair | Frontend Label order | Backend Label order |
| --- | --- | --- |
| Jet 4 / Jet 4 | General 1033, version 0 | General 1033, version 0 |
| ACE 2007 / ACE 2007 | General 1033, version 0 | General 1033, version 0 |
| ACE 2016 / ACE 2016 | General 1033, version 1 | General 1033, version 1 |
| ACE 2019 / ACE 2019 | General 1033, version 0 | General 1033, version 0 |
| Jet 4 / ACE 2016 | General 1033, version 0 | General 1033, version 1 |

These are the actual observed reference declarations. The mixed pair preserves
each source's field policies: cross-source conflicting text comparisons refuse
under stored-policy mode and succeed under explicit ordinal mode. Numeric joins
and unrelated text projections continue working. Header/version names alone do
not select or erase a field's stored policy.

## Explicit authority and refusal controls

The [engineering example](../../crates/backends/tablecraft-access/examples/linked_query.rs)
opens only its two host path arguments. It supplies exact root alias/native ID,
expected Database string, an already-open target handle, target table and target
native ID. All routes share one cumulative native pool; the same target handle
serves several tables and aliases. Comparison, query and output allowances remain
separate. Output is fully buffered and uses bounded tagged records with Base64
Text transport.

The CLI replay uses only explicit `--link ALIAS PATH` arguments. It captures the
mapping's current native metadata under the stable-source precondition; it does
not claim the engineering example's separately supplied historical ID pins.
No stored Database/Connect string determines a path, prompts a provider or opens
a link chain. Local queries succeed with no mappings despite unrelated root links;
accessing an unmapped link or a backend-only name refuses.

Five separately named frontend control copies per profile use lower-level public
MSysObjects updates to alter a selected link's provider, ODBC type, Database string,
native identity or ForeignName-to-chain. These are explicit metadata controls,
not additional native writer validity qualifications. Original inputs remain
unchanged. The engineering facade refuses stale supplied IDs/expected metadata,
extra/duplicate mappings, providers and link chains. Local queries with zero
mappings still succeed on those copies.

Matching the changed opaque Database string explicitly still uses the supplied
backend handle successfully. The actual CLI likewise reads the host-selected
backend despite that stored string naming a nonexistent file. This proves the
tested authority separation, not a native filesystem/path identity relationship.

A separate [public-link recipe](LinkedQueryCaseFixture.java) adds `Lowercase link`
with stored ForeignName `remote people` to fresh copies. Reopened getters verify
the exact stored spelling. CLI queries resolve it to the supplied backend's
canonical `Remote People` table, including case-varied host aliases. Provider,
ODBC, duplicate mapping, unknown target field, required missing index and late
division failures publish no stdout.

Zero CLI limit values are classified as argument/limit refusals, **not** runtime
depletion evidence. Twenty-five additional positive-one native byte/row/step,
execution-row and output ceilings pass argument admission and fail at runtime
without stdout. These do not independently prove every internal charge category;
the service's focused shared-pool tests cover aggregate accounting. The example's
native zero pools are accepted arguments and fail through its native read budget.

## Accepted results

Keep the two consumer results separate: they exercise the same getter model and
many of the same cells through different executables.

| Consumer | Passing controls | Main result cells | Unchanged pinned hashes |
| --- | ---: | ---: | ---: |
| [Already-open example](results/linked-queries-final-2026-10-24.json) | 335 | 1,779 | 49 |
| [Actual CLI](results/linked-cli-final-2026-10-24.json) | 330 | 1,779 | 48 |

The final immutable example SHA-256 is
`b8903dd27c229da51687b6607164e053f619b6d09335f8d6d5ad68a980dd3607`.
The final immutable CLI SHA-256 is
`820562569b83061a0129c9140ac95f4225849f404c831d4718202c0e40dd1b46`.
Both reports recheck originals, selected control copies, recipes, relevant source
files, the reviewed reference artifact and executable hashes after replay.
The CLI runner also verifies every engineering-report hash before and after its
own work. Reference JAR, databases, executables and preliminary probes remain
ignored durable local artifacts.

Scoped example check, strict Clippy, formatting and Python compilation pass.
The initial 335-control example report predating the grouping cleanup and
the initial 305-control CLI report without positive-one runtime limits are
preserved privately and excluded from the accepted final record. Initial V2016
producer probes are also excluded. No accepted source or input changed between
the final replay and hash review.

```sh
python3 docs/testing/compare_linked_queries.py \
  --jar .cache/tablecraft-fixtures/jackcess-5.0.4.jar \
  --reader .cache/tablecraft-bin/linked-native-example-final-20261024 \
  --directory .cache/tablecraft-fixtures/linked-query-final-20261024 \
  --report /tmp/linked-query-replay.json

python3 docs/testing/compare_linked_cli.py \
  --cli .cache/tablecraft-bin/linked-native-cli-final-20261024 \
  --jar .cache/tablecraft-fixtures/jackcess-5.0.4.jar \
  --directory .cache/tablecraft-fixtures/linked-query-final-20261024 \
  --engineering-report docs/testing/results/linked-queries-final-2026-10-24.json \
  --report /tmp/linked-cli-replay.json
```

Fresh engineering generation uses `--generate` with new per-profile directories.
Fresh CLI case additions use `--generate-case-copies`; ordinary replay writes no
database. [Fixture provenance](../FIXTURES.md#original-explicit-linked-query-inputs)
records original pairs and controlled-copy boundaries. Native Access query
comparisons, live cross-file coherence, authentication qualification, linked
writes, automatic provider traversal and application save/reopen remain separate.
