# Saved external-source reconstruction evidence

The [accepted replay](results/saved-external-2026-10-24.json) passes 188 CLI
controls across fresh Jet 4 (`V2000`), ACE 2007 (`V2007`) and ACE 16-family
(`V2016`, `V2019`) frontend/backend pairs. All 109 pinned file hashes remain
unchanged before and after replay. The frozen executable is
`.cache/tablecraft-bin/saved-external-cli-20261024`, SHA-256
`dc67d1ae174734112b412892c5bc8054d4408abffc1cc5f79ac6769bdbd09007`.

This is original inert metadata-authoring, independent-reader and explicitly
mapped TableCraft execution evidence. It is not Microsoft Access saved-query
creation, execution or save/reopen qualification. The lower-level authoring
method deliberately supplies native catalog/query fragments; it does not prove
that Access would author every tested representation.

## Producer and independent getter boundary

[SavedExternalFixture.java](SavedExternalFixture.java) uses Jackcess public
ordinary table builders for two root People rows and three distinct backend
People rows, plus four backend Orders rows. It writes original catalog and
MSysQueries rows through table APIs for 12 definitions per frontend. The
fragment stream contains type and SELECT flags followed by ordered fragment
rows; it has no synthetic START/END fragments. Query IDs, family flags,
parameter declarations and labelled UNION arms are explicit original inputs.
No reference query is executed.

[SavedExternalOracle.java](SavedExternalOracle.java) reopens each file read-only
with expression evaluation disabled and a throwing link resolver. It records
public query ID/family/type/SQL getters, SELECT/crosstab remote path/type getters,
exact raw catalog/query fields (including NULL, empty strings and Order bytes),
and ordinary table cells. Its tagged encoding distinguishes absent text from
present empty text. It asserts that no linked target handles were opened.
The inspected/rendered reference SQL is evidence only and is never executed.

All four profiles preserve these exact public remote getter pairs:

| Definition | Path | Connection |
| --- | --- | --- |
| EmptyPath | Present empty string | NULL |
| PathOnly | `remote-token` | NULL |
| EmptyConnection | `remote-token` | Present empty string |
| NullPathConnection | NULL | `MDB_FILE;` |
| WithConnection | `remote-token` | `MDB_FILE;` |
| LiteralPunctuation | `opaque ' é [x]; -- token` | `TYPE]; ' marker` |

The raw remote row has attribute 4, flag 2, NULL LvExtra and NULL Name2. The
reference SQL getter renders an empty path as `IN ''`, a NULL path with a
connection as `IN '' [MDB_FILE;]`, and an empty connection as
`IN 'remote-token' []`. TableCraft independently preserves the exact owned
strings and escapes punctuation for display; it does not interpret that
punctuation as authority or executable clauses.

The reader reference is Jackcess 5.0.4, SHA-256
`19a7494a56f439a57fda85c059b019b665802085cec5ae8186d1e028cc1bdfbc`.
Jackcess is Apache-2.0; no implementation source or third-party fixtures are
copied here. These original Java/Python recipes follow the repository license.
The original binary fixtures and getter captures remain ignored under
`.cache/tablecraft-fixtures/saved-external-20261024/`.

## Independent model and accepted controls

[compare_saved_external.py](compare_saved_external.py) models values directly
from reopened ordinary getters. It does not run reference SQL. Each profile has:

- 12 inert inspect controls comparing every fragment field and duplicate count
  against the raw getter records, including query ID, family flags and row count.
- 12 inert reconstruction controls, including exact opaque qualifier text,
  parameterized joins, fixed/dynamic crosstabs and two independently scoped UNION
  arms. The UNION behavior predates this reconstruction change.
- Nine successful execution controls: saved path-only, root named dependency,
  explicitly mapped empty token, UNION, parameterized join, both crosstabs,
  root-domain count and an ordinary unmapped root-local query.
- 14 buffered refusal controls: four connection-bearing requests, missing/wrong
  or duplicate mappings, a late division error, and six positive runtime resource
  limits (native bytes/steps, execution bytes/rows/steps and output bytes).

The total is 132 successful commands and 56 refusals. Refusals must have nonzero
exit status and empty stdout, plus the expected division/resource/output,
unsupported-feature or duplicate-argument diagnostic classification. Reconstruction
controls check qualifier display fragments rather than whole normalized AST
identity; owned AST contracts are tested separately in Rust. Positive limit controls exercise valid argument
values rather than clap's zero-range checks; they do not independently exhaust
every cumulative resource or replace service-level budget/cancellation tests.

Explicit mappings select already chosen files: `remote-token` selects backend
People/Orders, while `second-token` selects frontend People. Same-name rows
therefore distinguish real source selection. DCount remains in the root
namespace (count 2 rather than backend count 3); that is the documented
TableCraft engineering namespace contract, not independently qualified Microsoft
namespace behavior. Provider-bearing definitions reconstruct but refuse native
execution, including present-empty connections. Inspection/reconstruction never
need a mapping and never resolve the stored token automatically.

## Reproduction

Compile the original tools with the reviewed JAR and create each profile in a
fresh directory; the producer refuses existing outputs:

```sh
javac -cp .cache/tablecraft-fixtures/jackcess-5.0.4.jar \
  -d .cache/saved-external-classes-20261024 \
  docs/testing/SavedExternalFixture.java docs/testing/SavedExternalOracle.java
java -cp .cache/tablecraft-fixtures/jackcess-5.0.4.jar:.cache/saved-external-classes-20261024 \
  SavedExternalFixture .cache/tablecraft-fixtures/saved-external-20261024/V2016 V2016
java -cp .cache/tablecraft-fixtures/jackcess-5.0.4.jar:.cache/saved-external-classes-20261024 \
  SavedExternalOracle .cache/tablecraft-fixtures/saved-external-20261024/V2016/frontend.accdb
```

Repeat for V2019/V2007 and V2000 (`.mdb`). Capture each read-only oracle's stdout
as `frontend.accdb.getters.tsv` / `backend.accdb.getters.tsv` (or `.mdb`) and
stderr as the corresponding `.getters.stderr`. Fresh timestamps may change
binary hashes; the accepted report pins the actual preserved originals.

```sh
python3 docs/testing/compare_saved_external.py \
  --cli .cache/tablecraft-bin/saved-external-cli-20261024 \
  --jar .cache/tablecraft-fixtures/jackcess-5.0.4.jar \
  --directory .cache/tablecraft-fixtures/saved-external-20261024 \
  --report docs/testing/results/saved-external-2026-10-24.json
```

The runner refuses to overwrite an existing report. Every command, exit status,
stdout, stderr and source/input hash is in the accepted JSON. The actual producer/getter command inventory is pinned as
`.cache/tablecraft-fixtures/saved-external-20261024/generation.json`. The terminal log
is `.cache/saved-external-final-20261024.log`.

Excluded probes remain private: the first manual inspect used an invalid
`--saved-query` option; the first scripted replay used the unsupported parameter
spelling `int32` instead of the existing CLI `long`. Neither is production
failure or accepted evidence. The corrected 188-control exploratory pass and an
initial passing source-format checkpoint remain in `.cache`; only the linked
final report is accepted. Original files were not patched for any controls.

## References and remaining qualification

Microsoft's [IN clause](https://learn.microsoft.com/en-us/office/vba/access/concepts/miscellaneous/in-clause-microsoft-access-sql)
documents external database syntax. Jackcess's
[BaseSelectQuery getters](https://jackcess.sourceforge.io/apidocs/com/healthmarketscience/jackcess/query/BaseSelectQuery.html)
expose remote path/type, while upstream
[query tests](https://github.com/jahlborn/jackcess/blob/master/src/test/java/com/healthmarketscience/jackcess/query/QueryTest.java)
exercise remote SELECT/crosstab rendering. Actual pinned 5.0.4 reopened getters
establish the representations above, independently of upstream master changes.

This corpus does not qualify imported provider activation, target named-query
expansion, link chains, Access security/locking, native Windows execution or
broader saved-query metadata. It preserves the limits in the
[saved external-source contract](../access/queries/NATIVE_SAVED_EXTERNAL_QUERIES.md).
