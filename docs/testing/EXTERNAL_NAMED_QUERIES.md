# External named-source replay

The [accepted replay](results/external-named-2026-10-24.json) passes 588 controls
and 1,368 getter-modeled result cells over original root/A/B trios in Jet 4
(`V2000`), ACE 2007 (`V2007`) and ACE16-family (`V2016`, `V2019`) formats.
All 1,902 pinned file hashes remain unchanged, including every file in the
1,857-entry frozen Rust source inventory, original inputs and getter captures.

There are 192 exact raw fragment inspections, 192 inert reconstructions,
124 successful execution commands and 80 classified buffered refusals. These
are independent-reader and TableCraft engineering results. They do not qualify
Microsoft Access saved-query authoring, execution, Windows locale behavior or
native save/reopen compatibility.

## Reference and authoring boundary

Microsoft's [IN clause documentation](https://learn.microsoft.com/en-us/office/vba/access/concepts/miscellaneous/in-clause-microsoft-access-sql)
explicitly permits a saved query as the source table expression. That establishes
the external named-source use case. The lexical ownership, multiple explicit
host mappings, cache isolation and closed Eval rules tested here remain
TableCraft engineering contracts; that page does not independently establish
all of those namespace details.

[ExternalNamedFixture.java](ExternalNamedFixture.java) uses public Jackcess
ordinary table builders to create Items, and public system-table row APIs to
author original inert catalog/MSysQueries fragments. It disables expression
evaluation, rejects existing outputs and installs a throwing link resolver.
Lower-level fragment authoring is explicit: type/SELECT flags and ordered rows
are supplied directly, without START/END fragments. This is not a public
SaveQuery API or a record of Access-generated queries.

Each profile has three original files with deliberately conflicting names:

| Node | Item IDs | Scores | Quarters | Rows |
| --- | --- | --- | --- | --- |
| root | 1001, 1002 | 100, 110 | 9, 9 | 2 |
| one | 101, 102, 103 | 10, 20, 30 | 1, 2, 1 | 3 |
| two | 201, 202, 203, 204 | 1, 2, 3, 4 | 2, 3, 2, 3 | 4 |

The 12 files contain 36 ordinary rows overall. Items has Int32 ID/Score/Quarter
and Short Text Formula; Formula is the original closed expression `ID+100`,
using the row's literal ID. All files intentionally reuse the same native
saved-query IDs and names, so a global query-name or native-ID cache would be
incorrect. Each file also contains inert LinkedItems metadata with an unusable
stored target; it is never opened by the reference reader.

The 16 definitions per file cover Same/Nested, Owned, UNION ALL, fixed/dynamic
crosstabs, parameters, an explicit third-token redirect, and refusal cases.
Owned contains a nested local Count, base/named DCount and Eval(Formula). Saved
crosstabs compute Sum(Score) under a constant row key and fixed or discovered
Quarter headings. No reference SQL or saved definition is executed.

[ExternalNamedOracle.java](ExternalNamedOracle.java) reopens each file read-only,
disables expressions and installs a throwing resolver. It records public query
ID/family/type/SQL and remote path/type getters, raw catalog/query fields
including Order bytes, and ordinary Items column types and raw cells. UTF8/base64
encoding keeps NULL/empty text and binary Order distinct. It reads only Items,
not the declared linked table, and asserts zero linked handles.

The reviewed Jackcess 5.0.4 JAR is Apache-2.0, SHA-256
`19a7494a56f439a57fda85c059b019b665802085cec5ae8186d1e028cc1bdfbc`.
These original Java/Python tools follow the repository MIT license. No
third-party implementation source or fixture bytes are copied. Original binaries,
getter stdout/stderr and generation records stay ignored under
`.cache/tablecraft-fixtures/external-named-20261024/`. The generation ledger
associates each file/getter with its producer command; the producer is invoked
once per profile and creates all three fresh files.

## Independent execution model

[compare_external_named.py](compare_external_named.py) derives expected scans,
counts, integer sums, pivot headings and row products directly from ordinary
getter records. Formula expectations split the original two literal integers
and add them; Python eval and reference SQL are never used. Raw inspect compares
every fragment value and multiplicity with the getter, together with native ID,
family flags and row count. Reconstruction checks success, separately from
execution; it does not claim a full normalized AST equivalence oracle.

The host explicitly maps `one` to A, `two` to B and `third-token` to B. The
last mapping is independently authorized; a stored token never opens a file.
The execution controls distinguish:

- Same/Nested local dependencies in each owner, including a simultaneous
  root/A/B product with matching names and native IDs.
- Target-local nested SELECT, base/named domain counts and the written Formula
  argument. Root sibling domains and caller-written scalar subqueries without
  IN remain root-owned.
- UNION arms, fixed/dynamic crosstab discovery and reuse in three simultaneous
  namespaces, with different headings and values.
- Parameters in individual owners and a simultaneous three-owner parameter
  query, with one explicitly supplied compatible Floor value.
- A target definition's written IN resolving through the operation-root mapping
  registry, with missing authorization refusing rather than opening a path.
- Explicit root saved execution retaining its ordinary root namespace.

Closed Eval receives Formula text from the target row but no bindings from its
owner. Eval('ID') refuses an unbound field, and Eval containing DCount refuses
an unsupported expression. The corpus also requires dependency-cycle,
undeclared/absent/transitive parameter, missing/duplicate mapping, provider and
linked-base guard refusals. A final-row division error must leave stdout empty.

Positive native byte/row/step, execution byte/row/step and output ceilings produce
runtime resource/output refusals. The runner checks specific diagnostic classes
and empty stdout, using valid positive limits. This is not an independent proof
of every cancellation, revision-drift or exact cumulative charging boundary;
those retain their service-level tests.

## Frozen artifacts and reproduction

The accepted CLI is `.cache/tablecraft-bin/external-named-cli-20261024`, SHA-256
`8b9c8e89f6804f57e45a6bba74cf7b6ebbdb8b7a20917cd8b1170c6e31710d23`.
Its source inventory is `.cache/external-named-rust-source-20261024.manifest`,
SHA-256 `267d68a4ddc530286bae948e58a494bf04ff5b533af56cecb9cc00e8a90def9d`.
The runner validates every entry before and after execution, in addition to
pinning the manifest itself.

```sh
javac -cp .cache/tablecraft-fixtures/jackcess-5.0.4.jar \
  -d .cache/external-named-classes-20261024 \
  docs/testing/ExternalNamedFixture.java docs/testing/ExternalNamedOracle.java
java -cp .cache/tablecraft-fixtures/jackcess-5.0.4.jar:.cache/external-named-classes-20261024 \
  ExternalNamedFixture .cache/tablecraft-fixtures/external-named-20261024/V2016 V2016
java -cp .cache/tablecraft-fixtures/jackcess-5.0.4.jar:.cache/external-named-classes-20261024 \
  ExternalNamedOracle .cache/tablecraft-fixtures/external-named-20261024/V2016/root.accdb
```

Repeat for one/two and all four profiles; V2000 uses `.mdb`. Capture each getter
as `root.accdb.getters.tsv` / `.getters.stderr`, and similarly for other nodes.
Fresh timestamps may change file hashes; the accepted report pins the preserved
originals rather than requiring reproducible creation timestamps.

```sh
python3 docs/testing/compare_external_named.py \
  --cli .cache/tablecraft-bin/external-named-cli-20261024 \
  --cli-sha256 8b9c8e89f6804f57e45a6bba74cf7b6ebbdb8b7a20917cd8b1170c6e31710d23 \
  --jar .cache/tablecraft-fixtures/jackcess-5.0.4.jar \
  --directory .cache/tablecraft-fixtures/external-named-20261024 \
  --source-manifest .cache/external-named-rust-source-20261024.manifest \
  --manifest-sha256 267d68a4ddc530286bae948e58a494bf04ff5b533af56cecb9cc00e8a90def9d \
  --report docs/testing/results/external-named-2026-10-24.json
```

The runner refuses to overwrite an existing report. Accepted JSON retains every
command, exit status, stdout, stderr, expected refusal class and hash. Its terminal
log is `.cache/external-named-final-20261024.log`. The passing exploratory replay
and an older CLI's 16 inert reconstruction probes remain private and outside
accepted counts. No failed model cases or patched original inputs were hidden.

Provider activation, implicit filesystem traversal, live Access coordination,
target linked chains and native Windows saved-query qualification remain outside
this evidence. See the [working feature contract](../EXTERNAL_NAMED_QUERIES.md).
