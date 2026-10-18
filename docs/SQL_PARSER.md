# Bounded Access SELECT syntax

`crates/tablecraft-query` implements `parse_select(source, ParseLimits)` and
TableCraft-owned SELECT syntax. The library reads no files and evaluates no SQL.
The CLI still implements help/version only. There is no binding, stored-query
extraction, query execution, SQLite translation or Access comparison evidence.

## Access dialect checklist

Access SQL has its own Jet/ACE engine behavior; SQL Server syntax is not a
substitute. Microsoft describes it as generally ANSI-89 Level 1 compliant, with
extensions and omissions ([SQL comparison](https://support.microsoft.com/en-us/access/comparison-of-microsoft-access-sql-and-ansi-sql)).
That ancestry does not establish implementation or compatibility here.

| Characteristic | Current TableCraft status | Compatibility boundary |
| --- | --- | --- |
| Jet/ACE-specific dialect | Bounded Access adapter; no generic/SQL Server fallback | No engine execution or Jet/ACE comparison yet |
| LIKE patterns | Both `LIKE 'A*'` and `LIKE 'A%'` rejected in corpus | ANSI-89 uses `*`/`?`; ANSI-92 uses `%`/`_`. Database settings and access interface matter; no dialect-mode selection or pattern semantics implemented ([Microsoft wildcards](https://support.microsoft.com/en-us/access/access-wildcard-character-reference)) |
| `#date#` literals | Detected and rejected explicitly | Date parsing, locale rules and conversion to core date carriers deferred; see Microsoft WHERE below |
| Text concatenation | `&`, `+` and `||` rejected in corpus | Access supports both `&` and `+` for text. Their NULL behavior differs; do not translate them interchangeably. Numeric/string coercion also needs separate evidence ([Microsoft concatenation](https://support.microsoft.com/en-us/access/combine-text-values-by-using-an-expression)) |
| IIf and other Access functions | IIf, Nz and DateAdd rejected in corpus; CASE also rejected | Function parsing/evaluation deferred; IIf must not silently become CASE with assumed equivalent branch/coercion behavior ([Microsoft IIf](https://support.microsoft.com/en-gb/access/functions/iif-function)) |
| Bracketed names such as `[Customer Orders]` | Implemented and tested, including spaces and Unicode | Name resolution and collation deferred |

## Supported subset

- Exactly one SELECT, optionally terminated by one semicolon.
- One source table; optional table and output aliases require explicit AS.
- Ordered field projections, `*`, and table/alias-qualified fields or wildcards.
- ALL (also the omitted default), DISTINCT, DISTINCTROW, or TOP with an optional
  PERCENT modifier. These remain distinct syntax concepts. This slice accepts TOP
  integers from 1 through 2,147,483,647; percentages from 1 through 100. These are
  adapter representation limits, not an experimentally established Access range.
- WHERE: unresolved fields, signed i64 integer literals, single- or double-quoted
  text with doubled matching delimiters, TRUE/FALSE, NULL, comparisons `= <> < <= > >=`, AND/OR,
  parentheses, IS NULL and IS NOT NULL. Literal integers have no stored field type.
- ORDER BY field references, optionally qualified, with ASC/DESC. Omitted direction
  normalizes to ASC. No ordinal or computed sort keys.
- Unquoted letter/underscore-led names with alphanumeric/underscore continuations,
  or bracketed names. Upstream-reserved words require brackets. Empty names,
  control characters and embedded brackets, backticks, periods or exclamation
  marks are rejected. Name spelling is retained; there is no case folding,
  catalog lookup or Access collation implementation.

Identifiers in WHERE remain unresolved: a later binder must distinguish fields
from implicit parameters. Parsed TRUE/FALSE does not choose a numeric encoding.
Parsing `= NULL` does not equate it with IS NULL. DISTINCTROW is not DISTINCT;
TOP must retain Access tie behavior when execution is eventually implemented.

PARAMETERS, TRANSFORM/PIVOT, date literals delimited with `#`, joins, multiple
tables, functions, decimals/floats, arithmetic, NOT, LIKE/wildcards in criteria,
IN/BETWEEN, aggregates, subqueries, action queries, SELECT INTO, external IN and
OWNERACCESS are rejected. SQL Server TOP parentheses/WITH TIES, LIMIT, schema
names, alternate identifier quoting, `!=` and `==` are also rejected. A rejection can mean
valid Access syntax outside this slice; it is not an Access validity verdict.

## Adapter and formatting

The private `parser/` module supplies lexical identifier rules through a small
AccessDialect. It uses sqlparser's tokenizer and expression parser, and an
explicit outer SELECT grammar. It never uses MsSqlDialect or a generic fallback.
Only whitelisted upstream expression variants/operators become owned syntax;
all others fail. The lexer merges `!=` and `<>`, so original token spelling is
checked before accepting a not-equal operator.

`ParsedSelect::source()` retains the exact input. `query()` exposes an immutable
view of the owned model. Model Display produces normalized SQL: bracketed names,
uppercase keywords, single-quoted text, explicit sort directions and parenthesized expressions.
Comments and whitespace are retained only in source, not in normalized output.
The lexer accepts SQL line/block comments as source trivia; this has not been
qualified against Access. Formatting can increase source length/token count, so
caller-lowered input budgets also apply when reparsing formatted output.
Public syntax types are not a validated builder or a persistence format; no
upstream AST serialization or Serde support has been introduced.

Layout follows responsibilities: `select/name.rs`, `select/expression.rs` and
`select/format.rs` own the model and formatting; `parser/access.rs` owns the outer
grammar; `parser/expression.rs` converts upstream expressions; limits and
diagnostics live beside them. Entry points compose and re-export these features.

## Bounds and diagnostics

Default limits are also hard ceilings: 65,536 UTF-8 bytes, 1,024 non-trivia tokens,
32 levels of parentheses and 32 levels of owned expression depth. Callers may
lower them; larger values are clamped. Byte limits run before tokenization;
tokens and parentheses are checked before parsing. Expression depth also covers
unparenthesized AND/OR chains. Upstream parser recursion is limited to 128.
Source copies and diagnostic allocations remain bounded by those input limits.
This synchronous operation does not expose cancellation or promise a wall-clock
deadline.

SQLParser's recursive-protection feature remains enabled. A debug stress test
overflowed a standard test thread's stack without it, despite the recursion
counter. Its stacker/psm dependencies introduce native stack-switching and build
requirements; see [DEPENDENCIES.md](DEPENDENCIES.md). The CLI does not acquire them.

Errors use thiserror and stable Syntax/Unsupported/Limit categories, fixed
payload-free messages, and one-based line/Unicode-character columns. Locations
identify detection points, not complete expression spans; unsupported expression
conversion points to the beginning of WHERE's expression. Original source and
syntax-model Debug output can contain payloads; only error messages/Debug redact
them. There is no silent default, dialect retry or partial success.

## Corpus and reference mapping

The user-supplied [Microsoft introductory SQL guide](https://support.microsoft.com/en-us/access/access-sql-basic-concepts-vocabulary-and-syntax)
also serves as a clause-level checklist:

| Guide topic | Current coverage |
| --- | --- |
| SELECT/FROM/WHERE, multiline layout, bracketed names | Supported subset and round-trip cases |
| ORDER BY with multiple keys, DESC and default ASC | Supported and tested |
| Double-quoted text in criteria | Supported as text in WHERE; doubled double quotes decode to one quote; quoted identifiers remain rejected |
| GROUP BY/HAVING and aggregate projections | Explicit rejection cases; no grouping model or aggregate semantics |
| UNION and UNION ALL | Explicit rejection cases; no set-operation model or output-type compatibility rules |
| Statement semicolon | Normalized output always includes one; input deliberately permits omission |

The corpus is original TableCraft SQL under
`crates/tablecraft-query/src/parser/tests/corpus.rs`. It uses synthetic Samples,
Code, Label and related names; no SQL or objects were extracted from ACCDB.
These authored cases apply the grammar descriptions below, rather than copying
third-party examples or treating documentation as observed Access results.

Text delimiters also follow Microsoft's [quotation marks in string expressions](https://learn.microsoft.com/en-us/office/vba/access/concepts/criteria-expressions/include-quotation-marks-in-string-expressions).
The parser receives SQL directly, without decoding a surrounding VBA string.
Both quote styles produce the same owned Text node; Display uses single quotes
and doubles apostrophes. Original delimiter spelling remains in source. Tests
cover empty text, mixed quotes, Unicode, newlines, literal backslashes, SQL marker
characters inside text and malformed delimiters. Double quotes are not enabled
as identifier delimiters, and no ANSI-mode selection has been implemented.

| Corpus area | Primary reference | Evidence provided here |
| --- | --- | --- |
| SELECT projections, aliases and source | [Microsoft SELECT](https://learn.microsoft.com/en-us/office/client-developer/access/desktop-database-reference/select-statement-microsoft-access-sql), [FROM](https://learn.microsoft.com/en-us/office/vba/access/concepts/structured-query-language/from-clause-microsoft-access-sql) | Owned structure and round trips; joins/external sources rejected |
| ALL, DISTINCT, DISTINCTROW, TOP/PERCENT | [Microsoft predicates](https://learn.microsoft.com/en-us/office/vba/access/concepts/structured-query-language/all-distinct-distinctrow-top-predicates-microsoft-access-sql) | Distinct enum variants, supported integer bounds, SQL Server extensions rejected |
| Criteria, bracketed fields, date literals | [Microsoft WHERE](https://learn.microsoft.com/en-us/office/vba/access/concepts/structured-query-language/where-clause-microsoft-access-sql) | Supported comparisons/logical structure; functions, LIKE and dates rejected |
| ORDER BY directions | [Microsoft ORDER BY](https://learn.microsoft.com/en-us/office/vba/access/concepts/structured-query-language/order-by-clause-microsoft-access-sql) | Ordered sort keys; omitted ASC normalization |
| Parameter declarations | [Microsoft PARAMETERS](https://learn.microsoft.com/en-us/office/client-developer/access/desktop-database-reference/parameters-declaration-microsoft-access-sql) | Explicit rejection pending a parameter model |
| Crosstabs | [Microsoft TRANSFORM](https://learn.microsoft.com/en-us/office/client-developer/access/desktop-database-reference/transform-statement-microsoft-access-sql) | Explicit rejection pending crosstab representation |

Feature tests also cover original-source retention, name/alias structure, logical
precedence, text escaping, i64 boundaries, Unicode locations, malformed SQL,
multiple statements, resource limits and deep inputs. Three proptest properties
run 256 cases each with a fixed seed and default regression persistence: generated
integer/text/TOP queries round-trip through the owned model, bounded arbitrary
Unicode inputs return without panicking, and both text delimiters retain identical
generated Unicode contents through parsing/formatting. These are syntax checks; no native
Access, query-result, durability or cross-platform compatibility claim follows.
