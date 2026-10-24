# Access query parsing and owned model

The crate lives at `crates/query/tablecraft-sql`, with Rust imports through
`tablecraft_sql`. Its former package name was `tablecraft-query`.
See [ARCHITECTURE.md](ARCHITECTURE.md) for the distinction between this syntax
package and the implemented [retrieval](QUERY_EXECUTION.md) and
[scalar semantics](EXPRESSIONS.md) packages.

`tablecraft-sql` parses Access query syntax without reading files, connecting to
providers, executing functions or modifying databases. `parse_query(sql, limits)`
returns `ParsedQuery`: exact original source plus a TableCraft-owned Query model.
`parse_select` remains a single-SELECT convenience entry point; use `parse_query`
for declarations, unions, crosstabs, action and schema queries.

This is the first query parsing/model milestone, covering the
families below. It is not a claim of complete Jet/ACE compatibility. Name/type
binding, function arity, calendar interpretation, wildcard evaluation, coercion,
query execution and real Access comparison remain separate work. View parameter
references and output-column matching require binding; only declaration syntax
and retrieval-only bodies are checked here.

The CLI command `tablecraft query parse "<SQL>"` prints normalized SQL using this
API. Invalid/unsupported input exits 2 with a diagnostic and no SQL output. Action
queries are printed as syntax; this command does not execute them.
The separate `query eval` CLI command delegates supported source-free scalar
projections to [tablecraft-expressions](EXPRESSIONS.md). Parser APIs themselves
remain syntax-only and do not open databases or evaluate expressions.
`query run` separately invokes [bounded retrieval](QUERY_EXECUTION.md) over inline
sources; it does not add execution to parser APIs.

## Standalone expressions

`parse_expression(source, limits)` returns exactly one owned expression.
`parse_field_rule(source, field, limits)` additionally inserts an implied field
before bare comparisons, IS, BETWEEN, IN and LIKE in operand positions. Names
are quoted tokens, never interpolated SQL. Expanded bytes/tokens share limits;
extra SQL is rejected. Both APIs remain syntax-only: consumers must restrict
references and forms before evaluation. The native [constraint audit](NATIVE_CONSTRAINTS.md)
is the first working consumer.

## Implemented syntax

| Area | Owned representation and working coverage |
| --- | --- |
| Retrieval | SELECT, ALL, DISTINCT, DISTINCTROW, TOP and PERCENT; DISTINCT/TOP combinations; explicit or implicit aliases; wildcard and expression projections |
| Expressions | Width-preserving i64 literals; decimal/exponent/hex/octal spelling outside that carrier; adjacent unquoted &H/&O prefixes; single/double-quoted text; hash-delimited date/time spelling; GUID literals; @@IDENTITY; NULL and booleans |
| Operators | Arithmetic `^ - * / \\ Mod +`, concatenation `&`, comparisons, NOT/AND/OR/XOR/EQV/IMP, LIKE/NOT LIKE, IS NULL/IS NOT NULL, BETWEEN/NOT BETWEEN, IN/NOT IN |
| Function syntax | Built-in and user-defined names (including module/library qualification and `$` suffixes), expression arguments, COUNT-style wildcard arguments, and omitted optional positions distinct from NULL |
| References | Access SQL reserved-name classification independent of upstream keywords; bracketed identifiers, dot qualification and bang/property access; spelling preserved without catalog resolution |
| Relations | Ordered comma sources, INNER/LEFT/RIGHT joins with explicit ON trees, parenthesized joins, derived retrieval queries and aliases |
| Clauses | WHERE, expression GROUP BY, HAVING, expression ORDER BY with ASC/DESC; no-FROM constant/function SELECT; WITH OWNERACCESS OPTION |
| Subqueries | Scalar, EXISTS, IN/NOT IN and ALL/ANY/SOME comparisons; read-only retrieval bodies |
| Set operations | UNION/UNION ALL, independently retained per arm; SELECT, TABLE and stored-query-reference operands; final ordering |
| Declarations | PARAMETERS names, including bracketed literal prompts with punctuation; recognized Access type spellings and integer type arguments; ordered declarations retained |
| IEEE floating types | IEEESINGLE and IEEEDOUBLE preserve their type spelling in parameters/DDL; binding retains distinct Single/Double carriers |
| Crosstabs | TRANSFORM expression with optional AS alias, SELECT/grouping, PIVOT expression and optional ordered fixed headings, trailing owner-access option; headings are distinct from membership IN; subqueries allowed in predicates, rejected in output expressions |
| Append/update/delete | INSERT VALUES (one record) or retrieval source, ordered target fields; UPDATE relations and SET assignments; DELETE target wildcards, relations and filters |
| Make-table | SELECT INTO is a separate root statement kind; destination IN is distinct from source IN; make-table statements are rejected inside retrieval operands |
| Schema queries | CREATE [TEMPORARY] TABLE with ordered fields/constraints; ALTER TABLE one operation; CREATE [UNIQUE] INDEX with ordered directions and WITH policy; DROP TABLE/INDEX |
| Saved queries | CREATE VIEW with optional output names and retrieval body; CREATE PROCEDURE / PROCEDURE headers with parameters and one parsed body; DROP VIEW/PROCEDURE |
| Constraints | Inline/named primary, unique and NOT NULL constraints; foreign keys with NO INDEX, optional referenced fields and ON UPDATE/DELETE CASCADE or SET NULL |
| External databases | Quoted paths or simple bare database filenames, optional quoted type strings or bracketed DATABASE connection specifications; stored as text and never opened |

TOP accepts unsigned u32 syntax (including zero); PERCENT is restricted to
0..=100. This carrier boundary is not an Access execution-range qualification.
Type names/arguments are syntax annotations, not validated native field layouts.
Numeric strings are never converted through floating point. Date literals retain
trimmed contents without claiming valid calendar values or choosing a locale.
LIKE preserves the pattern expression without guessing ANSI-89 versus ANSI-92
wildcard mode. BETWEEN retains written bounds, including reversed/equal bounds.
IN retains list order, duplicates and NULL. Function calls do not implement IIf,
Nz or aggregate behavior, and omitted arguments never become stored NULL.

The model is organized by feature: `select/` owns expressions, sources and SELECT;
`query/` owns retrieval composition, shared type syntax, parameters and action
statements; `query/definition/` owns table, constraint and index syntax. The private
`parser/` contains lexical, expression, relation, parameter and statement adapters.
Upstream token/parser types do not appear in public APIs. Query types are not a
validated public builder, serialization protocol or execution IR.

## Access grammar and formatting

The adapter uses SQLParser's tokenizer and parser cursor/keyword primitives.
Access expression parsing uses a bounded Pratt grammar with explicit precedence:
exponentiation, unary numeric sign, multiply/divide, integer division, Mod,
add/subtract, ampersand, comparisons/predicates, NOT, AND, OR, XOR, EQV, IMP.
This avoids inheriting generic SQL meanings for Access operators. No generic or
SQL Server dialect fallback is used. Ordinary names such as ID and Ref are not
rejected merely because another upstream dialect reserves them. Access SQL
reserved names require brackets; unsupported foreign clause words remain guarded. Upstream ASTs are not serialized or exposed.

Hash and GUID literals are folded from original source spans before trivia filtering.
The lexer merges `!=` with `<>`, so original spelling is checked; `!=` and `==`
are rejected. Unsupported constructs fail rather than disappearing from output.

Display prints normalized SQL with bracketed names, explicit aliases/directions,
escaped single-quoted text, parentheses around expressions/joins and a final
semicolon. Original quoting, comments and whitespace remain in `source()`.
Function names/qualifiers, numeric/date/GUID spelling, UNION ALL flags, parameter type arguments,
fixed pivot headings, omitted argument positions and source/destination metadata
survive normalization. Public model Debug can contain SQL payloads; errors use
fixed payload-free messages and one-based line/Unicode-character locations.

## Limits and explicit boundaries

Defaults are hard ceilings: 65,536 UTF-8 bytes before tokenization, 1,024 non-trivia
tokens, 32 levels of parentheses, owned expression depth and relation depth.
Caller-supplied larger limits are clamped; smaller limits are respected. A shared
per-parse recursion budget of 96 covers expression/relation/subquery calls across
boundaries and procedure bodies, with RAII release on errors. Wide UNIONs use an iterative sequence.
Parameter lists also have a 255-declaration ceiling. Other lists/arguments are
bounded by total input/tokens. Normalization can increase
length/nesting/token count, so reparsing still requires an adequate input budget.
No wall-clock deadline or asynchronous cancellation API is claimed.

The earlier upstream expression-parser stress failure motivated an explicit
shared recursion budget. The current adapter no longer invokes that recursive
upstream grammar; recursive-protection/stacker/psm have been removed and stress
tests run with SQLParser's std feature alone. See DEPENDENCIES.md.

Pass-through/vendor SQL, SQL Server TOP parentheses/WITH TIES, LIMIT,
EXCEPT/INTERSECT, FULL/CROSS/APPLY/USING/NATURAL joins, window functions, CAST/CASE,
DISTINCT aggregate arguments, named function arguments, multi-row VALUES and
positional `?` placeholders are rejected. Schema DEFAULT/CHECK clauses,
IF EXISTS/IF NOT EXISTS and multi-operation ALTER TABLE are also rejected. Multi-word type spellings
(such as NCHAR VARYING), signed type arguments and provider-specific schema
extensions are outside this bounded grammar. Transaction, privilege and
pass-through command languages are not query-model features. Parameter references use Access names.
Provider-specific object qualification beyond the documented external IN forms
is not implemented. Unsupported is not a native Access validity verdict.

## Primary references and acceptance evidence

The corpus is original TableCraft SQL with synthetic names, not copied fixtures
or SQL extracted from ACCDB. Documentation examples are not Access execution
results. The following references map to implemented tests:

| Grammar area | Primary reference |
| --- | --- |
| Clauses and quoting | [Microsoft introductory SQL guide](https://support.microsoft.com/en-us/access/access-sql-basic-concepts-vocabulary-and-syntax), [SELECT](https://learn.microsoft.com/en-us/office/client-developer/access/desktop-database-reference/select-statement-microsoft-access-sql), [quotation marks](https://learn.microsoft.com/en-us/office/vba/access/concepts/criteria-expressions/include-quotation-marks-in-string-expressions) |
| Operators and references | [Access reserved words](https://learn.microsoft.com/en-us/office/client-developer/access/desktop-database-reference/sql-reserved-words), [Access expression syntax](https://support.microsoft.com/en-us/access/guide-to-expression-syntax), [precedence](https://learn.microsoft.com/en-us/office/vba/language/reference/user-interface-help/operator-precedence), [Access/ANSI comparison](https://support.microsoft.com/en-us/access/comparison-of-microsoft-access-sql-and-ansi-sql) |
| Predicates | [TOP/DISTINCTROW](https://learn.microsoft.com/en-us/office/vba/access/concepts/structured-query-language/all-distinct-distinctrow-top-predicates-microsoft-access-sql), [BETWEEN](https://learn.microsoft.com/en-us/office/vba/access/concepts/miscellaneous/between-and-operator), [IN](https://learn.microsoft.com/en-us/office/vba/access/concepts/miscellaneous/in-operator-microsoft-access-sql), [wildcard modes](https://support.microsoft.com/en-us/access/access-wildcard-character-reference) |
| Composition | [Joins](https://learn.microsoft.com/en-us/office/vba/access/concepts/structured-query-language/perform-joins-using-access-sql), [UNION](https://learn.microsoft.com/en-us/office/client-developer/access/desktop-database-reference/union-operation-microsoft-access-sql), [TRANSFORM](https://learn.microsoft.com/en-us/office/client-developer/access/desktop-database-reference/transform-statement-microsoft-access-sql) |
| Subqueries | [Microsoft subquery grammar and crosstab restrictions](https://learn.microsoft.com/en-us/office/client-developer/access/desktop-database-reference/sql-subqueries-microsoft-access-sql) |
| Schema | [CREATE TABLE](https://learn.microsoft.com/en-us/office/client-developer/access/desktop-database-reference/create-table-statement-microsoft-access-sql), [ALTER TABLE](https://learn.microsoft.com/en-us/office/client-developer/access/desktop-database-reference/alter-table-statement-microsoft-access-sql), [CREATE INDEX](https://learn.microsoft.com/en-us/office/client-developer/access/desktop-database-reference/create-index-statement-microsoft-access-sql), [CONSTRAINT](https://learn.microsoft.com/en-us/office/client-developer/access/desktop-database-reference/constraint-clause-microsoft-access-sql), [DROP](https://learn.microsoft.com/en-us/office/client-developer/access/desktop-database-reference/drop-statement-microsoft-access-sql) |
| Saved queries | [CREATE VIEW](https://learn.microsoft.com/en-us/office/client-developer/access/desktop-database-reference/create-view-statement-microsoft-access-sql), [CREATE PROCEDURE](https://learn.microsoft.com/en-us/office/client-developer/access/desktop-database-reference/create-procedure-statement-microsoft-access-sql) |
| Declarations | [PARAMETERS](https://learn.microsoft.com/en-us/office/client-developer/access/desktop-database-reference/parameters-declaration-microsoft-access-sql), [SQL types](https://learn.microsoft.com/en-us/office/client-developer/access/desktop-database-reference/sql-data-types) |
| IEEE type aliases | [Microsoft Access type comparison](https://learn.microsoft.com/en-us/office/vba/access/concepts/error-codes/comparison-of-data-types) lists IEEESINGLE and IEEEDOUBLE separately |
| Action and external queries | [INSERT](https://learn.microsoft.com/en-us/office/client-developer/access/desktop-database-reference/insert-into-statement-microsoft-access-sql), [UPDATE](https://learn.microsoft.com/en-us/office/client-developer/access/desktop-database-reference/update-statement-microsoft-access-sql), [DELETE](https://learn.microsoft.com/en-us/office/client-developer/access/desktop-database-reference/delete-statement-microsoft-access-sql), [SELECT INTO](https://learn.microsoft.com/en-us/office/client-developer/access/desktop-database-reference/select-into-statement-microsoft-access-sql), [external IN](https://learn.microsoft.com/en-us/office/vba/access/concepts/miscellaneous/in-clause-microsoft-access-sql) |

Tests cover every family above, normalized model round trips, precedence and
owned structure, source retention, diagnostics, malformed/foreign syntax,
read-only subquery restrictions, source/destination distinction and resource
limits. Fixed-seed properties use 256 cases and regression persistence. Exact
local verification is recorded in IMPLEMENTATION.md; Windows/Linux and native
Access comparison have not been observed for this implementation.

Keyword spellings are accepted in dot-member positions, preserving source spelling
for qualified field/member access, including `Tags.Value` and the registered
application's stored `msysobjects.Connect`. A dot selects one identifier token;
it does not admit a clause, statement or member call. Standalone reserved names,
source names, aliases and bang access keep their existing bracket requirements.
This context-specific syntax admission does not qualify every unbracketed keyword
against native Access; formatted fields retain bracketed names.
[Multivalue execution](MULTIVALUE_QUERIES.md)
is a separate explicit host-source contract; parser acceptance does not expose
native collections or activate object/VBA access.

Hash-delimited date literals now retain adjacent subtraction/comparison operators:
`#1900-01-01#-#1899-12-30#` and `#1900-01-01#>=#1899-12-30#` parse without
extra whitespace. The adapter separates upstream PostgreSQL hash operators using
source spans; quoted text and bracketed names remain inert. Parsing alone still
does not validate the date's calendar or qualify evaluation behavior.

## VALUE query parameters

Query parameter types also include [VALUE](VALUE_PARAMETERS.md), preserving the
Access Variant parameter spelling without admitting it as a stored column type.
Length arguments and the unqualified VARIANT spelling remain refused.

## Parsed statement source

`ParsedQuery::statement_source()` exposes the exact input prefix before an
optional final lexical statement terminator. `source()` still retains the full
original text. Semicolons in quoted names, strings, comments and PARAMETERS
declarations are not trimmed as text; trivia after the final terminator is
excluded only from the prefix. This allows stored standalone read definitions to
be embedded under separate consumer guards without accepting multiple statements
or relaxing inline subquery grammar. Existing byte/token/depth limits apply to
the complete original input before this prefix is available.

## Literal parameter prompt names

[Parameter prompts](PARAMETER_PROMPTS.md) now admit bracketed punctuation in
declarations and unqualified references, keeping `[P.Q]` distinct from the field
`P.Q`. Object/alias/bang-member/callable grammar stays guarded. Qualified literal dot
fields now also support [generated crosstab headings](NAMED_CROSSTABS.md). This is syntax support;
there is no implicit declaration, interactive prompt or application binding.

`parse_expression_strict(source, limits)` now returns the same owned expression
model while refusing a trailing SQL statement terminator. Quoted semicolons and
ordinary trailing comments retain their meaning. It uses the full original byte
ceiling without a synthetic wrapper; closed Eval and runtime domain expressions
consume it. The parser still does not bind names, decide which calls are allowed
or evaluate database/application content. See [Eval boundaries](DYNAMIC_EVALUATION.md).


[Radix evaluation](RADIX_LITERALS.md) now materializes qualified &H/&O literals in
the expression service. Parsing still owns syntax only. Numeric suffixes, decimal
literal carrier inference and complete native typing remain separate acceptance
work; malformed/separated/quoted radix prefixes do not silently normalize to a
valid number.


[SQL date/time evaluation](SQL_DATE_LITERALS.md) now accepts complete-year U.S.
and English spellings plus qualified times alongside ISO. This changes the
shared evaluator and closed literal validation, not the parser's syntax-only
raw-content contract. Regional text conversion and ISO typed parameters retain
separate admission policies; broader native literal syntax remains unqualified.
