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
[Format numeric masks and sections](FORMAT.md) now share explicit numeric policies
and the existing Decimal/IEEE display magnitude. [Gregorian date/time masks](DATE_FORMAT_MASKS.md)
now share calendar conversion, names and week settings. [Text layouts](TEXT_FORMAT_MASKS.md)
now provide placeholders, sections and explicit contextual casing.
[Scientific/scaling masks](SCIENTIFIC_FORMATTING.md) now retain exact coefficients
and finite IEEE exponents. [Interleaved literals](FORMAT.md#interleaved-numeric-literals)
now work across whole/fractional/exponent slots. Broader date sections
and native semantics remain unfinished; a Partial function entry does not complete the read/query acceptance scope.
Numeric masks and General Number now also display legacy DATE serials directly,
without calendar rounding or stored-value changes; Extended numeric conversion
remains separate open work.
[Explicit DATE numeric conversions](DATE_NUMERIC.md) now also reuse destination
rounding/range kernels; broader Variant and native coercion remain acceptance gaps.
[Conditional selectors](CONDITIONAL_FUNCTIONS.md) now share DATE and explicit-policy
Text coercion while retaining eager semantic failures and immediate operational
failure. [Exact integer logical operators](INTEGER_LOGIC.md) now feed numeric read predicates
and preserve per-item versus parent collection scope. [Rounded float/fixed/date/Text operator coercion](INTEGER_LOGIC.md#rounded-numeric-and-text-operands)
now reuses checked Long/LongLong conversion and explicit numeric policies.
[Left/Right/Mid count coercion](SLICE_COUNTS.md) now adds rounded Long and explicit
SQL Text count arguments. [Repetition counts and search starts](TEXT_CONSTRUCTION.md#contextual-repetition-counts)
now share DATE/explicit numeric Text conversion while retaining function-specific
NULL/default rules. [Replace start/count coercion](TEXT_REPLACEMENT.md#contextual-start-and-count-conversion)
now also shares those kernels without source copies. [Comparison modes, character codes and raw-byte numeric slots](STRING_COMPARISON.md#contextual-comparison-mode-conversion)
now also share checked DATE and explicit-policy SQL Text conversion.
[StrConv flag/LCID options](STRCONV.md#contextual-flag-and-lcid-conversion) now
reuse their Integer/Long targets without widening locale/flag support.
[Named numeric/date formatting options](STYLED_NUMBER_FORMATTING.md#contextual-numeric-options)
now also accept explicit-policy SQL numeric Text with progressive validation.
[General Format/DatePart/DateDiff/Weekday week options](WEEK_SETTINGS.md#contextual-week-option-conversion)
now share DATE and explicit-policy SQL Text conversion.
[Named numeric formatters](STYLED_NUMBER_FORMATTING.md#legacy-date-source-values)
now also format raw legacy DATE source serials.
[Date components](EXPRESSIONS.md#date-component-source-coercion) now reuse bounded
CDate numeric/Text source conversion. [Date interval functions](DATE_INTERVALS.md)
now do likewise after their owning control validation. [Date/time constructors](DATE_CONSTRUCTORS.md#argument-coercion)
now share Integer conversion for raw DATE and SQL numeric Text components.
Other source coercions,
complete Variant/subtype inference and native qualification remain unfinished.

Native saved [UNION reconstruction](NATIVE_SAVED_UNIONS.md) now feeds the shared
read executor through explicit saved/direct/named retrieval. Native saved
[crosstabs](NATIVE_SAVED_CROSSTABS.md) now reconstruct and explicitly execute; their
positive stored TOP/PERCENT now use the shared final row limiter.
[Named fixed schemas](NAMED_CROSSTABS.md) now compose without binding scans,
including qualified policy-independent computed headings. [Execution-stage discovery](NAMED_SCHEMA_DISCOVERY.md)
now resolves qualified parameter/context/data-dependent relational sources without
binding scans. Parameterless runtime domain targets use the same operation-local
staging. Inline TRANSFORM and wider native comparison remain open. Zero TOP counts remain rejected rather than scheduled for execution.

[Numbered Error message lookup](ERROR_MESSAGES.md) now has explicit provider and
optional Fluent catalog support. Omitted runtime state remains deferred; reference
message data does not qualify native Access error-code/localization behavior.

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

Read/query status reviewed 22 October 2026 against current profile selectors,
query binding/execution and native read limits. Working read profiles are Jet 4,
ACE 2007/2010 and the ACE 2016/2019 layout families. Header recognition for other
families is separate from usable table reads. Basic CLI inventory, counts from
stored metadata, row pages, ordinary scalar queries and qualified saved SELECT
dependencies already work; they are not remaining scaffolding tasks.

### Current acceptance status

The [current acceptance checklist](READ_QUERY_ACCEPTANCE.md) separates the working
baseline, remaining priorities and prerequisites for broader writes.

Encrypted local-file reads are implemented for the qualified schemes in
[NATIVE_ENCRYPTION.md](NATIVE_ENCRYPTION.md): Jet 4 page encoding and bounded
Agile, Binary RC4, CryptoAPI RC4 and standard/compatibility AES Office profiles.
Global -p/hidden terminal prompting and fresh add/remove/change-password copies
are working. Whole-image management retains its 64 MiB ceiling and checked native
password/SID layout restrictions. This is separate from the 2 GiB read-source
envelope and does not qualify encrypted table edits or native Access save/reopen.
Do not treat encryption discovery/opening as an unstarted implementation task.

| Acceptance area | Working baseline | Remaining work |
| --- | --- | --- |
| Local discovery and reads | Header/catalog/table inventory, declared counts, bounded scalar row pages and qualified index reads in priority families | Unqualified variants, advanced-field semantics, stronger large-file/coherent-read coverage and native comparisons |
| Scalar/query semantics | Owned SQL, bounded SELECT/UNION/crosstabs, domain functions, explicit comparison/date/number policies and the built-ins recorded in the tracker | Full Variant/subtype/result promotion, Unicode/regional semantics, wider query shapes and native qualification; Partial entries do not establish completion |
| Named query composition | Local SELECT/UNION dependencies, fixed-heading crosstabs and explicit staged relational/domain schema discovery under shared scopes/budgets | Inline TRANSFORM, native heading naming and wider native definitions/semantics; binding remains metadata-only |
| Native text | Exact stored policy identities and explicit General legacy/2010 routing, index checks and bounded string/search/LIKE services | Other locales, complete native mixed-policy precedence and safe text pushdown; Windows capture/replay is still pending |
| Encoding/encryption | Qualified encoded opens and bounded fresh-copy management | Unknown/extensible/unsupported providers and native-authored profile qualification; workgroup authorization is a separate gap |
| Distribution containers | Header-based raw runtime/compiled reads, bounded single-database ACCDC CAB input and inert ACCDT OPC/URI plus qualified inline-XSD/List Definition reads | Wider ACCDT schemas/defaults/package features, signature verification and native compiled/package qualification |
| Scale and host behavior | Incremental native scalar cursors/batches, explicit CLI TSV continuation/projections, bounded query materialization, explicit source/row/work/output limits and stable/offline inputs | Streaming/larger SQL execution, coherent shared-file reads and linked-source providers |

The execution order below remains the acceptance checklist. The next query work
must address composition and value semantics as well as individual functions.
The explicit execution-stage service now owns qualified data/context-dependent
relational and parameterless runtime-domain discovery, not binding. Extend remaining
query shapes without hiding scans in metadata binding. Unknown native text identities must remain
visible/refused where required, and no new coercion may silently turn unsupported
stored fields or expression Error states into NULL.

Any additional encryption codec must identify and validate its descriptor before
credential use, keep cipher primitives in maintained libraries and retain bounded
key-derivation/page work, cancellation and payload-free diagnostics. Prove known
answers, independent encrypted/plaintext pages, source-bound table/query results,
wrong/missing-password handling and unchanged originals. Keep actual Access-generated
files, native save/reopen and mixed-client write qualification as separate gates.
Jet encoding/password fields are not workgroup authentication. The owning encryption
contract retains the primary references, algorithms, provenance and exact limits.

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
   [CStr](CSTR.md) now covers Text/Boolean, explicit standard Error/Empty and
   numeric/date output profiles;
   full regional, Variant and native precision/rounding behavior remain open.
   [IsDate](ISDATE.md) now shares explicit Gregorian recognition and year context;
   broader regional/Variant behavior and native qualification remain open.
   [IsNumeric](ISNUMERIC.md) now recognizes current scalar types and whole numeric
   Text under explicit profiles; native/complete locale behavior remains open.
   [Explicit numeric Text formats](NUMBER_FORMAT.md) now extend the nine numeric
   conversion calls; complete regional behavior and native qualification remain open.
   [Numeric Text arithmetic](ARITHMETIC_TEXT.md) now routes mixed addition, other
   numeric operators and unary signs through that service. Exact Decimal and direct
   lexical integer conversion retain their boundaries; full Variant/declared-type
   result promotion and native operator/error semantics remain required. Contextual
   [`&` concatenation](CONCATENATION.md) now reuses explicit numeric/date CStr
   policies across scalar, bound and native queries; wider Variant/byte-array
   concatenation and native qualification remain open.
   [Conservative result metadata](QUERY_RESULT_TYPES.md) now retains declared
   field/parameter descriptors, constraints and nullability through qualified
   projections, joins, named/derived reads, unions and crosstabs without source
   scans. Stable successful arithmetic carriers now flow through derived outputs,
   aggregates, crosstabs and UNION widening; value-dependent arithmetic and broader
   inference remain Dynamic.
   [Exact floating means](testing/FLOATING_MEANS.md) now avoid intermediate
   Double overflow and preserve cancellation residuals under bounded scratch;
   [wider exact Decimal totals](AVERAGES.md) also retain mixed-scale residuals;
   mixed conversion order and native precision qualification remain.
   [Materialized +, - and * promotion](ARITHMETIC_PROMOTION.md) now retains qualified
   Byte/Integer/Long/Single results and widening, checked Currency multiplication
   and Decimal/DATE selection. Unary minus, division, power and rounded integer
   division/Mod now use their qualified materialized carrier rules. Declared-expression
   typing, decimal literal inference, full Variant behavior and native qualification remain open.
   [Radix literals](RADIX_LITERALS.md) now evaluate existing &H/&O syntax with
   explicit signed Integer/Long carriers; native SQL typing remains unqualified.
   [Numeric predicates](NUMERIC_PREDICATES.md) now supply explicit-policy numeric/Text
   and checked fixed/floating comparisons. Strict set/group keys and declared/Variant
   inference remain separate. Legacy DATE/Text now uses explicit CDate policies;
   extended temporal/Text coercion and native qualification remain gaps.
   [Hex/Oct/Str](NUMERIC_TEXT.md) now provide checked numeric spelling;
   native widths, floating notation and broader coercions remain open.
   [Val](VAL.md) now supports bounded numeric prefixes with checked suffixes;
   native subtypes and coercion/error qualification remain open.
   [Chr/Asc/ChrW/AscW](CHARACTER_FUNCTIONS.md) now use checked byte/UTF-16 profiles;
   native coercion, DBCS and lossless surrogate results remain gaps.
   [Pmt/PV/FV](FINANCIAL_FUNCTIONS.md) now share bounded Double annuity kernels
   across scalar, bound, domain and native saved/named queries. Remaining financial
   functions and complete native coercion/precision behavior remain open.
   [NPer/IPmt/PPmt](PERIOD_PAYMENTS.md) now provide bounded period inversion and
   payment components with independent scalar and saved-query ledger controls.
   [SLN/SYD/DDB](DEPRECIATION.md) now provide scalar depreciation and independent
   schedule controls, with native edge/rounding qualification still open.
   [StringFromGUID](GUID_CONVERSIONS.md#stringfromguid-inverse-conversion) now
   converts stored Guid fields and sixteen GUID-structure bytes to provisional
   literal Text; native output/coercion/query qualification remain open.
   [Eval](DYNAMIC_EVALUATION.md) now evaluates closed dynamic expressions under
   shared context, depth and budgets; dynamic host/UDF/form names and broader
   regional/Variant/native behavior remain open.
   [CurrentUser](SESSION_FUNCTIONS.md) now consumes explicit immutable Access
   session data; the CLI declares unsecured Admin. Workgroup authentication/ACLs
   and native session/host qualification remain separate open work.
   [CVErr](VARIANT_ERRORS.md) now constructs Error data for inspection, eager
   selection and explicit numeric consumers. Expression-valued query results and
   derived rows now preserve these cells. First/Last and DFirst/DLast/DLookup
   endpoint selection retain Error data. Count/DCount now consume Error nullness
   without code conversion. Arithmetic/logical operators reject Error with
   TypeMismatch after eager operand evaluation; qualified Error/Error predicates
   compare standard codes. Standard Error-only Min/Max/domain extrema now retain
   Error data under a provisional profile; numeric aggregates reject implicit code
   conversion. Broader Variant/operator/aggregate behavior remains
   required read/query work. Error is not stored
   NULL or an operational exception.
   [Rnd](RANDOM_FUNCTIONS.md) now uses caller-owned sequential state and explicit
   CLI seeds; native sequence/optimizer and volatile-key qualification remain open.
   [EuroConvert](EURO_CONVERSION.md) now adds fixed-rate Double conversion, target
   rounding and explicit significant-digit triangulation; later participant codes
   and native precision/coercion/default qualification remain open.
   [Rate](RATE.md) now adds bounded guess-directed solving, residual qualification
   and per-iteration metering; native basins/root selection remain open.
   Built-ins beyond implemented NULL/conditional
   (including eager Switch/Choose selection)
   and text/date-component/DateAdd/DateDiff/DatePart/DateSerial/TimeSerial, numeric Abs/Sgn/Int/Fix/Round and Double-valued Sqr/Exp/Log/Sin/Cos/Tan/Atn,
   plus checked integer/Boolean and explicit IEEE
   conversion calls (including further text, date arithmetic beyond the explicit legacy DATE +/-
   serial profile, and conversions),
   full Variant/Unicode behavior, broader exact Decimal/Currency arithmetic and
   result promotion remain absent. Computed pure grouping expressions and
   First/Last encounter aggregates now work with bounded structural membership
   checks. [Statistical aggregates](STATISTICAL_AGGREGATES.md) now provide bounded
   Var/VarP/StDev/StDevP with finite numeric conversion, centered/scaled moments
   and documented per-function minimum counts; native precision remains open. Top-level fixed/dynamic crosstab execution now works under explicit
   heading/resource policies; native saved crosstabs now reconstruct into this top-level
   executor. Positive stored TOP/PERCENT and qualified named fixed-heading schemas
   now execute; qualified relational schemas and parameterless runtime domain targets
   now have explicit execution-stage discovery. Inline TRANSFORM and wider native naming remain gaps. Zero TOP counts remain rejected rather than being
   scheduled as SQL Server-style empty-result behavior. Bounded scalar-table DISTINCTROW now compares whole selected
   records, retaining hidden fields. Complete logical derived/named rows now work,
   including UNION, grouped and fixed/discovered crosstab results. Scalar/predicate
   subquery projections now select record owners by scoped outer-field captures and
   retain per-joined-row evaluation. Field-free projections/counts/group outputs
   now follow the ignored DISTINCTROW path with all joined inputs. Active grouping
   in the enclosing query and
   native recordset/owner/optimizer comparisons remain unqualified. [Grouped subqueries](GROUPED_SUBQUERIES.md) now validate key-safe captures across
   nested SELECT/HAVING/predicate reads, UNIONs, join conditions and lowered parent
   membership. Wider aggregate ownership/native semantics remain unqualified.
   [Projected DISTINCT ordering](QUERY_ORDERING.md) now reuses qualified matching
   scalar/aggregate outputs with their text policies. Broader sort/alias expressions,
   read-plan identity and wider native saved-query reconstruction remain incomplete. Native saved SELECT/crosstab
   sources now support bounded sixteen-source join forests, disconnected comma
   products and inner predicate placement; [join scope](NATIVE_SAVED_JOINS.md)
   separates independent row-model evidence from pending native Access qualification.
   [Stored derived SELECT/UNION sources](NATIVE_SAVED_DERIVED.md) now reconstruct
   and execute through the existing non-lateral binder, including outer parameters
   and local named dependencies; remote qualifiers and native qualification remain.
   [Already-bracketed stored identifiers](NATIVE_SAVED_NAMES.md) now work across
   sources, aliases and literal parameters with raw endpoint matching and normalized
   duplicate checks. [Literal punctuated prompts](PARAMETER_PROMPTS.md) now parse,
   bind and reconstruct in native declarations, including Unicode prompts, nested reads,
   UNION/crosstabs and bracketed CLI assignments. Names use ASCII-insensitive,
   otherwise exact matching; qualified control bindings and native Unicode/NLS
   identity and regional rules remain gaps.
   [Native fragment ordering](NATIVE_SAVED_ORDERING.md) now supplies a stable keyed
   view to SELECT/UNION/crosstab reconstruction while preserving physical metadata
   and the reference whole-definition NULL policy; native Access evidence remains.
   Mixed numeric UNION outputs now select shared carriers from metadata across
   Boolean/integral/fixed/floating families. Fixed-point widening is exact;
   Double-result rounding and duplicate collapse are explicit. Text/numeric UNION
   outputs now use execution-local CStr formatting, preserving original Text/NULL
   and field comparison identities. Native precedence/formatting and broader
   non-numeric/Variant behavior remain open.
   [UNION result widening](UNION_TYPES.md) now establishes exact integer-width and
   Single/Double carriers before set/order operations, plus shared Text/Decimal
   descriptors without row scans. Broader mixed-family/native coercion and result
   precedence remain unfinished; this does not implement a full Variant matrix.
   [Unicode saved source labels](NATIVE_SAVED_JOINS.md#unicode-source-labels) now
   compose in named/derived/join forests using the existing ASCII-insensitive,
   otherwise exact policy. Native Unicode/NLS equivalence remains an open identity
   qualification task rather than being inferred from these independent controls.
   See [expression scope](EXPRESSIONS.md) and [query scope](QUERY_EXECUTION.md).
   The twelve read-only [domain functions](DOMAIN_FUNCTIONS.md) now use selected
   sources and shared budgets. Stored-query direct/saved/named controls now cover
   optional criteria, explicit parameters, grouping, UNION and crosstabs across
   the four priority header families. Broader criteria/collection/host semantics and
   native Variant/error-order qualification remain unfinished.
2. Complete and qualify native text behavior. Ordinal and plain LIKE modes remain
   separate from the explicit provisional native General providers below; they
   do not substitute for database locale/case/accent or character-unit semantics. The [Windows ACE capture recipe](testing/NATIVE_TEXT_ORACLE.md)
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
   complete native mixed-policy precedence, broader locales and safe
   SQL text-predicate pushdown remain next. Explicit native `--collation database`
   now selects stored General default/per-field policies through one shared
   comparison quota. Unknown required identities and expression conflicts
   fail without fallback; projection remains inert. Opted-in stored
   collection Text descriptors now route through the same resolver with
   metadata/snapshot checks; decoded Binary payloads carry no text identity.
   Query preparation now retains opaque scalar/collection text-policy identities, validates their declarations and checks stability through ordinary/
   collection/parent scans. Native snapshots retain exact descriptor identities.
   Bound field/output slots now carry static provenance through aliases, selected
   string-preserving expressions, aggregates, unions and fixed crosstabs, with
   conflict visibility and no row scans. Explicit host resolver routing now works
   across predicates, tuple operations, sets, sorting/TOP, aggregates and crosstabs.
   Native provider/CLI integration now works with independent mixed-column controls
   across Jet 4/ACE 2007/ACE 16-family files. Engine collection declaration/snapshot
   APIs now retain exact flat value/member descriptors without record scans.
   Collection policy identity/routing now works through query binding, native
   snapshots and the CLI. Other locales, native precedence and safe text pushdown
   remain unfinished. Explicit InStr LCIDs now route to a distinct supplied
   host search callback with exact alternate-sort identity and checked spans;
   concrete locale providers remain unfinished and are not supplied by global
   General/ordinal comparers.
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
   Unprojected HAVING/sort member inputs now drive bounded aligned child expansion;
   independent stored-row models cover grouping, ordering and same-child WHERE rules.
   Native FileData SQL representation, history member SQL and complete native
   semantics/qualification remain. ACE 16 calculated
   fields have explicit
   cached reads, not expression evaluation or freshness guarantees. Jet 4/ACE 12
   are not required to implement field types absent from those formats. See
   [advanced fields](features/ACCESS_ADVANCED_FIELDS.md).
4. Broaden native storage variants and scale. Deleted fixed/variable histories
   now have an identity/count-based read profile and independent derivative checks;
   ambiguous histories and other unqualified row/schema variants, codecs beyond
   the [qualified encryption/password profiles](NATIVE_ENCRYPTION.md), and
   Jet workgroup/replication-related variants need implementation/evidence.
   Native read sources now have a 2 GiB envelope, separately from 64 MiB complete
   images. Map/page, snapshot, payload and buffered output ceilings still restrict
   real-world coverage. Incremental page-address traversal and source-bound scalar
   cursors, including explicit ordinary scalar subsets of mixed schemas, now
   support bounded batches without earlier-root rescans. Sequential query snapshots
   materialize through these batches with cumulative controls and complete caches.
   Native CLI queries now offer an explicit cumulative byte allowance while
   preserving separate row/work/execution/output limits and the default budget.
   [CLI scalar row continuation](STREAMING_ROWS.md) now uses one source-bound cursor,
   explicit scalar projections and cumulative read/output controls.
   SQL execution remains buffered; [explicit larger scalar snapshots](LARGER_NATIVE_QUERIES.md)
   now preserve complete caches and independent native/query controls with 20,000-row
   comparisons. Explicit indexed loads now have independent 20,000-row comparisons across
   qualified key families, projections, typed seeks and full-cache isolation.
   Larger collection loads, streaming SQL and broader size/locality
   qualification remain work, with existing row/byte/output guards retained.
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
now work through explicitly selected indexes. The engine also has
[exact GUID/NULL point selection](NATIVE_INDEXES.md#exact-guid-point-selections);
backend-neutral GUID equality/NULL predicate integration now uses it for explicit
matching single-GUID indexes, including mixed-schema scalar projections. Broader typed
predicate pushdown and automatic index choice remain open. Schema discovery is separate from
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


The required ACCDT reader now has [bounded package inspection](ACCESS_TEMPLATES.md):
stored/deflated OPC snapshots, checked content types/internal relationships and
basic CLI reporting. Bounded generic-Object inline XSD data now supplies table browsing and query
integration. Qualified [List Definition agreement](ACCESS_TEMPLATE_LISTS.md) now checks stored
SOAP declarations before table reads. Broader defaults/types, wider XSD/native field
fidelity and Access-created qualification remain required; this does not complete
template read/query acceptance. See [stored-data scope](ACCESS_TEMPLATE_DATA.md).
