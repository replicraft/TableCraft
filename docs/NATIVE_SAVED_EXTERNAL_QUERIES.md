# Saved query external database sources

Saved SELECT and crosstab reconstruction now maps a qualified native global
remote-database fragment into the existing owned `ExternalDatabase` syntax.
Reconstruction remains inert: neither its path nor connection text opens a file,
connects a provider or authorizes execution.

## Stored metadata contract

The admitted profile has one attribute-4 row with Flag 2, no LvExtra and no
Name2. Name1 supplies the exact database token; Expression supplies an optional
connection string. At least one must be present. Empty strings remain distinct
from absent fields. With a NULL Name1 and a present connection, the owned token
is empty; the original definition still retains NULL. A present connection uses
the bracketed syntax marker, including an empty connection.

The adapter copies these strings directly into owned fields under cumulative
syntax, allocation and work allowances. It never inserts stored path or provider
text into a SQL parser. Quotes, brackets, Unicode and SQL-looking punctuation
remain literal data. Normalized output escapes them through the shared formatter.
Source identifiers and expression fragments retain their existing guards.

The complete SELECT table expression receives the qualifier, including aliases,
joins and comma roots. SELECT and crosstab share this composition step. External
metadata without FROM sources fails. Duplicate global fragments, unknown flags,
unexpected payloads and both fields NULL fail without publishing a query.

Per-table qualifiers stored in a Table row with both Name1 and Expression remain
unsupported; they are a different native representation. Action, DDL and
pass-through queries remain outside read reconstruction. A global attribute-4
row on a UNION wrapper remains unsupported. Labelled UNION arms already containing
their own explicit SQL IN clauses use the existing guarded arm parser.

## Explicit execution

```sh
tablecraft query saved --database frontend.accdb RemotePeople
tablecraft query run --database frontend.accdb --saved RemotePeople \
  --external remote backend.accdb
```

The second command works when the stored database token is exactly `remote` and
the query falls within the existing read execution profile. Named references to
the same root definition use the same mapping. Missing or differently cased
tokens fail; stored tokens are never interpreted as paths to open.

The native facade accepts path-only tokens. A stored connection string remains
inspectable and reconstructable, but native execution refuses it, including an
empty present connection. No connection parsing, network provider, target named
query expansion or recursive link traversal is introduced. Library hosts may
explicitly supply other resolvers through the existing backend-neutral contract.

The [external-source binding contract](EXTERNAL_QUERY_SOURCES.md) still captures
immutable binding keys during metadata-only preparation and revalidates them
before row reads. Root named definitions and domain targets retain root
ownership; external tables use their mapped target's metadata and policies.
All selected native files share one cumulative native allowance, separate from
query/comparison/output limits. Ordinary CLI results remain fully buffered.

## References and qualification

Microsoft's [IN clause](https://support.microsoft.com/en-us/access/in-clause)
documents external source syntax and table expressions. Reviewed Jackcess
[BaseSelectQuery getters](https://jackcess.sourceforge.io/apidocs/com/healthmarketscience/jackcess/query/BaseSelectQuery.html)
and [the renderer](https://raw.githubusercontent.com/jahlborn/jackcess/master/src/main/java/com/healthmarketscience/jackcess/impl/query/BaseSelectQueryImpl.java)
separate the path and connection from ordinary source names and place the global
qualifier after FROM. Those references inform this adapter; they do not establish
native Access save-query or execution equivalence. Original authored metadata,
raw getter observations and TableCraft tests remain engineering evidence.

The complete saved adapter target passes 77 tests, including eight new controls
for reconstruction and execution. Fifty CLI saved/external/linked tests pass,
including three new tests for explicit and named selection, exact token/provider
refusals, buffered late errors and exhausted allowances. Strict scoped Clippy,
standalone adapter checking and normal CLI building pass. These are bounded
engineering checks, not native Access comparisons.

Unknown native layouts, full provider support, coherent cross-file snapshots and
Windows Access compatibility remain separate acceptance work.
