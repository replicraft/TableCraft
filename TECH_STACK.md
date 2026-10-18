# Technology choices

This document distinguishes implemented dependencies from selected directions.
It describes current decisions, not completed database or application features.
Exact adopted versions and reviews are recorded in
[DEPENDENCIES.md](docs/DEPENDENCIES.md).

## Implemented foundation

| Component | Current implementation |
| --- | --- |
| Language and build | Rust 1.99.0, edition 2024, Cargo workspace |
| CLI | clap Parser derive, help and version output |
| Shared vocabulary | tablecraft-core values, scoped identities and resource budgets |
| Library errors | thiserror |
| Serialization | Optional serde for scalar value exchange; constructor validation on decode |
| Generated tests | proptest; serde_json is development-only for typed round-trip tests |

Other requested library choices are recorded in
[LIBRARY_POLICY.md](docs/LIBRARY_POLICY.md). Add them with the functionality that
uses them rather than as unused dependencies.

## Selected SQL parser direction

Select [Apache DataFusion SQLParser](https://github.com/apache/datafusion-sqlparser-rs)
(Cargo package `sqlparser`) for the first working query-parser slice. Reuse its
general lexer/parser and dialect extension points. Its Apache-2.0 license and
syntax-only scope were checked in the upstream documentation on 17 October 2026;
see [the source references](docs/REFERENCES.md).

The workspace does not yet contain tablecraft-query, an AccessDialect, or a
sqlparser dependency. This selection does not establish Access syntax coverage.
Choose a tested release, features and dependency closure when introducing the
working query slice. Do not scaffold its full future directory tree now.

| Component | Decision | Implementation status |
| --- | --- | --- |
| SQL lexer/parser | Apache DataFusion SQLParser (`sqlparser`) | Selected; not integrated |
| Access dialect | TableCraft extension and bounded parser adapter | Not implemented |
| Semantic analysis | TableCraft-owned Access semantics | Not implemented |
| Query intermediate representation | TableCraft-owned, independent of upstream AST | Not implemented |
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

## Evidence needed for integration

Build a provenance-recorded syntax corpus before treating parser integration as
Access support. Include supported and rejected cases for TOP, DISTINCTROW,
PARAMETERS, TRANSFORM/SELECT/PIVOT, bracketed names, date literals, joins,
wildcards, operators and action queries. Cover function-call syntax separately
from function behavior. Do not call documentation-derived examples real Access
results; query extraction from the supplied ACCDB has not been implemented.

Check bounded input/nesting and useful error locations; parse/format/parse
equivalence of the supported owned model; and explicit rejection without silent
loss of syntax. Preserve comments/source separately when required. Query designer
generation, SQLite translation and differential behavior require their own later
working implementations and tests. Record exact Access/DAO builds for oracle
results, and report unavailable oracle environments as skipped.
