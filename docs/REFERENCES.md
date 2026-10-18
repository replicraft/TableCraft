# Implementation references

Sources checked on 17 October 2026. References establish upstream interfaces and
documented behavior, not TableCraft compatibility or successful integration.
Implemented scalar types have their detailed source mapping in
[CORE_TYPES.md](CORE_TYPES.md); other library choices are in
[LIBRARY_POLICY.md](LIBRARY_POLICY.md).

## SQL parser selection

- [Apache DataFusion SQLParser repository](https://github.com/apache/datafusion-sqlparser-rs):
  selected implementation dependency for the upcoming parser slice and an
  architectural reference for reusing a syntax parser behind TableCraft's model.
  Not yet installed in the workspace. The README documents syntax/semantics
  separation, formatting normalization and incomplete source-span coverage.
- [Upstream license](https://github.com/apache/datafusion-sqlparser-rs/blob/main/LICENSE.TXT):
  Apache-2.0. Review the selected release's license and notices at adoption.
- [Dialect extension API](https://docs.rs/sqlparser/latest/sqlparser/dialect/trait.Dialect.html):
  hooks for statement/expression parsing and dialect behavior. Hooks return the
  upstream AST types; specialized TableCraft nodes require an adapter boundary.
- [Built-in dialects](https://docs.rs/sqlparser/latest/sqlparser/dialect/index.html):
  MsSqlDialect is identified as Microsoft SQL Server; SQLiteDialect is separate.
  A built-in Access dialect is not listed in the reviewed API.

The current selection and implementation boundaries are in
[TECH_STACK.md](../TECH_STACK.md). No parser version or feature set is adopted yet.

## Access SQL sources for the parser corpus

- [Microsoft Access SQL reference](https://learn.microsoft.com/en-us/office/client-developer/access/desktop-database-reference/microsoft-access-sql-reference):
  primary reference for Access grammar and statement coverage.
- [PARAMETERS declaration](https://learn.microsoft.com/en-us/office/client-developer/access/desktop-database-reference/parameters-declaration-microsoft-access-sql):
  declared parameter syntax and types.
- [TRANSFORM statement](https://learn.microsoft.com/en-us/office/client-developer/access/desktop-database-reference/transform-statement-microsoft-access-sql):
  Access crosstab syntax and its SELECT/PIVOT structure.

These documents guide test construction. No SQL was extracted from the supplied
fixture, evaluated in Access, translated to SQLite or parsed by TableCraft as
part of recording this decision.
