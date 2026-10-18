# Implementation references

Sources checked on 17 October 2026. References establish upstream interfaces and
documented behavior, not TableCraft compatibility or successful integration.
Implemented scalar types have their detailed source mapping in
[CORE_TYPES.md](CORE_TYPES.md); other library choices are in
[LIBRARY_POLICY.md](LIBRARY_POLICY.md).

## SQL parser selection

- [Apache DataFusion SQLParser repository](https://github.com/apache/datafusion-sqlparser-rs):
  adopted as sqlparser 0.63.0 behind TableCraft's owned SELECT model.
  The README documents syntax/semantics
  separation, formatting normalization and incomplete source-span coverage.
- [Upstream license](https://github.com/apache/datafusion-sqlparser-rs/blob/main/LICENSE.TXT):
  Apache-2.0; the registry package includes LICENSE.TXT and does not include a
  separate NOTICE file. Dependency records document the adopted feature closure.
- [Dialect extension API](https://docs.rs/sqlparser/latest/sqlparser/dialect/trait.Dialect.html):
  hooks for statement/expression parsing and dialect behavior. Hooks return the
  upstream AST types; specialized TableCraft nodes require an adapter boundary.
- [Built-in dialects](https://docs.rs/sqlparser/latest/sqlparser/dialect/index.html):
  MsSqlDialect is identified as Microsoft SQL Server; SQLiteDialect is separate.
  A built-in Access dialect is not listed in the reviewed API.

Current implementation boundaries are in [TECH_STACK.md](TECH_STACK.md).
[SQL_PARSER.md](SQL_PARSER.md) maps the supported/rejected corpus to additional
Microsoft SELECT, predicate, WHERE, FROM and ORDER BY references.

## Access SQL sources for the parser corpus

- [Access SQL: basic concepts, vocabulary, and syntax](https://support.microsoft.com/en-us/access/access-sql-basic-concepts-vocabulary-and-syntax):
  user-supplied introductory reference. Covers clause order, bracketed names,
  text quoting, sorting, aggregation and UNION/UNION ALL. Current coverage and
  explicit gaps are mapped in SQL_PARSER.md; this is not a complete grammar.
- [Microsoft Access SQL reference](https://learn.microsoft.com/en-us/office/client-developer/access/desktop-database-reference/microsoft-access-sql-reference):
  primary reference for Access grammar and statement coverage.
- [PARAMETERS declaration](https://learn.microsoft.com/en-us/office/client-developer/access/desktop-database-reference/parameters-declaration-microsoft-access-sql):
  declared parameter syntax and types.
- [TRANSFORM statement](https://learn.microsoft.com/en-us/office/client-developer/access/desktop-database-reference/transform-statement-microsoft-access-sql):
  Access crosstab syntax and its SELECT/PIVOT structure.

These documents guide original test construction. The current corpus is parsed
or explicitly rejected by TableCraft, without extracting SQL from the supplied
fixture, evaluating in Access or translating to SQLite.
