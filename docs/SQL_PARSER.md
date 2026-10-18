# Access query parsing and owned model

`tablecraft-query` parses data-query syntax without reading files, connecting to
providers, executing functions or modifying databases. `parse_query(sql, limits)`
returns `ParsedQuery`: exact original source plus a TableCraft-owned Query model.
`parse_select` remains a single-SELECT convenience entry point; use `parse_query`
for declarations, unions, crosstabs and action queries.

This is the first complete data-query parsing/model milestone, covering the
families below. It is not a claim of complete Jet/ACE compatibility. Name/type
binding, function arity, calendar interpretation, wildcard evaluation, coercion,
query execution and real Access comparison remain separate work.

The CLI command `tablecraft query parse "<SQL>"` prints normalized SQL using this
API. Invalid/unsupported input exits 2 with a diagnostic and no SQL output. Action
queries are printed as syntax; this command does not execute them.

## Implemented syntax

| Area | Owned representation and working coverage |
| --- | --- |
| Retrieval | SELECT, ALL, DISTINCT, DISTINCTROW, TOP and PERCENT; DISTINCT/TOP combinations; explicit or implicit aliases; wildcard and expression projections |
| Expressions | Width-preserving i64 literals; exact decimal/exponent/hex/octal spelling outside that carrier; single/double-quoted text; hash-delimited date/time spelling; NULL and booleans |
| Operators | Arithmetic `^ - * / \\ Mod +`, concatenation `&`, comparisons, NOT/AND/OR/XOR/EQV/IMP, LIKE/NOT LIKE, IS NULL/IS NOT NULL, BETWEEN/NOT BETWEEN, IN/NOT IN |
| Function syntax | Built-in and user-defined names, expression arguments, COUNT-style wildcard arguments, and omitted optional positions distinct from NULL |
| References | Bracketed identifiers, dot qualification and bang/property access; spelling preserved without catalog resolution |
| Relations | Ordered comma sources, INNER/LEFT/RIGHT joins with explicit ON trees, parenthesized joins, derived retrieval queries and aliases |
| Clauses | WHERE, expression GROUP BY, HAVING, expression ORDER BY with ASC/DESC; no-FROM constant/function SELECT; WITH OWNERACCESS OPTION |
| Subqueries | Scalar, EXISTS, IN/NOT IN and ALL/ANY/SOME comparisons; read-only retrieval bodies |
| Set operations | UNION/UNION ALL, independently retained per arm; SELECT, TABLE and stored-query-reference operands; final ordering |
| Declarations | PARAMETERS names, recognized Access type spellings and integer type arguments; ordered declarations retained |
| Crosstabs | TRANSFORM expression, SELECT/grouping, PIVOT expression and optional ordered fixed headings; headings are distinct from membership IN |
| Append/update/delete | INSERT VALUES (one record) or retrieval source, ordered target fields; UPDATE relations and SET assignments; DELETE target wildcards, relations and filters |
| Make-table | SELECT INTO is a separate root statement kind; destination IN is distinct from source IN; make-table statements are rejected inside retrieval operands |
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
`query/` owns retrieval composition, parameters and action statements. The private
`parser/` contains lexical, expression, relation, parameter and statement adapters.
Upstream token/parser types do not appear in public APIs. Query types are not a
validated public builder, serialization protocol or execution IR.

## Access grammar and formatting

The adapter uses SQLParser's tokenizer and parser cursor/keyword primitives.
Access expression parsing uses a bounded Pratt grammar with explicit precedence:
exponentiation, unary numeric sign, multiply/divide, integer division, Mod,
add/subtract, ampersand, comparisons/predicates, NOT, AND, OR, XOR, EQV, IMP.
This avoids inheriting generic SQL meanings for Access operators. No generic or
SQL Server dialect fallback is used. Upstream ASTs are not serialized or exposed.

Hash literals are folded from original source spans before trivia filtering.
The lexer merges `!=` with `<>`, so original spelling is checked; `!=` and `==`
are rejected. Unsupported constructs fail rather than disappearing from output.

Display prints normalized SQL with bracketed names, explicit aliases/directions,
escaped single-quoted text, parentheses around expressions/joins and a final
semicolon. Original quoting, comments and whitespace remain in `source()`.
Function names, numeric/date spelling, UNION ALL flags, parameter type arguments,
fixed pivot headings, omitted argument positions and source/destination metadata
survive normalization. Public model Debug can contain SQL payloads; errors use
fixed payload-free messages and one-based line/Unicode-character locations.

## Limits and explicit boundaries

Defaults are hard ceilings: 65,536 UTF-8 bytes before tokenization, 1,024 non-trivia
tokens, 32 levels of parentheses, owned expression depth and relation depth.
Caller-supplied larger limits are clamped; smaller limits are respected. A shared
per-parse recursion budget of 96 covers expression/relation/subquery calls across
boundaries, with RAII release on errors. Wide UNIONs use an iterative sequence.
Lists/arguments are bounded by total input/tokens. Normalization can increase
length/nesting/token count, so reparsing still requires an adequate input budget.
No wall-clock deadline or asynchronous cancellation API is claimed.

The earlier upstream expression-parser stress failure motivated an explicit
shared recursion budget. The current adapter no longer invokes that recursive
upstream grammar; recursive-protection/stacker/psm have been removed and stress
tests run with SQLParser's std feature alone. See DEPENDENCIES.md.

DDL, pass-through/vendor SQL, SQL Server TOP parentheses/WITH TIES, LIMIT,
EXCEPT/INTERSECT, FULL/CROSS/APPLY/USING/NATURAL joins, window functions, CAST/CASE,
DISTINCT aggregate arguments, named function arguments, multi-row VALUES and
positional `?` placeholders are rejected. Parameter references use Access names.
Provider-specific object qualification beyond the documented external IN forms
is not implemented. Unsupported is not a native Access validity verdict.

## Primary references and acceptance evidence

The corpus is original TableCraft SQL with synthetic names, not copied fixtures
or SQL extracted from ACCDB. Documentation examples are not Access execution
results. The following references map to implemented tests:

| Grammar area | Primary reference |
| --- | --- |
| Clauses and quoting | [Microsoft introductory SQL guide](https://support.microsoft.com/en-us/access/access-sql-basic-concepts-vocabulary-and-syntax), [SELECT](https://learn.microsoft.com/en-us/office/client-developer/access/desktop-database-reference/select-statement-microsoft-access-sql), [quotation marks](https://learn.microsoft.com/en-us/office/vba/access/concepts/criteria-expressions/include-quotation-marks-in-string-expressions) |
| Operators and references | [Access expression syntax](https://support.microsoft.com/en-us/access/guide-to-expression-syntax), [precedence](https://learn.microsoft.com/en-us/office/vba/language/reference/user-interface-help/operator-precedence), [Access/ANSI comparison](https://support.microsoft.com/en-us/access/comparison-of-microsoft-access-sql-and-ansi-sql) |
| Predicates | [TOP/DISTINCTROW](https://learn.microsoft.com/en-us/office/vba/access/concepts/structured-query-language/all-distinct-distinctrow-top-predicates-microsoft-access-sql), [BETWEEN](https://learn.microsoft.com/en-us/office/vba/access/concepts/miscellaneous/between-and-operator), [IN](https://learn.microsoft.com/en-us/office/vba/access/concepts/miscellaneous/in-operator-microsoft-access-sql), [wildcard modes](https://support.microsoft.com/en-us/access/access-wildcard-character-reference) |
| Composition | [Joins](https://learn.microsoft.com/en-us/office/vba/access/concepts/structured-query-language/perform-joins-using-access-sql), [UNION](https://learn.microsoft.com/en-us/office/client-developer/access/desktop-database-reference/union-operation-microsoft-access-sql), [TRANSFORM](https://learn.microsoft.com/en-us/office/client-developer/access/desktop-database-reference/transform-statement-microsoft-access-sql) |
| Declarations | [PARAMETERS](https://learn.microsoft.com/en-us/office/client-developer/access/desktop-database-reference/parameters-declaration-microsoft-access-sql), [SQL types](https://learn.microsoft.com/en-us/office/client-developer/access/desktop-database-reference/sql-data-types) |
| Action and external queries | [INSERT](https://learn.microsoft.com/en-us/office/client-developer/access/desktop-database-reference/insert-into-statement-microsoft-access-sql), [UPDATE](https://learn.microsoft.com/en-us/office/client-developer/access/desktop-database-reference/update-statement-microsoft-access-sql), [DELETE](https://learn.microsoft.com/en-us/office/client-developer/access/desktop-database-reference/delete-statement-microsoft-access-sql), [SELECT INTO](https://learn.microsoft.com/en-us/office/client-developer/access/desktop-database-reference/select-into-statement-microsoft-access-sql), [external IN](https://learn.microsoft.com/en-us/office/vba/access/concepts/miscellaneous/in-clause-microsoft-access-sql) |

Tests cover every family above, normalized model round trips, precedence and
owned structure, source retention, diagnostics, malformed/foreign syntax,
read-only subquery restrictions, source/destination distinction and resource
limits. Fixed-seed properties use 256 cases and regression persistence. Exact
local verification is recorded in IMPLEMENTATION.md; Windows/Linux and native
Access comparison have not been observed for this implementation.
