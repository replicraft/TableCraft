# Stored derived-query sources

Native saved SELECT and crosstab queries now reconstruct a Table fragment with
NULL Name1 and a non-NULL Expression as a derived read source. Optional Name2 is
its alias, including [already-bracketed spellings](NATIVE_SAVED_NAMES.md).
A non-NULL Name1 together with Expression is a remote qualifier and
remains unsupported; missing both name and expression is inconsistent metadata.
No native reader, query executor, package dependency or write behavior changed.

This distinction follows the reviewed Jackcess 5.0.4
[QueryImpl.getFromTables](https://github.com/jahlborn/jackcess/blob/be0a2139b087a227d14cafd4ea231306a5a3a862/src/main/java/com/healthmarketscience/jackcess/impl/query/QueryImpl.java)
metadata renderer. [Microsoft's SQL subquery reference](https://learn.microsoft.com/en-us/office/client-developer/access/desktop-database-reference/sql-subqueries-microsoft-access-sql)
provides the general SELECT-subquery vocabulary; it does not independently qualify
this stored FROM representation or TableCraft's execution profile.

## Inert reconstruction

The source text must parse as one owned read query without its own PARAMETERS
declaration. SELECT/UNION and nested retrieval syntax retain their original owned
structure. Action/DDL/make-table/crosstab statements, extra statements, unmatched
delimiters and attempts to escape the surrounding FROM source fail explicitly.
Nothing executes while reading metadata or reconstructing syntax.

The adapter parses the original query, then parses it again inside a guarded FROM
source. The guarded source must contain exactly the same read tree. A newline
terminates a trailing line comment before the closing delimiter. Native text and
identity remain unchanged, including comments. The boxed read carrier moves from
the bounded parser; the adapter does not clone it or allocate an unchecked box.
Final join-forest parsing verifies the derived query and alias again before
publication. Original, guarded and complete source text/AST work charge the same
cumulative budget and clamped syntax allowances; cancellation precedes publication.

Anonymous derived sources can be standalone or comma roots. They have no synthetic
endpoint label. An alias lets a derived source participate in the existing
[join forest](NATIVE_SAVED_JOINS.md); ordinary duplicate-label and ASCII multi-source
identity rules still apply. A join cannot address an anonymous source through an
invented name. The same sixteen-source/condition ceilings and further parser
byte/token/depth limits remain in force.

## Explicit execution

`query run --saved` and named saved SELECT sources use the existing derived-read
binder and executor. Derived sources are non-lateral: they cannot reference a
sibling source's fields. Outer declared parameters remain available under existing
isolated named-query scope/type rules. Named local SELECT/UNION dependencies use
the usual shared budgets, cycle checks and snapshot cache. No external provider
is opened implicitly. UDF and owner-access syntax can remain inert metadata, but
unsupported execution fails rather than acquiring authority or calling code.
All results retain buffered CLI publication.

Native crosstabs can consume these sources through the shared SELECT-body adapter.
This does not supply named/derived crosstab schemas. Remote qualifiers, unqualified
native source spellings, lateral queries, broader promotion/collation and native
Access FROM/admissibility/error behavior remain explicit compatibility gaps.

## Evidence

Four feature tests cover expected owned syntax, alias/anonymous roots, nested
SELECT/UNION, trailing comments, malformed/action/parameter/injection cases,
Jet 4/ACE 2007/2016/2019 nullable joins, every measured cancellation checkpoint
and byte/work/syntax limits. One crosstab test checks the shared source path.
One CLI test covers saved/named retrieval, inert unknown calls and a later arithmetic
error without partial stdout. Source bytes remain unchanged.

The original [SavedDerivedFixture.java](testing/SavedDerivedFixture.java) recipe
extends only our original forest recipe. Four fresh local files contain its
fourteen baseline queries and eleven additional inert derived definitions.
[compare_saved_derived.py](testing/compare_saved_derived.py) passes 176 independent
SQL-rendering/direct/saved/named execution/refusal controls using explicit expected
rows. The reference renders metadata but never executes SQL. [Fixture records](FIXTURES.md#saved-derived-source-inputs)
give provenance and observed hashes. Windows Microsoft Access execution and
save/reopen remain unrun; these controls are independent evidence, not native
compatibility qualification.

```sh
python3 docs/testing/compare_saved_derived.py \
  --jar path/to/jackcess-5.0.4.jar --cli target/release/tablecraft \
  --directory path/to/fresh-derived-sources --generate
```
