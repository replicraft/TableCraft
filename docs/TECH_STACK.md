# Technology choices

This document distinguishes implemented dependencies from selected directions.
It describes current decisions, not completed database or application features.
Exact adopted versions and reviews are recorded in
[DEPENDENCIES.md](DEPENDENCIES.md).

## Implemented foundation

| Component | Current implementation |
| --- | --- |
| Language and build | Rust 1.99.0, edition 2024, Cargo workspace |
| CLI | clap Parser derive, help and version output |
| Shared vocabulary | tablecraft-core values, scoped identities and resource budgets |
| Query syntax | tablecraft-query bounded data-query parser and owned statement model |
| Library errors | thiserror |
| Serialization | Optional serde for scalar value exchange; constructor validation on decode |
| Generated tests | proptest; serde_json is development-only for typed round-trip tests |

Other requested library choices are recorded in
[LIBRARY_POLICY.md](LIBRARY_POLICY.md). Add them with the functionality that
uses them rather than as unused dependencies.

## SQL parser integration

The first query slice uses [Apache DataFusion SQLParser](https://github.com/apache/datafusion-sqlparser-rs)
(Cargo package `sqlparser` 0.63.0), with std enabled and default features disabled.
The adapter reuses its lexer and parser primitives; Access precedence and statement grammar are owned by TableCraft. Its Apache-2.0 license and
syntax-only scope were checked in the upstream documentation on 17 October 2026;
see [the source references](REFERENCES.md).

`crates/tablecraft-query` contains lexical AccessDialect rules, an explicit outer
query grammar and a bounded Access expression parser. [SQL_PARSER.md](SQL_PARSER.md)
records the exact supported subset, limits, corpus provenance and limitations.
This is syntax integration, without binding or execution evidence from Access.

| Component | Decision | Implementation status |
| --- | --- | --- |
| SQL lexer/parser | Apache DataFusion SQLParser (`sqlparser`) | Integrated behind private adapter |
| Access dialect | TableCraft extension and bounded parser adapter | Retrieval, declarations, crosstabs and action syntax |
| Semantic analysis | TableCraft-owned Access semantics | Not implemented |
| Query intermediate representation | TableCraft-owned, independent of upstream AST | Owned query/statement model; no semantic/execution IR |
| SQL translation | TableCraft-owned, supported subsets with semantic evidence | Not implemented |
| Native Access query evaluation | TableCraft-owned engine behavior | Not implemented |

MsSqlDialect is a SQL Server dialect, not Access SQL. Access-specific syntax must
be evaluated explicitly. The Dialect hooks return upstream Statement/Expr types;
that API is not a promise that a custom dialect can represent every Access
construct. The adapter may need specialized TableCraft nodes or upstream changes.
Keep the original SQL independently where preservation matters: upstream SQL
formatting normalizes text, and its source-span support is still incomplete.

Keep upstream AST types inside the parser adapter. Database adapters, forms,
reports and future query designers consume TableCraft definitions. Feature slices
should separate parsing, owned query definitions, expression semantics, analysis
and formatting as actual working responsibilities appear. Do not enable AST serde
merely because it is available or make upstream AST serialization a project format.

Parsing a function name does not implement IIf, Nz, DateAdd, coercion, NULL
behavior, collation, updateability or data macros. A successful syntax rewrite
does not establish equivalent SQLite results. Unrepresentable or unsupported
constructs must remain explicit failures rather than silently using a permissive
generic or SQL Server interpretation.

Full DataFusion and Arrow are not selected for this parser work. The native
engine must not require Microsoft's Jet/ACE runtime for supported cross-platform
query execution; real Access remains a separate comparison oracle.

## Evidence and further integration

The authored corpus includes supported TOP, DISTINCTROW, bracketed names and
field wildcards, PARAMETERS, TRANSFORM/PIVOT, date spelling, joins,
LIKE/function syntax and action queries, plus malformed/foreign rejection cases. See SQL_PARSER.md for
reference mapping. These documentation-derived syntax cases are not observed
Access results; query extraction from the supplied ACCDB is unimplemented.

Tests check bounded input/nesting, error locations, parse/format/parse
equivalence and explicit rejection. Original source is preserved independently
from normalized formatting. Query designer
generation, SQLite translation and differential behavior require their own later
working implementations and tests. Record exact Access/DAO builds for oracle
results, and report unavailable oracle environments as skipped.
