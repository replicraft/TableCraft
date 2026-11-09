# Explicit external IN getter evidence

This corpus compares read-only queries with an independent model built from
ordinary native table getters. Both the engineering example and CLI receive
explicit host paths. An SQL database token selects an authorized mapping; its
spelling never grants filesystem access or identifies a physical file.

Microsoft's [IN clause](https://learn.microsoft.com/en-us/office/vba/access/concepts/miscellaneous/in-clause-microsoft-access-sql)
describes external table expressions containing a table, saved query or join.
The [FROM contract](https://support.microsoft.com/en-gb/access/from-clause)
places the tables in the selected external database. These references support
the direct table-expression namespace. They do not independently settle nested
query inheritance, correlation, domain lookup or named-definition expansion.
The following are explicit current engineering rules, pending Access comparison:

- IN selects every direct table leaf of its SELECT, including joined/comma leaves.
- Derived SELECTs and scalar/predicate subqueries choose their own namespace:
  an absent IN selects the root; their own IN selects its explicit binding.
- Correlation retains the outer bound row, independently of the inner namespace.
- Root named definitions expand in the root namespace. Domain functions use root.
- External FROM exposes local target tables; target named queries, linked tables
  and provider activation remain refused.

The target is not opened from SQL. Already-open borrowed native handles share
one cumulative native budget, distinct catalog scopes and invocation-local source
keys. The CLI opens only paths explicitly supplied with `--external TOKEN PATH`
or existing `--link ALIAS PATH`. The engineering example receives both explicit
paths before constructing mappings. Neither mode executes reference SQL or
imported active database content.

## Original inputs and independent model

The ignored corpus has five freshly authored pairs: Jet 4/V2000, ACE 2007/V2007,
ACE 16-family V2016/V2019 and a mixed Jet 4 root/V2016 target. It reuses the original
[linked-query producer](LinkedQueryFixture.java), then applies public writer
additions through [ExternalQueryFixture.java](ExternalQueryFixture.java). Earlier
accepted linked inputs are unchanged.

Root and target have deliberately conflicting `Remote People`, `Remote Orders`
and `Rates` values. Root has three people while target has six; root rates differ
from target rates by both values and membership. A root-only table catches any
silent fallback from target lookup. Root `RootOnlyIndex` and target `ExternalId`
are independently authored indexes on their corresponding ID columns.

[ExternalQueryOracle.java](ExternalQueryOracle.java) reads only explicitly opened
local tables, index declarations and ordinary cell getters. The reference
expression evaluator is disabled, and a throwing link resolver records zero
target traversals. The pinned reference implementation's `ColumnImpl` metadata
getter supplies sort-order codes; this is not an exclusively public API oracle.
The existing linked getter supplies inert mapping IDs and root saved-query text.
Its lower-level `QueryImpl.Row`/MSysQueries fragments are syntax fixtures, not
evidence of public query saves or reference/Access SQL execution.

[The independent runner](compare_external_queries.py) builds joins, grouping,
filters, correlations and expected namespace differences from these ordinary
getter records. Numeric keys and simple ASCII comparison controls avoid inventing
native linguistic rules. The example's ordinal provider uses explicit Unicode
scalar lexicographic comparison, matching the CLI ordinal profile; it is not a
native binary/UTF-16 comparison qualification. Mixed stored comparison policies require explicit
compatibility: the mismatching mixed-profile text join refuses rather than
silently selecting one database's policy.

The reviewed reference is Jackcess 5.0.4, Apache-2.0, SHA-256
`19a7494a56f439a57fda85c059b019b665802085cec5ae8186d1e028cc1bdfbc`.
Recipes, oracle, runner and engineering example are original repository code.
No third-party fixture binary or implementation source is copied or tracked.

## Controls and exclusions

Controls distinguish direct IN from root lookup, joined/comma leaves from
separate derived SELECTs, nested default-root from nested own-IN and outer bound
correlation from source selection. Root named retrieval composes with an external
scalar subquery. Domain counts remain root-local. Unmapped tokens, case changes,
provider strings, target link chains, target named lookup and missing target
tables refuse with no stdout.

A positive authority control explicitly maps the same token to an alternative
already-open database and verifies its different rows. Conversely, an existing
file path written in SQL remains unavailable without an exact mapping. Neither
observation establishes native path identity semantics. Dedicated CLI external
index controls select `ExternalId`, reject missing/root-only indexes and keep
ordinary root index configuration separate.

Positive native byte/row/step, query-row and output limits check buffered refusal.
A final-row division failure and eager AND failure check that no successful
result prefix reaches stdout. These controls exercise actual runtime errors;
they do not establish every possible quota or cancellation path.

The initial unparenthesized JOIN probe failed in parsing before native query
execution. Its command/result and initial probe runner remain ignored diagnostic
artifacts, excluded from accepted evidence. Parenthesized table expressions
passed the initial engineering replay; the final corpus also retains equivalent
unparenthesized joins after the parser correction. A preliminary unindexed fresh
pair inventory is likewise excluded and unchanged. Accepted input files use the
separate indexed corpus directory.

An initial final-binary attempt stopped on the new `VarType(ID)` control because
the engineering formatter lacked its Int16 result carrier. The formatter now
encodes that bounded integer with the existing integer tag; the rebuilt example
retains the carrier control. That failed attempt is excluded, with no production
query behavior or expected result weakened.

## Accepted replay

The [2026-10-24 final report](results/external-native-2026-10-24.json) records
350 engineering controls with 2,154 modeled cells and 375 actual CLI controls
with 2,214 modeled cells. The two consumer counts remain separate; they reuse
the same original getter records and include equivalent grouped/ungrouped SQL.
All 75 source, executable, manifest, reference, input and getter-record hashes
remain unchanged before and after execution. Ten original databases and their
five captured getter outputs remain ignored under
`.cache/tablecraft-fixtures/external-query-indexed-20261024/`.

The immutable example is
`.cache/tablecraft-bin/external-native-example-final-v2-20261024`, SHA-256
`e8649bf8fbf4e27ce0c43b32540382db088c519e24acead5e0cfe49818a34213`.
The actual CLI is `.cache/tablecraft-bin/external-native-cli-final-20261024`,
SHA-256 `2fea30893b358d283b895049abc47d484d946e8232e8359dc6eed175fdcf1a83`.
Exact runner arguments and all 725 command outcomes are in the report. Replays
must choose a fresh report path: the runner refuses to overwrite evidence.

Fresh inputs are authored by invoking `LinkedQueryFixture.java` with a fresh
profile directory and its root/target format arguments, then invoking
`ExternalQueryFixture.java` with the resulting frontend/backend paths. Run
`ExternalQueryOracle.java` read-only on that pair and capture UTF-8 `getter.tsv`
beside it. Use the reviewed JAR for each Java source invocation. Formats are
V2000/V2000, V2007/V2007, V2016/V2016, V2019/V2019 and V2000/V2016 for Mixed.
The ordinary data and indexes are public-writer additions; lower-level saved
syntax provenance remains distinct as described above.

The final durable local log is
`.cache/external-native-final-v2-replay-20261024.log`. The initial 300/325-control
probe report, initial parser failure, prior probe runner and Int16 formatter
failure remain excluded ignored diagnostics. No database bytes or frozen
executables are tracked. Windows Access execution and broader external
named-query/provider interoperability remain unqualified.
