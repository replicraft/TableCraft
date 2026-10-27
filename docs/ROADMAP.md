# Compatibility-driven roadmap

## Current product priority

As of 26 October 2026, the active goal is faithful Microsoft Access behavior on
native files, with ACE 16 primary. SQLite and `.tcraft`/`.tcraftproj/` implementation,
native-project Python, conversion and modernization are paused until explicitly
resumed. They do not gate the backend-first path to the UI. Existing functionality
and future architecture plans are retained. The
[behavioral-parity decision](decisions/0003_ACCESS_BEHAVIORAL_PARITY.md) governs
shared services and future UI workflows, defaults, restrictions and quirks.

## Delivery snapshot — 27 October 2026

The numbered milestones below are broad epics, rather than individual tasks.
Recent delivery has primarily advanced local reads, query composition and scale;
it has not completed the general writer or application runtime. This checklist
makes delivered subtasks visible without reducing the full milestone scope.
Each item retains the limits in its linked contract.

Delivered read/query and engineering foundations:

- [x] Local catalog/table browsing, system-table filtering, stored counts and
  scalar row pages through the [CLI](cli/CLI_BROWSING.md).
- [x] Bounded [buffered SQL retrieval](query/QUERY_EXECUTION.md), including joins,
  grouping/HAVING, UNION, qualified crosstabs and saved/named dependencies.
- [x] Explicitly authorized [linked](access/queries/NATIVE_LINKED_QUERIES.md) and
  [external native query sources](access/queries/NATIVE_EXTERNAL_QUERIES.md).
- [x] Qualified [encryption opens and fresh-copy password management](access/storage/NATIVE_ENCRYPTION.md).
- [x] [Progressive rows and queries](cli/STREAMING_QUERIES.md): eligible SELECT,
  five global aggregates, Text/exact-key grouping, terminal HAVING, flat UNION ALL
  and static named/derived chains with eligible row-local or aggregate/HAVING
  stages. Text membership uses complete policies.
- [x] [Cursor/read failure controls](access/storage/NATIVE_READ_FAILURES.md), cumulative limits,
  cancellation and [high-offset Memo/OLE checks](access/storage/NATIVE_HIGH_OFFSET_PAYLOADS.md).
- [x] Explicit [ACE 16 Memo/OLE storage checks](access/storage/LONG_VALUE_CHECKS.md)
  for qualified column maps, live descriptors, owned pages, orphan counts and
  sharing. Unknown profiles remain visible; this does not authorize writes.
- [x] Offline Jet 4 fixed Int32 UPDATE copies and separate
  [ACE 16 engineering write previews](testing/ACE16_WRITE_PREVIEWS.md).
  [Deletion candidates](testing/ACE16_DELETE_PREVIEWS.md) retain original key/row
  boundaries, including qualified single Short Text/Binary keys. Existing-page
  contraction retires empty branches through checked owned/free maps and repairs
  complete internal chains by height. A frozen committed tool now passes all
  [32 deep/indirect original-file controls](testing/results/ace16-delete-internal-chains-originals-2026-10-26.json)
  without changing the earlier selectors, requests or budgets. Required splits
  still refuse. A separate [existing-width Text/Binary UPDATE preview](testing/ACE16_WRITE_PREVIEWS.md#existing-width-short-text-and-binary-slots)
  now performs actual slot and single/mixed tuple key replacements, with full
  constraints and no record growth or ordinary save capability.
  A separate [same-page Text/Binary service](testing/ACE16_PAGE_UPDATES.md) now
  fits records within retained capacity or appends hidden forwarding targets
  while preserving original logical roots. General allocation, index splitting and
  ordinary saves remain unfinished.
  Its [separate original-file replay](testing/PAGE_UPDATE_OBSERVATIONS.md) now
  passes 180 controls: 156 candidates and 24 typed refusals, with complete
  record/index/map and protected-byte proofs.
  A further [variable-update service](testing/ACE16_VARIABLE_UPDATES.md) now adds
  alternate owned destinations and EOF data-page allocation through existing map
  representation, with independent placement/map replay. Map growth, arbitrary
  free-page reuse and production saves remain unfinished. Its
  [eight original-file candidates](testing/VARIABLE_UPDATE_OBSERVATIONS.md) now
  pass complete byte/value/index proofs, separately from the earlier PAGE results.
  A further [eight Binary allocation candidates](testing/BINARY_VARIABLE_UPDATE_OBSERVATIONS.md)
  pass on independently constructed space-exhaustion sources with exact retained
  target padding, maps and original primary references.
  A separate [Int32 root-leaf split service](testing/ACE16_INDEX_SPLITS.md) now
  grows qualified overflowing single-field root leaves into a retained root and
  two represented EOF leaves, with request-derived row authority and independent
  map/framing audits. Non-root leaves now split with a new left page, a retained
  right page and repaired neighbors/ancestor bounds when every parent fits.
  Qualified internal-parent splitting and retained-root growth now propagate
  recursively under independent count/capacity/EOF/framing audits and 108 scoped
  runtime checks. Its [four recursive original-file candidates](testing/RECURSIVE_INDEX_SPLIT_UPDATE_OBSERVATIONS.md)
  now pass first-run complete-image/getter/map proofs across retained-root growth
  and below-root internal splits. Other-codec splits, normal saves and native
  Access qualification remain separate. Separate
  [Byte/Int16/Int64/Currency sources](testing/INTEGRAL_INDEX_SPLIT_SOURCE_OBSERVATIONS.md)
  now pass measured root-leaf overflow gates and eight byte-identical typed no-ops
  in both ACE 16 formats; that source phase alone does not establish typed split admission.
  Its [two unchanged-original candidates](testing/ROOT_SPLIT_UPDATE_OBSERVATIONS.md)
  now pass their first complete-image, typed-value and index/reference proofs
  through a frozen committed consumer.
  Its [two non-root original-file candidates](testing/NONROOT_SPLIT_UPDATE_OBSERVATIONS.md)
  also pass first-attempt complete-image proofs, allocating one index page and
  retaining the source's unchanged Memo/OLE storage.
  Separate [sparse Int32 sources](testing/PARENT_INDEX_SPLIT_SOURCE_OBSERVATIONS.md)
  now establish measured leaf-and-parent overflow with only 782 live target rows,
  plus a sentinel, and pass byte-identical no-op admission. Their larger generation
  histories are not qualified mutation inputs. The subsequent
  [nine-process public-reference phase](testing/PARENT_INDEX_SPLIT_OBSERVATIONS.md)
  now measures actual retained-root growth to height two and three EOF index
  pages on both sources; the prior frozen kernel refuses those two edits
  atomically. Recursive candidate acceptance now passes separately on the new committed consumer.
  A distinct [below-root internal-parent source](testing/INTERNAL_PARENT_INDEX_SPLIT_SOURCE_OBSERVATIONS.md)
  now retains 963 live target rows under a height-two root after bounded public
  pruning, with all 497 index pages and three sibling levels preserved. Its
  source phase contains no MAX update or native candidate qualification.
  Its [separate public MAX actions](testing/INTERNAL_PARENT_INDEX_SPLIT_OBSERVATIONS.md)
  now establish new-left/retained-right internal splitting on both originals,
  with a retained height-two root and two EOF pages. The old consumer still
  refuses those edits; new-kernel candidate acceptance is recorded separately above.
  The [exact typed facade](testing/ACE16_FIXED_INDEX_SPLITS.md) now implements
  Byte/Int16/Int32/Int64/Currency recursive splits with request-derived final
  authority and a fresh-output engineering consumer; 124 distinct scoped runtime
  functions and strict/minimal/docs checks pass. The subsequent
  [repeated-overflow extension](testing/ACE16_REPEATED_INDEX_SPLITS.md) now splits
  oversized halves with independent capacity/topology audits, passing 139 distinct
  scoped functions, including fifteen new controls. Other codecs, general
  allocation and normal saves remain unfinished.
  Its [eight first original-file candidates](testing/FIXED_INDEX_SPLIT_UPDATE_OBSERVATIONS.md)
  now match independent complete images and fresh getter/index/map checks on the
  unchanged sources, through a committed frozen consumer and sixteen successful
  fixture commands. Recursive typed originals and native Access remain separate.
  A [repeated-overflow witness](testing/REPEATED_INDEX_SPLIT_OBSERVATIONS.md)
  now passes nine source commands and thirteen separate reference commands:
  four facade no-ops preserve complete originals, four prior MAX attempts refuse
  atomically and four public updates split the remaining oversized halves. Their
  six-EOF-page topology is measured. The subsequent
  [four first-run candidates](testing/REPEATED_INDEX_SPLIT_UPDATE_OBSERVATIONS.md)
  now match their frozen complete images and fresh typed/index/map inspections
  through a new committed consumer. All eight commands pass without changing the
  originals, selectors, expectations or budgets. Map growth and native saves
  remain separate work.
  Their [separate MAX reference actions](testing/INTEGRAL_INDEX_SPLIT_OBSERVATIONS.md)
  now record eight successful public updates and eight old-consumer atomic
  refusals on unchanged originals. All sixteen physical roots split; new typed
  candidate acceptance and native Access qualification remain separate.
  A separate [ordinary Memo/OLE replacement service](testing/ACE16_LONG_VALUE_UPDATES.md)
  now composes qualified column-owned LVAL append/EOF allocation with parent
  descriptor forwarding and independently replays request/map/payload authority.
  Old chunks remain allocated. Broader field/map profiles, reclamation and
  production saves remain unfinished. Its
  [revised committed-consumer replay](testing/LONG_VALUE_UPDATE_ACCEPTANCE_OBSERVATIONS.md)
  now passes all 60 unchanged raw Memo/OLE controls with complete image/getter/map
  proofs, following the preserved initial zero-of-60 census refusals and narrow
  ownership/deleted-slot correction. Exact ordinary compressed Memo now shares
  this preview under its measured narrow/raw encoding rule and independent byte
  audit, with 68 scoped runtime checks. Its
  [first original-file replay](testing/COMPRESSED_MEMO_ACCEPTANCE_OBSERVATIONS.md)
  now passes 132 exact candidate images/getters and sixteen typed atomic refusals
  under unchanged inputs/budgets. Other extended/history/map profiles, broader
  encoding, reclamation and normal saves stay open.
  A subsequent [map-authority correction](testing/LONG_VALUE_MAP_AUTHORITY.md)
  now rejects empty cross-owner map aliases and overlapping metadata footprints
  before allocation, with immutable-original final recensus. Its 85 scoped runtime
  functions pass; 44 read-only source-admission probes are distinct from candidate
  replay. Indirect candidate maps and production saves remain open.
  A [separate indirect-map source phase](testing/INDIRECT_LONG_VALUE_MAP_OBSERVATIONS.md)
  now records mixed indirect-owned/inline-candidate Memo maps on both originals,
  with thirteen successful fixture commands and complete saved storage proofs.
  No map admission, growth or replacement qualification follows from that phase.
  The subsequent [mixed-map implementation](testing/results/ace16-indirect-lval-native-2026-10-27.json)
  now admits qualified indirect-owned/inline-candidate pairs in the default
  storage checker and engineering replacement, with 112 distinct runtime checks.
  A working read-only checker matches all four reports on the unchanged originals.
  Its [twelve committed-consumer candidates](testing/MIXED_LONG_VALUE_UPDATE_OBSERVATIONS.md)
  now match complete independently frozen images and fresh getter/maps on both
  unchanged originals, through twenty-five successful first-attempt fixture commands.
  Candidate-indirect maps, metadata growth and production saves remain separate work.
  A separate [48-control Text/Binary corpus](testing/ENCODED_DELETE_OBSERVATIONS.md)
  checks 36 candidates and twelve unsupported-profile refusals on the same
  committed tool, including the previously refused height-two delete-all cases.
  Qualified ignore-NULL deletion now derives checked per-index request subsets
  and physical empty-root ownership under 224 scoped engine checks. Its
  [fifty original-file candidates](testing/IGNORED_DELETE_OBSERVATIONS.md) all pass,
  including physically empty indexes with live NULL table rows and unchanged
  index bytes for zero-effective requests.
  The separate acyclic Cascade service now passes
  [24 original-file candidates and eight self/cycle refusals](testing/CASCADE_DELETE_OBSERVATIONS.md),
  retaining independent association, index-entry and unchanged-byte checks.
  General ACE 16 saves are still unavailable.

- [x] [Retained-key split transport](testing/ACE16_RETAINED_SPLIT_PLANNER.md)
  supports real profile-owned keys internally while preserving numeric public
  consumers. Independent fast-path framing now enforces fresh/original prefix
  policy. Its 115 runtime functions and scoped checks pass. The subsequent
  [existing-fit composite audit](testing/ACE16_COMPOSITE_FITTING_AUDIT.md) separately
  derives exact tuple wire bytes and original topology under 62 scoped runtime
  functions, including four new controls. Text/Binary split mutation, composite
  allocation and normal native saves remain separate work.

Separate [Text/Binary source observations](testing/ENCODED_INDEX_SPLIT_SOURCE_OBSERVATIONS.md)
now establish measured root overflow and repeated demand for both General text
families and Short Binary on two unchanged 2,701-row sources. The corrected five
source commands pass after an earlier compile failure. The subsequent
[old-slot baseline](testing/ENCODED_INDEX_SPLIT_BASELINES.md) passes six
byte-identical no-ops and six exact IndexedColumn refusals with no output. The
subsequent [public references](testing/ENCODED_INDEX_SPLIT_UPDATE_OBSERVATIONS.md)
pass six replacements and six getters, each retaining row identities while adding
six index pages. Committed-consumer candidate comparisons remain separate.

- [x] A separate [exact-slot Text/Binary split preview](testing/ACE16_ENCODED_INDEX_SPLITS.md)
  now supplies a working General legacy/modern and Binary consumer of retained
  keys, with independent complete-image, request-authority, alias and map checks.
  It preserves field widths and original row locations. Original-file candidate
  replay, tuple allocation, broader profiles and normal saves remain unfinished.

- [x] Qualified [index-owned map promotion](testing/ACE16_INDEX_MAP_GROWTH.md)
  now composes same-extent pointer-row rewrites and distinct bitmap metadata with
  recursive/repeated splits. Independent images cover late promotion and mixed
  represented peers. Its 92 distinct runtime functions and scoped build/lint/doc
  checks pass. Its [first two-original replay](testing/INDEX_MAP_GROWTH_UPDATE_OBSERVATIONS.md)
  now matches both independently frozen complete images and fresh row/index/map
  inspections through four successful commands. General allocation, other map
  roles/windows, normal saves and native Access qualification remain unfinished.

- [x] Shared [native map authority](testing/ACE16_MAP_AUTHORITY.md) now serves the
  existing long-value engineering consumer, with explicit canonical mutable
  owners and complete catalog/current/historical footprint checks. Its 43 scoped
  runtime checks and 44 separate unchanged-original admissions pass. Missing
  physical index ownership coverage refuses; actual map growth remains open.

- [x] Independent [usage-map growth observations](testing/USAGE_MAP_GROWTH_OBSERVATIONS.md)
  distinguish selected index children from newly allocated bitmap metadata and
  preserve the original map extent, root identity and unrelated bytes. The
  eighteen-command history explicitly retains one failed reference action with
  separately validated partial output. Engine promotion, generic allocation and
  production saves remain unfinished.

- [x] [Stateful streaming composition](query/PROGRESSIVE_QUERIES.md#stateful-retained-stages)
  for static saved/named/derived SELECT chains, with retained intermediate values,
  independent TOP/HAVING phases and successful logical EOF before outer totals.
  Inner UNION, sorting, joins and runtime schema discovery remain outside streaming.

Major gates still open before the full backend is complete:

- [ ] Remaining [read/query acceptance](testing/READ_QUERY_ACCEPTANCE.md), including full
  Variant/result promotion, native text semantics and wider native definitions.
- [ ] Broader ACE 16 validation/preservation and required data-macro safety.
- [ ] General physical mutation: record growth/allocation, index splits/merges,
  general INSERT/DELETE and wider relationship/write-query actions. Deletion
  candidates now support qualified existing-page contraction and variable updates
  support represented EOF data-page allocation. General allocation/map growth
  and production saves remain unfinished.
- [ ] Production transaction/recovery qualification, native shared-file
  coordination and mixed-client behavior.
- [ ] Remaining Access backend contracts, application session/cursor lifecycle,
  object lifecycle, background work and change notifications. SQLite and native
  projects remain outside this active scope while paused.

Windows Access comparisons are deferred by the user. Their evidence status is
separate from implemented engineering functionality; independent fixtures do not
establish native conformance. The detailed milestone and acceptance tables below
remain authoritative and are not replaced by this delivery snapshot.

Priority decision updated 26 October 2026. The native Jet/ACE engine and faithful
Access interoperability lead implementation. The eventual application builder
must preserve Access workflows; UI implementation follows the backend-first goal.
Compatibility research can progress alongside storage. [IMPLEMENTATION.md](IMPLEMENTATION.md) records
working behavior: bounded native metadata/scalar/indexed reads, owned Access SQL,
read-only retrieval, exact values, generic I/O and a headless CLI. Read-only [SQLite snapshots](backends/SQLITE.md) feed the same query evaluator. Bounded
offline Jet 4 edits now work; native shared-file coordination, forms/reports and UI
hosts remain unimplemented.

LCase/UCase now have [explicit Unicode casing](expressions/text/TEXT_CASING.md) in local read/query
execution. Windows locale/NLS comparison remains an acceptance gap.
[Format numeric masks and sections](expressions/formatting/FORMAT.md) now share explicit numeric policies
and the existing Decimal/IEEE display magnitude. [Gregorian date/time masks](expressions/formatting/DATE_FORMAT_MASKS.md)
now share calendar conversion, names and week settings. [Text layouts](expressions/formatting/TEXT_FORMAT_MASKS.md)
now provide placeholders, sections and explicit contextual casing.
[Scientific/scaling masks](expressions/formatting/SCIENTIFIC_FORMATTING.md) now retain exact coefficients
and finite IEEE exponents. [Interleaved literals](expressions/formatting/FORMAT.md#interleaved-numeric-literals)
now work across whole/fractional/exponent slots. Broader date sections
and native semantics remain unfinished; a Partial function entry does not complete the read/query acceptance scope.
Numeric masks and General Number now also display legacy DATE serials directly,
without calendar rounding or stored-value changes; Extended numeric conversion
remains separate open work.
[Explicit DATE numeric conversions](expressions/dates/DATE_NUMERIC.md) now also reuse destination
rounding/range kernels; broader Variant and native coercion remain acceptance gaps.
[Conditional selectors](expressions/CONDITIONAL_FUNCTIONS.md) now share DATE and explicit-policy
Text coercion while retaining eager semantic failures and immediate operational
failure. [Exact integer logical operators](expressions/numeric/INTEGER_LOGIC.md) now feed numeric read predicates
and preserve per-item versus parent collection scope. [Rounded float/fixed/date/Text operator coercion](expressions/numeric/INTEGER_LOGIC.md#rounded-numeric-and-text-operands)
now reuses checked Long/LongLong conversion and explicit numeric policies.
[Left/Right/Mid count coercion](expressions/text/SLICE_COUNTS.md) now adds rounded Long and explicit
SQL Text count arguments. [Repetition counts and search starts](expressions/text/TEXT_CONSTRUCTION.md#contextual-repetition-counts)
now share DATE/explicit numeric Text conversion while retaining function-specific
NULL/default rules. [Replace start/count coercion](expressions/text/TEXT_REPLACEMENT.md#contextual-start-and-count-conversion)
now also shares those kernels without source copies. [Comparison modes, character codes and raw-byte numeric slots](expressions/text/STRING_COMPARISON.md#contextual-comparison-mode-conversion)
now also share checked DATE and explicit-policy SQL Text conversion.
[StrConv flag/LCID options](expressions/text/STRCONV.md#contextual-flag-and-lcid-conversion) now
reuse their Integer/Long targets. [Standalone Japanese kana](expressions/text/STRCONV.md#explicit-japanese-kana-conversion)
now adds basic fullwidth script conversion with exact LCID 1041.
[Japanese width](expressions/text/STRCONV.md#explicit-japanese-width-conversion) adds standalone 4/8
with bounded ASCII/kana mappings and voiced composition. Combined settings,
broader width/kana and native NLS qualification remain open.
[Named numeric/date formatting options](expressions/formatting/STYLED_NUMBER_FORMATTING.md#contextual-numeric-options)
now also accept explicit-policy SQL numeric Text with progressive validation.
[General Format/DatePart/DateDiff/Weekday week options](expressions/dates/WEEK_SETTINGS.md#contextual-week-option-conversion)
now share DATE and explicit-policy SQL Text conversion.
[Named numeric formatters](expressions/formatting/STYLED_NUMBER_FORMATTING.md#legacy-date-source-values)
now also format raw legacy DATE source serials.
[Date components](expressions/EXPRESSIONS.md#date-component-source-coercion) now reuse bounded
CDate numeric/Text source conversion. [Date interval functions](expressions/dates/DATE_INTERVALS.md)
now do likewise after their owning control validation. [Date/time constructors](expressions/dates/DATE_CONSTRUCTORS.md#argument-coercion)
now share Integer conversion for raw DATE and SQL numeric Text components.
Other source coercions,
complete Variant/subtype inference and native qualification remain unfinished.

Native saved [UNION reconstruction](access/queries/NATIVE_SAVED_UNIONS.md) now feeds the shared
read executor through explicit saved/direct/named retrieval. Native saved
[crosstabs](access/queries/NATIVE_SAVED_CROSSTABS.md) now reconstruct and explicitly execute; their
positive stored TOP/PERCENT now use the shared final row limiter.
[Named fixed schemas](query/NAMED_CROSSTABS.md) now compose without binding scans,
including qualified policy-independent computed headings. [Execution-stage discovery](query/NAMED_SCHEMA_DISCOVERY.md)
now resolves qualified parameter/context/data-dependent relational sources without
binding scans. Parameterless runtime domain targets use the same operation-local
staging. Wider native comparison remains open. Proposed inline TRANSFORM sources
require [native syntax evidence](query/NAMED_CROSSTABS.md#inline-transform-is-not-an-established-access-requirement) before implementation. Zero TOP counts remain rejected rather than scheduled for execution.

[Numbered Error message lookup](expressions/ERROR_MESSAGES.md) now has explicit provider and
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

## Deferred native project direction

[The retained project decision](decisions/0001_PROJECT_FORMAT_AND_EXECUTION.md)
records future native SQLite-backed `.tcraft`/`.tcraftproj/` with
Python/visual actions, and native MDB/ACCDB interoperability with Access objects,
macros and preserved VBA. Python supersedes the earlier TypeScript-default idea.
The native-project implementation is paused by the
[current priority](decisions/0003_ACCESS_BEHAVIORAL_PARITY.md). It must not change
Access-compatible behavior or introduce generalized project requirements into
the active backend. Retained contracts are future plans, not implemented features.

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

The current native read/query priorities below remain in force. Resuming native
projects requires explicit user reprioritization. The retained future plan is to
prove a minimal project
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
now works through AccessInput, including explicit host-owned storage for larger
qualified databases with preserved small-capture defaults. Bounded ACCDT package inspection, UTF-8/BOM-qualified UTF-16 XML, qualified
inline-XSD table reads and CLI queries also work; wider template schemas and
native package qualification remain pending. Reading them does not depend on VBA
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

[Jet 4 qualification](access/profiles/NATIVE_JET4.md) adds exact legacy Decimal indexes/tuples,
relationship checks and preservation around unrelated offline Int32 edits, plus
saved-SELECT comparisons. ACE-only advanced fields are not Jet 4 parity targets.
The user clarified on 23 October 2026 that the entire remaining local-file
read/query checklist, including storage reliability, comes before further write
expansion. Complete value/text semantics, query composition, applicable advanced
fields and storage/scale work in the order below; existing write previews remain
available but do not change that implementation priority. Other legacy expansion
and broad SQLite work remain paused. Windows testing will be performed later by
the user. Independent read/query implementation and fixture work continue now;
do not label native Access conformance or mixed-client safety established without
actual evidence. Required write-safety and native qualification gates remain
separate from independent read/query checks.

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
relationship-action coverage for those additional carriers remains unimplemented.
The separate deletion preview now admits qualified cross-table enforced RESTRICT
relationships, retaining complete before/after membership and exact missing-parent
failures. Self relationships and other delete actions remain refused. Broader
delete actions, remaining indexed carriers and production saves remain explicit
blockers. Reciprocal pointer/role/action
inspection, relationship-catalog agreement and qualified physical reads/checks
are implemented. Bounded final-child nodes now have authored ACE preview regression
coverage; independent/native qualification and broader indexed carrier mutation
remain. Write work also requires positive
data-macro evidence/absence detection and broader
physical mutation, allocation and recovery; these are prerequisites to a writable
frontend, not features supplied by the preview.

Native [larger payload reads](access/storage/LARGE_PAYLOADS.md) now have separate encoded,
decoded Memo, attachment-file, query and output policies. Independent inputs
include 20 MiB OLE, 3 MiB Memo and 3 MiB incompressible attachments.
[Explicit decoded Memo/query Text ceilings](access/storage/LARGER_MEMOS.md) reach 16 MiB without
raising defaults or cumulative limits; rendered output retains its own ceiling.

## Remaining local-file read/query work

The explicit [arithmetic declaration profile](expressions/numeric/DECLARED_ARITHMETIC.md) now carries
Scalar/Variant provenance through integral/Single-selected query execution with
`--declared-arithmetic` (`--declared-integral` remains an alias).
Known Scalar String/Variant predicates now select metered String coercion under
the same explicit profile; unresolved categories fail when they change dispatch.
Default arithmetic/comparisons remain unchanged. Static inference for unknown outputs,
wider carrier rules and native SQL qualification remain P0 acceptance work.

Read/query status reviewed 23 October 2026 against current profile selectors,
query binding/execution and native read limits. Working read profiles are Jet 4,
ACE 2007/2010 and the ACE 2016/2019 layout families. Header recognition for other
families is separate from usable table reads. Basic CLI inventory, counts from
stored metadata, row pages, ordinary scalar queries and qualified saved SELECT
dependencies already work; they are not remaining scaffolding tasks.

### Current acceptance status

The [current acceptance checklist](testing/READ_QUERY_ACCEPTANCE.md) separates the working
baseline, remaining priorities and prerequisites for broader writes.

Encrypted local-file reads are implemented for the qualified schemes in
[NATIVE_ENCRYPTION.md](access/storage/NATIVE_ENCRYPTION.md): Jet 4 page encoding and bounded
Agile, Binary RC4, CryptoAPI RC4 and standard/compatibility AES Office profiles.
Global -p/hidden terminal prompting and fresh add/remove/change-password copies
are working. Page-buffered management now avoids the whole-image ceiling under
checked native password/SID layouts and cumulative allowances; the image API
retains its 64 MiB cap. Page copies share the 2 GiB read-source
envelope; neither API qualifies encrypted table edits or native Access save/reopen.
Do not treat encryption discovery/opening as an unstarted implementation task.

| Acceptance area | Working baseline | Remaining work |
| --- | --- | --- |
| Local discovery and reads | Header/catalog/table inventory, declared counts, bounded scalar row pages and qualified index reads in priority families | Unqualified variants, advanced-field semantics, stronger large-file/coherent-read coverage and native comparisons |
| Scalar/query semantics | Owned SQL, bounded SELECT/UNION/crosstabs, domain functions, explicit comparison/date/number policies and the built-ins recorded in the tracker | Full Variant/subtype/result promotion, Unicode/regional semantics, wider query shapes and native qualification; Partial entries do not establish completion |
| Named query composition | Local SELECT/UNION dependencies, fixed-heading crosstabs and explicit staged relational/domain schema discovery under shared scopes/budgets | Native heading naming and wider native definitions/semantics; qualify proposed inline TRANSFORM syntax before implementation; binding remains metadata-only |
| Native text | Exact stored policy identities, runtime domain-result provenance, explicit General legacy/2010 routing, index checks and bounded string/search/LIKE services; exact French legacy ordinary Short Text physical index reads/checks and a separate provisional full-string query comparison policy | French matching/Text/LCID policies, broader stored orders and native comparison qualification, complete native mixed-policy precedence and safe text pushdown; Windows capture/replay is still pending |
| Encoding/encryption | Qualified encoded opens and bounded fresh-copy management | Unknown/extensible/unsupported providers and native-authored profile qualification; workgroup authorization is a separate gap |
| Distribution containers | Header-based raw runtime/compiled reads, bounded single-database ACCDC CAB input and inert UTF-8/UTF-16 ACCDT OPC/URI plus qualified inline-XSD/List Definition reads | Wider ACCDT schemas/defaults/package features, signature verification and native compiled/package qualification |
| Scale and host behavior | Incremental native scalar cursors/batches, explicit CLI TSV continuation/projections, bounded query materialization, explicit source/row/work/output limits, stable/offline inputs, explicitly authorized one-hop linked native sources and exact host-bound external IN/per-table targets with scoped saved reads and explicit progressive local scalar SELECT/WHERE, global Count/Sum/Avg/Min/Max, Text/integral/Currency/Decimal grouping with terminal HAVING, flat UNION ALL and static named/derived chains, explicit one-hop linked/external/recomputed streams and bounded exact integral GROUP/DISTINCT/UNION keys | Wider streaming/larger SQL execution, coherent shared-file reads, remaining stored remote layouts and broader providers |

Explicit linked streams retain the same target authorization, independent text
policies and cumulative budgets as buffered linked reads. Microsoft directs links
to original local tables rather than already linked tables; the current one-hop
refusal is a documented [workflow boundary](access/queries/NATIVE_LINKED_QUERIES.md), not an
unestablished chain-parity milestone. Broader providers remain separate work.
[Exact integral query keys](query/keys/EXACT_QUERY_KEYS.md) now avoid representative scans for
proven GROUP/DISTINCT/UNION tuples, and [borrowed equijoins](query/QUERY_EQUIJOINS.md)
reject admitted nonmatching pairs before copying candidates. Both remain bounded
strategies with explicit general fallbacks, not complete optimizer qualification.

[Continuation failure checks](access/storage/NATIVE_READ_FAILURES.md) now cover read/seek/page
hooks after a delivered prefix, terminal cursors and bounded interrupted short
reads across the priority layouts. The shared page reader observes cancellation
before interpreting decoded buffers. These checks do not establish coherent
live-file reads or native Access qualification.

[Retained String keys](query/keys/RAW_STRING_KEYS.md) now cover grouping, DISTINCT/active
DISTINCTROW, UNION duplicate removal, sorting and TOP ties under explicit
comparison policies. Fixed-heading named crosstabs also retain raw cells for
outer observers; actual pivot/heading names keep their separate publication
contract. Min/Max and domain extrema now also retain
[lossless Strings](query/aggregates/EXPRESSION_AGGREGATES.md#retained-string-extrema) under strict
key comparison and scalar publication rules. Other raw consumers, broader Variant coercion,
additional locales and native qualification remain in the read/query workstream.

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

1. Complete Access scalar/query semantics. The [built-in tracker](expressions/ACCESS_BUILTINS.md)
   records individual implementation status and remaining gaps; explicit
   [StrComp modes](expressions/text/STRING_COMPARISON.md) and [InStr/InStrRev search](expressions/text/STRING_SEARCH.md#scalar-search-operands)
   and [Replace](expressions/text/TEXT_REPLACEMENT.md#scalar-string-operands) now have bounded implementations.
   StrComp, substring search, replacement, slicing, trimming, casing, Asc/AscW,
   reversal and Val reuse explicit CStr policies for ordinary scalar string operands;
   wider coercion/native qualification remain open. These work alongside
   [Space/String/StrReverse](expressions/text/TEXT_CONSTRUCTION.md) with explicit current Unicode limits
   and [selected single-byte character pages](expressions/text/CHARACTER_CODE_PAGES.md).
   [CDate/DateValue/TimeValue](expressions/dates/DATE_CONVERSIONS.md) now use explicit Gregorian
   text/year controls and checked component extraction; native/full regional gaps remain.
   [Date/Time/Now](expressions/dates/CLOCK_FUNCTIONS.md) now have explicit clock providers and CLI
   local/fixed hosts; native precision/platform/volatility qualification remains open.
   [FormatNumber/Percent/Currency](expressions/formatting/STYLED_NUMBER_FORMATTING.md) now provide bounded
   explicit-profile output; full NLS/Variant/native formatting remains open.
   [Standalone Weekday](expressions/dates/WEEKDAY.md) now shares date conversion and week policy,
   with native/full coercion qualification still open.
   [Host week settings](expressions/dates/WEEK_SETTINGS.md) now resolve DatePart/DateDiff system
   options while preserving defaults; native NLS and w anchors remain open.
   [MonthName/WeekdayName](expressions/dates/CALENDAR_NAMES.md) now support explicit names and
   host-issued first-day context; broader calendars/native NLS remain open.
   [FormatDateTime](expressions/formatting/DATE_FORMATTING.md) now provides five named modes under
   explicit profiles; broader regional/native formatting remains open.
   [CStr](expressions/text/CSTR.md) now covers Text/Boolean, explicit standard Error/Empty and
   numeric/date output profiles;
   full regional, Variant and native precision/rounding behavior remain open.
   [IsDate](expressions/dates/ISDATE.md) now shares explicit Gregorian recognition and year context;
   broader regional/Variant behavior and native qualification remain open.
   [IsNumeric](expressions/numeric/ISNUMERIC.md) now recognizes current scalar types and whole numeric
   Text under explicit profiles; native/complete locale behavior remains open.
   [Explicit numeric Text formats](expressions/formatting/NUMBER_FORMAT.md) now extend the nine numeric
   conversion calls; complete regional behavior and native qualification remain open.
   [Numeric Text arithmetic](expressions/numeric/ARITHMETIC_TEXT.md) now routes mixed addition, other
   numeric operators and unary signs through that service. Exact Decimal and direct
   lexical integer conversion retain their boundaries; full Variant/declared-type
   result promotion and native operator/error semantics remain required. Contextual
   [`&` concatenation](expressions/text/CONCATENATION.md) now reuses explicit numeric/date CStr
   policies across scalar, bound and native queries; wider Variant/byte-array
   concatenation and native qualification remain open.
   [Conservative result metadata](query/QUERY_RESULT_TYPES.md) now retains declared
   field/parameter descriptors, constraints and nullability through qualified
   projections, joins, named/derived reads, unions and crosstabs without source
   scans. Stable successful arithmetic carriers now flow through derived outputs,
   aggregates, crosstabs and UNION widening; value-dependent arithmetic and broader
   inference remain Dynamic.
   [Exact floating means](testing/FLOATING_MEANS.md) now avoid intermediate
   Double overflow and preserve cancellation residuals under bounded scratch;
   [wider exact Decimal totals](query/aggregates/AVERAGES.md) also retain mixed-scale residuals;
   mixed conversion order and native precision qualification remain.
   [Materialized +, - and * promotion](expressions/numeric/ARITHMETIC_PROMOTION.md) now retains qualified
   Byte/Integer/Long/Single results and widening, checked Currency multiplication
   and Decimal/DATE selection. Unary minus, division, power and rounded integer
   division/Mod now use their qualified materialized carrier rules. Declared-expression
   typing, full Variant behavior and native qualification remain open. Explicit
   preparation now captures decimal literal carriers; native literal typing remains
   unqualified.
   The [declared query profile](expressions/numeric/DECLARED_ARITHMETIC.md) now propagates known
   Scalar/Variant provenance through fields, parameters, conversions, retained
   cells, named/derived/correlated reads and predicates. Qualified integral,
   Single and Date overflow, mixed String/numeric addition and comparisons, and
   Decimal division overrides are integrated. Paired result metadata preserves
   default versus declared execution. Category-dependent unknown aggregate/UNION
   outputs remain refused rather than inferred; full native typing/promotion
   and qualification remain open.
   [Radix literals](expressions/numeric/RADIX_LITERALS.md) now evaluate existing &H/&O syntax with
   explicit signed Integer/Long carriers; native SQL typing remains unqualified.
   [Numeric predicates](expressions/numeric/NUMERIC_PREDICATES.md) now supply explicit-policy numeric/Text
   and checked fixed/floating comparisons. Strict set/group keys and declared/Variant
   inference remain separate. Legacy DATE/Text now uses explicit CDate policies;
   extended temporal/Text coercion and native qualification remain gaps.
   [Hex/Oct/Str](expressions/numeric/NUMERIC_TEXT.md) now provide checked numeric spelling;
   native widths, floating notation and broader coercions remain open.
   [Val](expressions/numeric/VAL.md) now supports bounded numeric prefixes with checked suffixes;
   native subtypes and coercion/error qualification remain open.
   [Chr/Asc/ChrW/AscW](expressions/text/CHARACTER_FUNCTIONS.md) now use checked byte/UTF-16 profiles;
   native coercion, DBCS and lossless surrogate results remain gaps.
   [Pmt/PV/FV](expressions/financial/FINANCIAL_FUNCTIONS.md) now share bounded Double annuity kernels
   across scalar, bound, domain and native saved/named queries. The remaining
   admitted scalar financial calls and [NPV/IRR/MIRR](expressions/financial/CASHFLOW_FUNCTIONS.md) also
   execute under their explicit numeric/cashflow profiles. Full arrays and native
   coercion/precision behavior remain open.
   [NPer/IPmt/PPmt](expressions/financial/PERIOD_PAYMENTS.md) now provide bounded period inversion and
   payment components with independent scalar and saved-query ledger controls.
   [SLN/SYD/DDB](expressions/financial/DEPRECIATION.md) now provide scalar depreciation and independent
   schedule controls, with native edge/rounding qualification still open.
   [StringFromGUID](expressions/GUID_CONVERSIONS.md#stringfromguid-inverse-conversion) now
   converts stored Guid fields and sixteen GUID-structure bytes to provisional
   literal Text; native output/coercion/query qualification remain open.
   [Eval](expressions/DYNAMIC_EVALUATION.md) now evaluates closed dynamic expressions under
   shared context, depth and budgets; dynamic host/UDF/form names and broader
   regional/Variant/native behavior remain open.
   [CurrentUser](expressions/SESSION_FUNCTIONS.md) now consumes explicit immutable Access
   session data; the CLI declares unsecured Admin. Workgroup authentication/ACLs
   and native session/host qualification remain separate open work.
   [CVErr](expressions/VARIANT_ERRORS.md) now constructs Error data for inspection, eager
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
   [Rnd](expressions/numeric/RANDOM_FUNCTIONS.md) now uses caller-owned sequential state and explicit
   CLI seeds; native sequence/optimizer and volatile-key qualification remain open.
   [EuroConvert](expressions/financial/EURO_CONVERSION.md) now adds fixed-rate Double conversion, target
   rounding and explicit significant-digit triangulation; later participant codes
   and native precision/coercion/default qualification remain open.
   [Rate](expressions/financial/RATE.md) now adds bounded guess-directed solving, residual qualification
   and per-iteration metering; native basins/root selection remain open.
   The [built-in tracker](expressions/ACCESS_BUILTINS.md) is authoritative for admitted
   function coverage; its Partial rows retain function-specific omissions rather
   than implying that implemented text/date/financial calls are absent. Remaining
   scalar work includes complete Variant/result promotion, wider Unicode/regional
   behavior and native operator/coercion qualification. Error/Empty/Missing
   admission now follows reviewed function-specific control/NULL/source stages;
   this does not complete the Variant matrix. Computed pure grouping expressions and
   First/Last encounter aggregates now work with bounded structural membership
   checks. [Statistical aggregates](query/aggregates/STATISTICAL_AGGREGATES.md) now provide bounded
   Var/VarP/StDev/StDevP with finite numeric conversion, centered/scaled moments
   and documented per-function minimum counts; native precision remains open. Top-level fixed/dynamic crosstab execution now works under explicit
   heading/resource policies; native saved crosstabs now reconstruct into this top-level
   executor. Positive stored TOP/PERCENT and qualified named fixed-heading schemas
   now execute; qualified relational schemas and parameterless runtime domain targets
   now have explicit execution-stage discovery. Wider native naming remains a gap;
   proposed inline TRANSFORM sources need native syntax evidence before expansion. Zero TOP counts remain rejected rather than being
   scheduled as SQL Server-style empty-result behavior. Bounded scalar-table DISTINCTROW now compares whole selected
   records, retaining hidden fields. Complete logical derived/named rows now work,
   including UNION, grouped and fixed/discovered crosstab results. Scalar/predicate
   subquery projections now select record owners by scoped outer-field captures and
   retain per-joined-row evaluation. Field-free projections/counts/group outputs
   now follow the ignored DISTINCTROW path with all joined inputs. Active grouping
   in the enclosing query and
   native recordset/owner/optimizer comparisons remain unqualified. [Grouped subqueries](query/GROUPED_SUBQUERIES.md) now validate key-safe captures across
   nested SELECT/HAVING/predicate reads, UNIONs, join conditions and lowered parent
   membership. Wider aggregate ownership/native semantics remain unqualified.
   [Projected DISTINCT ordering](query/QUERY_ORDERING.md) now reuses qualified matching
   scalar/aggregate outputs with their text policies. Broader sort/alias expressions,
   read-plan identity and wider native saved-query reconstruction remain incomplete. Native saved SELECT/crosstab
   sources now support bounded sixteen-source join forests, disconnected comma
   products and inner predicate placement; [join scope](access/queries/NATIVE_SAVED_JOINS.md)
   separates independent row-model evidence from pending native Access qualification.
   [Stored derived SELECT/UNION sources](access/queries/NATIVE_SAVED_DERIVED.md) now reconstruct
   and execute through the existing non-lateral binder, including outer parameters
   and local named dependencies; remote qualifiers and native qualification remain.
   [Already-bracketed stored identifiers](access/queries/NATIVE_SAVED_NAMES.md) now work across
   sources, aliases and literal parameters with raw endpoint matching and normalized
   duplicate checks. [Literal punctuated prompts](cli/PARAMETER_PROMPTS.md) now parse,
   bind and reconstruct in native declarations, including Unicode prompts, nested reads,
   UNION/crosstabs and bracketed CLI assignments. Names use ASCII-insensitive,
   otherwise exact matching; qualified control bindings and native Unicode/NLS
   identity and regional rules remain gaps.
   [Native fragment ordering](access/queries/NATIVE_SAVED_ORDERING.md) now supplies a stable keyed
   view to SELECT/UNION/crosstab reconstruction while preserving physical metadata
   and the reference whole-definition NULL policy; native Access evidence remains.
   Mixed numeric UNION outputs now select shared carriers from metadata across
   Boolean/integral/fixed/floating families. Fixed-point widening is exact;
   Double-result rounding and duplicate collapse are explicit. Text/numeric UNION
   outputs now use execution-local CStr formatting, preserving original Text/NULL
   and field comparison identities. Native precedence/formatting and broader
   non-numeric/Variant behavior remain open.
   [UNION result widening](query/UNION_TYPES.md) now establishes exact integer-width and
   Single/Double carriers before set/order operations, plus shared Text/Decimal
   descriptors without row scans. Broader mixed-family/native coercion and result
   precedence remain unfinished; this does not implement a full Variant matrix.
   [Unicode saved source labels](access/queries/NATIVE_SAVED_JOINS.md#unicode-source-labels) now
   compose in named/derived/join forests using the existing ASCII-insensitive,
   otherwise exact policy. Native Unicode/NLS equivalence remains an open identity
   qualification task rather than being inferred from these independent controls.
   See [expression scope](expressions/EXPRESSIONS.md) and [query scope](query/QUERY_EXECUTION.md).
   The twelve read-only [domain functions](query/DOMAIN_FUNCTIONS.md) now use selected
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
   SQL policy is enabled by it. [Exact native sort descriptors](access/indexes/NATIVE_TEXT.md) now
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
   the opt-in [ICU host text provider](expressions/text/ICU_TEXT_PROFILE.md) already implements
   Unicode substring search across nineteen explicit locale profiles, with one
   exact execution-local LCID registration through `--text-locale`/`--text-lcid`.
   Its expanded independent ICU4C replay records 14,498 controls; this is ICU
   profile evidence, not Windows NLS or native ACE equivalence. General/ordinal
   comparers do not infer LCID search, and automatic LCID mapping, additional
   profiles, native mixed-policy precedence and NLS/ACE qualification remain open.
   Windows comparisons are deferred at the user's direction; continue
   implementation using reviewed references and independent checks, retaining
   native qualification as a separate pending gate.
   Explicit engine [Short Text equality candidates](access/indexes/NATIVE_TEXT.md#conservative-short-text-equality-candidates)
   now provide conservative primary-prefix pruning under the General model.
   They retain possible false positives and require complete residual equality;
   [direct query equality](access/indexes/TEXT_EQUALITY_SEEKS.md) now uses comparer/source agreement,
   complete residual evaluation and independent query/benchmark evidence.
   Wider predicate and locale qualification remain unfinished.
   Single Short Binary index reads/checks now work;
   typed Binary ranges also work for explicit library hosts. Compound Binary
   reads now share the owned tuple reader and whole-entry digest checks. Automatic
   non-NULL binary predicate pushdown, compound seeks and SQL text inequality bounds remain
   unsupported. NULL-only General Text/Short Binary predicates now use qualified
   explicit or automatic indexes without invoking a string comparator; see
   [NULL selection scope](access/indexes/AUTOMATIC_INDEXES.md#null-only-text-and-binary-selection).
   Text-bearing compound indexes now support mixed two-to-ten-column reads and
   projections, including per-column General descriptors and whole-entry digests;
   Memo/full-width uniqueness and native text writes remain unqualified. Exact Currency and [Byte/Int16 intervals](access/indexes/NARROW_RANGES.md) now join Int32/Int64 ranges under typed bounds and
   original-filter retention. Opt-in [automatic selection](access/indexes/AUTOMATIC_INDEXES.md)
   now chooses matching qualified single-key indexes. Additional typed/composite
   seeks and cost-based selection remain performance work.
3. Complete applicable ACE advanced-field reads/query access. Stored multivalues,
   attachments and Memo history have qualified browsing APIs. Backend-neutral
   scalar multivalue `.Value` expansion now works over explicit host collection
   views and explicit native `--multivalues` input. Direct scalar-parent membership
   criteria now work. Explicit typed results now also preserve complete bare-field
   lists through row-local/static named/derived projections, scalar ordering/TOP
   and direct hidden-member criteria. INNER/CROSS parent joins now transport whole
   lists through nested/static named/derived sources and hidden criteria. Typed
   LEFT/RIGHT results now retain explicit parent absence separately from empty
   lists and NULL children. Scalar `.Value` aggregation and per-parent Count
   already work; [Microsoft's counting reference](https://support.microsoft.com/en-us/access/count-data-by-using-a-query)
   excludes direct complex-list Count, and its multivalue guide prohibits bare
   list grouping. Mixed whole-list/displayed-member SELECTs now reuse aligned
   scalar expansion while retaining complete list sidecars, including through
   joins and static named/derived sources. The typed service now also expands
   retained named/derived list members, preserving actual element policies and
   binding origins. Repeated aliases align; independent relation instances remain
   independent. All 150 original-getter replay controls pass; ordinary scalar and
   progressive APIs keep their retained-list refusal. Whole-list UNION admission
   and native collection semantics still need qualification; do not invent
   unsupported native list operations merely to clear a generic aggregate/key checklist.
   Explicit `--collections` now
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
   [Attachment presence criteria](query/collections/COLLECTION_PRESENCE.md) now distinguish empty
   containers from nullable metadata without reading FileData. Native FileData
   SQL representation and complete native semantics/qualification remain.
   Explicit [stored-history query members](access/collections/STORED_HISTORY_QUERIES.md) now use
   actual hidden-field declarations and aligned stored records under --collections,
   with complete caches and independent three-profile evidence. Native history SQL
   and latest-version behavior remain separately unqualified. ACE 16 calculated
   fields have explicit
   cached reads and [selected recomputed query views](access/calculations/RECOMPUTED_CALCULATION_VIEWS.md)
   using the existing same-row syntax/dependency service. Natural carriers remain
   separate from stored ResultType conversion and native freshness qualification. Jet 4/ACE 12
   are not required to implement field types absent from those formats. See
   [advanced fields](features/ACCESS_ADVANCED_FIELDS.md).
4. Broaden native storage variants and scale. Deleted fixed/variable histories
   now have an identity/count-based read profile and independent derivative checks.
   Known fixed prefixes of ambiguous deleted tails now read without inventing their
   storage classes; required visible variable tails retain checks. Unobserved
   deleted payloads, broader row/type histories, other unqualified schema variants, codecs beyond
   the [qualified encryption/password profiles](access/storage/NATIVE_ENCRYPTION.md), and
   Jet workgroup/replication-related variants need implementation/evidence.
   Native read sources now have a 2 GiB envelope, separately from 64 MiB complete
   images. Map/page, snapshot, payload and buffered output ceilings still restrict
   real-world coverage. Incremental page-address traversal and source-bound scalar
   cursors, including explicit ordinary scalar subsets of mixed schemas, now
   support bounded batches without earlier-root rescans. Sequential query snapshots
   materialize through these batches with cumulative controls and complete caches.
   Native CLI queries now offer an explicit cumulative byte allowance while
   preserving separate row/work/execution/output limits and the default budget.
   [CLI scalar row continuation](cli/STREAMING_ROWS.md) now uses one source-bound cursor,
   explicit scalar projections and cumulative read/output controls.
   Ordinary SQL execution remains buffered; explicit
   [progressive scalar SELECT/WHERE and flat UNION ALL](cli/STREAMING_QUERIES.md) now uses fresh native
   cursors without complete-table retention. [Explicit larger scalar snapshots](access/queries/LARGER_NATIVE_QUERIES.md)
   now preserve complete caches and independent native/query controls with 20,000-row
   comparisons. Explicit indexed loads now have independent 20,000-row comparisons across
   qualified key families, projections, typed seeks and full-cache isolation.
   Qualified partial index reads now also support table data ownership maps above
   4,096 pages without materializing all addresses; independent ACE 16-family
   originals contain 5,000 rows on 5,000 owned pages. Explicit row-capped complete
   indexed snapshots now use the same ownership service with mandatory complete
   counts and row/key validation. Legacy complete/full-check and index-owned/
   mutation map ceilings remain unfinished scale work. See
   [large-map indexed reads](testing/LARGE_INDEX_SELECTIONS.md).
   A separate 100,000-insert history recipe now qualifies 99,110 retained
   sequential rows, added-column NULLs, Memo growth and deleted roots across
   ACE 16/ACE 2007/Jet 4. Explicit query byte/row allowances preserve defaults
   while keeping native/query/output budgets separate.
   Explicit complete collection-table limits now share batched parent/child
   traversal, with ACE 16/ACE 2007 multivalue controls at 20,000/400 parents and
   48,000/40,000 children. A separate corpus now qualifies 20,000 attachments
   and 20,000 stored versions per ACE 16/ACE 2007 input, including exact decoded
   bytes/metadata and stored DATE bits. Decoded columns reuse a bounded inflater
   workspace without increasing default budgets. Buffered collection browsing
   now exposes explicit read ceilings with independently checked offsets, empty
   pages and atomic `--all` output. Broader histories, wider streaming
   SQL shapes and size/locality qualification remain work, with existing
   row/byte/output guards retained.
5. Complete read-only host behavior where needed. Stable/offline inputs are the
   working contract. Native CLI inputs now hold shared OS locks against cooperating
   exclusive writers; captured package inputs can release locks after capture.
   This does not implement native Access shared-file coordination. Coherent reads while native Access is modifying a shared
   file require compatible snapshot/coordination behavior. [Linked-table definitions](access/schema/NATIVE_LINKED_TABLES.md)
   now expose inert target/provider fields without opening them. The Access
   adapter's [shared native quotas](access/queries/NATIVE_SOURCE_BUDGETS.md) can bound explicitly
   supplied sources together. [Explicit one-hop routing](access/queries/NATIVE_LINKED_QUERIES.md)
   now validates authorized handles and target metadata, forwarding root linked
   aliases to local target tables. CLI `--link ALIAS PATH` selects explicit targets;
   providers, chains and live cross-file coherence remain separate work.
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
reports are outside this read/query checklist and remain in the broader Access
roadmap. SQLite/native projects are paused, rather than remaining active backend
milestones. This checklist does not mark unfinished Access work completed or waived.

## Stored GUID inspection

[Fixed GUID Byte-array views](expressions/VALUE_INSPECTION.md#stored-guid-inspection-and-fixed-bounds)
now supply type/subtype/array inspection and bounds across scalar and bound
queries without rewriting stored values or materializing bytes. Native subtype,
bound and error-precedence qualification remains separate; full P0 remains open.

## GUID-structure predicate coercion

[GUID/byte-structure equality](expressions/GUID_CONVERSIONS.md#guid-structure-predicate-equality)
now lets filters compare stored GUIDs against the existing GUIDFromString byte
result. Ordinary and declared predicate consumers share the fixed-size rule;
strict keys, storage and native index hints retain their existing contracts.
Broader array semantics, native comparison qualification and full P0 remain open.

## Successful unknown-category arithmetic composition

Declared-profile arithmetic with unknown provenance now has
[successful selected-carrier descriptors](query/QUERY_RESULT_TYPES.md#successful-arithmetic-with-unknown-declaration-provenance),
using its existing runtime agreement/refusal rule. Outer UNION consumers can
apply common-carrier conversions without inventing aggregate/UNION declarations.
Full Variant promotion, native declaration qualification and remaining P0 work
remain open.

## Explicit deeper compound queries

[Query binding depth](query/QUERY_BINDING_LIMITS.md) can now be selected explicitly up
to 32, retaining default 16 and independent node, allocation, parser and named-path
ceilings. Prepared plans capture the limit through runtime domains, policy walks
and progressive admission; staged discovery shares the selection and work budget.
Broader native definitions and complete P0 value/text semantics remain open.

## Next implementation and measurement

[ACE 16 bounded reads](access/profiles/NATIVE_ACE16.md) now have original V2016/V2019 Jackcess
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

Native [field properties](access/schema/NATIVE_PROPERTIES.md) and [relationship declarations](access/schema/NATIVE_RELATIONSHIPS.md)
now have bounded readers and independent fixtures. Bounded field/rule and Int32
relationship audits are implemented. Complete-image retention, [database contracts](foundation/DB.md),
[journaled fixed Int32 edits](access/writes/NATIVE_WRITES.md) and [query UPDATE copies](access/writes/QUERY_EDITING.md)
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
Native [Byte/Int16](access/indexes/NARROW_RANGES.md), Int32/Int64 and [Currency seeks](access/indexes/CURRENCY_RANGES.md), plus [safe predicate pushdown](query/QUERY_EXECUTION.md#safe-source-constraints)
now work through explicitly selected indexes. Qualified [legacy DATE constraints](access/indexes/DATE_INDEX_CANDIDATES.md)
now retain calendar-safe candidates through explicit or opt-in automatic indexes;
the original four-format 20,000-row replay passes 192 controls without changing
snapshot defaults or residual filtering. The engine also has
[exact GUID/NULL point selection](access/indexes/NATIVE_INDEXES.md#exact-guid-point-selections);
backend-neutral GUID equality/NULL predicate integration now uses it for explicit
matching single-GUID indexes, including mixed-schema scalar projections. Broader typed
predicate pushdown and cost-based index choice remain open. Opt-in
[automatic selection](access/indexes/AUTOMATIC_INDEXES.md) now uses matching qualified single-key
definitions. Schema discovery is separate from
row loading. Physical traversal stays in ACE, source adaptation in Access and
backend-neutral planning in query. Cost-based choices and TOP pushdown remain
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
or change the next native task. Further SQLite/native-project work requires
explicit user reprioritization and is excluded from the active sequence.

| Step | Milestone | Completion work remaining |
| --- | --- | --- |
| 1 | Native schema metadata | Bounded ACE 16-family metadata, inert saved-query fragments and owned local SELECT reconstruction, scoped complex links/child scalars, bounded typed multivalue/attachment/stored-history collections and mixed read-only browsing pages, explicit stored scalar calculations and original attachment decoding work; remaining collection/query semantics, advanced types and native Access evidence remain |
| 2 | Constraint validation | Qualified complete native index checks (including ACE 16 Decimal tuples) and one-to-ten-column Boolean/integer/Currency/GUID plus same-precision/scale ACE 16 Decimal FK tuple audits implemented (Boolean membership remains read-only) alongside bounded field/pure-rule audits, including statically Text/NULL Len and explicit binary StrComp; broader FK/expression profiles, collations, other carriers and native Access evidence remain |
| 3 | Preservation | Complete images, unchanged-byte patch proofs and bounded direct catalog-child inspection implemented; owned children block edits; actual ACE/VBA application save/reopen and full dependency discovery remain unqualified |
| 4 | Native physical mutation | Fixed Int32 replacements and ordinary/unique existing-tree maintenance work on offline Jet 4 copies; ACE 16 previews extend this to integral tuples, NULL transitions, checked foreign indexes and explicit multi-column/table batches within existing pages. Bounded integral/Currency/GUID and same-precision/scale Decimal update cascades now expand original associations across chains/cycles. Existing-width Decimal slot/index previews and ordinary deletion candidates with qualified existing-page contraction and single Short Text/Binary preserved keys are implemented. Explicit multi-table RESTRICT-only deletion batches compose shared retirement bits and validate final membership. A separate engineering service now expands qualified acyclic cross-table Cascade associations over original rows; self/cyclic actions and native SQL timing remain unqualified. A separate existing-width Short Text/Binary UPDATE service now re-encodes single/mixed tuple keys with full final constraints. A separate variable-record service now supports fitted Short Text/Binary changes, hidden-target forwarding, alternate owned-page placement and represented EOF data-page allocation. Ordinary raw Memo/OLE replacement and single-Int32 root/nonroot leaf splits now have separate engineering services and original-file candidate proofs. General allocation/map growth, broader Memo/OLE profiles and reclamation, other-codec index splits, composite deletion, broader relationship actions, self/cyclic cascades, SET NULL, INSERT and production ACE saves remain unfinished |
| 5 | Native transactions and recovery | Optional validated image journal integration and local fault/reopen tests implemented; native Access/power-loss/network-filesystem qualification remains |
| 6 | Backend-neutral database contracts | Shared snapshots and consuming edits implemented with an Access consumer; connection/cursor/runtime APIs remain separate |
| 7 | Query completion for editing | Bounded single-table/column UPDATE planning and CLI copies implemented; broader expressions, collations, actions and result-set updateability remain explicit gaps |
| 8 | Native collaboration | Native Access shared-file coordination and mixed-client recovery testing; SQLite and native projects are paused |
| 9 | Application runtime | Sessions, cursors, object lifecycle, bounded background work, cancellation and change notifications |
| 10 | Desktop UI | Accessible shell/navigation/ribbon using the selected [Selawik UI font](architecture/TECH_STACK.md#desktop-typography-decision), read-only datasheets and SQL editor, then qualified editing and visual builders |

Compatibility experiments and behavioral trace planning can progress alongside
this work. UI implementation follows the backend-first goal; its conformance
tests begin with the first UI components.
Jet 1/2 research and complete historical-format coverage do not gate the UI.
Native saves still require the relevant preservation/constraint/recovery evidence.

## Implementation priorities

| Order | Direction | Scope |
| --- | --- | --- |
| 1 | Native ACE 16 compatibility | Primary 100% target: reading, writing, indexes, transactions, unknown-object and existing VBA preservation; other Jet/ACE formats secondary |
| 2 | Microsoft Access collaboration | Native shared-file locking, concurrent writes, durability and recovery; actual mixed-client tests |
| 3 | Access query engine | Native tables, Access SQL, relationships, expression semantics and editable results |
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
[format target matrix](access/NATIVE_READING.md#compatibility-targets) records this
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

[VBA_COMPATIBILITY.md](architecture/VBA_COMPATIBILITY.md) owns preservation/detection and deferred
integration. [WINDOWS_COMPATIBILITY.md](architecture/WINDOWS_COMPATIBILITY.md) retains provider
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
remain required. See [conversion contracts](expressions/EXPRESSIONS.md#explicit-currency-conversion).

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
reliability and remaining read/query gaps. Broader Access query-function work
follows those priorities; SQLite expression work remains paused. Private reference packs remain
research inputs; their planned scenarios are not implemented acceptance evidence.


The required ACCDT reader now has [bounded package inspection](access/containers/ACCESS_TEMPLATES.md):
stored/deflated OPC snapshots, checked content types/internal relationships and
basic CLI reporting. Bounded generic-Object inline XSD data now supplies table browsing and query
integration. Qualified [List Definition agreement](access/containers/ACCESS_TEMPLATE_LISTS.md) now checks stored
SOAP declarations before table reads. Broader defaults/types, wider XSD/native field
fidelity and Access-created qualification remain required; this does not complete
template read/query acceptance. See [stored-data scope](access/containers/ACCESS_TEMPLATE_DATA.md).
