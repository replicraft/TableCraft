# Compatibility-driven roadmap

Priority decision updated 18 October 2026. The native Jet/ACE engine, Access
interoperability, visual application builder and SQLite lead implementation.
This orders work without imposing a strict sequence; UI and compatibility testing
can progress alongside storage. [IMPLEMENTATION.md](IMPLEMENTATION.md) records
working behavior: bounded native metadata/scalar/indexed reads, owned Access SQL,
read-only retrieval, exact values, generic I/O and a headless CLI. Read-only [SQLite snapshots](SQLITE.md) feed the same query evaluator. Bounded
offline Jet 4 edits now work; native shared-file coordination, forms/reports and UI
hosts remain unimplemented.

LCase/UCase now have [explicit Unicode casing](TEXT_CASING.md) in local read/query
execution. Windows locale/NLS comparison remains an acceptance gap.

Native saved [UNION reconstruction](NATIVE_SAVED_UNIONS.md) now feeds the shared
read executor through explicit saved/direct/named retrieval. Native saved
[crosstabs](NATIVE_SAVED_CROSSTABS.md) now reconstruct and explicitly execute; their
positive stored TOP/PERCENT now use the shared final row limiter.
[Named literal fixed schemas](NAMED_CROSSTABS.md) now compose without binding scans;
dynamic/computed named schemas, inline TRANSFORM, TOP 0 and wider native comparison
remain open.

## Primary target: 100% ACE 16 compatibility

ACE 16 is the primary implementation and compatibility-testing target. The
long-term goal is 100% compatibility, not a permanently restricted feature subset.
Drive native engine, query and interoperability decisions from ACE 16 behavior.
Immediate read/query priorities are ACE 16, ACE 2007/ACE 12 and Jet 4.
The registered example is ACE 2007; its successful bounded reads remain part of
regression coverage. Jet 1/2/3 and ACE 14/15 remain secondary priorities. Preserve existing working
coverage and regression checks; expand those formats only when it directly
supports ACE 16 or the user explicitly changes priorities. Further SQLite
expansion remains paused while ACE 16 compatibility is the focus.

The goal covers native database reads/writes, scalar and advanced field types,
text/collation and expression/query semantics, schema/relationships/constraints,
indexes/allocation, encoding/encryption, table data macros, transactions/recovery,
native shared-file coordination and preservation/interoperability of application
objects. Fixtures with recorded ACE 16 builds and actual Microsoft Access comparisons
must establish each capability. Keep unimplemented, unsupported and unqualified
cases visible; no partial profile or Jackcess result proves 100% compatibility.

Existing VBA execution, editor/debugger and Windows/COM/Wine deferrals remain in
force. They are explicit compatibility gaps under their owning documents; the
100% goal does not label them implemented or quietly waive them.

## Accepted native application direction

[The project decision](decisions/0001_PROJECT_FORMAT_AND_EXECUTION.md) records two
first-class application paths: native SQLite-backed `.tcraft`/`.tcraftproj/` with
Python/visual actions, and native MDB/ACCDB interoperability with Access objects,
macros and preserved VBA. Python supersedes the earlier TypeScript-default idea.
These are accepted targets, not implemented features. Shared designers will use
backend-independent models with explicit format capabilities.

Required contracts now cover [versioning](formats/SCHEMA_VERSIONING.md),
[project lifecycle](architecture/PROJECT_LIFECYCLE.md),
[application commands](architecture/APPLICATION_EXECUTION.md),
[backend-owned transactions](architecture/TRANSACTIONS.md),
[Python deployment/trust](features/PYTHON_SCRIPTING.md),
[application conversion/upgrades](features/PROJECT_MIGRATIONS.md) and
[peer schema epochs](features/COLLABORATIVE_SCHEMA_EVOLUTION.md).
ACCDB-to-`.tcraft` conversion precedes export back to Access; native Python does
not become Access-executable automation. Web/PHP/TypeScript generation is later
work and does not reopen the native language decision.

The current native read/query priorities below remain in force; this does not restart broad
SQLite expansion. Once the native priority permits, prove a minimal project
identity/version gate and recoverable internal migration before broad Python/UI
work. Then prove a SQLite/form/Python/report workflow across qualified platforms,
real native Access save/reopen plus separately mixed-client writes, and a full
supported Access-to-native application conversion. Peer evolution follows its
own convergence/upgrade proofs. [Qualification requirements](testing/PROJECT_COMPATIBILITY.md)
replace subjective completeness ratings. Project health, deployment updates and
actionable recovery precede optional AI or additional scripting languages.

## Shared editing and agent workstream

[The editing decision](decisions/0002_SHARED_APPLICATION_EDITING.md) adds future
structured application edits, revision-aware changesets, headless previews and
application scenarios shared by human and agent hosts. Begin with typed discovery,
version/capability gates and safe semantic operations; then prove staging/diff/
validation/authorization/apply, rendering and scenario execution before expanding
MCP tool count. `tablecraft-edit` can begin as a module; no empty package is added.

[MCP](interfaces/MCP.md) uses rmcp when implemented, with stdio first and explicit
owned job/changeset handles. Python generation/checking, conversion and Agent
Activity follow working shared services; P2P agent coordination and built-in AI
remain later work. Keep `tablecraft-builders`, not the proposal's old design name.
[Agent acceptance](testing/APPLICATION_SCENARIOS.md) includes stale revisions,
approval/content mismatch, permission/version bypass, duplicate calls, cancellation,
uncertain outcomes, prompt injection and concurrent GUI edits. This remains a
future application workstream; current implementation focuses on reliable ACE 16
and Jet 4 reads and queries, with SQLite expansion still paused.

## Current execution order

The immediate user priority is reliable read-only access and query execution for
**both ACE 16 `.accdb` and Jet 4 `.mdb`**. This supersedes the previous write-first
execution order. ACE 16 remains the long-term primary compatibility target;
Jet 4 read/query reliability is part of the immediate objective, not optional
legacy expansion.

The [distribution read scope](features/ACCESS_DISTRIBUTION.md#required-read-scope)
also includes `.accdr`, `.accde`, `.mde`, `.accdt` and `.accdc`. Raw runtime/compiled
databases reuse header-selected native readers; templates and deployment packages
need separate bounded container/data readers. Single-database ACCDC CAB reading
now works through AccessInput; ACCDT reading and native package qualification
remain pending. Reading them does not depend on VBA
execution, application instantiation or signing support. Filename-routing tests
are separate from native compiled/package fixture qualification.

Prioritize native decoding and preservation of exact scalar values, reliable
table/saved-query discovery, end-to-end SELECT behavior and useful explicit
failures for unsupported features. Qualify both families with independent fixture
comparisons, malformed-input/resource/cancellation checks and unchanged-source
checks. Decimal query arithmetic now has a bounded rust_decimal implementation; extend
promotion, overflow, scale and rounding coverage through explicit contracts and
evidence rather than developing a general decimal arithmetic engine.
Access collation and expression equivalence still need actual Access evidence.

[Jet 4 qualification](NATIVE_JET4.md) adds exact legacy Decimal indexes/tuples,
relationship checks and preservation around unrelated offline Int32 edits, plus
saved-SELECT comparisons. ACE-only advanced fields are not Jet 4 parity targets.
Further write expansion follows the read/query reliability work;
other legacy expansion and broad SQLite work remain paused. Windows testing will
be performed later by the user. Independent fixture work and engineering write
experiments may proceed now, but do not label native Access conformance or
mixed-client safety established without actual evidence.

The [ACE 16 engineering preview](testing/ACE16_WRITE_PREVIEWS.md) now supports
Boolean masks, nine nullable fixed scalar carriers and qualified ordinary/unique non-NULL
integral index reordering plus eligible indexed Byte, Int16/32/64 and Currency NULL transitions within
existing pages, independently
compared on fresh 2016/2019 files. It is separate from the normal save/journal API.
Checked type-2 foreign indexes now participate in eligible integral previews;
complete relationship membership must pass before and after each candidate.
Two-to-ten-column integral composite indexes now participate, including eligible
foreign tuples and existing-page repacking. Explicit multi-column/table batches
now stage all fields and index components against the original image and validate
the complete candidate once. A separate bounded update-cascade planner now follows
original integral/Currency associations across chains, overlapping paths and cycles.
GUID keys/tuples and their declared cascades now participate in existing-page
previews. Decimal previews now reuse existing 17-byte variable slots and qualified
indexes/tuples, including same-precision/scale declared update cascades, without
resizing records. Boolean/IEEE/legacy DATE index updates now participate in fixed batches;
relationship-action coverage for those additional carriers remains unimplemented. Delete actions, remaining indexed carriers and production saves remain explicit
blockers. Reciprocal pointer/role/action
inspection, relationship-catalog agreement and qualified physical reads/checks
are implemented. Bounded final-child nodes now have authored ACE preview regression
coverage; independent/native qualification and broader indexed carrier mutation
remain. Write work also requires positive
data-macro evidence/absence detection and broader
physical mutation, allocation and recovery; these are prerequisites to a writable
frontend, not features supplied by the preview.

## Remaining local-file read/query work

Read/query status reviewed 20 October 2026 against current profile selectors,
query binding/execution and native read limits. Working read profiles are Jet 4,
ACE 2007/2010 and the ACE 2016/2019 layout families. Header recognition for other
families is separate from usable table reads. Basic CLI inventory, counts from
stored metadata, row pages, ordinary scalar queries and qualified saved SELECT
dependencies already work; they are not remaining scaffolding tasks.

**Next priority: encrypted local-file reads**, ahead of further binary compound
keys, automatic index selection and broad function expansion. Start with ACE 16
password-based opening and page decoding, then qualify the applicable ACE 2007
and Jet 4 schemes separately. Format family alone does not select a cipher.
Current header inspection reports a nonzero page-encoding key; it does not
identify an encryption scheme, verify a password or decrypt pages. Explicit
[`encryption inspect PATH`](NATIVE_ENCRYPTION.md) now recognizes bounded Office
envelope versions/families, without validating cipher parameters or credentials.
Separate library Agile APIs now validate AES-CBC/SHA descriptors, verify explicit
credentials and decode data pages with independent raw-page evidence. Source-bound
table/query integration and global CLI `--password`/`-p` now work for qualified
Agile inputs, including ordinary ACE 2010-family reads required by the reference
files. Hidden interactive password input now works; noninteractive callers retain `-p`.
Explicit remove-encryption and change-password copies now work across all qualified
read families/codecs, preserving sources and existing destinations under a 64 MiB
image cap. Add-encryption now creates Agile AES-256/SHA-512 ACCDB copies and
native Jet 4 encoding/password copies under the same limits; actual native
Access save/reopen remains unqualified. After this encryption work, the user has
selected **item 2 below: native text behavior**, ahead of broader scalar work. Jet 4 encoded reads and qualified Binary RC4, CryptoAPI RC4 and standard/compatibility
AES Office reads now work alongside Agile. Legacy password management, external
providers and other unqualified schemes remain gaps; encrypted updates stay blocked.

The first implementation slice must identify supported encryption descriptors,
accept explicit credentials at the host boundary, verify them where the scheme
permits and decode pages before the existing structural parsers. Keep cipher
primitives in maintained libraries and native key derivation/page mapping in the
ACE engine. Passwords and keys must stay out of normal diagnostics. Distinguish
missing/incorrect credentials, unsupported schemes and malformed descriptors;
bound key-derivation work and preserve cancellation/resource checks. Existing
unencrypted reads must retain their current behavior.

Qualification requires known-answer crypto checks and independently generated
encrypted/plaintext pairs with recorded provenance, unchanged-source checks,
wrong/missing-password and malformed-descriptor cases, and matching table/row/query
results. Actual Access-generated encrypted files and Windows comparisons remain
separate acceptance evidence. This slice opens existing files read-only; it does
not qualify encrypting files, changing passwords or encrypted writes.

Jet 4 file encoding and its database password are separate features; modern
ACCDB password encryption and legacy workgroup authorization also need distinct
policies. Primary references are [Microsoft's database encryption guidance](https://support.microsoft.com/en-us/access/encrypt-a-database-by-using-a-database-password)
and [Jackcess Encrypt](https://jackcessencrypt.sourceforge.io/), whose separate
codec provider and Jet encoding/password APIs provide independent comparison
paths. Review exact schemes, source revisions and library choices before adoption.

1. Complete Access scalar/query semantics. The [built-in tracker](ACCESS_BUILTINS.md)
   records individual implementation status and remaining gaps; explicit
   [StrComp modes](STRING_COMPARISON.md) and [InStr/InStrRev search](STRING_SEARCH.md)
   and [Replace](TEXT_REPLACEMENT.md) now have bounded implementations, alongside
   [Space/String/StrReverse](TEXT_CONSTRUCTION.md) with explicit current Unicode limits
   and [selected single-byte character pages](CHARACTER_CODE_PAGES.md).
   [CDate/DateValue/TimeValue](DATE_CONVERSIONS.md) now use explicit Gregorian
   text/year controls and checked component extraction; native/full regional gaps remain.
   [Date/Time/Now](CLOCK_FUNCTIONS.md) now have explicit clock providers and CLI
   local/fixed hosts; native precision/platform/volatility qualification remains open.
   [FormatNumber/Percent/Currency](STYLED_NUMBER_FORMATTING.md) now provide bounded
   explicit-profile output; full NLS/Variant/native formatting remains open.
   [Standalone Weekday](WEEKDAY.md) now shares date conversion and week policy,
   with native/full coercion qualification still open.
   [Host week settings](WEEK_SETTINGS.md) now resolve DatePart/DateDiff system
   options while preserving defaults; native NLS and w anchors remain open.
   [MonthName/WeekdayName](CALENDAR_NAMES.md) now support explicit names and
   host-issued first-day context; broader calendars/native NLS remain open.
   [FormatDateTime](DATE_FORMATTING.md) now provides five named modes under
   explicit profiles; broader regional/native formatting remains open.
   [CStr](CSTR.md) now covers Text/Boolean and explicit numeric/date output profiles;
   full regional, Variant and native precision/rounding behavior remain open.
   [IsDate](ISDATE.md) now shares explicit Gregorian recognition and year context;
   broader regional/Variant behavior and native qualification remain open.
   [IsNumeric](ISNUMERIC.md) now recognizes current scalar types and whole numeric
   Text under explicit profiles; native/complete locale behavior remains open.
   [Explicit numeric Text formats](NUMBER_FORMAT.md) now extend the nine numeric
   conversion calls; complete regional behavior and native qualification remain open.
   [Hex/Oct/Str](NUMERIC_TEXT.md) now provide checked numeric spelling;
   native widths, floating notation and broader coercions remain open.
   [Val](VAL.md) now supports bounded numeric prefixes with checked suffixes;
   native subtypes and coercion/error qualification remain open.
   [Chr/Asc/ChrW/AscW](CHARACTER_FUNCTIONS.md) now use checked byte/UTF-16 profiles;
   native coercion, DBCS and lossless surrogate results remain gaps.
   Built-ins beyond implemented NULL/conditional
   (including eager Switch/Choose selection)
   and text/date-component/DateAdd/DateDiff/DatePart/DateSerial/TimeSerial, numeric Abs/Sgn/Int/Fix/Round and Double-valued Sqr/Exp/Log/Sin/Cos/Tan/Atn,
   plus checked integer/Boolean and explicit IEEE
   conversion calls (including further text, date arithmetic beyond the explicit legacy DATE +/-
   serial profile, and conversions),
   full Variant/Unicode behavior, broader exact Decimal/Currency arithmetic and
   result promotion remain absent. Computed pure grouping expressions and
   First/Last encounter aggregates now work with bounded structural membership
   checks. Top-level fixed/dynamic crosstab execution now works under explicit
   heading/resource policies; native saved crosstabs now reconstruct into this top-level
   executor. Positive stored TOP/PERCENT and named literal fixed-heading schemas
   now execute; dynamic/computed named schemas, inline TRANSFORM, TOP 0 and wider
   native naming remain gaps. Bounded scalar-table DISTINCTROW now compares whole selected
   records, retaining hidden fields; active grouping/derived-record cases and native
   comparisons remain unqualified. Broader sort/alias expressions and wider native
   saved-query reconstruction also remain incomplete. Native saved SELECT/crosstab
   sources now support bounded sixteen-source join forests, disconnected comma
   products and inner predicate placement; [join scope](NATIVE_SAVED_JOINS.md)
   separates independent row-model evidence from pending native Access qualification.
   [Stored derived SELECT/UNION sources](NATIVE_SAVED_DERIVED.md) now reconstruct
   and execute through the existing non-lateral binder, including outer parameters
   and local named dependencies; remote qualifiers and native qualification remain.
   [Already-bracketed stored identifiers](NATIVE_SAVED_NAMES.md) now work across
   sources, aliases and ASCII parameters with raw endpoint matching and normalized
   duplicate checks. [Literal punctuated prompts](PARAMETER_PROMPTS.md) now parse,
   bind and reconstruct in native ASCII declarations, including nested reads,
   UNION/crosstabs and bracketed CLI assignments; qualified control bindings and
   native Unicode/regional rules remain gaps.
   [Native fragment ordering](NATIVE_SAVED_ORDERING.md) now supplies a stable keyed
   view to SELECT/UNION/crosstab reconstruction while preserving physical metadata
   and the reference whole-definition NULL policy; native Access evidence remains.
   See [expression scope](EXPRESSIONS.md) and [query scope](QUERY_EXECUTION.md).
2. Qualify native text behavior. Current ordinal comparison and explicitly selected
   LIKE modes do not implement database locale/case/accent ordering or native
   character-unit behavior. The [Windows ACE capture recipe](testing/NATIVE_TEXT_ORACLE.md)
   now prepares original Short/Long Text comparison probes with a portable
   provenance/completeness checker and an explicit-policy CLI replay report;
   Windows execution/replay remains unrun and no native
   SQL policy is enabled by it. [Exact native sort descriptors](NATIVE_TEXT.md) now
   expose database and text-column LCID/variant/version, including unknown values;
   General legacy/General 2010 key payload encoding now has exhaustive independent
   UTF-16-unit and multi-character checks. Single Short/Long Text index reads now
   validate traversal, overflow digests and exact row-key agreement, including
   projections. Explicit physical text start/end ranges now work for library hosts,
   including conservative pruning and partial-snapshot checks. A standalone
   provisional full-string General weight comparison API now works with widened
   positions, cumulative budgets and no index-prefix truncation. The Access
   adapter and read-only CLI now expose explicit General policies across query
   comparisons with a separate cumulative budget. General database-mode
   InStr/InStrRev/Replace now use full-weight consumed spans, including expansions,
   with bounded pruning and independent short-key controls. A derived immutable
   General LIKE element lookup now recognizes single-unit expansion equivalents,
   with exact regeneration and FST/BTree/sorted-vector benchmarks. Matcher
   UTF-16/literal/list transitions now work through explicit General policies,
   with literal spaces and excluded-prefix negation. Independent short models
   agree; native LIKE qualification and broader
   search/text-mode behavior,
   complete mixed column/database policy resolution, broader locales and safe
   SQL text-predicate pushdown remain next. Explicit native `--collation database`
   now selects a stored General family with exact homogeneous scalar-schema
   checks during metadata-only binding; unknown/mixed and collection descriptors
   fail without fallback. Query preparation now retains opaque scalar text-policy
   identities, validates their declarations and checks stability through ordinary/
   collection/parent scans. Native snapshots retain exact descriptor identities.
   Bound field/output slots now carry static provenance through aliases, selected
   string-preserving expressions, aggregates, unions and fixed crosstabs, with
   conflict visibility and no row scans. Explicit host resolver routing now works
   across predicates, tuple operations, sets, sorting/TOP, aggregates and crosstabs.
   Native provider/CLI integration, child descriptors and native precedence remain
   unfinished; this does not complete native mixed resolution.
   Windows comparisons are deferred at the user's direction; continue
   implementation using reviewed references and independent checks, retaining
   native qualification as a separate pending gate.
   Single Short Binary index reads/checks now work;
   typed Binary ranges also work for explicit library hosts. Compound Binary
   reads now share the owned tuple reader and whole-entry digest checks. Automatic
   binary predicate pushdown, compound seeks and SQL text seeks remain unsupported.
   Text-bearing compound indexes now support mixed two-to-ten-column reads and
   projections, including per-column General descriptors and whole-entry digests;
   Memo/full-width uniqueness and native text writes remain unqualified. Additional typed/composite seeks and index selection are
   performance work beyond existing Int32/Int64 ranges.
3. Complete applicable ACE advanced-field reads/query access. Stored multivalues,
   attachments and Memo history have qualified browsing APIs. Backend-neutral
   scalar multivalue `.Value` expansion now works over explicit host collection
   views and explicit native `--multivalues` input. Direct scalar-parent membership criteria now work. Explicit `--collections` now
   exposes aligned stored attachment metadata members; an additional explicit
   `--decoded-attachments` option supplies bounded original-file Binary views.
   Unprojected siblings of displayed/grouped collection members now retain
   aligned child predicates/sorts, including qualified joins and correlated uses.
   Scalar-only parent membership now composes over joined/derived relations under
   explicit nullable/absence rules, with native comparisons still pending.
   Mixed independent parent/flattened WHERE criteria now preserve displayed
   cardinality under the same explicit policy. Crosstab WHERE membership now
   preserves pivot-only expansion and the dynamic pre-WHERE heading domain.
   Scalar/set/quantified and nested correlated subqueries now retain explicit
   collection-item scopes and shared budgets. ON member dependencies now
   drive child expansion without output fields, preserving duplicates, outer joins
   and mixed WHERE membership. Native collection ON admissibility/cardinality
   remains unqualified. Multiple hidden members in one WHERE atom now retain
   same-child alignment, bounded independent products and scoped tuple captures;
   deeply correlated large products still hit the existing work ceilings.
   Native FileData SQL representation, independent hidden HAVING/ORDER BY,
   history member SQL and complete native semantics remain. ACE 16 calculated
   fields have explicit
   cached reads, not expression evaluation or freshness guarantees. Jet 4/ACE 12
   are not required to implement field types absent from those formats. See
   [advanced fields](features/ACCESS_ADVANCED_FIELDS.md).
4. Broaden native storage variants and scale. Deleted fixed/variable histories
   now have an identity/count-based read profile and independent derivative checks;
   ambiguous histories and other unqualified row/schema variants, encryption beyond
   the working Agile profile, legacy password protection and
   Jet workgroup/replication-related variants need implementation/evidence.
   Native read sources now have a 2 GiB envelope, separately from 64 MiB complete
   images. Map/page, snapshot, payload and buffered output ceilings still restrict
   real-world coverage. Incremental page-address traversal and source-bound scalar
   cursors, including explicit ordinary scalar subsets of mixed schemas, now
   support bounded batches without earlier-root rescans. Sequential query snapshots
   materialize through these batches with cumulative controls and complete caches.
   Native CLI queries now offer an explicit cumulative byte allowance while
   preserving separate row/work/execution/output limits and the default budget.
   CLI row continuation,
   streaming and larger query
   execution need their own resource/cancellation checks, not merely raised limits.
5. Complete read-only host behavior where needed. Stable/offline inputs are the
   working contract. Coherent reads while native Access is modifying a shared
   file require compatible snapshot/coordination behavior. Linked-table metadata
   is visible, but accessing external sources remains a separate provider target.
6. Expand real Access qualification. Independent Jackcess fixtures, corruption
   tests and unchanged-source checks cover the bounded profiles; they do not
   establish native Access SQL, locale or file-variant compatibility. Compare
   exact values, NULLs, ordering, expressions and saved-query outcomes against
   actual Access-generated files/builds on Windows. Windows evidence remains
   pending and does not stop independent implementation work.

Broader ACE 14/15 field/index qualification beyond ordinary-table reads, Jet 3 table decoding and optional Jet 1/2
research remain secondary format expansion, rather than prerequisites for the
current Jet 4/ACE 2007/ACE 16 target. VBA/UDF execution remains explicitly deferred;
queries depending on it must report that limitation. Writes, allocation/mutation,
write transactions/recovery, schema editing, data-macro write safety, UI, forms/
reports and SQLite expansion are outside this read/query checklist. They remain
in the broader backend roadmap; this list does not mark them completed or waived.

## Next implementation and measurement

[ACE 16 bounded reads](NATIVE_ACE16.md) now have original V2016/V2019 Jackcess
fixtures and independent comparisons, including exact Large Number and Date/Time
Extended reads. Inert advanced-field declarations now work through
[feature inspection](features/ACCESS_ADVANCED_FIELDS.md#working-inert-metadata-slice).
Scoped complex child links/scalars, bounded original attachment payloads, stored
scalar calculations and opaque catalog payload inspection now work. Continue
complete advanced-field/collection semantics, exact physical index/constraint
coverage and required data-macro handling before enabling ACE 16 writes. The
[data-macro evidence dependency](testing/DATA_MACROS.md) requires positive ACE 16
fixtures/native comparisons; current catalog/XML observations cannot permit writes. Continue
acquiring actual Microsoft Access fixtures with recorded builds and provenance;
independent-reader evidence does not qualify native Access conformance.

Native [field properties](NATIVE_PROPERTIES.md) and [relationship declarations](NATIVE_RELATIONSHIPS.md)
now have bounded readers and independent fixtures. Bounded field/rule and Int32
relationship audits are implemented. Complete-image retention, [database contracts](DB.md),
[journaled fixed Int32 edits](NATIVE_WRITES.md) and [query UPDATE copies](QUERY_EDITING.md)
now form a working bounded editing path. Unsupported constraint/content profiles
remain write blockers. Further SQLite expansion is paused at the requested boundary;
remaining native qualification and broader write coverage stay explicit below. Qualified Int32/Int64 index checks already have
independent fixture comparisons; native Access conformance remains separate.
Native schema allocation paths are
fallible and sorting avoids scratch allocation. Read-only consistency reports combine
sequential rows and eligible indexes with explicit coverage gaps. Usage-map ordering
and chained definitions have bounded corruption/budget and Jet 4/ACE evidence.
Writes still require preservation, required macro semantics, transactions and
recovery qualification; successful reads do not supply those guarantees.
Native Int32/Int64 seeks and [safe predicate pushdown](QUERY_EXECUTION.md#safe-source-constraints)
now work through explicitly selected indexes. Schema discovery is separate from
row loading. Physical traversal stays in ACE, source adaptation in Access and
backend-neutral planning in query. Automatic index choice and TOP pushdown remain
future work.

Continue original minimal fixture recipes and independent-reader comparisons;
add real Microsoft Access comparisons as the oracle environment becomes available.
The [application corpus](testing/ACCESS_APPLICATIONS.md) still has one candidate
and no qualified application open/preserve/edit/save-and-reopen baseline. A Jackcess
comparison is independent evidence, not Microsoft Access conformance.

## Ordered implementation path to the UI

Apply each unfinished native milestone below to ACE 16 first. Existing Jet 4 and
ACE 12 implementations are reusable foundations, not substitutes for ACE 16
qualification. The SQLite read-only
foundation was implemented early; that does not complete the preceding milestones
or change the next native task. Further SQLite expansion follows this sequence.

| Step | Milestone | Completion work remaining |
| --- | --- | --- |
| 1 | Native schema metadata | Bounded ACE 16-family metadata, inert saved-query fragments and owned local SELECT reconstruction, scoped complex links/child scalars, bounded typed multivalue/attachment/stored-history collections and mixed read-only browsing pages, explicit stored scalar calculations and original attachment decoding work; remaining collection/query semantics, advanced types and native Access evidence remain |
| 2 | Constraint validation | Qualified complete native index checks (including ACE 16 Decimal tuples) and one-to-ten-column integer/Currency/GUID plus same-precision/scale ACE 16 Decimal FK tuple audits implemented alongside bounded field/pure-rule audits; broader FK/expression profiles, collations, other carriers and native Access evidence remain |
| 3 | Preservation | Complete images, unchanged-byte patch proofs and bounded direct catalog-child inspection implemented; owned children block edits; actual ACE/VBA application save/reopen and full dependency discovery remain unqualified |
| 4 | Native physical mutation | Fixed Int32 replacements and ordinary/unique existing-tree maintenance work on offline Jet 4 copies; ACE 16 previews extend this to integral tuples, NULL transitions, checked foreign indexes and explicit multi-column/table batches within existing pages. Bounded integral/Currency/GUID and same-precision/scale Decimal update cascades now expand original associations across chains/cycles. Existing-width Decimal slot/index previews are implemented; text/binary indexed mutation, broader relationship actions, delete actions, record resizing, allocation, splits/merges and INSERT/DELETE remain unimplemented |
| 5 | Native transactions and recovery | Optional validated image journal integration and local fault/reopen tests implemented; native Access/power-loss/network-filesystem qualification remains |
| 6 | Backend-neutral database contracts | Shared snapshots and consuming edits implemented with an Access consumer; connection/cursor/runtime APIs remain separate |
| 7 | Query completion for editing | Bounded single-table/column UPDATE planning and CLI copies implemented; broader expressions, collations, actions and result-set updateability remain explicit gaps |
| 8 | Additional backend/collaboration work | SQLite type/write policies beyond its current read-only source; native Access shared-file coordination and mixed-client recovery testing |
| 9 | Application runtime | Sessions, cursors, object lifecycle, bounded background work, cancellation and change notifications |
| 10 | Desktop UI | Accessible shell/navigation/ribbon using the selected [Selawik UI font](TECH_STACK.md#desktop-typography-decision), read-only datasheets and SQL editor, then qualified editing and visual builders |

Compatibility experiments and UI prototypes can progress alongside this work.
Jet 1/2 research and complete historical-format coverage do not gate the UI.
Native saves still require the relevant preservation/constraint/recovery evidence.

## Implementation priorities

| Order | Direction | Scope |
| --- | --- | --- |
| 1 | Native ACE 16 compatibility | Primary 100% target: reading, writing, indexes, transactions, unknown-object and existing VBA preservation; other Jet/ACE formats secondary |
| 2 | Microsoft Access collaboration | Native shared-file locking, concurrent writes, durability and recovery; actual mixed-client tests |
| 3 | SQLite and query engine | Tables, Access SQL, relationships, expression semantics and explicitly qualified translation |
| 4 | Custom Office-style desktop UI | Ribbon, navigation pane, datasheets and visual query builder; accessibility from the first custom controls |
| 5 | Forms and reports | Designers, binding, headless layout/rendering and native object interoperability |
| 6 | P2P / CRDT collaboration | Offline-first replication, conflict handling, deterministic merges and constrained relational apply |
| 7 | Broader tooling | CLI expansion, web frontend, plugins, MCP, migrations and diagnostics with real consumers |

The current CLI and diagnostics continue supporting engine work. Preserve useful
working tools while following this order. Feature contracts cover [advanced fields](features/ACCESS_ADVANCED_FIELDS.md),
[distribution/signatures](features/ACCESS_DISTRIBUTION.md), [startup/session state](features/ACCESS_APP_STARTUP.md),
[imported ribbons](features/ACCESS_RIBBON_COMPAT.md), [expression editing](features/EXPRESSION_BUILDER.md),
[linked/saved data tasks](features/IMPORT_EXPORT_TASKS.md), [analysis tools](features/DATABASE_ANALYSIS.md)
and [wizards](features/APP_TEMPLATES_WIZARDS.md). They are requirements, not completed features.

## Native format expansion

Retain Jet 4/ACE 12 as the current bounded read baseline while focusing new work
on ACE 16. Qualify its observed layouts and features first. ACE 14/15 and Jet 3
expansion are secondary and do not precede ACE 16 work merely because their
format generations are older.
Acquire original minimal fixtures with recorded provenance and compare catalog,
scalar, text and index behavior before enabling each profile. Access engine
releases do not map one-to-one to file layouts; the
[format target matrix](NATIVE_READING.md#compatibility-targets) records this
distinction and current limitations.

Jet 1.0/1.1 and Jet 2 are deferred optional read-only feasibility studies; they
must not divert implementation from ACE 16.
Establish fixtures, layout references and a comparison environment first; retain
them as deferred research if reliable extraction requires substantial work. This does
not gate the main engine, safe writes for qualified profiles or the UI.

Writes require separate preservation, constraints, macro, transaction and recovery
evidence for each profile. Encryption and advanced fields are qualified separately;
neither newer headers nor successful scalar reads imply support for every feature.

## VBA deferral

| Capability | Decision |
| --- | --- |
| VBA parser/interpreter, execution, new/edited module authoring, editor and debugger | Deferred to future standalone RustVBA work |
| DAO/ADO interfaces specifically for VBA | Deferred; backend-neutral data/cursor work remains useful independently |
| Windows API emulation, COM and Wine compatibility | Deferred |
| RustVBA integration | Deferred until a usable standalone project release exists |
| Preserving existing VBA modules/projects through unrelated edits | Required before qualifying native saves |
| Detecting VBA-dependent buttons, events, startup, ribbon and query features | Required; report unavailable actions and unresolved dependencies |
| Access table data macros | Retained database compatibility scope; implement required semantics or block affected writes |

[VBA_COMPATIBILITY.md](VBA_COMPATIBILITY.md) owns preservation/detection and deferred
integration. [WINDOWS_COMPATIBILITY.md](WINDOWS_COMPATIBILITY.md) retains provider
research only. Remove `tablecraft-vba`, `tablecraft-wincompat` and `tablecraft-wine`
from active implementation; reserve `tablecraft-vba-host` in docs only. Keep
`tablecraft-macros` and `tablecraft-automation` for Access-native macros, event dispatch
and non-VBA actions. No empty crates or speculative dependencies are added.

## Collaboration and browser boundaries

For TableCraft-native P2P projects, show invite authorization, peer/device identity,
revocation, connection/offline state, sync progress, concurrent values, resolution
history and backup/restore state. Define which changes peers acknowledged; a
disconnected peer cannot justify a global "all synchronized" claim. Preserve
conflicting values until explicit resolution. Resolving data conflicts does not
undo external actions. Collaborative design needs object/dependency merge rules.

Native Access shared-file mode shows connections, locks/contention and recovery.
It uses engine locking/transactions, not CRDT conflict semantics. P2P apply cannot
replay external effects or bypass required database validation.

Before browser delivery, specify supported file/replica workflows, permission
denial/revocation, persistence/quota/eviction, backup/export, offline edits and
reconnect/conflicts. Do not assume native locks, COM/DLL access, unrestricted
background tasks or durable browser storage. Web publishing is a later deployment
capability with an explicitly supported application subset.

## Scope limits

Keep Excel/Power Query reimplementation, a general spreadsheet editor, a full
Windows/Wine emulator, an unrelated new relational storage engine, a cloud database
service and a general website builder outside this critical path. The Rust-native
Jet/ACE implementation remains required. Arbitrary third-party ActiveX is long-term
research. AI assistance/agent authoring follows reliable CLI/MCP operations,
capability checks and reviewable changes; an MCP tool list cannot prove native
authoring works.

Adopt data adapters progressively under [import/export profiles](features/IMPORT_EXPORT_TASKS.md).
Historical formats, compiled projects and native signing retain separate fidelity
levels. Public progress claims must cite observed results and limitations.

## Accepted External Data direction

[External Data](features/EXTERNAL_DATA.md) is a planned first-class ribbon and
shared-service workflow: import, append, link, export, connection management and
saved data tasks. Built-in Access/SQLite integrations now live under `backends/`,
DB contracts under `foundation/`, and future external providers under `adapters/`.
Within that future workstream, native SQL Server comes first, then generic ODBC
and live SharePoint Lists; PostgreSQL/MySQL follow. The earlier SharePoint-clone
and CSV-only proposals are superseded. Direct Office file handling and explicit
system-default application opening avoid dependencies on installed Office apps.
Command search, full control/property catalogs, attachment conversion and
per-backend security remain explicit application requirements. These additions
do not reorder the ACE 16 milestones or enable new connectors.

### Currency conversion progress

CCur now has a library-backed explicit numeric conversion through the shared
expression/query layer, independently checked across Jet 4 and ACE 2007/2016/2019
read inputs. This advances read/query expression coverage; broader Decimal
arithmetic, Variant promotion, locale conversions and native Access qualification
remain required. See [conversion contracts](EXPRESSIONS.md#explicit-currency-conversion).

### Decimal expression envelope and CDec progress

The scalar Decimal carrier and optional Serde now cover 96-bit magnitude and
scale 0..=28. CDec reuses rust_decimal through the shared conversion layer;
field constraints and native codecs remain limited to precision 28. This enables
wider expression values without granting wider field storage. Decimal arithmetic,
aggregates, promotion and native floating-conversion qualification remain required.

### Checked Decimal arithmetic progress

Decimal operands now select library-backed checked +, -, *, /; numeric floating
operands follow the explicit normalized Decimal conversion profile. Exponentiation
returns Double. The shared query Sum/Avg fold now handles Decimal inputs, retaining
bounded intermediate/input-order behavior. Native field representation is unchanged.
Decimal/float comparison, remaining Currency operations, full Variant promotion
and native Access rounding/aggregate qualification remain required.

### Immediate sequencing

After checked Decimal arithmetic, prioritize native reader
reliability and remaining read/query gaps. Broader function-catalog expansion and
SQLite expression work follow those priorities. Private reference packs remain
research inputs; their planned scenarios are not implemented acceptance evidence.
