# Implementation references

The [complete library/reference review](DEPENDENCY_REVIEW.md) records each supplied
candidate's source, license metadata, owner, disposition and limitations. Review
does not adopt an implementation or qualify native behavior. VBA execution and
Windows/COM/Wine integration are [deferred](VBA_COMPATIBILITY.md); preservation and
inert dependency detection remain required.

The 18 October 2026 application review has feature-local Microsoft source mappings:
[startup](features/ACCESS_APP_STARTUP.md), [custom ribbons](features/ACCESS_RIBBON_COMPAT.md),
[DAO/ADO](features/DAO_ADO.md), [advanced fields](features/ACCESS_ADVANCED_FIELDS.md)
and [distribution/signatures/version limits](features/ACCESS_DISTRIBUTION.md).
These inform future contracts; the [corpus policy](testing/ACCESS_APPLICATIONS.md)
separates those requirements from observed compatibility evidence.
The workflow review also maps [expression editing](features/EXPRESSION_BUILDER.md),
[saved data tasks](features/IMPORT_EXPORT_TASKS.md), [analysis](features/DATABASE_ANALYSIS.md)
and [templates/wizards](features/APP_TEMPLATES_WIZARDS.md) to Microsoft references.

Sources checked on 17 October 2026. References establish upstream interfaces and
documented behavior, not TableCraft compatibility or successful integration.
Implemented scalar types have their detailed source mapping in
[CORE_TYPES.md](CORE_TYPES.md); other library choices are in
[LIBRARY_POLICY.md](LIBRARY_POLICY.md).

The working [retrieval profile](QUERY_EXECUTION.md) maps SELECT/TOP, grouping,
UNION, parameters and SQL type widths to primary references reviewed on
18 October 2026. Its acceptance/property tests are not Access oracle results.

Pinned native-format research inputs and source-review licenses are recorded in
[NATIVE_READING.md](NATIVE_READING.md). Header tests are independent of native
Access application/oracle qualification.

## Storage and SQL compatibility study

- [MDB Tools format notes](https://github.com/mdbtools/mdbtools/blob/dev/HACKING.md)
  and [Jackcess](https://github.com/spannm/jackcess) inform native page/row/index
  research. Existing pinned source/JAR comparison baselines remain in the native
  documents; this review does not upgrade them or copy upstream implementation.
- [UCanAccess](https://github.com/spannm/ucanaccess) is a high-value reference for
  SQL translation, Access functions and native-file changes over Jackcess/HSQLDB.
  Study where storage and SQL behavior diverge; no JVM dependency is adopted and
  its output is independent comparison evidence, not Microsoft Access evidence.
- [SQLite architecture](https://www.sqlite.org/arch.html) and
  [locking/recovery](https://www.sqlite.org/lockingv3.html) inform pager/transaction
  separation. Their on-disk formats and locking protocols cannot replace Jet/ACE.
- [DataFusion](https://github.com/apache/datafusion) informs optimizer/operator
  and predicate-pushdown design. TableCraft retains its owned model/semantics;
  the full engine is not a required dependency.
- [jetdb](https://github.com/dominion525/jetdb), redb, Fjall, heed/LMDB and
  db-internals have separate study/reuse decisions in the
  [review registry](DEPENDENCY_REVIEW.md#other-storage-and-execution-references).
  Their existence is not qualification of TableCraft writes or recovery.

## Preservation and deferred language/provider research

[MS-OVBA](https://learn.microsoft.com/en-us/openspecs/office_file_formats/ms-ovba/575462ba-bf67-4190-9fac-c275523c75fc)
remains relevant to project-stream preservation. [MS-VBAL](https://learn.microsoft.com/en-us/openspecs/microsoft_general_purpose_programming_languages/ms-vbal/d5418146-0bd2-45eb-9c7a-fd9502722c74)
and [MS-OAUT](https://learn.microsoft.com/en-us/openspecs/windows_protocols/ms-oaut/2e27b533-5de4-46e9-9b3c-a6b5bcb161de)
remain references for deferred independent RustVBA language/dispatch work.
The [review registry](DEPENDENCY_REVIEW.md#reference-projects-specifications-and-non-cargo-tools)
also records tree-sitter-vba, Wine and LibreOffice, with licensing/provenance
limits. No interpreter, grammar or provider code is copied or linked.

## SQL parser selection

- [Apache DataFusion SQLParser repository](https://github.com/apache/datafusion-sqlparser-rs):
  adopted as sqlparser 0.63.0 behind TableCraft's owned query model.
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

- [Comparison of Microsoft Access SQL and ANSI SQL](https://support.microsoft.com/en-us/access/comparison-of-microsoft-access-sql-and-ansi-sql):
  user-supplied dialect comparison. Records reversed BETWEEN bounds, provider-
  dependent keywords/wildcards, expression grouping/sorting and aggregate
  differences. Use alongside the more specific statement and wildcard references;
  its general wording is not a complete grammar or an execution oracle.
- [Between…And operator](https://learn.microsoft.com/en-us/office/vba/access/concepts/miscellaneous/between-and-operator):
  SQL predicate syntax, optional NOT, expression operands, inclusive comparison,
  NULL propagation and literal treatment of wildcard characters. Current code
  preserves syntax and written bound order only; evaluation is unimplemented.
- [In operator (Microsoft Access SQL)](https://learn.microsoft.com/en-us/office/vba/access/concepts/miscellaneous/in-operator-microsoft-access-sql):
  optional NOT and expression lists, with separate subquery and external-database
  forms. Current code preserves non-empty expression lists and retrieval subqueries,
  without membership evaluation.
- [Access SQL: basic concepts, vocabulary, and syntax](https://support.microsoft.com/en-us/access/access-sql-basic-concepts-vocabulary-and-syntax):
  user-supplied introductory reference. Covers clause order, bracketed names,
  text quoting, sorting, aggregation and UNION/UNION ALL. Current coverage and
  explicit gaps are mapped in SQL_PARSER.md; this is not a complete grammar.
- [Quotation marks in string expressions](https://learn.microsoft.com/en-us/office/vba/access/concepts/criteria-expressions/include-quotation-marks-in-string-expressions):
  Access text delimiters and construction of criteria/SQL strings. Parser input
  is SQL text itself; it does not unescape an enclosing VBA string literal.
- [Microsoft Access SQL reference](https://learn.microsoft.com/en-us/office/client-developer/access/desktop-database-reference/microsoft-access-sql-reference):
  primary reference for Access grammar and statement coverage.
- [PARAMETERS declaration](https://learn.microsoft.com/en-us/office/client-developer/access/desktop-database-reference/parameters-declaration-microsoft-access-sql):
  declared parameter syntax and types.
- [TRANSFORM statement](https://learn.microsoft.com/en-us/office/client-developer/access/desktop-database-reference/transform-statement-microsoft-access-sql):
  Access crosstab syntax and its SELECT/PIVOT structure.

These documents guide original test construction. The current corpus is parsed
or explicitly rejected by TableCraft, without extracting SQL from the supplied
fixture, evaluating in Access or translating to SQLite.

## Expanded query grammar

The working family corpus also follows Microsoft [expression syntax](https://support.microsoft.com/en-us/access/guide-to-expression-syntax),
[VBA operator precedence](https://learn.microsoft.com/en-us/office/vba/language/reference/user-interface-help/operator-precedence),
[Access joins](https://learn.microsoft.com/en-us/office/vba/access/concepts/structured-query-language/perform-joins-using-access-sql),
[UNION](https://learn.microsoft.com/en-us/office/client-developer/access/desktop-database-reference/union-operation-microsoft-access-sql),
[INSERT](https://learn.microsoft.com/en-us/office/client-developer/access/desktop-database-reference/insert-into-statement-microsoft-access-sql),
[UPDATE](https://learn.microsoft.com/en-us/office/client-developer/access/desktop-database-reference/update-statement-microsoft-access-sql),
[DELETE](https://learn.microsoft.com/en-us/office/client-developer/access/desktop-database-reference/delete-statement-microsoft-access-sql),
[SELECT INTO](https://learn.microsoft.com/en-us/office/client-developer/access/desktop-database-reference/select-into-statement-microsoft-access-sql)
and [external IN](https://learn.microsoft.com/en-us/office/vba/access/concepts/miscellaneous/in-clause-microsoft-access-sql).
SQL_PARSER.md maps these to implemented syntax and explicit semantic limitations.

[Subquery grammar](https://learn.microsoft.com/en-us/office/client-developer/access/desktop-database-reference/sql-subqueries-microsoft-access-sql)
also defines crosstab predicate/output restrictions.
[WHERE](https://learn.microsoft.com/en-us/office/vba/access/concepts/structured-query-language/where-clause-microsoft-access-sql)
defines GUID literal delimiters; the SQL types reference documents @@IDENTITY.

Schema query types follow Microsoft [CREATE TABLE](https://learn.microsoft.com/en-us/office/client-developer/access/desktop-database-reference/create-table-statement-microsoft-access-sql),
[ALTER TABLE](https://learn.microsoft.com/en-us/office/client-developer/access/desktop-database-reference/alter-table-statement-microsoft-access-sql),
[CREATE INDEX](https://learn.microsoft.com/en-us/office/client-developer/access/desktop-database-reference/create-index-statement-microsoft-access-sql),
[CONSTRAINT](https://learn.microsoft.com/en-us/office/client-developer/access/desktop-database-reference/constraint-clause-microsoft-access-sql)
and [DROP](https://learn.microsoft.com/en-us/office/client-developer/access/desktop-database-reference/drop-statement-microsoft-access-sql).
Field type spelling is not a storage layout, and referential actions are never executed.

Saved query definitions follow [CREATE VIEW](https://learn.microsoft.com/en-us/office/client-developer/access/desktop-database-reference/create-view-statement-microsoft-access-sql)
and [CREATE PROCEDURE](https://learn.microsoft.com/en-us/office/client-developer/access/desktop-database-reference/create-procedure-statement-microsoft-access-sql),
including the documented PROCEDURE declaration form. The parser does not register saved queries.

[Access SQL reserved words](https://learn.microsoft.com/en-us/office/client-developer/access/desktop-database-reference/sql-reserved-words)
define SQL identifier classification independently of SQLParser's multi-dialect
keywords. Reserved names remain available through brackets; Access UI/VBA and
catalog-dependent binding are separate concerns.

## Scalar evaluation references

[EXPRESSIONS.md](EXPRESSIONS.md) maps the implemented scalar rules to Microsoft
Access operator/text documentation and VBA truth-table, numeric comparison and
conversion references. It distinguishes the tested TableCraft carrier profile
from deferred Variant promotion, locale and native Access behavior. These
references guide original tests; they do not constitute native oracle results.

## VBA and automation architecture references

[VBA_COMPATIBILITY.md](VBA_COMPATIBILITY.md) links the reviewed MS-OVBA project
storage/versioning, HasModule, form BeforeUpdate/binding and DAO transaction
references. They guide future preservation, host contracts and differential
acceptance; no source extraction, language execution or native authoring has
been verified by the current implementation.

[WINDOWS_COMPATIBILITY.md](WINDOWS_COMPATIBILITY.md) adds reviewed CreateObject,
GetObject, IDispatch, FileSystemObject, Declare/64-bit VBA and database trust
references for future per-API providers. It records portability, security and
verification requirements separately from current functionality.

## Native catalog references

[NATIVE_READING.md](NATIVE_READING.md) records pinned MDB Tools and Jackcess
format sources, their license/research boundary and exact independent metadata
comparison evidence. [MODEL.md](MODEL.md) defines the shared descriptors consumed
by that reader; native type/flag values stay outside the shared model. No native
Microsoft Access comparison was performed.
