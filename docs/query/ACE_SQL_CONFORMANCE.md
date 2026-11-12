# ACE SQL conformance

TableCraft implements substantial Access SQL, but grammar acceptance, executable
semantics, native persistence and SQL View availability are separate capabilities.
This matrix tracks the remaining work toward full ACE 16 compatibility. It is not
a completion percentage or a replacement for feature-specific contracts.

The baseline is the source audit of 11 November 2026. Update a row only when its
implementation and acceptance evidence change. Passing fixture queries does not
establish coverage of other statements, expression states or invocation contexts.

## Invocation context

Access SQL View and ACE through DAO/OLE DB do not accept identical languages.
For example, Microsoft documents `DEFAULT` through OLE DB/ADO and explicitly says
it errors in Access SQL View. The syntax-only parser may represent an engine
construct without granting a GUI execution capability. Provider SQL, stored VBA
and application references likewise require their own supplied capabilities.

The database's ANSI wildcard mode, sort declaration and field comparison policies
must remain independent of UI language, display preferences and file-header
recognition. A database format identifier is not an Access application version.

## Current matrix

“Working profile” means an executable subset exists. “Parsed” means only an owned
syntax representation exists. Neither status establishes unrestricted native
compatibility. The owning documents describe the exact admitted shapes.

| Area | Current implementation | Remaining acceptance |
| --- | --- | --- |
| SELECT and expressions | Working projection, aliases, WHERE, ordering, literals and Access operators. | Complete expression-state, type/category, error and regional behavior under the [scalar contracts](../expressions/EXPRESSIONS.md). |
| Joins and subqueries | Working INNER/LEFT/RIGHT joins, comma products and correlated scalar/EXISTS/IN/quantified subqueries. | Finish documented composition and category restrictions; preserve eager errors and actual outer absence. See [retrieval](QUERY_EXECUTION.md). |
| Grouping and aggregates | GROUP BY/HAVING, standard/statistical aggregates and domain functions have working profiles. | Aggregate return categories, promotion, empty/NULL/Error behavior and downstream operations require complete coverage, not just matching displayed totals. See [reviewed cases](../../tests/reports/queries/READ_QUERY_P0_GAPS.md). |
| DISTINCT, DISTINCTROW and TOP | Working profiles include source identity and ordered boundary ties. | Complete admitted-shape and native edge semantics; source-order TOP results are not a guarantee of one native subset. |
| UNION | UNION/UNION ALL and retained type conversion work. | Complete mixed-family precedence and declared-category behavior, including empty arms. See [UNION types](UNION_TYPES.md). |
| Crosstabs | Qualified TRANSFORM/PIVOT and named dependency discovery work. | Inline TRANSFORM sources and broader native/heading/type combinations remain restricted. See [crosstabs](CROSSTABS.md). |
| Parameters | Declared typed scratch/saved parameters and GUI prompts work. Explicit local [unresolved-name discovery](VALUE_PARAMETERS.md) now supplies implicit VALUE prompts through issued metadata plans. | Native contextual type inference, complete prompt order and broader dependent query/application scopes remain incomplete. |
| LIKE and comparison | Explicit ANSI modes and qualified database comparers work. | Full sort-order/LCID families, field-policy precedence and native comparison equivalence remain incomplete. See [pattern matching](../expressions/text/PATTERN_MATCHING.md) and [native query policies](../access/queries/NATIVE_QUERIES.md). |
| Built-ins | The tracker records working implementations for the currently identified query functions. | Complete documented argument, carrier, formatting, calendar, locale and error gaps. `Partial` is not a missing function name; `Full` is not native qualification. See [the tracker](../expressions/ACCESS_BUILTINS.md). |
| Collections | Multivalue members and qualified attachment/history recordsets work. | Broader whole-recordset operations and source/profile combinations require explicit semantics. See [collection queries](collections/MULTIVALUE_QUERIES.md). |
| UPDATE and DELETE | Reviewed native/GUI [joined UPDATE](JOINED_ACTION_QUERIES.md), local UPDATE/DELETE with qualified correlated EXISTS/IN predicates, and explicit single-target DISTINCTROW joined DELETE retain original target identities and consuming preparation. Joined DELETE deduplicates original handles after all predicate work and uses the existing native save/reopen service. | Multiple targets, broader correlated joined predicates, constraints and required programs remain separately qualified. See [mutation functions](MUTATION_SCALAR_FUNCTIONS.md) and [reviewed updates](../access/writes/REVIEWED_UPDATES.md). |
| INSERT | Reviewed VALUES and SELECT append routes work. | Complete source/assignment/default/constraint semantics and native target coverage. See [INSERT SELECT](INSERT_SELECT.md). |
| Make-table | SELECT INTO has [complete typed query staging](MAKE_TABLE_QUERIES.md), including collision checks, empty-result declarations and original source context. Qualified direct-local native preparation retains WHERE, DISTINCT/DISTINCTROW and TOP/PERCENT with native comparison/LIKE policies and supplied expression context. Fixed computed outputs, native wildcard expansion and explicit declared inputs now feed that same atomic native preparation. Runtime review and normal SQL View Run save schema plus complete rows through one original-intent V3 journal and guarded reopen. | Groups, named/joined/external sources, computed Text/Decimal physical declarations and broader native types remain unsupported by publication. The [release checkpoint](../../tests/results/queries/ace-sql-release-workflows-2026-11-12.json) covers actual parameter prompts and wildcard publication. New Text uses qualified destination ROOT General Legacy/2010 defaults; other defaults refuse Text construction. Full ACE SQL/source coverage remains incomplete. |
| SQL DDL | Issued [runtime schema reviews](../application/RUNTIME.md#reviewed-sql-schema-statements) route qualified CREATE TABLE, DROP TABLE, ALTER TABLE DROP COLUMN, qualified unadorned scalar ADD COLUMN and one-field ordinary nonunique LONG CREATE/DROP INDEX through existing native planning/save services. The [scalar ADD original checkpoint](../../tests/results/queries/unadorned-scalar-add-originals-2026-11-11.json) retains native declaration parameters, full-source binding and recovery evidence; application acceptance is checked separately. | [GUI SQL Run](../ui/SQL_WORKSPACE.md#reviewed-schema-sql) now confirms these qualified operations and publishes only their actual guarded AFTER reopen; destructive targets must have closed tabs. General ALTER/type changes, broader index/constraint DDL, saved-definition DDL and broader source/declaration coverage remain unfinished. See [parser scope](SQL_PARSER.md). |
| DDL grammar details | CHECK expressions, DEFAULT annotations and ALTER COLUMN nullability/default/inline constraints now parse into owned fields, alongside primary/unique/not-null/foreign-key declarations. Same-limit normalization covers deep CHECK/default expressions, including nested retrievals and precedence. | Provider expression validity, native constraint enforcement and GUI execution remain separate. Parser tests do not qualify any native mutation. |
| OWNERACCESS | The clause is parsed. | Retrieval binding refuses it. Implement the actual applicable security policy rather than ignoring the clause. |
| Saved definitions | Qualified native retrieval and restricted saved SELECT expression edits work. | General QueryDef creation, Save As, rename, deletion and other definition bodies remain unfinished. See [saved query editing](../application/SAVED_QUERY_EDITING.md). |
| External/application contexts | Explicit host-bound external sources and named definitions have working read profiles. | No stored path opens a provider implicitly. Forms/Reports/TempVars and user-defined calls need supplied host contracts; deferred VBA execution is not activated by parsing. |

This list intentionally excludes foreign SQL features such as window functions,
LIMIT or FULL JOIN as presumed ACE requirements. A rejected construct is a gap
only after its availability in the intended Access context has been established.
Deferred VBA-only tracker entries are not unfinished native query built-ins.

## Active implementation priorities

ACE SQL conformance is the current implementation focus. Work proceeds across
three independent tracks; a track stays incomplete until its consuming workflow
passes, even when an earlier layer has working functionality.

| Track | Next acceptance boundary |
| --- | --- |
| SQL DDL | Extend qualified statement/declaration/source coverage beyond current CREATE and DROP services while preserving reviewed persistence, invocation restrictions and actual constraint enforcement. |
| Make-table queries | Extend the now-working direct-local reviewed atomic publication profile to broader query/source/declaration contexts with exact native shape and destination-policy evidence. Do not infer inherited keys, indexes, defaults or generation from a SELECT projection. |
| Joined action queries | Extend the admitted joined UPDATE, single-target DISTINCTROW joined DELETE and correlated local UPDATE/DELETE contexts. Broader target/predicate shapes need their own identity and updateability proofs; joined read support alone does not make a source updateable. |

Expression, comparison, parameter, saved-definition and security gaps in the
matrix remain in scope after these statement-level boundaries. These priorities
do not expand SQLite, project scripting or deferred VBA execution.

## Evidence required for a completed row

1. Establish the Microsoft contract or recorded native behavior, including engine
   version, invocation API, ANSI mode and relevant database/field policies.
2. Test owned syntax and normalized round trips, including invalid framing and
   existing parser work/depth boundaries. Parsing must not scan user records.
3. Exercise binding and execution with independent expectations for values,
   result declarations, NULL/empty/error states, eager failures and composition.
4. For mutations, verify original selection, constraints/programs, exact byte
   preservation and fresh reconstruction during pending/committed/settled recovery.
5. Verify the shared runtime and actual GUI journey when that context exposes the
   feature. An unavailable command must remain disabled; a parsed statement is
   not permission to execute it.

Known implementation gaps can be addressed before a Windows comparison machine
is available. Native comparison evidence remains necessary for a full Access
conformance claim. Keep independent models, local implementation tests, headless
presentation and actual Microsoft Access execution distinguishable.

## Primary references

- [Microsoft Access SQL reference](https://learn.microsoft.com/en-us/office/client-developer/access/desktop-database-reference/overview-of-the-access-sql-reference)
- [Access SQL versus ANSI SQL](https://support.microsoft.com/en-us/access/comparison-of-microsoft-access-sql-and-ansi-sql)
- [Modify a table's design using Access SQL](https://learn.microsoft.com/en-us/office/vba/access/concepts/structured-query-language/modify-a-table-s-design-using-access-sql)
- [SELECT INTO](https://learn.microsoft.com/en-us/office/client-developer/access/desktop-database-reference/select-into-statement-microsoft-access-sql)
- [WITH OWNERACCESS OPTION](https://learn.microsoft.com/en-us/office/client-developer/access/desktop-database-reference/with-owneraccess-option-declaration-microsoft-access-sql)
