# Implementation status

## ACE 16 raw complex-child storage

Qualified fixed Int32 child-FK fields with exactly extended flag 0x08 now decode
on the 2016/2019 profiles. Resolved multivalue, attachment-storage and history flat
tables feed the existing scalar snapshot/query adapters. Parent 0x12 collections
remain rejected, including empty tables. Wrong width/type/extra bits and older
unqualified profiles stay unsupported. Common native feature flag constants now
have one owner in layout. [Advanced field scope](features/ACCESS_ADVANCED_FIELDS.md#raw-complex-child-storage-reads)
separates raw storage from collection semantics and attachment extraction.

Original populated fixtures match nine exact raw child rows each with the
independent reader, including Unicode, empty values, opaque encoded attachments
and fixed history timestamps. CLI queries return the expected counts, parent
scalar queries fail without partial output, and input hashes remain unchanged.
New synthetic checks cover exact carriers and strict profile/type/width/flag gates.
All 388 workspace tests/doc tests pass, plus workspace Clippy, release CLI build,
warnings-denied ACE documentation, formatting, whitespace and documentation/fixture
checks. No dependency was added. Native Access remains unrun and milestones 1–9
remain in progress; full parent collection/extraction/calculation is not completed.

## ACE 16 complex-schema reference resolution

Continuing milestone 1, `inspect_complex_schema` now joins native complex IDs,
MSysComplexColumns declarations, catalog references and value/flat table schemas.
It validates owner/name/identity, duplicate/dangling/self/alias references and the
qualified fixed-Int32 child FK/identity shape. Native schemas stay separate from
scalar collections and mutation authorization. CLI `inspect --table NAME --complex`
reports resolved names/keys/count declarations with no partial stdout on failure.
[Advanced fields](features/ACCESS_ADVANCED_FIELDS.md#resolved-complex-field-schemas)
own current scope; parent keys/child values, decompression and calculation remain next.

Both original advanced-family inputs match three independently resolved complex
rows each, including raw type IDs, type/flat names, FK/identity names and declared
counts. Hashes remain unchanged. Four new synthetic tests cover resolution,
corrupt/missing/duplicate mappings, geometry, ordinary tables and cancellation/
limits; a CLI integration test covers successful output and corrupt-map atomicity.
All 144 ACE all-feature tests/doc tests and three feature-mode CLI tests pass,
as do workspace Clippy, ACE all-feature Clippy, warnings-denied ACE docs,
formatting/whitespace and local docs/fixture checks. No dependency was added.
Native Access remains unrun; milestones 1–9 are ongoing, not complete.

## ACE 16 inert advanced-field declarations

The first implementation slice of the authorized pre-UI milestones adds
`inspect_table_features`, borrowed column-feature views and CLI
`inspect PATH --table NAME --features`. It exposes calculation flags/expression/
result tags, raw ACE complex type IDs, child-FK/AutoNumber/Hyperlink markers and
explicit AppendOnly/TextFormat declarations. Unknown/absent properties remain
retained/unknown; wrong typed/width values fail rather than becoming defaults.
Schema and original properties remain available without executing active content.
[Advanced fields](features/ACCESS_ADVANCED_FIELDS.md#working-inert-metadata-slice)
own the precise read-only scope. Structured children/recalculation and ACE writes
remain unimplemented; this does not complete milestone 1 or the nine-milestone path.

Original V2016/V2019 fixtures independently match ten field declarations each,
including generated attachment/multivalue and append-only/history metadata. Raw
complex IDs have independent synthetic golden checks; the oracle checks presence,
not numeric ID values. Scalar queries on their empty complex/calculated tables
fail with no stdout. Input hashes remain unchanged; native Access remains unrun.

All 380 workspace tests/doc tests and workspace Clippy passed. ACE all-feature
Clippy and 140 tests/doc tests passed, including two additional strict byte-property
tests added after the workspace run. Release CLI build, warnings-denied ACE API
documentation, formatting, doc links and fixture hash pass. No dependency was added.
Next in milestone 1: resolve complex child-table references and qualify their
structured data paths, with unsupported behaviors still blocking writes.

## Structured editing and agent requirements

The [shared editing decision](decisions/0002_SHARED_APPLICATION_EDITING.md) records
common discover/design/preview/validate/apply services for human and agent hosts.
Focused contracts specify typed changesets, stable project identities and revision
preconditions, semantic diffs, digest-bound applicable approval, idempotency,
revalidation and recovery. They explicitly preserve native write/format gates,
backend-specific transaction scope and the no-locks-during-review rule.

New [preview](features/HEADLESS_PREVIEWS.md),
[application scenario](testing/APPLICATION_SCENARIOS.md) and
[MCP adapter](interfaces/MCP.md) requirements cover semantic plus visual artifacts,
reproducible workflows, capabilities, bounded/redacted data, untrusted project
content, job ownership, executable trust and protocol negotiation. Verified the
MCP 2026-07-28 baseline and current SDK documentation against primary sources;
Tasks cancellation remains cooperative and durable state is a host responsibility.
The candidate review records rmcp, Ruff, type checking and pytest dispositions.

`tablecraft-edit` is a prospective/module-first boundary. The target map now has
38 prospective libraries across 15 groups, still twelve actual packages. The
`tablecraft-builders` name is retained. No editing/MCP/preview/scenario runtime,
new manifest/dependency/lockfile change or broader native write support was added.
ACE 16 advanced-field/write-safety implementation remains next; this requirements
slice does not resume SQLite expansion or native VBA execution.

Verification covers local documentation links/anchors, target-map uniqueness,
original fixture hash, formatting/whitespace and the existing package-boundary
policy guard with its ten regression tests. Those tests exercise dependency
policy, not agent capabilities or changeset behavior. Rust runtime tests were not
rerun for this documentation-only change; examples/tool names and acceptance
scenarios are not working commands or observed application test results.

## Native project decisions and dependency enforcement

Reconciled the seven supplied proposals into an
[accepted decision](decisions/0001_PROJECT_FORMAT_AND_EXECUTION.md) and focused
format/versioning, lifecycle, execution, transaction, Python, conversion and peer
schema-evolution contracts. Python supersedes the earlier JS/TypeScript default;
SQLite-backed `.tcraft` and equivalent `.tcraftproj/` representations are the
native project direction. MDB/ACCDB remain first-class, with ACE 16 still the
immediate implementation priority and existing VBA/Windows deferrals unchanged.

The target ownership map adds project/Python/codegen responsibilities only in
documentation (37 prospective libraries / 15 groups; still twelve actual Cargo
packages). No new manifest, registry dependency, lockfile change, project format,
Python host, migration implementation or generic transaction API was introduced.
The review records all supplied runtime/tool alternatives and optional ecosystem
packages separately from adopted dependencies. Internal container migrations,
developer data migrations and application releases have distinct contracts.

The working [dependency guard](architecture/DEPENDENCY_BOUNDARIES.md) checks all
declared local-package edges, including optional, renamed, platform, build and
development dependencies. New/unreviewed local packages fail. CI runs the guard
and its ten regression tests using an engineering Python installation; this is
not the application scripting host. All twelve packages pass, as does metadata
inspection with a fresh Cargo cache. Guard tests cover engine/adapter/DB separation,
query independence, host isolation, path/alias bypasses and library placement/kinds.
External third-party transitive/feature closures remain a separate review concern.

Formatting, whitespace, public local links/anchors, target-map uniqueness and the
original registered fixture hash pass checks. Rust production sources and Cargo
manifests are unchanged, so the Rust runtime suite was not rerun for this slice;
[project qualification](testing/PROJECT_COMPATIBILITY.md) is an acceptance plan,
not passing behavioral evidence. CI's Linux/Windows runs remain unrun locally.
Next implementation remains ACE 16 advanced fields/write safety; the native
project versioning/migration proof follows the current native priority.

## ACE 16 exact modern scalars

Native Large Number (0x13) now yields exact Int64 on codes 5/6; Date/Time Extended
(0x14) yields validated civil dates with 100ns precision on code 6. Width, fixed
storage and profile gates apply even to empty tables. Jiff performs Gregorian
calendar conversion for the validated 42-byte extended encoding. Calculated
columns and native Int64 indexes remain explicitly unsupported; all ACE writes
stay blocked. [NATIVE_ACE16.md](NATIVE_ACE16.md#modern-scalar-reads) owns details.

Same-kind extended dates support exact expression/query comparison and sorting.
The CLI preserves all seven fractional digits; mixed legacy/extended comparison
and implicit conversion remain unsupported. New tests cover full signed integer
extremes, values beyond double precision, exact typed parameter filters, NULL,
years 1–9999, malformed rows with no partial output, profile/width rejection,
and fixed-seed generated Gregorian dates and arbitrary native byte payloads.

All 374 workspace tests/doc tests pass, as do workspace Clippy, ACE journal-enabled
tests/Clippy, standalone expression minimal/all-feature tests/Clippy, the release
workspace build, formatting and warnings-denied all-feature API documentation.
Both original modern scalar fixtures match the independent reader exactly
(7/13 rows), query ordering/filtering and unsupported-feature checks pass, and
the earlier twelve-fixture ACE 16 comparison still passes. Inputs remain unchanged.
The dependency record now includes ACE's working use of the existing Jiff
workspace dependency; no registry package was added. Documentation links and the
original registered fixture hash pass verification. A full-disk build failure was
resolved by clearing generated incremental build cache and rerunning checks.
Native Microsoft Access qualification remains unrun. Next: ACE 16 advanced-field
metadata/safety gates, then broader index and native write coverage.

## ACE 16-family bounded readers

ReadProfile now selects unencoded subversion-zero 2016/2019-family layouts
(codes 5/6). Existing catalog/schema/scalar/property/relationship/audit and Int32
index/range algorithms work through the Access adapter and CLI. All ACE writes,
encoded/unknown-subversion inputs and unsupported field/index semantics remain
blocked. [NATIVE_ACE16.md](NATIVE_ACE16.md) records scope and reference review.

Twelve original generated-family fixtures match Jackcess across 480 catalog
records, mixed scalars/long values, properties, complete relationship inventories,
constraints, ranges and 200-column chained definitions. All input hashes remain
unchanged. The oracle now includes built-in navigation relationships in V2019;
chained-definition comparison selects native metadata fields from the current CLI
header rather than confusing newly added derived constraint columns with them.
All 364 workspace tests/doc tests pass, including three engine regressions
and one CLI regression using clearly identified
retagged disposable copies for code-path coverage only. Workspace Clippy passes.
No registry dependency was added and no Microsoft Access oracle ran. Modern
scalar qualification is recorded in the subsequent slice above.

## ACE 16 priority decision

ACE 16 is now the primary implementation and compatibility-testing focus, with
100% compatibility as the long-term target. Other Jet/ACE generations are
secondary, and further SQLite expansion remains paused. Existing working profiles
and regression checks are retained. [ROADMAP.md](ROADMAP.md) records the scope,
evidence requirements and unchanged explicit VBA/Windows deferrals.

The priority decision itself did not expand format support. Subsequent ACE 16
reader qualification is recorded above; native edits remain restricted to the
documented Jet 4 profile.

Documentation checks passed 490 local links/anchors across 49 public Markdown
files, whitespace checks and the unchanged original fixture hash. Runtime tests
were not rerun for this documentation-only decision.

## Bounded query UPDATE and fresh native copies

`plan_update` now binds one local table/one Int32 assignment through DB snapshot
contracts, evaluating supported pure expressions and WHERE before one backend
call. It reuses retrieval scalar/Between/IN/NULL behavior and explicit collation,
with checked integral assignment narrowing. Calls, parameters, joins, subqueries
and broader actions remain unsupported. A late error produces no backend plan.

CLI `query update --database INPUT --output NEW_COPY` composes this with the Access
adapter and engine gates. It opens the offline input read-only under an OS lock,
creates a fresh output after validation, syncs it and reports matched/changed rows.
Existing paths are never overwritten. Copy I/O failure may leave incomplete output;
this is separate from the optional library in-place journal boundary.

[QUERY_EDITING.md](QUERY_EDITING.md) records the profile, primary reference and
verification. All 360 minimal workspace tests/doc tests and 166 optional-feature
ACE/I/O/SQLite tests pass locally, along with workspace all-target Clippy, release
build and warnings-denied all-feature API docs. Additional targeted assertions
check cancellation after staged rows, row-budget failure, CLI lock contention and
aliased table rejection. The independent Jet 4 recipe confirms CLI and engine
copies are identical: Score 20 becomes 27, exactly one byte changes, and Jackcess
plus native primary-index reopen pass. Original fixtures remain unchanged.

Only existing local dependencies were added. SQLite production code/capabilities
are unchanged; optional tests cover the shared query runtime change. Further
SQLite expansion stops at the requested boundary. The roadmap explicitly retains
ACE writes/data macros, allocation/index mutation, INSERT/DELETE, full application
preservation/dependency discovery, broader query updateability and native Access
qualification as unfinished work. The entire engine is not declared complete.

## Working database snapshot and edit contracts

The twelfth package, `tablecraft-db`, owns exact validated owned snapshots,
process-local row handles and consuming batch edit contracts. `NativeEditor`
implements them over an immutable native image, preserving standalone engine
independence. Cross-snapshot handles and wrong carriers fail before planning;
ACE reports updates unavailable and Jet 4 retains all native gates.

[DB.md](DB.md) records responsibilities and limits. Two contract tests and two
adapter integration tests pass, including a generic native planning consumer;
standalone checks and all-target Clippy pass. No registry dependency or SQLite
behavior changes. CI now includes standalone DB and optional native journal
checks; configuring CI is not evidence those remote jobs ran. Next is bounded
query editing over these working contracts.

## Conservative native payload gates

Native image inspection now reports Lv/LvModule/LvExtra null-mask presence without
following object streams. Missing columns remain Uninspected, distinct from NULL.
The Jet 4 writer additionally requires explicit absence on its selected table.
Two additional focused tests cover opaque-content rejection, missing metadata,
foreign identities and cancellation; existing mutation/preservation/journal tests
and all-target all-feature ACE Clippy pass. The independent Jet 4 update recipe
still succeeds. [Coverage](NATIVE_PRESERVATION.md#inert-payload-presence) is physical
evidence, not complete VBA/data-macro discovery or ACE write qualification.

## Journaled offline native edits

The optional ACE journal feature connects range-proven edit plans to the existing
generic image protocol. `prepare_expected` checks the complete validated preimage
before journal writes. Native prepare/commit/rollback and explicit recovery retain
journals and require exclusive offline resources with host-owned durable namespace
setup/cleanup. Recovery captures the resulting native image.

Three native integration tests and the additional generic preimage test pass,
including a hybrid payload after an injected write failure, repeatable rollback,
stale targets, retained-journal rejection and locked file close/reopen recovery.
All-target all-feature ACE/I/O Clippy passes. The feature adds no registry package
and stays off for normal readers. [Scope](NATIVE_WRITES.md#optional-journaled-persistence)
excludes native Access coordination, actual power-loss and network-filesystem
qualification. Next is working backend-neutral database contracts.

## Fixed Int32 native edit plans

The bounded Jet 4 engineering profile in [NATIVE_WRITES.md](NATIVE_WRITES.md)
plans existing non-indexed fixed Int32 replacements with before/after constraints
and unchanged-byte preservation proofs. AutoNumber, unsupported metadata/rules,
relocated rows, indexed fields and ACE writes are blocked. Source images remain
immutable; the fresh-copy laboratory harness never modifies its input.

All 344 workspace tests/doc tests and all-target Clippy pass, including five
focused mutation tests and a 256-case independent row model. Jackcess
reopens an original generated Jet 4 copy with Score 20 changed to 27; exactly one
byte differs and unchanged primary-index traversal remains valid. Rule/stale/index
rejections publish no output. Unicode memo/binary payloads remain byte-identical.
No new registry dependency or tracked binary fixture was added. Native Access
remains unrun. Next is journaled native persistence and recovery.

## Complete native image capture

`NativeImage` now retains complete immutable Jet 4/ACE 2007 bytes and exposes
borrowed readers plus conservative catalog dependency coverage. Opaque VBA,
unknown objects, unused pages and padding survive capture exactly. The original
example retains all bytes and 146 objects; its module/application declarations
report unavailable execution without decoding bodies. Three focused tests pass,
including unknown bytes, budgets and midstream cancellation.

[Preservation scope](NATIVE_PRESERVATION.md) distinguishes capture evidence from
unimplemented edit/save/reopen guarantees and complete dependency discovery. No
new dependency or fixture was added. Next is range-proven native physical edits.

## Bounded field and table validation rules

Native constraint audits now evaluate the pure profile in
[NATIVE_CONSTRAINTS.md](NATIVE_CONSTRAINTS.md), using owned standalone expression
parsing and token-based implied field references. Field rules bind only their own
field; table rules bind their own table. False results fail with typed violations;
NULL results remain Unsupported. Calls, text comparisons and unimplemented forms
are rejected before row evaluation, including empty tables. No defaults or active
content execute and normal reads are unchanged.

All 336 workspace tests/doc tests and all-target Clippy passed locally. Original
Jet 4/ACE fixtures match six row-count/coverage reports per format against the
independent reader; input bytes remain unchanged. New focused tests cover parser
limits/trailing SQL, rule rejection/failure, budgets and cancellation. Existing
SQL/expression packages add no registry dependencies. Native Access remains unrun.
The bounded constraint slice is ready for preservation work; broader constraints
remain explicit gaps and cannot authorize writes.

## Explicit field and foreign-key record audits

`check_table_constraints` and `inspect --table NAME --constraints` now check
explicit Required/AllowZeroLength declarations and qualified single-Int32 foreign
keys touching the selected table. Checks verify parent uniqueness, child membership
and declared one-to-one child uniqueness; NULL children and explicitly unenforced
relationships retain separate behavior. Composite/unknown/unbound profiles and
validation expressions report Unsupported. Normal reads and structural/index
checks remain separate. CLI dispatch now uses typed inspection modes.

[Constraint coverage and primary references](NATIVE_CONSTRAINTS.md) document remaining
default/expression/index/macro/write gaps. Shared schema owns allocation-free scalar
field policies and typed violations; native audit errors omit payloads. Original
Jet 4/ACE relationship fixtures match row counts and coverage across all ten child
checks. In-memory mutation tests detect Required, empty Text, orphan-child and
duplicate-parent failures; a 256-case independent set model checks key semantics.
All 331 workspace tests/doc tests and workspace all-target Clippy passed locally.
Registered fixture bytes remain unchanged; native Access remains unrun. Step 2
continues with bounded field/table validation expressions before preservation work.

## Native relationship declarations

Shared schema now owns validated ordered parent/dependent relationship shapes and
qualified rule/action metadata. ACE retains native flags and bounded grouping of
MSysRelationships rows; unknown/inherited/conflicting profiles remain unqualified.
`inspect --relationships` reports inert declarations without opening providers,
checking user records or executing cascades.

[Scope and primary references](NATIVE_RELATIONSHIPS.md) record shape validation,
resource charging and unresolved endpoint policy. Original Jet 4/ACE recipes match
five relationships/six pairs per format against Jackcess, including composite,
one-to-one, unenforced and cascade/set-null declarations. Partially NULL composite
behavior remains explicitly unqualified pending native comparison. All 322 workspace
tests/doc tests and all-target Clippy passed locally, including constructor,
malformed metadata, cancellation/budgets, a fixed-seed ordinal property and CLI
checks. Registered fixture bytes remain unchanged. Step 1's bounded metadata slice
is complete; next is step 2, field/relationship record validation. Newer formats,
native Access comparisons and relationship writes remain unimplemented.

## Native field/table property declarations

`inspect_table_properties` and CLI `inspect --table NAME --properties` now decode
bounded MR2 table/column/index property maps in the qualified Jet 4/ACE profile.
Boolean/text declarations remain inert; unknown values/flags and the full original
LvProp stream remain available. Scoped groups distinguish identically named columns
and indexes. Internal metadata projection skips unrelated module/object bodies.
Ordinary schema/row APIs keep their conservative nullability contract.

[Property scope and references](NATIVE_PROPERTIES.md) record framing, limits,
cancellation and typed access. Original generated Jet 4/ACE fixtures match all 22
property declarations against Jackcess, including quoted defaults and Unicode.
All 312 workspace tests/doc tests and workspace all-target Clippy passed locally;
new tests cover malformed streams, budgets/cancellation, a fixed-seed property,
real fixture declarations, CLI options/output and unchanged input bytes. Native
Access remains unrun. Rule enforcement/default evaluation and native property
mutation are not implemented. Next in step 1: relationship declarations.

## Optional tokio-rusqlite reader

The `tablecraft-sqlite/async` feature supplies worker-owned Access query retrieval
through tokio-rusqlite 0.8.0 and Tokio 1.53.3 sync admission. Each request owns its
snapshot, TableCraft binding/evaluation and rollback. One job is admitted per reader;
additional requests fail immediately. Dropping its future sets cancellation while
the actual worker retains admission until cleanup. Cleanup failure prevents reuse.
No raw SQLite handles, arbitrary SQL execution or cross-request transactions escape.

[Async scope and cancellation](SQLITE.md#optional-async-worker) document callback/I/O
limits and host ownership. [All eighteen added registry packages](DEPENDENCIES.md#optional-async-sqlite-worker)
are recorded, including upstream cache and Wasm-only defaults. Native CLI dependency
inspection excludes Tokio/tokio-rusqlite and their cache/worker closure. CI includes
standalone all-feature SQLite tests and Clippy using existing version-tagged actions.

All 17 SQLite tests plus the compiled async API example and all-feature Clippy passed
locally. Four worker tests exercise successful requests, cancellation/exhaustion,
cleanup and a deliberately blocked host callback to verify abandoned requests cannot
release admission early. Workspace release build and warning-denied all-feature API
docs passed. Windows/Linux/browser execution and native Access remain unrun.

## Read-only SQLite source and CLI

`tablecraft-sqlite` now supplies bounded ordinary-table schemas and scans to the
existing query evaluator. `query run --sqlite PATH` opens an existing file read-only,
retains a SQLite read transaction and buffers results before output. Access SQL
is evaluated by TableCraft; only generated metadata/table reads reach SQLite.
Carrier mismatches, unknown encodings and unsupported types fail explicitly;
views/virtual tables are unavailable and generated columns are rejected.

[SQLite scope](SQLITE.md) records snapshots, limits, immediate busy handling,
shared cancellation, VM interruption and internal-allocation/I/O limitations.
The bundled rusqlite dependency is [reviewed separately](DEPENDENCIES.md#sqlite-reader).
All 304 workspace tests/doc tests and targeted adapter/CLI all-target Clippy passed
locally on macOS ARM, including 13 SQLite tests (one 256-case property) and two CLI
integration tests. Original native fixtures and disposable CLI SQLite database
bytes remain unchanged. Windows/Linux, browser storage, async hosts and native
Access comparisons remain unrun. Native writes/field properties/relationships and
expanded format qualification remain separate unfinished work.

## Explicit native read profiles

`Header::read_profile()` and catalog decoding now share one qualified read-profile
selector. Header recognition remains broader than decoding: only unencoded Jet 4
and ACE 2007 with subversion zero select working profiles. Unsupported families,
subversions and codecs still fail before catalog reads. Structural ceilings moved
from `profile.rs` to `limits.rs`; shared feature implementations remain unchanged.

All native package tests/doc tests and all-target Clippy passed locally, including
an independent header/profile matrix. No new format support, writes, dependency
or native Access comparison is claimed. See [selection](NATIVE_READING.md#read-profile-selection).

## Optional image commit and recovery foundations

The tablecraft-io/journal feature implements bounded fixed-length image prepare,
commit, rollback and restartable recovery over explicit DurableStorage resources.
The prepared journal is synced before target changes; the target is synced before
its digest-bound commit marker. Any commit error requires explicit recovery, and
existing journals are never silently overwritten/deleted/reused. Framing, SHA-256
and complete paired-target checks precede recovery writes. Interrupted rollback
leaves its journal unchanged so partial markers cannot become mixed by overwrites.
Bounded short-write/Interrupted handling is also available independently.

[Protocol ownership, resource costs and durability preconditions](IMAGE_JOURNAL.md)
are explicit. Hosts own exclusive access, correct resource identity, durable
journal namespace setup/cleanup and timeouts. Only equal nonempty images up to
64 MiB are supported. This is not the Jet/ACE recovery format, an integrated native
writer or native shared-file coordination. Existing native writes remain unavailable.

Optional sha2 0.11.1 supplies SHA-256 integrity; [exact dependency records](DEPENDENCIES.md#optional-image-journal-integrity)
cover all six new lockfile packages and feature/license/MSRV boundaries. Defaults
stay empty, and the observed normal CLI/native-reader closure excludes sha2/digest.
The standard lock wrappers implement DurableStorage without exposing raw handles.
No security/database facade crate, async runtime or unused scaffold is added.

Tests interrupt every commit/recovery callback before and after side effects,
force short writes and partial markers, and model survival/loss of unsynced bytes.
They verify barrier order, repeatable recovery, every truncation/byte corruption,
independent framing/digest goldens, cancellation/limits, mismatched targets and
existing-journal refusal. Two fixed-seed properties cover mixed old/new target
bytes and bounded arbitrary journals; disposable locked files reopen and recover
both outcomes. No registered fixture bytes change.

Local Rust 1.99.0 checks passed all 288 workspace tests/doc tests, 23 optional-I/O
tests in debug and release, workspace/all-feature-I/O Clippy, standalone package
checks, minimal/all-feature expressions, all-feature core, release build and
warning-denied API docs. CI now includes the optional I/O profile using existing
version-tagged actions. Physical power-loss, native Access, other target execution,
network filesystems and remote CI remain unrun. Next: native field properties and
relationship constraints, then preservation-aware native mutation on disposable
offline copies; OS locks and generic journaling alone do not qualify those writes.

## Offline read transactions and OS lock guards

OfflineReadTransaction captures a bounded immutable image under an owned shared
OS file lock and implements Read + Seek for existing engine/query consumers.
SharedFile/ExclusiveFile provide nonblocking OS lock guards, typed contention,
reported explicit close and best-effort drop release; they never create/truncate
files. Explicit drop unlock prevents transiently inherited child descriptors from
extending the guard's lifetime. No native write API or Access lock protocol is
introduced, and CLI defaults remain unchanged.

[Transaction scope](NATIVE_TRANSACTIONS.md) documents offline capture, cumulative
memory/read/work charges, callback timeout ownership and remaining write gates.
Original tests verify cooperating handle/process exclusion, repeatable image reads,
existing table checks through transactions, cancellation/limits and lock release.
A compile-fail test excludes Write. Workspace Clippy and all 285 workspace tests/
doc tests passed locally on macOS ARM. Native Access, Windows/Linux and network
filesystems remain unrun. Generic journal/recovery ordering is the next slice;
native commit, preservation, required macros and mixed-client coordination remain
unimplemented.

## Qualified native index constraints

Native index inspection decodes known primary, unique, required-key and ignore-NULL
declarations without changing field nullability or guessing unknown/foreign
profiles. Primary semantics propagate across logical aliases of physical backing;
multiple logical primary declarations are rejected. Explicit check_table operations
now validate unique/non-NULL rules over complete qualified Int32 traversals and
return sanitized typed constraint failures. CLI inspection exposes declarations
and checked constraint coverage. Normal reads retain their structural contract.

[Constraint scope and primary references](NATIVE_CHECKS.md#qualified-index-constraints)
record the distinction between index declarations and field properties. Independent
Jackcess-generated Jet 4/ACE fixtures match all six index declarations/checks and
all 18 tagged rows, including duplicate NULLs in a unique index. Mutation tests
and a 256-case set-model property cover violations and aliases. Workspace Clippy
and all 278 workspace tests/doc tests passed locally; original fixture bytes are
unchanged. Native Access remains unrun. Composite/Text/foreign-key checks, field
validation and write-time enforcement remain unimplemented. Transaction/locking
and recovery foundations are the next active slice; native writes stay unavailable.

## Optimized Jiff calendar adapters

Legacy DATE calendar conversion retains Jiff's checked civil types while avoiding
redundant date arithmetic and an i128 time-duration conversion. Rounded midnight
carry is folded into one civil-day addition; exact stored/Serde bits, negative
DATE rules, field bounds and checked errors are unchanged. No dependency or
feature change was needed. Stored-date comparison does not construct calendars.

The [20-run follow-up](testing/NATIVE_BENCHMARKS.md#jiff-conversion-follow-up)
records every sample, including scheduling outliers. Five-million round-trip
medians improve from 39.50 to 27.15 ns per operation (about 31%); the time baseline
is 23.20 ns, leaving a roughly 17% median gap on this macOS ARM host. Mean
improvement is about 28%. These are calendar conversion results, not overall
engine/query speed claims. The original migration measurements remain recorded.

Five new tests include independent golden boundary/rounding cases and three
fixed-seed properties against Jiff's general arithmetic. Local Rust 1.99.0 checks
passed 273 workspace tests/doc tests, all-target Clippy, minimal package checks,
minimal/all-feature expression and all-feature core tests/Clippy, release and
benchmark builds, and warning-denied API docs. Native Access, other targets and
remote CI remain unrun. Next: qualify native constraint metadata and semantics
before considering native writes.

## Native schema allocation audit

Native column/table schema names now use the same fallible reserve/copy path
instead of two remaining infallible clones. Caller byte charges remain explicit.
Column-number sorting uses the in-place unstable sort: valid layouts already
reject duplicate numbers, so this preserves ordering without an uncharged stable-
sort scratch allocation. Schema constructors/validation and supported profiles
remain unchanged; no dependency was added.

Existing schema, budget, mutation, row, query and consistency suites cover the
unchanged behavior. This is allocation-failure hardening, without a new performance
claim. Local Rust 1.99.0 verification passed 268 workspace tests/doc tests,
all-target Clippy, minimal package checks, release build and warning-denied API
docs. All 32 independent query comparisons still match; documentation links and
the original fixture SHA-256 pass. Native Access, remote CI and other target
execution remain unrun.

## Explicit native table consistency reports

`check_table` now combines a complete sequential scalar scan with every eligible
Int32 index, using one cumulative budget. It verifies matching schema/identity,
live-root counts and full tree/key/root agreement. Known unsupported profiles are
reported per index without claiming validation; metadata or selected traversal
failures return no report. Index eligibility has one owner shared with normal
indexed reads. No native constraints, orphan discovery, write safety or whole-file
integrity certificate is implied.

CLI `inspect PATH --table NAME --check` buffers explicit checked/unverified
coverage, preserving inertness, limits, redaction and no partial failure output.
Tests prove detection of an extra live root missed by an otherwise count-correct
index, skipped corrupt Text-index coverage, cumulative allowances/cancellation and
every I/O callback failure. Generated Indexed tables report 1,000 rows with 2/3
checked/unverified indexes; Wide/EmptyWide report 7/0 rows with both indexes checked
in both Jet 4 and ACE. [Check scope](NATIVE_CHECKS.md) and
[Hyperfine evidence](testing/NATIVE_BENCHMARKS.md#table-consistency-checks) record
the current limitations and all timing samples.

Local Rust 1.99.0 checks passed 268 workspace tests/doc tests, all-target Clippy,
minimal package checks, release/benchmark builds and warning-denied API docs.
Dependencies and registered fixture bytes are unchanged. Native Access, other
targets and remote CI remain unrun. Next: audit remaining native allocation failure
paths before expanding qualified constraint semantics.

## Linear usage-map traversal

Owned-page bitmap traversal now verifies strict order/uniqueness against the last
emitted page, replacing repeated searches through prior pages. Inline bit order and
disjoint indirect ranges establish the sorted contract required by native index
ownership lookups. Existing reference, allocation/work, file/page and cancellation
limits remain. Four focused tests include a 256-case independent bit model.

The original 2,000-page recipe and standalone clap-based table_data example match
all 4,000 tagged Jet 4/ACE rows, including every Binary byte, against Jackcess.
The engineering consumer supplies 256 MiB explicitly; CLI defaults remain 16 MiB.
[Usage-map limits](NATIVE_READING.md#usage-map-ordering-and-ownership-bounds),
[fixture identities](FIXTURES.md#original-generated-usage-map-inputs) and
[Hyperfine samples](testing/NATIVE_BENCHMARKS.md#usage-map-traversal) record the
evidence. Timings are noisy, including a worse ACE sequential mean; no broad
whole-reader speedup is claimed for this complexity/invariant change.

Local Rust 1.99.0 checks passed 261 workspace tests/doc tests, all-target Clippy,
release/benchmark/example builds, minimal package checks and warning-denied API
docs. No dependency or original fixture bytes changed. Native Access, other
targets and remote CI remain unrun. Next: read-only consistency checks across
sequential rows and qualified indexes, with skipped validation reported explicitly.

## Distinct Binary read and creation limits

The native Binary read ceiling remains 510 declared bytes, separately named from
Text's UTF-16 width limit. A temporary restriction to the writer's 255-byte creation
limit broke valid system-table queries in the complete workspace suite and is
corrected: MSysObjects/MSysACEs have independently read 510-byte Binary metadata.
Authoring limits do not establish limits for reading existing native files.
Boundary tests now retain these actual schemas and 255/256/510/511-byte cases.
Zero-length declarations remain outside the qualified reader profile.

Targeted schema tests and all-target ACE Clippy passed on Rust 1.99.0; registered
fixture bytes and dependencies remain unchanged. This corrects reader qualification,
not an observed Microsoft Access comparison. Usage-map work continues separately.

## Bounded chained table definitions

The native shared definition reader now joins at most 64 pages/261,640 logical
bytes for catalog, schema and index consumers. It validates declared length,
signatures, termination, cycles and references, with caller allocation/work/I/O
budgets and cancellation. One-page definitions avoid join allocation. Columns,
Unicode names and index blocks can cross continuation headers; malformed chains
return no partial metadata. Unknown properties/write semantics remain unqualified.

An original 200-column recipe generates two seven-page definitions in each format.
Jackcess comparisons match 800 column descriptions, eight index definitions and
12 sequential/bounded CLI outputs across Jet 4/ACE, including empty tables.
Independent golden assembly, malformed chains, fixed-seed mutation, cancellation,
budgets and every I/O callback failure are covered. [Definition limits](NATIVE_TABLES.md#bounded-chained-table-definitions),
[fixture provenance](FIXTURES.md#original-generated-chained-definition-inputs)
and [Hyperfine evidence](testing/NATIVE_BENCHMARKS.md#chained-table-definitions)
record the accepted profile. Ordinary-reader means changed about -0.1% to +3.3%;
the new capability is not presented as a speedup.

Local Rust 1.99.0 checks passed 256 workspace tests/doc tests, all-target Clippy,
minimal package checks, release/benchmark builds and warning-denied API docs.
Dependencies and original fixture bytes remain unchanged. Native Access, other
targets and remote CI remain unrun. Next: harden usage-map traversal and its
ownership/order evidence before broadening native operations.

## Schema discovery and safe native filter pushdown

NativeSource binding now reads bounded scalar metadata without user records;
complete snapshots remain cached and selected snapshots remain ephemeral. ACE
exposes budgeted scalar schema construction with fallible owned-name allocation.
Query's optional backend-neutral Source constraint defaults to a full scan. Safe
single-Int32 comparisons/NULL tests and same-column conjunctions can select native
ranges through an explicitly required matching index; the original WHERE remains.
Arithmetic, OR, coercions, mixed-column predicates and outer join/subquery
conditions keep full scans, preserving eager failures. Index choices never fall
back silently. A one-table plan avoids a redundant unit cross-product.

Tests cover exact selected root counts, later whole-table cache behavior, schema
discovery with corrupt user data, matching/ignored constraints, parameter reuse,
NULL, eager errors, callback/late source failures and 256 fixed-seed intervals.
The release CLI matched 32 independently filtered/sorted Jackcess outputs across
generated Jet 4/ACE files. [Query contracts](QUERY_EXECUTION.md#safe-source-constraints),
[native source limits](NATIVE_QUERIES.md) and
[Hyperfine evidence](testing/NATIVE_BENCHMARKS.md#query-filter-pushdown) record the scope.

Local Rust 1.99.0 checks passed 249 workspace tests/doc tests, all-target Clippy,
minimal package checks, release/benchmark builds and warning-denied API docs.
The only lockfile edge adds existing clap to the adapter's development dependencies
for its original benchmark harness; normal library closures retain no clap.
Fixture bytes remain unchanged. Native Access, other targets and remote CI are
unrun. Next: qualify bounded chained table definitions.

## Bounded native Int32 seeks

`read_table_index_range` selects NULL or numeric open/closed/unbounded intervals in
either physical index direction. Separator pruning skips disjoint children and
only selected live roots are fetched. Duplicate boundary keys, selected key/row
agreement, visited sibling links, ownership, cancellation and caller budgets remain
checked. Full reads retain complete traversal/count validation; seeks validate
only visited pages and selected records, not skipped tree integrity.

The reproducible example and independent Jackcess comparison match 28 selections
across generated Jet 4/ACE ascending/nullable-descending indexes. Golden and
fixed-seed range tests, exact I/O pruning assertions, visited/skipped corruption
and every-callback read/seek failures cover the new API. Hyperfine's four
500-read workloads measure roughly 6–13× faster selected reads than full snapshots
plus filtering on this host, with all samples retained, including outliers.
See [native index limits](NATIVE_INDEXES.md#native-int32-range-selections) and
[benchmark evidence](testing/NATIVE_BENCHMARKS.md#native-index-seeks).

Local Rust 1.99.0 checks passed 239 workspace tests/doc tests, all-target Clippy,
release/benchmark/example builds and warning-denied API docs. No dependency or
fixture bytes changed. Query source discovery/predicate pushdown is next; CLI
queries still load full snapshots. Native Access, other targets and remote CI
remain unrun.

## Measured civil/calendar and native byte libraries

Jiff replaces time for checked civil calendars and bounded ISO date input while
retaining exact date carriers/Serde, negative Automation DATE rules and field
year/precision bounds. Endpoint and literal tests guard the adapter: Jiff's broader
parser cannot silently accept zones/leap seconds, and civil day spans cover the
full year-9999 endpoint. The unpublished calendar APIs now expose Jiff civil types.

ACE uses zerocopy for explicitly little-endian unaligned index headers/long-value
descriptors. Wide/bytemuck classify private raw-byte ASCII text, avoiding an
intermediate UTF-16 vector while preserving strict Unicode fallback. Bitmap work
uses precharged eight-byte word blocks with bounded cancellation polling.
No semantic Value/Decimal/date receives POD or unchecked construction.

[Hyperfine samples and reproduction](testing/NATIVE_BENCHMARKS.md) record 20 runs
after three warmups per command. Catalog/table workloads improve about 13–22% on
this macOS ARM host; the initial Jiff calendar round trip was about 65% slower.
The [adapter follow-up](testing/NATIVE_BENCHMARKS.md#jiff-conversion-follow-up)
records the subsequent improvement and remaining conversion cost.
Bitmap microbenchmarks vary by density, and a whole-catalog comparison established
no reliable SIMD advantage over the simpler word iterator. These are measured
tradeoffs, not general or cross-platform speed guarantees.

Local Rust 1.99.0 verification passed 231 workspace tests/doc tests, workspace
Clippy, minimal package checks, minimal/all-feature expression and all-feature core
tests/Clippy, release build and warning-denied docs. Independent Jackcess index
comparisons retain all six previous exact row hashes. The fixture SHA-256 is
unchanged; native Access, remote CI and other targets remain unrun. Dependency
records distinguish actual host dependencies from inactive lockfile packages.

## Complete dependency/reference review and purpose grouping

[DEPENDENCY_REVIEW.md](DEPENDENCY_REVIEW.md) individually assesses 103 crates/tools,
reference projects/specifications and all ten proposed standalone RustVBA components.
It covers every candidate in the supplied library, Wine and RustVBA proposals with
source/license metadata, disposition, owner and adoption conditions. The library
policy and technology/reference docs link that registry; installed dependencies
remain separately recorded. Candidates, prototypes and deferred providers were not
added as unused dependencies.

Root workspace dependency declarations are grouped by purpose. Parsed Cargo
manifest semantics, complete locked/offline metadata (including resolved features)
and Cargo.lock match the pre-grouping state exactly; ten packages remain. Local
Rust 1.99.0 formatting, locked/offline workspace check, documentation links/anchors,
candidate/component completeness, target-map counts and diff whitespace checks
passed. The original fixture SHA-256 is unchanged. Runtime suites/builds were not
rerun for this docs/comment/reordering change; no benchmark, advisory scan, license
gate, fuzz run or native Access comparison was performed by the review.

## VBA scope revision

The 18 October 2026 priority decision supersedes the historical VBA/Windows plans
below. VBA execution, authoring, editor/debugger, VBA-specific DAO/ADO, Windows API
emulation, COM/Wine and RustVBA integration are deferred. Existing-code preservation
through unrelated saves and inert dependency detection remain requirements, both
unimplemented. Access table data macros remain in database compatibility scope:
required semantics must run or affected writes must fail.

The active target map removes `tablecraft-vba` and `tablecraft-wincompat`; no
`tablecraft-wine` is added. `tablecraft-vba-host` is reserved in docs only for a
future standalone RustVBA release. Macro/automation ownership remains. Current
working packages and read-only CLI behavior are unchanged. See [VBA scope](VBA_COMPATIBILITY.md)
and the revised [priority order](ROADMAP.md).

Local documentation checks verified 301 links/anchors across 37 public Markdown
files, the revised 34-library/13-domain active target map and the unchanged fixture
SHA-256. These are documentation checks, not VBA preservation or execution tests.

## Explicit bounded native indexed reads

ACE read_table_index traverses qualified single-Int32 index trees and fetches live
referenced rows, checking prefix/boundary encodings, ordering, separator bounds,
ownership, sibling links, cycles/depth, counts and decoded key agreement. The Access
adapter can require a named index and cache the complete result; CLI exposes
`--database PATH --index TABLE INDEX`. Unsupported or corrupt indexes never trigger
sequential fallback. Query clauses still run after full snapshot loading; no range
seek, predicate/TOP pushdown or automatic optimizer selection is claimed.
[NATIVE_INDEXES.md](NATIVE_INDEXES.md) records exact limits, tests and primary references.

Independent Jackcess cursor comparisons matched 1,000-row ascending/nullable-descending
indexes in both generated Jet 4 and ACE databases, plus two 38-row candidate indexes.
Byte goldens, two-level/corrupt trees, mutation, cache/no-fallback, every-callback I/O
failure, mid-read cancellation and CLI semantic tests cover the implementation.
Local macOS / Rust 1.99.0 checks passed 224 workspace tests/doc tests, workspace
Clippy, minimal package and scalar feature checks, release build and warning-denied
API docs. No registry dependency was added and original fixture bytes remain unchanged.
Native Access, other platforms and remote CI remain unrun.

## Native index metadata

ACE now returns explicit logical/physical native index metadata through
inspect_table_indexes; CLI inspection uses `--table NAME --indexes`. Native
numbers/references, raw flags/types, ordered key columns and directions are retained
with strict bounds and caller budgets. [NATIVE_INDEXES.md](NATIVE_INDEXES.md) records
pinned research references, unsupported layouts, original recipes and independent
metadata matches for the registered candidate plus generated Jet 4 / ACE samples.
No index traversal, optimizer path, writing or native constraint validation is
claimed by this inspection slice.

Local macOS / Rust 1.99.0 checks passed 211 workspace tests/doc tests, workspace
Clippy, release build and warning-denied API docs. Static/malformed/cancellation
and 256-case mutation tests plus CLI coverage were added. Native Access, remote
CI and other targets were not run; original fixture bytes remain unchanged.

## Opt-in structured diagnostics

CLI `-v` enables debug operation spans; `-vv` adds low-level I/O detail. Output
is stderr-only with monotonic timing, explicit counts/cache/outcome fields and
no automatic function arguments. Libraries emit through tracing and never install
subscribers. Query stdout and error/exit contracts are unchanged; no environment
filter, network exporter or async runtime is introduced. [TRACING.md](TRACING.md)
and dependency records document the implementation.

Local macOS / Rust 1.99.0 checks passed 207 workspace tests/doc tests, workspace
Clippy, release build and warning-denied API docs. New CLI tests verify quiet
behavior, unchanged stdout, level filtering and omission of SQL/value/name/path
payloads on success/failure. Remote CI, other targets and native Access were not run.

## Workspace-owned dependencies

Registry requirements, local package paths and shared dependency defaults now live
in root `[workspace.dependencies]`; every member normal/development dependency
inherits them. Consumer-specific features and optional flags stay local. The
resolved graph, standalone minimal/all-feature expression and all-feature core
feature trees, and Cargo.lock are unchanged. No registry dependency was added.
`sys-locale` is recorded as a future host-localization candidate in the library
policy; current query/output locale rules remain explicit and unchanged.

Local macOS / Rust 1.99.0 checks passed 205 workspace tests/doc tests, standalone
package checks, workspace Clippy, minimal/all-feature expression and all-feature
core lint/tests, release build and warning-denied API docs. Other targets, remote
CI and native Access were not run.

## Platform I/O extraction

`tablecraft-io` now owns working format-independent synchronous exact reads and
read-at-offset helpers under `crates/platform/`. It consumes core budgets and
thiserror; no registry package or runtime was added. Generic read retry/cancellation
tests moved out of ACE. New focused cases cover nonzero-offset short reads,
byte-charge ownership, EOF, invalid reader counts, sanitized read/seek failures
and pre-seek cancellation/exhaustion. [IO.md](IO.md) documents the API and limits.

ACE header/page reads now consume this crate and translate its errors to the
existing native error contract. Page geometry, allocation, row interpretation and
native locking ownership remain in ACE. Absolute seeks now explicitly charge a
step; byte charges remain at the native caller. Locks, durability and async I/O
were not added. CI includes the new standalone check and uses existing action tags.

Local macOS / Rust 1.99.0 checks passed 205 workspace tests/doc tests, standalone
I/O and engine/adapter checks, workspace Clippy, release build and warning-denied
API docs. An external Rust consumer built and exercised read-at-offset/EOF with
no engine, query, adapter or application dependency. Original fixture bytes remain
unchanged. Other platforms, remote CI and native Access were not run.

## Dense appended-column row reading

The native row reader now derives older-row geometry under a qualified dense
append-only column profile. Missing fields follow independently observed native
storage semantics (NULL, or FALSE for Boolean), while current defaults remain
inert. It handles rows written before any variable field existed and retains
strict checks on present payloads. Gapped/deleted histories and type changes
remain unqualified. [NATIVE_TABLES.md](NATIVE_TABLES.md#dense-appended-column-histories)
records the three-stage independent fixture comparison and limits.

Local macOS / Rust 1.99.0 checks passed 200 workspace tests/doc tests, minimal
engine/adapter/query checks, workspace Clippy, release build and warning-denied
API docs. Scalar feature checks remain as recorded at their unchanged milestone.
The original native fixture remains unchanged. Native Access, other targets and
remote CI remain unrun; no writing/schema mutation was added.

## Mixed-scalar Jet 4 / ACE independent comparison

An original Java recipe now generates fresh V2000 MDB and V2007 ACCDB samples
with three rows/fourteen columns spanning the entire qualified scalar profile.
The external Rust consumer and Jackcess agreed on every tagged cell, including
IEEE signed zero, Currency extremes, Unicode, MEMO/OLE chains, GUID, Decimal and
a negative Automation date. Both formats produced identical normalized output.
[NATIVE_TABLES.md](NATIVE_TABLES.md#mixed-scalar-jet-4-and-ace-comparison) records
inputs, reproduction commands, hash and limitations. The Java comparison helper
now tags GUIDs canonically. Generated binaries and the JAR remain outside Git/CI.

The current Rust baseline remains 197 passing workspace tests/doc tests plus the
recorded lint/release/doc checks; no Rust code changed in this comparison milestone.
Java helpers ran locally on Temurin 21.0.8. This adds independent-writer Jet 4
evidence, not native Access/provider or other-platform qualification.

## Retrieval completion and allocation hardening

A new regression reproduced cancellation inside the final filter comparison
returning a successful empty result. Execution now checks cancellation/work before
publishing the completed result. Parameter-reference/output-column containers,
the implicit source row and ungrouped aggregate container now consume the shared
byte allowance with fallible reservation. Outer joins build NULL padding directly
inside the charged output row instead of allocating unchecked temporary vectors.
Default query results are unchanged; small budgets may now reject work previously
omitted from accounting. Sort comparisons and aggregate errors remain fallible.

Local macOS / Rust 1.99.0 checks passed 197 workspace tests/doc tests, minimal query
check, workspace Clippy, release build and warning-denied API docs. Standalone
expression/core feature checks passed at the preceding unchanged-scalar milestone.
Native Access, other targets and remote CI remain unrun.

## Opaque OLE and shared long-value storage

Native OLE fields now expose bounded opaque Binary through the same checked
inline/single/chained long-value storage as MEMO. Text decoding and its decoded
UTF-8 ceiling remain separate. No content is activated. Synthetic row tests cover
raw/empty/NULL bytes and a decoded-text expansion regression protects the existing
MEMO limit. [NATIVE_TABLES.md](NATIVE_TABLES.md#opaque-long-binary--ole) records the
four-row independent comparison, including a 12,000-byte chain, and the complete
146-row MSysObjects comparison: non-date cells match exactly; fractional calendar
conversion differs by at most 313 ns. CLI Count over MSysObjects now returns 146.
No dependency or generated binary fixture was added.

Local macOS / Rust 1.99.0 checks passed 195 workspace tests/doc tests, minimal
package checks, workspace Clippy, standalone minimal/all-feature expression and
all-feature core lint/tests, release build and warning-denied API docs. Original
fixture SHA-256 is unchanged. Native Access, other targets and remote CI were not run.

## Bounded I/O failure handling

Header/page reads now charge work and check cancellation for every read attempt,
including interrupted retries and short reads. This closes an unbounded retry path
inside the standard exact-read helper. Host callbacks still own individual I/O
timeouts. EOF and other failures remain typed and omit source messages.
[NATIVE_READING.md](NATIVE_READING.md#io-failure-and-retry-boundaries) records the
boundary and tests. Fault injection covers every read/seek in scalar and Long Text
snapshots; adapter tests cover no partial publication, explicit recovery and
cumulative limits on repeated failures. No dependency was added.

Local macOS / Rust 1.99.0 checks passed 192 workspace tests/doc tests, minimal
package checks, workspace Clippy, release build and warning-denied API docs.
Other platforms, remote CI and Microsoft Access were not run.

## Exact native Decimal reading

Qualified 17-byte native NUMERIC fields now decode into validated exact Decimal
values, preserving declared scale and checking precision before conversion. Native
precision/scale metadata is exposed separately from shared scalar carriers. Golden
byte-order and malformed-input tests plus actual row tests cover the codec. The
[Decimal profile](NATIVE_TABLES.md#exact-native-decimal) records unsupported layouts,
negative-zero semantics and the independent four-row Jackcess comparison, which
matched coefficients/scales exactly. CLI projection/filtering/ordering was checked.
No new dependency, arithmetic implementation or generated binary fixture was added.

Local macOS / Rust 1.99.0 checks passed 186 workspace tests/doc tests, workspace
Clippy, release build and warning-denied API docs. Original fixture bytes remain
unchanged. Microsoft Access, other targets and remote CI were not run.

## GUID decoding and complete scalar output

Native GUID fields now decode through `uuid`'s mixed-endian conversion and support
projection, equality/inequality, literals and explicit GUID parameters. Nil/NULL
stay distinct; native GUID ordering remains unqualified and unsupported when
required. Core semantic IDs remain unrelated to UUID generation. Binary values
already supported by the reader now have bounded hexadecimal CLI output.
[NATIVE_TABLES.md](NATIVE_TABLES.md#guid-and-binary-output) and dependency records
capture accepted behavior and the minimal dependency selection.

A temporary independent Jackcess-generated three-row table verified known/nil/NULL
GUID and nonempty/empty/NULL Binary output. A static native byte-order golden test
and CLI parser/equality/ordering/parameter/source tests cover the integration.
No Microsoft provider, RNG, GUI/network runtime or active content is introduced.

Local macOS / Rust 1.99.0 checks passed 182 workspace tests/doc tests, workspace
Clippy, standalone minimal/all-feature expression lint/tests, release build and
warning-denied API docs. Original fixture bytes remain unchanged. Other targets,
remote CI and Microsoft Access execution were not run.

## Bounded Long Text reading

The native MEMO decoder now follows inline, single-row and chained LVAL storage
with explicit lengths, signatures, row flags, cycles, termination, cancellation
and allocation/work ceilings. NULL and empty text retain distinct native meanings.
Scalar-cell work now consumes the shared step budget. [NATIVE_TABLES.md](NATIVE_TABLES.md#bounded-long-text)
records precise accepted layouts, limits and independent evidence.

CLI queries read CMD_LINE_TB descriptions and USysRibbons XML as inert text. The
independent comparison expanded to 1,622 rows across eleven tables. Three native
codec tests include a new 256-case mutation property, and CLI tests verify the
registered copy's counts/XML without activating it. Unsupported-type tests now
use complex/GUID fields because Date/Time and Long Text are implemented. No new
dependency was introduced, and original fixture bytes remain unchanged.

Local macOS / Rust 1.99.0 checks passed 178 workspace tests/doc tests, workspace
Clippy, release build and warning-denied API docs. Scalar/parser feature checks
from the preceding date milestone remain applicable; no scalar code changed here.
Native Access, remote CI and other-platform runs were not performed.

## Legacy Date/Time end to end

The library-policy user edit was committed before continuing engine work. Native
Date/Time fields now retain validated raw DATE bits and participate in temporal
query comparisons, ordering, grouping and DISTINCT. ISO date literals/parameters
and explicit rounded calendar CLI output are implemented. [NATIVE_TABLES.md](NATIVE_TABLES.md#legacy-datetime)
records precise semantics, unsupported coercions and comparison evidence.

Core calendar validation/conversion uses the reviewed `time` library; SQL-enabled
expressions use its static parsing formats. Manual extended-calendar validation
was replaced with checked library APIs. [Dependency records](DEPENDENCIES.md)
record the six added packages/features/licenses/MSRVs and unsafe-code boundary.
Independent Jackcess agreement now covers 1,606 rows across nine tables. The CLI
returns dated CUM_VAL_TB rows (starting 1986-06-01) and accepts typed date parameters.
Native Access and other-platform execution remain unobserved.

Local macOS / Rust 1.99.0 validation passed 174 workspace tests/doc tests,
workspace Clippy, standalone expression minimal checks, expression/core all-feature
lint/tests, release build and warning-denied API docs. Tests include invalid native
NaN dates, equivalent/negative DATE representations, calendar endpoints, literals,
parameters, output and failures on empty sources. The fixture hash is unchanged.

## Numeric-contract cleanup

A workspace source audit replaced unexplained native offsets/masks/tags/page sizes,
query/binding/evaluation limits, Pratt binding powers, CLI budgets and scalar
scale/calendar/identity widths with names or type-derived widths. Format facts and
structural policies remain separately owned; constants follow all imports.
CLI native inspection/retrieval share one explicit host-budget/scope policy.
Public limit/carrier constants expose existing contracts; their values, validation,
accepted SQL/native profiles and output remain unchanged. Independent fixture
bytes and ordinary arithmetic/test inputs remain explicit regression evidence.
No dependency or workspace-package change was introduced.

Local macOS / Rust 1.99.0 checks passed workspace Clippy and 169 tests/doc tests,
standalone schema/query/model/ACE/adapter checks, expression minimal/all-feature
and core all-feature lint/tests, release build, warning-denied rustdoc, formatting
and whitespace. Import ordering was checked across changed Rust modules and the
original fixture hash is unchanged. Native Access comparison, other targets and
remote CI were not rerun for this refactor.

## Native scalar retrieval CLI

`tablecraft-access` is a working storage adapter over ACE snapshots and the existing
query Source interface; the workspace now has nine packages. ACE and query retain
their independent dependency boundaries. The CLI's `query run --database PATH`
opens only that explicit stable/offline source read-only and uses the existing
retrieval/expression engine. [NATIVE_QUERIES.md](NATIVE_QUERIES.md) records supported
formats/carriers, eager table loading, caching, budgets and failure behavior.
Without --database, inline retrieval is unchanged. No active content or writes run.

Three adapter tests cover real query execution, caching/read counts, immediate
visitor rejection, unsupported/missing and empty tables, limits and cached-scan
cancellation. Three new CLI tests cover native TOP/filter/parameter/aggregate,
self-join/empty/text-collation and payload-free error results with unchanged copies.
The existing help test now describes explicit --database capability.

Local macOS / Rust 1.99.0 verification passed all standalone package checks,
workspace Clippy and 169 tests/doc tests, expression minimal/all-feature and core
all-feature lint/tests, release build, warning-denied rustdoc, formatting and
whitespace. The release binary's documented native TOP example matched exactly.
Dependency metadata verified nine packages, independent engine/query closures and
no GUI/network/provider runtime in the CLI. Public links across 33 Markdown files
and unchanged original/comparison-copy fixture SHA-256 passed. Eighteen ACE tests
include three 256-case fixed-seed properties. Native Access, remote CI and
Windows/Linux execution were not observed.

## Native scalar row snapshots

ACE's `read_table` now returns a bounded complete snapshot over the supported
scalar schema profile. [NATIVE_TABLES.md](NATIVE_TABLES.md) describes exact
carriers, NULL/empty/Boolean handling, physical order, structural rejection and
shared budget/cancellation behavior. No registry dependency was added.

Seventeen ACE tests passed locally, including new fixed/variable row fixtures,
exact representation/NULL/empty tests, unsupported/calculated/corrupt inputs,
mid-operation cancellation and a 256-case mutation property. An external Rust
consumer matched Jackcess 5.0.4 for 1,488 rows across seven explicitly named
candidate tables; format-specific source review and normalized result hash are
recorded in NATIVE_TABLES.md. Original fixture bytes remain unchanged. The initial
row API initially had no native query adapter; the adapter milestone above adds
that consumer. No write path or Microsoft Access qualification was added.
ACE Clippy/tests passed on macOS / Rust 1.99.0; other targets were not observed.

## Native table schemas

ACE now exposes bounded local-table schema inspection through `inspect_table_schema`
and CLI `inspect --table NAME`. [NATIVE_TABLES.md](NATIVE_TABLES.md) records the
implemented native metadata and shared Column mapping, unsupported types and
conservative nullability policy. Shared page/layout/Unicode code remains internal;
catalog-specific validation is separate. No registry dependency was added.

All 36 local-table schemas / 187 columns matched Jackcess 5.0.4 for name, native
type, normalized byte length and order on a disposable candidate copy. No active
content or table rows were evaluated. Original fixture bytes are unchanged.
Focused ACE/CLI Clippy and tests passed locally on macOS / Rust 1.99.0.
Native Access comparison and remote CI were not run.

## Shared object model and native catalog inspection

`tablecraft-model` now provides scoped object descriptors and validated read-only
catalog lookup, with real consumers in `tablecraft-ace` and the CLI. [MODEL.md](MODEL.md)
records its identity/name/category/parent contracts and ownership boundaries.
The workspace has eight packages. No placeholder application models were added.

`inspect --catalog` reads five selected MSysObjects fields through dynamic column
metadata, owned-page allocation maps and bounded relocation following. It preserves
unknown native types and unresolved parent references, rejects unsupported/malformed
structures explicitly, and buffers escaped output before stdout. Native origins
stay in ACE rather than entering the shared model. [NATIVE_READING.md](NATIVE_READING.md)
records format profiles, limits, pinned research and the manual comparison recipe.

A disposable copy of the registered candidate yielded 146 metadata records; all
146 `(Id, ParentId, Type, Flags, Name)` tuples matched a read-only Jackcess 5.0.4
scan exactly. The original SHA-256 remains unchanged. No active content, saved
query, linked source or object body was executed/interpreted. This is independent
reader evidence, not a Microsoft Access oracle result. General table data, object
contents, edits, locking and native application execution remain unavailable.

Local verification on aarch64-apple-darwin / Rust 1.99.0 passed standalone
model/ACE/schema/query checks, minimal workspace Clippy and 154 tests/doc tests,
expression minimal/all-feature and core all-feature lint/tests, release build,
warning-denied rustdoc, formatting and whitespace checks. ACE has ten tests,
including two 256-case fixed-seed properties; model has two focused tests and a
runnable public doctest; the CLI has 22 integration tests. The release catalog
command produced the expected 146 rows. Public links across 31 Markdown files,
eight-package metadata and the fixture hashes were checked.

An unrelated temporary Rust package outside the workspace also consumed ACE/core/
model, decoded all 146 objects and resolved AutoKeys as a Macro. Its normal
closure contained only these foundation/engine libraries and thiserror's macro
closure, without CLI, SQL, GUI, network, Java or provider dependencies. No new
registry dependency was added to TableCraft. Native Access, remote CI, Windows
and Linux execution were not observed.

## Initial native header inspection

`tablecraft-ace` now performs bounded read-only header inspection independently
of application packages. The CLI exposes it through `tablecraft inspect <path>`.
[NATIVE_READING.md](NATIVE_READING.md) records exact supported fields, limits and
pinned MDB Tools/Jackcess research references. A copy of the registered example
reports Ace2007/subversion 0, 4 KiB pages and 827 pages with a zero encoding-key
field. No active content ran and the original fixture remains unchanged. Native
catalog/records, writes, locking, runtime execution and Access oracle evidence
were unimplemented at that header-only milestone. The workspace then had seven packages.

Local macOS/Rust 1.99.0 verification passed workspace Clippy, native/CLI tests,
formatting, whitespace and public local links. Three engine tests include a
256-case fixed-seed arbitrary-prefix property; two new CLI tests cover read-only
output and payload-free failures. The copy's observed header was checked and the
original SHA-256 is unchanged. Native Access comparison was not run.

## Shared schema ownership

[SCHEMA.md](SCHEMA.md) records the extracted SchemaName, TypeDescriptor and explicit
SchemaPolicy/NameComparison contracts. Columns, memory table identities and bound
parameters now share validated names. Fields and parameters share declared Text
constraints and exact carrier checks; nullability and runtime resource/finiteness
checks remain separate. SQL type-spelling conversion stays in the query adapter.

Generic schema construction no longer imposes the query profile's 255-byte names
and 256-column ceilings. Query binding and MemoryTable still enforce those same
limits through an explicit query-owned policy. The standalone validate_columns
API now requires an explicit comparison policy. Column::new retains its carrier
constructor, while Column::with_type supports reusable declared constraints.
No new crate, dependency, Serde support, storage behavior or native claim was added.

Verification on aarch64-apple-darwin with Rust 1.99.0 on 18 October 2026 passed
workspace minimal Clippy/tests, standalone schema/query checks, the release build,
rustdoc with warnings denied, formatting and whitespace checks. Five schema tests,
the schema API doctest and three new query/schema integration tests cover the
extracted contracts; existing query/property/CLI tests also passed. The Unicode
Text parameter CLI example returned its expected result. Local links across 29
public Markdown files, unchanged six-package metadata/schema dependency boundary
and the original fixture hash passed. Other targets and native Access comparison
were not run for this refactor.

## Current retrieval milestone

`tablecraft-schema` and `tablecraft-query` now provide validated scalar relation
contracts, explicit read-only sources, prepared slot binding and bounded retrieval
execution. [QUERY_EXECUTION.md](QUERY_EXECUTION.md) defines the implemented profile:
projection/filter/order, joins, grouping/aggregates, DISTINCT/TOP/UNION, parameters
and derived/correlated retrieval subqueries. The workspace has six packages. No
native file source, write execution, full Access expression profile or native
comparison is implemented. Later sections retain evidence from earlier milestones.

Local verification on aarch64-apple-darwin with Rust 1.99.0 on 18 October 2026
passed workspace minimal Clippy/tests, standalone expression minimal/all-feature
Clippy/tests, serde-enabled core Clippy/tests, release build and rustdoc with
warnings denied. The new suites have 14 retrieval acceptance tests, three
fixed-seed properties (256 cases each), one schema test and a runnable public
API doctest. Public local links, formatting, diff checks and the original fixture
hash passed. No Windows/Linux, file-backed source or native Access comparison
was run. CLI retrieval integration is recorded separately below.


## Inline retrieval CLI

`tablecraft query run` now consumes prepared retrieval over an empty host catalog,
with inline SELECT/UNION/derived sources, filtering/sorting, joins, grouping and
subqueries. Repeated typed `--param` arguments validate exact declared carriers;
`--collation ordinal` selects the explicit non-locale text profile. The shared
formatter emits one header and zero or more rows under its 1 MiB logical ceiling.
Execution and formatting finish before stdout; errors exit 2 without partial
results or query/value payloads. Parse/eval behavior is unchanged.

No native file/source adapter, VBA, GUI/network runtime or new registry dependency
was added. CI checks standalone scalar-schema and query builds using the existing
version-tagged actions. The CLI suites now have 18 tests, including seven new
retrieval tests for queries, bindings/collation, failures and output exhaustion.

Local verification on aarch64-apple-darwin on 18 October 2026 passed standalone
schema/query checks, minimal workspace Clippy/tests, the release build, rustdoc
with warnings denied, formatting and whitespace checks. Both documented examples
ran against the release executable with exact expected output. Public local links
across 28 Markdown files, six-package metadata, the normal CLI dependency closure
and the original ACCDB hash were checked. No new runtime dependency enters the
CLI. Native Access comparison, remote CI and other targets were not observed.

## Domain-oriented workspace

The workspace now groups `tablecraft-core` under `crates/foundation/` and the
SQL parser under `crates/query/tablecraft-sql`. The parser was renamed from
`tablecraft-query` to `tablecraft-sql`, including Rust imports, its CLI dependency,
lockfile and CI package selection. Parsing APIs and CLI behavior are unchanged.
The existing feature modules, tests and proptest regression seed moved with their
owning packages. No unused packages or dependencies were added.

[ARCHITECTURE.md](ARCHITECTURE.md) records the implemented package layout,
the target domain map and boundaries for progressive extraction. The future
`tablecraft-query` package is reserved for binding, planning and execution;
`tablecraft-expressions` now implements the initial scalar profile described
below. The planner was absent at that relocation milestone; the bounded retrieval
implementation is now described above. The private architecture
pack remains ignored.

Local verification on aarch64-apple-darwin with Rust 1.99.0 on 17 October 2026
passed locked/offline standalone core and SQL checks, minimal workspace
Clippy/tests, serde-enabled core Clippy/tests, the release build and rustdoc with
warnings denied. Formatting, CLI version/parsing smoke checks and local public
documentation links passed. The suites retain 45 SQL tests plus its doctest,
five CLI tests, and the existing core contracts and optional serde tests. All 71
relocated files retain their contents except for the intended parser manifest
and doctest import rename. Remote CI and other native targets were not observed.

## Headless build foundation

Implemented the minimal Cargo workspace, pinned Rust toolchain, clap-based
headless CLI, build/feature/licensing policy and three-platform CI configuration.
The MIT license is preserved.

The CLI implements help/version, syntax-only `query parse` and source-free scalar
`query eval`. Integration tests launch it
without display environment variables and check output streams, version identity
and rejection of unsupported commands/extra arguments.

Local verification passed on aarch64-apple-darwin with Rust 1.99.0 on 17 October 2026:

- rustfmt check and Clippy with warnings denied.
- All three CLI integration tests (help, version and invalid arguments).
- Locked, offline release build and executable version smoke check.
- Dependency tree contains clap and its derive dependencies; the CLI remains
  free of graphics, network and database runtimes. Versions, licenses, features
  and minimum toolchains are recorded in DEPENDENCIES.md.
- Git whitespace check.

Remote CI and native Windows/Linux execution have not yet been observed.
Database inspection/editing commands and a desktop host do not exist yet.

## Shared core types

`crates/foundation/tablecraft-core` implements distinct object/table/operation/cursor IDs,
table- and generation-scoped record locators, cursor bookmarks, scalar values,
typed validation failures and checked resource budgets. Errors use `thiserror`;
optional `serde` support is limited to scalar value exchange. The CLI consumes
values and resource budgets for scalar evaluation.

Exact Currency, 28-digit field Decimal, width-preserving integers, bit-preserving
floats, GUIDs and distinct legacy/extended timezone-free dates are implemented.
NULL stays separate from empty text, and conversion rejects expression-only
Empty/Missing/Error states. Value and locator diagnostic output redacts payloads.
[CORE_TYPES.md](CORE_TYPES.md) maps the implemented contracts to Microsoft primary
references and records source inconsistencies and remaining boundaries.

Local core verification on aarch64-apple-darwin with Rust 1.99.0 on 17 October
2026 includes 17 feature-local unit/property tests, three public-API contract
tests, four compile-fail doctests and the three existing CLI tests. With the
optional serde feature, seven additional serialization tests check typed round
trips and rejection of invalid constructor inputs. Eight proptest properties
across both configurations use 256 cases each with a fixed seed and regression
persistence. The exhaustive small-coefficient decimal sweep remains in place.
Formatting, Clippy in both feature configurations, minimal and serde-enabled
release builds, and rustdoc with warnings denied also passed locally.

Serde preserves scalar integer/bit representations without JSON-specific string
policies. JSON is a test-only format, with direct typed decoding for wide numeric
values. IDs, generations, expression states, locators, budgets and errors have
no serde implementations. Native codecs, production serialization protocols,
Access oracle comparison and native database query execution remain unimplemented.

The user-requested future library choices and decimal candidate assessment are
recorded in [LIBRARY_POLICY.md](LIBRARY_POLICY.md); those future libraries have
not been added as unused dependencies.

## Access data-query parser and owned model

`tablecraft-sql` implements `parse_query` for retrieval, parameter declarations,
unions, crosstabs, append/update/delete, make-table and schema queries. `parse_select`
remains a single-SELECT convenience entry point. Expressions, clauses, relations,
parameter/type syntax and statement variants are TableCraft-owned; upstream
parser/token types do not escape the adapter. Source is retained independently
from normalized SQL. See [SQL_PARSER.md](SQL_PARSER.md) for precise grammar,
limits, reference mapping and unsupported forms.

The Access Pratt grammar implements operator precedence explicitly, including
ampersand, integer division, Mod, Eqv and Imp. Numeric/date literals retain syntax
without floating conversion or calendar/locale interpretation. Function calls
preserve qualified names and wildcard/omitted arguments; GUID literals and
@@IDENTITY have distinct syntax nodes; LIKE keeps pattern syntax without assuming
wildcard mode. Joins, ordered UNION ALL modifiers, subqueries, fixed pivot headings
and external source/destination metadata survive round trips. Make-table queries
are rejected inside retrieval operands and distinguished as root action syntax.
CREATE/ALTER TABLE, indexes and named/inline constraints retain syntax without
applying schema changes. Saved views/procedures retain ordered parameter headers
and one parsed body; procedure recursion shares the parser budget. Identifier
classification uses Access SQL reserved words instead of upstream multi-dialect
keyword status, including ordinary ID/Ref names. No SQL is executed, and external
source strings are never opened.

Local parser verification on aarch64-apple-darwin with Rust 1.99.0 on 17 October
2026 passed formatting, Clippy with warnings denied, 45 query tests and the API
doctest. Tests cover each query family, structure/precedence, original source,
normalization, malformed/foreign rejection and bounded recursion across nested
queries. Six fixed-seed properties use 256 cases each with regression persistence.
The grammar uses SQLParser std only; recursive-protection and native stack helpers
were removed after moving expression grammar behind an explicit shared budget.

The CLI integrates the parser through clap Parser/Subcommand derives and
`tablecraft query parse "<SQL>"`. Integration tests check normalized output,
parameter/action syntax without execution, malformed/multiple-query rejection,
byte limits, help/version and invalid arguments. All five parse/help/version CLI tests pass locally.
The CLI dependency closure remains free of GUI, network and database runtimes.

The final local workspace checks also passed minimal workspace tests/Clippy,
serde-enabled core tests/Clippy (including seven serde round trips), the release
build, rustdoc with warnings denied, CLI version output, local public-document
links and diff whitespace checks. These checks cover this checkout on macOS.

Binding, schema/type validation, function evaluation, collation, wildcard matching,
native query execution and ACCDB query extraction remain unimplemented. No native Access
comparison or Windows/Linux execution evidence is available for this slice.

## Initial scalar expression semantics

`tablecraft-expressions` implements Boolean/NULL logic, exact numeric comparison,
explicit half-even integral coercion, checked Int64/Double arithmetic, exact
Currency addition/subtraction and multiplication by integers, and bounded
Text/NULL concatenation. Its direct scalar functions consume core values;
they do not use representational equality as SQL equality. Text comparisons
require an explicit collation. The supported carrier profile and deferred
Variant/locale/Decimal behavior are in [EXPRESSIONS.md](EXPRESSIONS.md).

Local formatting, Clippy with warnings denied and 12 scalar tests pass on
aarch64-apple-darwin with Rust 1.99.0 on 17 October 2026. Four properties use 256
fixed-seed cases each. No native Access comparison or query execution is claimed.
The package reuses core, thiserror and development-only proptest; no new registry
dependencies, serialization or runtime libraries were introduced.

The optional `sql` feature now evaluates supported owned expression trees over
host-supplied borrowed values. It supports arithmetic/comparison/logical/text
operators and IS NULL, with explicit unsupported errors for calls, subqueries and
other deferred forms. Binary operands evaluate eagerly in written order. The
evaluator checks cancellation per node, charges core step/Text-allocation budgets
before work, and enforces nesting and per-value byte limits. Its numeric literal
carrier choices are explicit, without claiming full native type inference.

With `sql`, nine more tests (one generated property) and an API doctest pass;
default and SQL-enabled Clippy checks pass with warnings denied. See
[EXPRESSIONS.md](EXPRESSIONS.md) for reference mapping and current limitations.

Final local checks passed locked/offline minimal workspace Clippy/tests,
SQL-enabled expression Clippy/tests, serde-enabled core Clippy/tests, the minimal
release workspace build, rustdoc with warnings denied, CLI version output,
formatting and public-document link/whitespace checks. Dependency-tree inspection
confirms that the default expression package excludes SQLParser/log. The CLI's
subsequent evaluator integration is described below. Other platforms and native
Access evaluation remain unverified.

## CLI scalar evaluation

`tablecraft query eval "<SELECT>"` now invokes the existing evaluator for one
source-free SELECT's scalar projections. Optional aliases label a two-line,
tab-separated result table; Text is quoted/escaped and NULL remains distinct
from empty text. The command rejects database sources, query clauses, parameters,
UNION, action/definition statements and unsupported expressions. It supplies no
field bindings or text collation. `query parse` retains its syntax-only behavior.

One shared budget covers all columns, with 1,024 steps and 65,536 cumulative
Text bytes; depth, per-value limits and the parser input ceilings still apply.
Output is buffered with a 1 MiB logical cap and fallible allocation. Any parse,
validation, evaluation or formatting failure exits 2 without partial stdout.
Projection errors identify the column index without exposing values or SQL.
No query clause is ignored or external content opened.

The CLI now consumes core values/budgets and the expression crate's `sql` feature.
No new registry dependencies were introduced. CI checks both standalone minimal
expressions and the SQL-enabled profile explicitly, since the CLI enables SQL
evaluation in the workspace's unified feature set. CLI command handling and output
policy live in feature modules; library semantics remain in the expression crate.

All 11 CLI integration tests pass locally: the five existing tests and six eval
tests covering results, NULL/text formatting, controls, unsupported query clauses,
eager errors, limits, diagnostics, help and absence of partial output.
The documented arithmetic/concatenation/NULL example was also run successfully.

Local verification on aarch64-apple-darwin with Rust 1.99.0 on 17 October 2026
also passed formatting, standalone minimal and SQL-enabled expression
Clippy/tests, workspace Clippy/tests, serde-enabled core Clippy/tests, release
build, rustdoc with warnings denied, CLI version output and public-document
links/whitespace checks. Dependency inspection confirms the CLI adds only the
existing local core/expression packages, with no Serde, GUI, async/network or
database runtime. Remote CI, other targets and native Access comparison were
not observed.

## Standalone native engine architecture

[NATIVE_ENGINE.md](NATIVE_ENGINE.md) now makes standalone Rust engine usability
an architectural requirement. The target map adds `engines/tablecraft-ace` and
a later `tablecraft-ace-ffi`, while `tablecraft-access` becomes the TableCraft
adapter. Reusable query components and backend-neutral contracts must not import
that adapter, application services or runtime; hosts compose concrete adapters.
The document separates Rust-first implementation, later C ABI/ODBC, engine scope,
publication requirements and native compatibility evidence.

This is a documentation/guidance change. The current four-package workspace is
unchanged. No engine, adapter, FFI, driver or public transaction API has been
implemented or published. The proposal's positional-parameter example is not a
working API; the current SQL parser still rejects positional `?` placeholders.
Private plans and the original ACCDB fixture remain untouched.

Documentation verification passed local links across 14 public documents, target
map counts (33 distinct package names across 13 domains), acyclic proposed
dependency arrows, rustfmt and whitespace checks. Locked/offline Cargo metadata
still lists the same four implemented packages. Rust runtime tests/builds were
not rerun for this documentation-only change; it introduces no native execution
or compatibility evidence.

## VBA and macro architecture

[VBA_COMPATIBILITY.md](VBA_COMPATIBILITY.md) adopts preservation, documented
execution and native authoring as separate compatibility tracks. The target map
adds `tablecraft-vba` and `tablecraft-macros`, with `tablecraft-automation` narrowed
to reusable host contracts. It records interpreter-first language work, project
metadata/unknown-stream preservation, module absence, DAO/DoCmd and form/report
events, cancellation/transaction boundaries, SQL callable injection and debugger
ownership. Required native data macros must work headlessly or affected writes
must fail explicitly. Execution authorization remains separate from preservation.

Microsoft primary references were reviewed on 18 October 2026 and linked in the
owning document. These are requirements and research inputs, not implemented
VBA, macros, module discovery/writing, DAO objects, callable registry or execution
evidence. The current four-package Cargo workspace, runtime behavior and original
fixture are unchanged; no interpreter dependencies or empty packages were added.

## Windows compatibility architecture

[WINDOWS_COMPATIBILITY.md](WINDOWS_COMPATIBILITY.md) adds the planned
`compatibility/tablecraft-wincompat` product boundary, distinct from engineering
oracle tooling. It records per-API Rust/emulated/native/service providers through
shared automation contracts, with VBA independent of concrete providers. Mechanism,
coverage and authorization are separate facts; partial, blocked and unsupported
calls cannot return fabricated success or retry around a denial.

The document covers inert dependency discovery, object dispatch, explicit virtual
paths, filesystem/environment/registry boundaries, qualified Declare signatures,
optional bridges, compatibility reporting and WIN-0 through WIN-5 acceptance.
It separates replicated data validation from replayed UI events/external side
effects, without allowing required data macros to be skipped. Microsoft primary
references were reviewed on 18 October 2026 and linked in the owning document.

No provider, COM/Win32 bridge, DLL loader, isolation helper or new runtime/library
dependency was implemented. The four working packages, CLI behavior, fixtures
and private planning material are unchanged. VBA and Windows tracks are design
requirements, not passing native or cross-platform compatibility evidence.

Documentation verification on 18 October 2026 passed local links across 16 public
documents, the target map (36 distinct package names in 14 domains), acyclic
arrows in all three engine/VBA/provider dependency diagrams, rustfmt and whitespace
checks. Locked/offline Cargo metadata still lists the same four working packages.
Rust runtime tests/builds and native comparisons were not rerun for these
documentation-only changes.

## Application compatibility contracts and corpus policy

The application review now has explicit future contracts for startup/session
state, imported Ribbon XML, DAO/ADO recordsets, advanced field behavior and native
distribution/signatures, linked from [ARCHITECTURE.md](ARCHITECTURE.md). Microsoft
primary references were reviewed on 18 October 2026, including build-qualified
form/report limits and the distinction between code and package signatures.

[ACCESS_APPLICATIONS.md](testing/ACCESS_APPLICATIONS.md) defines separate
open/preserve/execute/edit/save-and-reopen results, provenance/privacy handling,
oracle profiles and representative application selection. The existing MIT
example is its sole candidate, with every native operation unrun. No application
harness, native format support, new fixture, crate or dependency was added. Private
planning and the original fixture bytes remain unchanged.

Documentation verification passed local links across 22 public Markdown files,
the unchanged 36-package/14-domain target map, locked/offline metadata for the
four actual packages, original fixture SHA-256, rustfmt and whitespace checks.
Rust runtime suites and native Access comparisons were not rerun for this
documentation-only change.

## Application workflow priorities

[ROADMAP.md](ROADMAP.md) records P0–P4 priorities and bounded native inspection as
the next native implementation slice. Feature contracts cover reusable expression
editing, saved import/export versus linked tasks, dependency/refactoring analysis,
Documenter/Compare/search/application tests and templates/wizards. The roadmap
also deepens forms/reports, linked-source, native shared-file laboratory, P2P UX
and browser requirements, with explicit scope limits. Existing engine/VBA/Windows
documents remain canonical; the engine document adds public API and ODBC
qualification requirements.

This is documentation only. The parser and initial scalar evaluator already work;
visual editing, application tools and native database features remain future work.
There are no new crates, dependencies, application test runs or fixture changes.

Verification passed links across 27 public Markdown files, three acyclic proposed
dependency diagrams, the unchanged target map and four-package Cargo metadata,
fixture SHA-256, rustfmt and diff whitespace checks. Runtime suites/builds,
diagram rendering and native Access comparisons were not run for this doc-only
revision.

## Access example

The supplied [Access example](examples/Access_Example_VBA.accdb) is registered in
[FIXTURES.md](FIXTURES.md) with its
size and SHA-256. It matches upstream Access-projects/Access-examples commit
`d0ffd681d431f38d91c138559763c30df0699c55` byte for byte, and its original MIT
notice is retained. It has not been parsed, executed or qualified against Access.

Real Access comparison still requires a separately provisioned Windows oracle.
The fixture has not established native format or application compatibility.
