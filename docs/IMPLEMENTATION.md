# Implementation status

## Native saved parameter declarations and complete query model

`SavedSelect::query()` now returns the complete owned Query, retaining ordered
declarations and the SELECT retrieval statement. Final sorting/owner-access use
the same outer ReadQuery fields as SQL parsing. The parser-created statement
carrier receives existing owned syntax by move; no new unchecked Box allocation
or AST clone is introduced. Original native identity/fragments remain separate.

Twelve qualified type flags, literal ASCII prompts/names and nullable/zero/positive
Text syntax lengths work. Missing/unknown types, unhandled fields, negative lengths,
duplicate names, unsupported name forms and missing/duplicate/reversed ordering
fail without defaults. The declaration ceiling is 255 under existing SQL policy;
explicit budgets and cancellation remain cumulative. Variant/Memo/newer type tags,
qualified/Unicode name matching and native runtime-value limits remain gaps.

Four new adapter tests cover mapping/model equality, explicit memory-only carrier
binding, unsupported metadata, the 255/256 boundary, budgets and late cancellation.
One CLI test prints declarations without values or prompts, rejects an unknown
type, verifies private diagnostics and preserves input bytes. An original recipe
matches twenty-one models/eight parameterized definitions per V2016/V2019 family,
with twelve type tags and eight inspectable-but-rejected mutations each. The
[contract and provenance](NATIVE_SAVED_QUERIES.md#native-parameter-declarations)
separate syntax evidence from native Access/runtime qualification. All ACE writes
stay blocked and roadmap 1–9 remain unfinished.

All 590 workspace tests/doc tests and 304 ACE all-feature tests pass, alongside
both all-target Clippy profiles, release CLI/example builds, warnings-denied SQL/
Access docs, dependency policy/tests and documentation checks. The earlier SELECT
and join comparison corpora also pass with unchanged canonical hashes after the
complete-model change. No registry dependencies or features were added.

## Access IEEE floating type spellings

The SQL parser now recognizes IEEESINGLE and IEEEDOUBLE in parameter/DDL syntax,
preserving their distinct uppercase spellings. Existing parameter binding already
mapped them to Single/Double, but those branches were unreachable from SQL text.
One syntax round-trip test and one execution test cover both names, exact carrier
bits and rejection of a Double supplied for Single. The Microsoft type comparison
linked in [SQL parser references](SQL_PARSER.md) supplies the synonym facts.

All 585 workspace tests/doc tests, all-target workspace Clippy, release CLI and
dependency/documentation checks pass. No dependencies or native storage behavior
changed; native saved-parameter reconstruction remains the next adapter task.

## Qualified native two-source saved joins

The Access saved SELECT adapter now accepts exactly two declared named sources
connected by one INNER/LEFT/RIGHT fragment. Alias/name endpoints determine left/
right orientation independently of table storage order. ASCII case-insensitive
labels, distinct self-join aliases and Unicode table names with ASCII aliases
work. Missing/ambiguous endpoints fail; compound/larger graphs, Cartesian source
lists and non-ASCII label comparison remain explicit reconstruction gaps.

Conditions parse independently before the bounded source parser constructs the
owned Join node. The single root, kind and condition are checked before moving
its source vector into the model. Shared expression adaptation now also requires
stored and guard-enclosed text to parse to the same owned expression: delimiters
cannot repair malformed stored parentheses. Both parses charge existing allowances;
row/work/allocation limits and cancellation remain cumulative. No AST is cloned
and no query/source/function is bound or executed.

Three additional focused tests exercise kinds/orientations, self/Unicode sources,
missing/ambiguous/unsupported metadata, expression boundaries, budgets and late
cancellation. An original fixture mode adds nine joins to thirteen baseline
SELECTs. Independent V2016/V2019 comparisons match all twenty-two models through
both hosts; eight mutations/family retain inert inspectability but fail reconstruction
without stdout. [Contract and evidence](NATIVE_SAVED_QUERIES.md#two-source-native-joins)
separate these results from native Access and execution qualification.

All 583 workspace tests/doc tests, all-target workspace Clippy, release CLI/example
builds, warnings-denied Access docs and dependency/documentation checks pass.
The original thirteen-SELECT comparison also passes after the shared expression
boundary change, with unchanged hashes. No dependencies were added. All ACE writes
remain blocked and roadmap 1–9 remain unfinished.

## Saved SELECT CLI

`query saved --database PATH NAME` now composes the inert native definition reader
and qualified Access SELECT adapter. It opens only the explicit stable/offline
copy read-only, shares the native budget across both operations and buffers
normalized SQL under the common 1 MiB output ceiling. It does not bind functions,
expand saved sources, activate providers or execute SQL. Query dispatch parses
SQL only for commands that accept SQL text. No CLI or registry dependency was added.

Four additional integration tests cover normalized syntax with unresolved calls,
required arguments and rejected execution/write options, private verbose
diagnostics, namespace/unsupported/late-corruption rejection, unchanged input and
the registered example's quoted leading-digit source. The independent runner now
checks both the engineering host and public command for thirteen original SELECTs
and eight rejected corruption copies per V2016/V2019 family, plus the separately
qualified example identity regression. Canonical hashes remain unchanged.

All 580 workspace tests/doc tests, all-target workspace Clippy, release CLI and
local dependency/documentation checks pass. ACE all-feature qualification remains
304 tests from the preceding adapter chunk; this chunk changes only the host and
comparison runner. Native joins/parameters/external fragments and broader saved
query families remain unsupported for reconstruction. All ACE writes stay blocked;
roadmap 1–9, native Access conformance and application save/reopen remain unfinished.

## Owned native saved SELECT syntax

`tablecraft-access::reconstruct_saved_select` consumes an explicitly read native
definition into an owned SELECT while retaining its original fragments/identity.
The qualified zero/one-source profile preserves projections/aliases, wildcards,
WHERE, GROUP BY/HAVING, sort keys, DISTINCT/DISTINCTROW/TOP/PERCENT and owner-access
syntax. It rejects unhandled native metadata, actions, joins, parameters, external
source fragments, ambiguous singleton clauses and unsupported ordering/identifier
forms. Expression parsing preserves unresolved calls/subqueries without activation.
Reconstruction does not bind, execute, authorize owner-access or expand saved sources.

Strings/vectors reserve fallibly after budget checks; parser AST allowances and
clamped byte/token/depth limits bound adaptation. Fragment visits, cumulative
work/allocation and cancellation precede publication. Parenthesized expression
parsing keeps clause/list/statement syntax out of fragment boundaries; aliases
are parsed separately into owned names. A consuming SQL syntax accessor avoids
AST cloning. An engineering example buffers normalized output before stdout.

Seven integration tests include full model equality, separate flag semantics,
unknown/conflicting metadata, injection/identifier/order boundaries, budgets,
late cancellation, original example identity and a 128-case Unicode alias/literal
model. Independent V2016/V2019 recipes match thirteen reconstructed SELECTs and
reject eight corruption copies each. The existing example's reference renderer
uses a bare leading-digit name outside the SQL parser's current spelling profile;
that exact observation and an explicitly quoted expected identity are checked
separately. [Contract and provenance](NATIVE_SAVED_QUERIES.md) distinguish these
results from native Access conformance or application save/reopen.

All 576 workspace tests/doc tests and 304 ACE all-feature tests pass, alongside
both all-target Clippy profiles, release workspace/example builds, warnings-denied
workspace all-feature docs, dependency policy/tests and local documentation checks. Access now
uses the existing workspace SQL package as a normal dependency and existing
Proptest for generated tests; no registry versions/features were added. All ACE
writes stay blocked, SQLite expansion is paused and roadmap 1–9 remain unfinished.

## Inert native saved-query fragments

`read_saved_query_definition` now binds one explicit query through the scoped
catalog and matches its signed native ID against MSysQueries.ObjectId without
page masking or scalar coercion. Owned rows retain all eight known field carriers,
NULL/empty distinctions, unknown attributes, raw flags and native storage/order
bytes. Payloads move out of validated snapshots without cloning. Fallible
allocation, shared whole-system-table budgets and cancellation precede publication.
This inspects metadata without SQL reconstruction, validation, provider access or
execution; empty/partial/broken-index fragments are not labeled valid queries.

CLI `inspect --query NAME` buffers escaped/quoted fields using borrowed output
helpers and the existing output limit. It explicitly conflicts with every other
inspection mode, including combinations where Clap would suppress required-table
arguments. Table names do not become query definitions; saved-query names do not
become retrieval FROM sources. Verbose diagnostics remain payload-free.

Five native tests cover negative/scoped IDs, namespace/empty-definition behavior,
unknown/nullable values, carrier/geometry failures, unchanged input, budgets/
cancellation and a 128-case representation model. A retained seed guards an empty
synthetic usage map; a unit sweep round-trips all 256 attribute bytes. Three CLI
tests cover exact inert output, quoted NULL/empty fields, every conflicting mode,
private diagnostics and buffered late failures. Independent V2016/V2019 originals
match 11 definitions/75 fragments per family, plus 59 queries/332 fragments in the
unchanged registered ACE-code-2 example. Four inspection mutations and two buffered
failures/profile also pass. [Contract and evidence](NATIVE_SAVED_QUERIES.md) keep
index/header/query integrity, SQL reconstruction/execution, actual Access and
application save/reopen unqualified. All ACE writes stay blocked; 1–9 remain unfinished.

All 569 workspace tests/doc tests and 304 ACE all-feature tests pass, along with
both all-target Clippy profiles, release CLI/all-example builds, warnings-denied
ACE docs, dependency policy/tests and local documentation checks. No dependency
was added; local test debug symbols remain disabled and release defaults unchanged.

## Stored Memo version histories

`read_version_history_column` now resolves a visible Memo name to its checked
hidden history storage without generated-name heuristics. The exact ACE 16
two-field template, observed hidden-key marker and flat schema/reference checks
must agree. Borrowed `StoredVersion` records preserve scoped identity, nullable
text, exact nullable DATE bits and storage order. No current value, timestamp,
newest-first order or latest-value policy is synthesized. Resolved metadata feeds
the shared link reader directly, avoiding duplicate schema resolution.

Five CI tests cover visible/hidden/source/profile binding, template reordering,
exact representation/storage order, NULL/empty distinctions, orphan rejection,
unchanged source bytes, resource/cancellation limits and a 128-case Unicode/DATE
sequence model. A retained seed guards valid synthetic memo encoding outside
the reserved compressed-text header. Original V2016/V2019 fixtures independently
match six parent collections/18 versions per family, with 13 literal timestamp
bit probes, different visible Memo names across two tables and nullable records.
Public reference API membership agrees; its equal-time tie ordering is observed
separately. Six corrupt copies fail without stdout, while a NULL-time mutation
still matches raw storage. Earlier link/multivalue/attachment comparisons pass
unchanged after the shared-reader extraction.

[Contract and evidence](features/ACCESS_ADVANCED_FIELDS.md#stored-version-history-collections)
keep current-value reconciliation, default ordering, history mutation, query
expansion, actual Access and application save/reopen unqualified. The reference
builder's one-append-only-Memo-per-table restriction is not an Access claim.
All ACE writes remain blocked; milestones 1–9 remain unfinished.

All 560 workspace tests/doc tests and 298 ACE all-feature tests pass, as do both
all-target Clippy profiles, release CLI/all-example builds, warnings-denied ACE
docs, dependency policy/tests and local documentation checks. No dependency was
added; local test debug symbols stay disabled and release defaults are unchanged.

## Typed stored attachment collections

`read_attachment_column` now returns checked `AttachmentColumn` collections with
borrowed typed `StoredAttachment` metadata and wrapped bytes. Nullable URLs/names/
type labels/data, exact DATE serial bits and raw flags stay distinct from empty
values; scoped child identities and original storage order remain available.
The six named conceptual/flat fields and observed template flags are qualified
before publication. Borrowed accessors return typed errors without unchecked
casts or extra payload copies; record checks charge work and cancellation.

Explicit `StoredAttachment::decode` delegates to the existing bounded decoder:
NULL payload stays None, while present invalid bytes fail. Stored field labels
do not replace wrapper labels, and metadata inspection does not inflate data.
The engineering attachment example now consumes this API, retaining its original
decoded output and adding an explicit encoded-metadata mode. No extraction,
URL resolution, active-content execution or write permission was introduced.

Four CI tests cover exact metadata, NULL/empty distinctions, template reordering/
guards, inert Unicode/NUL/path-like labels, negative DATE bits, independent zlib
bytes, resource/cancellation limits and unchanged source bytes. Independent
V2016/V2019 comparisons match all six stored records/profile. Six disposable
mutations/profile verify preserved NULL/corrupt-wrapper metadata and buffered
decode/template/reference failures, including a late final-file failure. The
65,810-byte original decode comparison and input hashes remain unchanged.
[Contract and evidence](features/ACCESS_ADVANCED_FIELDS.md#typed-stored-attachment-collections)
keep native Access, mutation, query expansion, streaming and application save/
reopen unqualified. All ACE writes stay blocked; milestones 1–9 remain unfinished.

All 555 workspace tests/doc tests and 293 ACE all-feature tests pass, with both
all-target Clippy profiles, release CLI/example builds, warnings-denied ACE docs,
dependency policy/tests and local documentation checks. No dependency was added;
local test debug symbols remain disabled and release defaults remain unchanged.

## Bounded stored multivalue collections

`read_multivalue_column` now returns an owned, validated `MultivalueColumn` with
borrowing scalar-value iterators and the existing raw schema/identity links.
Exact storage order, duplicates, NULL cells and empty/absent parent distinctions
are retained. Qualified scalar templates include exact Decimal metadata; unknown
flags/templates and more than 100 values per parent fail before publication.
No SQL expansion, lookup display translation, sorting/deduplication or mutation
is inferred. The existing fallible storage reads/budgets/cancellation remain shared.

An original fixture exposed the reviewed reference's empty Decimal conceptual
template: precision 18/scale 0 and declared width 9, paired with ordinary 17-byte
child values. A focused metadata exception now accepts that exact hint; no
nine-byte record decoder was added. Strict synthetic tests reject changed hints,
nonempty templates and incompatible children. Seven new tests also cover exact
Large Number values, 100/101 boundaries, reference/type/resource/cancellation
failures, unchanged source bytes and a 128-case nullable sequence model.

Independent V2016/V2019 comparisons match 32 collections/137 stored values per
family through the reviewed Jackcess 5.0.4 collection API. Twelve disposable corrupt
copies fail without output; NULL/empty/duplicates and exact IEEE/Decimal carriers
match, and the separate 101-item control remains readable through raw links.
Earlier complex-link comparisons still pass unchanged. [Contract and evidence](features/ACCESS_ADVANCED_FIELDS.md#bounded-stored-multivalue-collections)
keep SQL expansion, lookup display semantics, mutation, actual Access and
application save/reopen unqualified. All ACE writes stay blocked; 1–9 remain unfinished.

All 551 workspace tests/doc tests and 289 ACE all-feature tests pass, with both
all-target Clippy profiles, release CLI/example builds, warnings-denied ACE docs,
dependency policy/tests and local documentation checks. No dependency was added;
local test debug symbols remain disabled and release defaults remain unchanged.

## Exact ACE 16 Decimal foreign-key audits

Native constraint reports now check one-to-ten-column tuples containing Decimal
fields with matching paired precision/scale on ACE 16-family profiles. Separate
wider equality atoms retain exact i128 coefficients and scale; compact integer/
Currency/GUID tuple allocations remain unchanged. All-NULL exemption, exact
partial NULLs, parent/one-to-one uniqueness, fallible allocation and bounded
sorting/search/cancellation use the existing audit. Older Decimal profiles and
metadata mismatches remain Unsupported; physical index checks stay separate.

Six integration tests cover all 29 scales, exact extrema/adjacent coefficients,
mixed carriers, empty/NULL metadata/profile rejection, unchanged source bytes,
resource/cancellation limits and a 256-case independent tuple-set model. A focused
unit test retains scale/domain distinctions for zero. Original V2016/V2019
recipes independently match 116 exact rows and 12 table audits/profile; 17 corrupt
copies fail without stdout, two precision/scale mismatches decline, and all-NULL
exemption remains. Currency's 48-row comparison also passes on unchanged inputs.
[Contract and evidence](NATIVE_CONSTRAINTS.md#exact-decimal-relationship-membership)
keep native Access, cross-scale semantics, cascades and Decimal mutation
unqualified. All ACE writes remain blocked; milestones 1–9 are not complete.

All 544 workspace tests/doc tests and 282 ACE all-feature tests pass, as do both
all-target Clippy profiles, release CLI/example builds, warnings-denied ACE docs,
dependency policy/tests and local documentation links. Local test debug symbols
remain disabled; release defaults/runtime assertions are unchanged. No dependency
or SQL coercion policy changed.

## Separate compact and Decimal column metadata

Composite profiles now select compact numeric/GUID column sets or wider Decimal
column sets statically. Both use the same bounded tuple policy and key models.
The compact component codec no longer carries a Decimal branch, addressing the
3–5% instruction regression measured in the initial tuple implementation. Native
type detection chooses the set; the builders retain full format, flag, column
mapping and precision/scale qualification. The read/check API, entry-buffer caps,
resource/cancellation checks and write gates are unchanged. No dependency or
new compatibility profile was added.

All 537 workspace tests/doc tests and 275 ACE all-feature tests pass, as do both
all-target Clippy profiles, release CLI/examples/benchmark builds, warnings-denied
ACE docs and dependency/documentation checks. Independent numeric/GUID/Decimal
tuple runners match 12,537 ordered rows per family on unchanged inputs. Local
test debug symbols remain disabled; release defaults are unchanged.
[The follow-up benchmark](testing/NATIVE_BENCHMARKS.md#separate-column-set-follow-up)
retains 520 samples: five/ten numeric tuples improve 0.559/0.514 and 0.369/0.330 ms
before/after, with 2.8%/4.4% fewer instructions. Mixed GUID/Decimal wall results
vary; no uniform improvement is claimed. Both reports retain the original
regression and all 1,040 samples. Actual Access, application save/reopen and all
ACE writes remain unqualified; milestones 1–9 continue.

## Complete Decimal-bearing composite indexes

ACE 16-family two-to-ten-component Decimal tuples now retain each field's exact
i128 coefficient, precision and scale, including mixed GUID/numeric/Boolean/legacy
DateTime keys and reordered directions. A dedicated wider atom model is selected
only for Decimal-bearing tuples; compact numeric/GUID-only models stay separate.
The existing derived 198-byte buffer still bounds every supported combination.
Actual allocation sizes and per-component work remain charged; NULL/zero and
unique/primary/required semantics retain the common complete reader/checker.

Six new tests cover every other qualified numeric carrier, per-field scales,
nullable typed models, maximum compressed prefixes, exact constraints, corruption,
profile/resource/seek/cancellation boundaries and ACE edit rejection. Original
V2016/V2019 recipes independently match 3,512 ordered rows/profile, maximum-width
ten-Decimal node trees (188/184-byte entries), residual queries, complete checks
and buffered failure for corrupt roots/unqualified Text tuples. The earlier
6,116-row single-Decimal comparison now also checks its empty tuple; original
hashes remain unchanged. [Encoding/evidence](NATIVE_INDEXES.md#complete-decimal-bearing-composite-indexes)
keep Decimal FK/seeks/mutation, old variants, actual Access and application
save/reopen unqualified. No dependency or SQL coercion/ordering policy changed.
All ACE writes remain blocked; milestones 1–9 continue.

All 537 workspace tests/doc tests and 275 ACE all-feature tests pass, as do both
all-target Clippy profiles, release CLI/examples/benchmark builds, warnings-denied
ACE docs and dependency/documentation checks. The 4,512-row GUID tuple and 6,116-row
single-Decimal comparisons pass on both unchanged inputs. Local test debug
symbols remain disabled; runtime assertions/release defaults are unchanged.
[Initial benchmarks](testing/NATIVE_BENCHMARKS.md#decimal-tuple-measurements) retain
520 samples: existing numeric tuples cost 3–5% more instructions and about 10%
more user time/cycles, despite retaining their key models. Shared component
metadata dispatch is the next optimization target. New complete mixed 1,000-row
reads cost 0.929 ms indexed/0.454 ms sequential; ten Decimal fields over 500 rows
0.650/0.258 ms. No speed or parity claim is made.

## Reuse current root data pages during indexed reads

Complete and typed-range index readers now reuse one current data-page buffer
for consecutive references. Returning to another page fetches it again; each
native operation starts fresh. The existing borrowed-root accessor retains
owner/slot/deletion checks, row/key agreement and per-row work/cancellation.
Relocation targets still take physical reads, including same-page targets. No
global cache, dependency, format eligibility or write permission was added.

Four new tests trace hits/misses, fresh operations, short reads, empty selections,
physical relocation failures, late cached corruption and every cancellation point.
Range tests retain disjoint-child pruning and exact selected-row budget charging.
Access adapter tests now track required-index reads separately from data-page
reads, proving that partial snapshots stay separate from complete-table caches
and declined hints cannot silently select a sequential fallback.

Independent GUID tuple, IEEE, DATE and Decimal comparisons still match 25,696
ordered rows per V2016/V2019 family, with unchanged original hashes, exact bit/
coefficient checks, complete constraints and buffered corrupt-root failures.
Native Access, shared-file snapshots and application save/reopen remain unrun;
all ACE writes stay blocked and milestones 1–9 continue.

All 531 workspace tests/doc tests and 269 ACE all-feature tests pass, as do both
all-target Clippy profiles, release CLI/examples/benchmark builds, warnings-denied
ACE docs and dependency/documentation checks. Local test debug symbols remain
disabled; release flags/default features match the frozen baseline. [Recorded benchmarks](testing/NATIVE_BENCHMARKS.md#indexed-root-page-reuse)
retain all 800 samples: 1,000-row Int32 reads improve 0.538/0.361 ms before/current,
GUID 0.413/0.222 ms, Single/Double about 0.41/0.21 ms, DateTime 0.406/0.203 ms and
scale-four Decimal 0.476/0.284 ms. Scattered mixed GUID/DATE tuples gain little.
These are complete public API reads over memory-backed Cursors, not disk or
application timings. No additional I/O refactor or dependency was needed.

## Complete ACE 16 single-Decimal index reads/checks

ACE 16-family single-Decimal indexes now preserve exact i128 coefficients and
declared scale/precision through complete reads/checks. A separate corrected
fixed-point codec and 26-byte entry buffer avoid widening existing compact key
models. Every key, including internal bounds, validates sign and precision;
row agreement rejects different carriers/scales without coercion. NULL and zero
remain distinct. No dependency was added.

Original V2016/V2019 recipes independently match 6,116 ordered rows/profile,
precisions 4/28, scales 0/2/4/28, beyond-i64/2^53 neighbors, full extrema, complete
node trees and unique/primary/NULL checks. Required-index residual queries and
corrupt-root buffering pass with unchanged inputs. Seven new tests include an
independent i128 model, carrier/scale gates, sign/word-order goldens, compressed
prefixes, invalid parent bounds, metadata/profile/resource failures and ACE write
gates. [Encoding/evidence](NATIVE_INDEXES.md#complete-single-decimal-indexes) keep
Decimal composites, old numeric index variants, seeks/key mutation, foreign keys,
actual Access and application save/reopen explicitly unqualified. All ACE writes
remain blocked; milestones 1–9 continue beyond this slice.

All 527 workspace tests/doc tests and 265 ACE all-feature tests pass, as do both
all-target Clippy profiles, release CLI/examples/benchmark builds, warnings-denied
ACE docs and dependency/documentation checks. Local test debug symbols remain
disabled; runtime assertions/release defaults
are unchanged. [Recorded benchmarks](testing/NATIVE_BENCHMARKS.md#single-decimal-index-measurements)
retain 360 samples: complete 1,000-row Decimal reads take about 0.483–0.487 ms
indexed versus 0.229–0.233 ms sequential. Existing Int32 controls remain close in
this run; no speed improvement is claimed.

## Complete legacy DateTime index reads/checks

Legacy DATE now participates in complete single/mixed indexes using exact serial
bits, with focused finite/calendar-envelope validation for every key, including
internal separators. Signed zero, negative day-zero aliases and sub-millisecond
values remain unchanged. Physical serial order stays separate from SQL civil-time
equality/sorting. Existing entry buffers, limits and typed seek gates remain
unchanged; no dependency was added.

Original V2016/V2019 recipes independently match 4,528 ordered rows/profile,
exact serial bytes, sequential controls, complete node-tree/constraint checks and
buffered failure for corrupt roots. Residual SQL checks explicitly prove alias
equality and civil ordering over required physical indexes. Six new tests include
an arbitrary valid-bit model, wire goldens, a malformed parent separator with a
valid live child, mixed/ten-component keys, constraints, resource failures and
unrelated Jet 4 edit preservation. [Encoding/evidence](NATIVE_INDEXES.md#complete-legacy-datetime-indexes)
keep the distinction between artifact and actual Access behavior. DateTime key
mutation/seeks, foreign-key qualification, Extended DateTime indexes, all ACE
writes and application save/reopen remain unqualified; milestones 1–9 continue.

All 520 workspace tests/doc tests and 258 ACE all-feature tests pass, as do both
all-target Clippy profiles, release CLI/examples/benchmark builds, warnings-denied
ACE docs and dependency/documentation checks. The existing 10,540-row IEEE
comparison passes on both unchanged inputs after disabling oracle expression
evaluation explicitly. Local test debug symbols remain disabled; runtime
assertions/release flags are unchanged. [Recorded benchmarks](testing/NATIVE_BENCHMARKS.md#legacy-datetime-index-measurements)
retain 320 samples: 1,000-row DateTime reads take 0.412 ms indexed/0.157 ms
sequential, mixed rows 0.627/0.212 ms. Int32 controls remain close in this run.

## Exact DATE comparison oracle correction

The independent date-bit helper now serializes the reviewed Jackcess DATE carrier
to an in-memory buffer, validating ordinary widths or the known calculated
prefix/length/trailer. Its former `toDateDouble` path can normalize aliases and
round through milliseconds. Original [precision probes](testing/compare_date_bits.py)
now match 28 ordinary/cached cells per V2016/V2019 family against golden bits,
including signed zero, subnormals, negative aliases and sub-millisecond values.
Generation demonstrates the old conversion loses at least one probe's bits.
The earlier 30-cell stored-scalar comparison still passes with unchanged input
hashes. No production Rust, dependencies, database bytes or expression execution
changed; native Access remains unrun. DateTime index qualification follows this
stronger oracle rather than treating a calendar rendering as exact bit evidence.

## Complete Single/Double index reads/checks

Complete single and mixed indexes now retain exact Single/Double bits. A focused
integer-only transform preserves signed zeros, subnormals, infinities and all NaN
signs/payloads under private physical ordering, without new SQL coercion or float
equality. Typed integer seeks reject floating carriers and existing bounded
buffers/allocations remain unchanged. Six new tests include arbitrary IEEE bits,
exact constraints, compressed prefixes, mixed/ten-component keys, resource errors
and unrelated Jet 4 preservation. No dependency was added.

Original V2016/V2019 recipes independently match 10,540 ordered rows/profile,
exact tagged IEEE bytes, sequential controls and complete node-tree checks. Their
unique indexes distinguish signed zeros/quiet NaN payloads and reject identical
keys. Finite-only controls pass required-index CLI/residual queries; non-finite
tables retain query's explicit rejection, even for ID-only projections. New
engineering-only tagged index modes avoid changing the CLI output protocol.
[Encoding and evidence](NATIVE_INDEXES.md#complete-single-and-double-indexes) retain
the native Access/non-finite qualification limits. All ACE writes and floating
key mutation/seeks/foreign keys remain outside this slice; milestones 1–9 continue.

All 514 workspace tests/doc tests and 252 ACE all-feature tests pass, as do both
all-target Clippy profiles, release CLI/examples/benchmark builds, warnings-denied
ACE docs and dependency/documentation checks. Local test debug symbols remain
disabled; runtime assertions/release flags are unchanged. [Recorded benchmarks](testing/NATIVE_BENCHMARKS.md#boolean-and-ieee-index-measurements)
retain 600 samples: complete 1,000-row IEEE reads take about 0.415–0.420 ms indexed
versus 0.158–0.159 ms sequential; mixed rows 0.641/0.218 ms. Numeric tuple controls
cost about 1.6–2.1% more instructions; wall medians vary. No speed gain is claimed.
Actual Microsoft Access and application save/reopen remain unrun.

## Complete Boolean index reads/checks

Native Yes/No now participates in complete single and mixed numeric/GUID index
reads/checks. A focused strict two-value codec preserves true-before-false native
ascending order and descending complementation. False remains a value, never NULL;
malformed bytes/NULL markers fail and row agreement requires Boolean carriers.
Existing buffers, allocation charging, SQL semantics and typed seek boundaries
remain unchanged. No dependency was added. Six new tests cover arbitrary Boolean
sequences, compression, every value byte, exact constraints, mixed/ten-component
tuples, resource failures and unrelated Jet 4 edit preservation.

Original V2016/V2019 recipes independently match 4,509 ordered rows/profile,
actual node trees, required-index residual queries and complete checks. Generation
also asserts stored NULL-to-false conversion and duplicate NULL-input rejection
on unique/primary indexes. Input hashes remain unchanged. [Encoding and evidence](NATIVE_INDEXES.md#complete-boolean-indexes)
retain the native stored-field scope. All ACE writes, Boolean key mutation/seeks,
foreign-key qualification and actual Microsoft Access evidence remain unqualified;
milestones 1–9 continue beyond this slice.

All 508 workspace tests/doc tests and 246 ACE all-feature tests pass, as do both
all-target Clippy profiles, release CLI, formatting and dependency/documentation
checks. The earlier 4,512-row GUID tuple comparison passes on both families.
Test debug symbols remain disabled for local disk space; runtime assertions and
release flags are unchanged. Native Access and application save/reopen remain unrun.

## Complete GUID-bearing composite indexes

Complete index reads/checks now support two-to-ten mixed numeric/GUID components,
including reordered directions, exact NULL shapes and unique/primary/required
tuples. Composite models, column codecs and profiles live in focused modules.
Static profiles keep compact numeric tuples and their earlier buffer limits;
only GUID-bearing tuples use wider atoms and a derived 198-byte entry buffer.
Single/compound GUIDs share strict segment validation. Actual allocation sizes
and bounded component work remain charged, with no new SQL coercion/ordering.

Original V2016/V2019 recipes match 4,512 ordered rows/profile, required-index NULL
filters, complete checks and immutable input hashes. Ten-GUID trees reach depth
two, with maximum expanded entries 198/194 bytes. The separately committed
explicit-final-child fix resolved their valid zero-tail roots. Six new tests
cover mixed carriers, a generated tuple model, ten-GUID/193-byte prefix goldens,
NULL/constraint/type/corruption/resource boundaries and unrelated Jet 4 edit proofs.
Affected composite key edits, integer seeks over composites and all ACE writes
remain blocked. The earlier empty GUID/Int32 corpus now qualifies too.

All 502 workspace tests/doc tests and 240 ACE all-feature tests, both Clippy profiles,
release CLI/examples/bench builds, warnings-denied ACE docs and dependency checks
pass. Test debug symbols were disabled after local disk exhaustion; release flags
are unchanged. [Two benchmark reports](testing/NATIVE_BENCHMARKS.md#guid-bearing-tuple-measurements)
retain all 1,040 samples: numeric tuple instructions cost about 1.7–2.1% more;
wall medians are noisy. An inconclusive inlining contender was removed. Final
source fingerprints match the original tested implementation. No dependency was
added. Composite seeks/mutation, other index encodings, native Access, application
save/reopen and milestones 1–9 remain ongoing.

## Explicit final-child index nodes

Complete reads, checks and typed single-key seeks now accept nonempty internal
nodes whose children all have explicit upper-bound entries and whose separate
tail pointer is zero. The walker no longer follows an absent pointer into page
zero. Childless internal nodes, nonzero leaf tails, duplicate child traversal,
cycles and invalid bounds still fail. The existing key writer explicitly rejects
this valid read topology until its mutation is qualified.

Three independent synthetic tests cover full results, check coverage, a pruned
seek, corruption and mutation rejection. The fix was also applied to a clean
archive of its parent commit and validated separately from the in-progress wider
GUID-tuple feature. This changes structural read coverage without enabling ACE
writes or claiming Microsoft Access conformance.
All 234 ACE all-feature tests/doc tests and all-targets/all-feature Clippy pass
in that isolated parent-plus-fix checkout. Test debug symbols were disabled to
fit local disk space; runtime/debug assertions and the release profile are unchanged.

## Complete single-GUID index reads/checks

Single-GUID indexes now retain canonical 128-bit keys through the shared complete
tree/row reader and explicit unique/primary/required checks. A dedicated bounded
segment codec validates the fixed intermediate/final tags, descending rules and
byte-order boundary, with a separate 27-byte entry buffer. NULL and zero GUIDs
remain distinct. Core Guid and SQL comparison semantics do not gain general
ordering; integer seeks and GUID composites remain unsupported.

Fresh V2016/V2019 recipes independently match 2,034 ordered rows/profile, actual
node trees, complete checks, required-index NULL filters and buffered failure for
corrupt selected roots/unqualified GUID composites. Original hashes stay unchanged.
Five new tests include arbitrary-byte ordering, compressed segment/row prefixes,
exact constraints, tag/width/key/profile/resource failures and existing write gates.
Unrelated qualified Jet 4 Int32 edits preserve the full GUID index and every byte
outside their field; GUID targets and all ACE writes remain blocked.

All 493 workspace tests/doc tests and 231 ACE all-feature tests pass, as do both
Clippy profiles, release CLI, warnings-denied ACE docs and dependency/documentation/
fixture checks. [Encoding/evidence](NATIVE_INDEXES.md#complete-single-guid-indexes)
records the reviewed reference artifact, original recipes and limits. No dependency
was added. GUID seeks/mutation/composites, other index carriers, native Access,
application save/reopen and milestones 1–9 remain ongoing.

## Complete numeric composite indexes

Complete indexed reads/checks now support two-to-ten Byte/Int16/Int32/Int64/Currency
components with independent directions, native key order, exact NULL shapes and
row agreement. Shared bounded page/tree validation uses private static key
policies, retaining the 17-byte single-key buffer and a separately bounded
98-byte tuple buffer. Actual allocation sizes and component work remain charged.
Unique tuples exempt only entirely NULL keys; primary/required indexes reject
any NULL component. Typed integer seeks still reject composite indexes.

Original V2016/V2019 fixtures match 4,513 ordered rows/profile, actual mixed-width/
maximum-width node trees, complete checks and residual queries. The existing
12-file ACE16 runner additionally matches 1,000 original two-Int32 composite rows
per family. Seven new tests include exact tuple properties, 93-byte prefix
goldens, all I/O/cancellation failures and write boundaries. Composite key edits
remain blocked; unrelated qualified Jet 4 fields have an unchanged-index proof.
The CLI now labels index constraints exact unique/required across key types.

All 488 workspace tests/doc tests and 226 ACE all-feature tests pass, along with
both Clippy profiles, release CLI/examples/bench builds, warnings-denied ACE docs
and dependency checks. Existing Int64 range/query and Currency comparisons pass.
[Recorded benchmarks](testing/NATIVE_BENCHMARKS.md#composite-numeric-index-follow-up)
retain all samples: single-index controls stay within observed noise; current full
compound reads are slower than sequential scans (1,000 mixed rows: 0.583/0.212 ms).
No dependency was added. Composite seeks/mutation, other index carriers, native
Access qualification, all ACE writes and milestones 1–9 remain ongoing.

## Exact Currency foreign-key audits

Currency now participates in one-to-ten-column exact relationship tuples,
including mixed integer/GUID components. A distinct private scaled-integer atom
retains the Currency domain; paired field types must match even for equal-width
Currency/Int64. Parent/one-to-one uniqueness, exact partial NULLs and all-NULL
exemption reuse the bounded shared audit without floating conversion.

Fresh V2016/V2019 recipes independently match 48 exact rows, six table audits and
eight corrupt disposable-copy failures per family. One-quantum/partial-NULL
mismatches and duplicate complete/partial parent or child tuples fail without
partial output; original hashes remain unchanged. Five new CI tests include an
independent 256-case tuple-set model, mixed carriers and same-width type rejection.
No dependency was added. Physical composite indexes, broader keys, cascades,
native Access evidence, ACE writes and milestones 1–9 remain ongoing.

All 481 workspace tests/doc tests and 219 ACE all-feature tests pass, as do both
Clippy profiles, release CLI/example builds, warnings-denied ACE docs and
dependency/documentation/fixture checks. The existing 32-row GUID and 59-row
narrow-key comparisons also pass on both families. [Feature evidence](NATIVE_CONSTRAINTS.md#exact-currency-relationship-membership)
records exact scope, original recipes and comparison outcomes.

## Exact complete Currency indexes

Single Currency indexes now share bounded complete traversal, decoded row/key
agreement and explicit unique/required checks. Private key types distinguish
Currency from Int64 despite identical widths; Int32/Int64 range APIs reject
Currency even for All/Null. Exact signed scaled integers retain every value and
the result remains Currency. Shared carrier extraction also removes duplicated
integer checking without changing coercion policies.

Fresh V2016/V2019 recipes match 1,426 independently ordered rows/profile, real
node trees, complete table checks, NULL/sign SQL filters and original file hashes.
Six CI tests include an arbitrary exact-key model, compressed duplicates, type/
constraint/corruption/resource failures and existing write gates. A synthetic
Jet 4 unchanged-byte regression verifies unrelated qualified Int32 updates leave
Currency indexes untouched; Currency targets and all ACE writes remain rejected.
No dependency was added. Currency ranges/mutation, composite/text indexes,
Microsoft Access qualification and milestones 1–9 remain ongoing.

All 476 workspace tests/doc tests, 214 ACE all-feature tests, both Clippy profiles,
release CLI/examples, warnings-denied ACE docs and dependency/documentation/fixture
checks pass. [The feature evidence](NATIVE_INDEXES.md#complete-exact-currency-indexes)
records the independent artifact, exact rows, original recipes and limitations.

## Current-page reuse during native scans

Catalog and sequential table scans now borrow their already-fetched data page
for live roots. The shared physical-row decoder retains signature/owner/slot/
range validation, relocation reads and cycle/hop checks. Nothing is cached across
pages or operations, no extra allocation is introduced, and stable/offline sources
remain required. Avoided physical reads no longer consume read/byte allowance;
row/work/copy allocations still use the shared budget.

Read-count regressions confirm one root-page fetch and separate relocation reads;
corrupt target owners/signatures/ranges/cycles fail without partial snapshots.
All 470 workspace tests/doc tests, 208 ACE all-feature tests, both Clippy profiles
and the independent 12-file ACE16 fixture comparisons pass. The
[recorded release benchmark](testing/NATIVE_BENCHMARKS.md#reusing-the-current-scan-page)
measures 1,000-row sequential reads at 0.504 → 0.279 milliseconds over a
memory-backed Cursor; the 1,000-row indexed control remains within noise.
All timing samples are retained. No new dependency, write permission, native
shared-file behavior, Microsoft Access conformance or milestone completion follows.

## Bounded opaque catalog payload inspection

`NativeImage::read_object_payloads` now returns exact bounded assembled
Lv/LvModule/LvExtra bytes with explicit Present/Absent/Uninspected states and a
captured object identity. Empty inline bytes remain distinct from NULL. Strict
native OLE/long-value geometry, existing byte/chunk caps and complete-result
failure behavior apply. A present empty definition is rejected without invented
NULL/empty output. Duplicate payload-column names now fail presence inspection and
the existing write gate. Payload models and reads live in focused feature modules;
the existing projected row decoder supplies the shared native geometry/codec.

Original V2016/V2019 fixtures independently match all bounded catalog payloads
(37/41 objects), with explicit rejection of one 65,537-byte object/profile. The
registered Access example matches all 146 catalog records/24,693 bytes. Twelve
module declarations have NULL local payloads, reinforcing that those fields alone
do not establish VBA absence. Input hashes remain unchanged. Synthetic inline/
external/chained, metadata, empty/NULL, identity, late failure/cancellation and
budget tests pass. No dependency was added and no application content executes.
This is opaque storage inspection, without VBA extraction, native Access
save/reopen, ACE write permission or completion of milestones 1–9.

All 468 workspace tests/doc tests and workspace Clippy pass, as do ACE
all-feature Clippy and 206 tests/doc tests, release CLI/engineering builds,
warnings-denied ACE docs, dependency-boundary and documentation/fixture checks.

## Exact GUID relationship membership

GUID fields now join bounded mixed integer/GUID tuple membership. Private typed
atoms retain canonical GUID bytes, integer values and NULL separately; parent/
child storage types must match per component. The all-zero GUID stays a value,
partial NULLs stay exact tuples and all-NULL keys remain exempt. Private set order
does not define SQL GUID ordering or physical indexes. Carrier extraction and
generic membership validation now live in focused feature modules with actual
allocation/work/cancellation accounting.

Original V2016/V2019 fixtures match 32 exact rows, six table audits and six
corrupted-copy failures/profile with no partial output; input hashes remain
unchanged. Arbitrary GUID byte/tuple models, zero-GUID/NULL distinctions, mixed
pair-type gates and exact orphan checks pass. A read-only probe records a
Jackcess default-cursor case limitation; canonical fixture input avoids importing
that limitation into TableCraft. The CLI now labels successful coverage exact key
membership; existing integer comparisons retain their results. No dependency was
added. GUID/composite indexes, ordering, cascades, native Access and milestones
1–9 remain ongoing.

All 463 workspace tests/doc tests and workspace Clippy pass, as do ACE
all-feature Clippy and 201 tests/doc tests, release CLI/example builds,
warnings-denied ACE docs, dependency-boundary and documentation/fixture checks.
Existing 59-row narrow and 35-row Int64 relationship comparisons also pass on
both families with the new CLI coverage label.

## Exact Byte and Int16 relationship audits

One-to-ten-column relationship keys now include unsigned Byte and signed Int16
alongside Int32/Int64. Private exact integer tuples retain matching paired storage
types, lossless values, all-NULL exemption and exact partial-NULL matching. No SQL
coercion, collation or new generic key model is introduced.

Original V2016/V2019 recipes independently assert insert rejections, match all 59
rows and nine table audits/profile, and exercise six corrupt disposable copies.
Orphans, cross-matched components, partial-NULL mismatches and duplicate parent/
one-to-one child keys fail without partial output; entirely NULL tuples remain
exempt. Original hashes stay unchanged. Synthetic CI covers both widths, mixed
tuples, storage-type mismatches and same-type orphans. No dependency was added.
Composite physical trees, cascades, native Access and milestones 1–9 remain ongoing.

All 458 workspace tests/doc tests and workspace Clippy pass, as do ACE
all-feature Clippy and 196 tests/doc tests, release CLI/example builds,
dependency-boundary and documentation/fixture checks.

## Complete Byte and Int16 native indexes

Complete native integer index reads/checks now include unsigned Byte and signed
Int16 keys with width-qualified decoding, compressed prefixes and exact row/key
validation. Existing unique/required rules use the same exact integer carrier.
Typed Int32/Int64 range APIs still reject narrow widths; no implicit widening or
narrow-key mutation is enabled. Query sources retain complete selected-index
reads for narrow fields and existing exact numeric filtering semantics.

Original V2016/V2019 fixtures independently match 3,067 ordered rows/profile across
both directions, actual node trees, unique/nullable/empty tables and SQL filters.
The cursor oracle normalizes Java's Byte into Access's unsigned value. Complete
checks and unchanged input hashes pass. Synthetic tests cover signed/unsigned
boundaries, compressed duplicates, row agreement, constraint/corruption/resource
failures and a 256-case independent key model; CLI coverage preserves carriers.
No dependency was added. Narrow relationships/ranges/writes, native Access
comparison and the remaining milestones 1–9 remain ongoing.

All 456 workspace tests/doc tests and workspace Clippy pass, as do ACE
all-feature Clippy and 194 tests/doc tests, release CLI, warnings-denied ACE docs,
dependency-boundary and documentation/fixture checks.

## Explicit stored scalar calculations

`read_stored_calculation` now inspects Boolean, Int32, Double, Currency, legacy
DateTime and Short Text caches through an owned exact-value snapshot. The strict
Int32 API remains. Matching declarations and known variable capacities/flags/
wrappers are required even on empty tables. Variable calculated Boolean retains
its declared capacity and uses a wrapped value; ordinary fixed Boolean mask
semantics remain unchanged. NULL/false and NULL/empty Text stay distinct. Formula
execution and ordinary scalar query eligibility remain gated.

Original V2016/V2019 fixtures independently match 30 cached cells each, including
compressed/uncompressed Unicode, signed zero/subnormal Double, full-range scaled
Currency and exact date serial bits. An original read-only oracle retrieves the
date bits from Jackcess's Date carrier to avoid its calendar precision loss.
The observed 65 ns rendering difference is documented explicitly. Empty tables
and corrupted disposable caches preserve output atomicity and original hashes.
Synthetic exact bit/UTF-16 models, strict profile/capacity/encoding failures and
late cancellation/budget checks pass. CLI inspection uses the same bounded output
formatter as queries; its Int32 output stays unchanged. No dependency was added.
Native Access conformance, remaining result types/recalculation and milestones
1–9 remain ongoing.

All 452 workspace tests/doc tests and workspace Clippy pass, as do ACE
all-feature Clippy and 191 tests/doc tests, release CLI, warnings-denied ACE docs,
dependency-boundary and documentation/fixture checks. Original stored Int32
comparisons still match seven rows plus corruption/empty cases per family.
The Unicode model caught an invalid raw compression-marker prefix; the corrected
generator and explicit U+FEFF escape regression now pass without weakening decoding.

## Safe exact Int64 query seeks

Backend-neutral `ScanConstraint::Int64` now carries full-width integer bounds.
The planner admits wholly safe same-column comparisons, NULL tests and AND
intersections over Int32/Int64 fields with exact integer literals/plain parameters.
Every original WHERE remains residual. Mixed floats, arithmetic, unary parameter
expressions and partial unsafe conjunctions retain complete scans and eager errors.
Bounds are neither saturated nor converted to floats; Int32 bounds still require
an exact narrowing conversion.

The Access adapter selects the matching typed seek only for an explicitly required
index on that field. Missing/corrupt indexes still fail; wrong-width host hints
retain the required complete path. Partial snapshots remain ephemeral. Counters
prove an equality above 2^53 fetches one matching record, NULL fetches its two
records and a later full scan/cache remains complete. A 256-case independent
Int64 filter model, parameter reuse and eager failure tests pass. CLI tests retain
exact extrema/parameter output and buffered failures. Original V2016/V2019 files
match 20 additional query intervals/profile against independent ordered cursors;
hashes remain unchanged. No dependency was added. Native Access comparison,
broader actions/collations and milestones 1–9 remain ongoing.

All 447 workspace tests/doc tests and workspace Clippy pass, as do standalone
no-default-feature query/Access checks, release CLI, warnings-denied query/Access
docs, dependency-boundary and documentation/fixture checks.

## Exact native Int64 index seeks

`Int64IndexSelection` and `read_table_int64_index_range` now select exact Int64
intervals/NULL keys from qualified ACE 16-family indexes. A shared integer bounds
implementation retains Int32 behavior, direction-independent numeric endpoints,
duplicate separator handling and strict width gates. Full reads retain complete
count checks; partial reads qualify only visited pages/returned rows. Int64
extrema and adjacent values beyond double precision never pass through floats.

Two-level synthetic trees and a 256-case independent numeric model cover both
directions, both profiles, duplicate boundaries, NULLs, empty/reversed/extreme
ranges, corruption, limits and cancellation. Traced I/O proves a disjoint corrupt
leaf is skipped, while full/visited reads fail. Original V2016/V2019 fixtures
independently match 24 direct selections per format plus the prior 1,212 ordered
rows and complete index checks; original hashes stay unchanged. The engineering
example uses clap Parser and explicit width/table selectors. No dependency was
added. This initial seek slice left query pushdown Int32-only; the extension above
now consumes the typed Int64 API. Native Access qualification and the
remaining milestones 1–9 remain ongoing.

All 441 workspace tests/doc tests and workspace Clippy pass, as do ACE
all-feature Clippy and 187 tests/doc tests, release CLI/range-example builds,
warnings-denied ACE docs, dependency-boundary and documentation/fixture checks.

## Qualified existing-tree unique index maintenance

Offline Jet 4 Int32 edits now retain existing multi-page unique index trees.
Sorted key/row references are repartitioned using existing leaf counts and node
separators are rebuilt from exact subtree maxima. The writer requires balanced
nonempty child subtrees with matching recorded levels and zero write stamps;
source checks establish ownership and row coverage. Only entry-data spans change:
page counts, allocation, links, levels, prefixes, bitmaps, free space, counters and
opaque padding stay identical. Prefix/width changes, allocation, nonunique indexes
and all ACE writes remain blocked.

A fresh original 1,000-row Jet 4 fixture has ascending and descending three-leaf
trees. Independent Jackcess comparisons match three cross-leaf moves and a full
1,000-key swap, exact row references, memo/binary digests and unchanged bytes
outside original field/entry spans. Native and CLI copies agree; cross-leaf
duplicates publish no output. A 256-case independent model exercises NULL,
arbitrary Int32 values, both directions and two tree heights, including a
zero-separator tail subtree. Journal faults before the root and between rewritten
index pages restore the exact complete original image. No dependency was added.
Actual Access writes, shared-file coordination and milestones 1–9 remain ongoing.

All 438 workspace tests/doc tests and workspace Clippy pass, as do ACE
all-feature Clippy and 184 tests/doc tests, release CLI/example builds,
warnings-denied ACE docs, dependency-boundary and documentation/fixture checks.
Late cancellation and a one-byte-short allocation budget publish no plan.

## Qualified unique leaf-index maintenance

Offline Jet 4 fixed Int32 plans now update affected unique/primary indexes when
their complete tree fits one root leaf and existing prefixes/entry widths remain
usable. The writer binds changes to original root references/keys, reorders
entries, checks uniqueness and emits one patch per physical index, including
logical aliases. Nullable ordinary unique indexes retain existing NULL keys.
Nonunique/node/recompression/unknown-header profiles remain unsupported. All ACE
writes, AutoNumber changes, allocation and row resizing remain blocked.

Exact field/index-entry spans now feed a shared independent unchanged-byte proof;
index headers, bitmaps, allocation, counters and opaque padding remain identical.
Candidates recheck complete index/row consistency plus field/rule/relationship
audits. Duplicate keys and later field-rule failures publish no plan. CLI UPDATE
copies consume the same native implementation.

The original Jet 4 recipe independently reopens Id 2-to-27 edits with index order
1/3/27 and unchanged Unicode memo/binary records. Native/CLI images match; exact
byte comparisons admit only the selected field and original entry-data span.
Synthetic tests cover directions, aliases, NULL, no-op, node/prefix/header/profile
rejection, rule failures, CLI atomicity and 256 generated row/key models. Optional
journal integration commits/reopens indexed images and rolls back a deliberately
inconsistent data-before-index partial write; no-op multi-page replay also passes.
[Native write scope](NATIVE_WRITES.md#unique-existing-tree-int32-index-maintenance)
records current qualification. No dependency was added. Native Access and broader
B-tree mutation remain unqualified; milestones 1–9 remain ongoing.

All 436 workspace tests/doc tests and workspace Clippy pass, as do ACE
all-feature Clippy and 182 tests/doc tests, warnings-denied ACE documentation,
release CLI/example builds, dependency-boundary and documentation/fixture checks.

## Bounded original attachment decoding

`decode_attachment` now returns exact original file bytes/type labels from
qualified direct and zlib FileData wrappers, without opening files or choosing
paths. Exact body/header/type lengths, UTF-16 terminators, checksums, stream end
and input consumption are validated. One extra output byte detects expansion
beyond advertised sizes. Unknown tags/dictionaries and malformed/trailing/oversized
inputs fail without partial data. Normal scalar/complex views still preserve raw
encoded Binary; decoding is explicit. The current 64 KiB input/file, 68 KiB body
and bounded native text/header limits leave large streaming attachments separate.

Flate2 1.1.10 uses only the pure Rust backend; five reviewed registry packages
were added through workspace declarations. Project output buffers reserve
fallibly; codec blocks check cancellation/resources. A conservative logical codec
workspace charge does not intercept upstream allocations. [Dependency records](DEPENDENCIES.md#native-attachment-compression)
own versions/features/licenses and allocation limits.

Original V2016/V2019 fixtures each independently match six files/type labels
(65,810 bytes), including both wrapper modes, empty/Unicode/binary files, inert
PNG/ZIP signatures and a 64 KiB file. Input hashes remain unchanged. An independent
Python-zlib golden, 256 generated files and corruption, size, cancellation and
budget tests cover the codec. All 430 workspace tests/doc tests and workspace
Clippy pass, as do ACE all-feature Clippy and 176 tests/doc tests, release CLI,
warnings-denied ACE docs, dependency-boundary checks and local docs/fixture checks.
Full validation passed after clearing generated artifacts exhausted during an
earlier link attempt. No native Access comparison, attachment mutation, host file
publication or general streaming qualification follows; milestones 1–9 continue.

Twenty-run/three-warmup Hyperfine measurements over 20,000 synthetic 60,000-byte
payloads/process report about 1 microsecond for direct wrappers, 22 microseconds
for compressible zlib and 167 microseconds for noisy zlib. A large noisy-case
outlier is retained. [Absolute benchmark evidence](testing/NATIVE_BENCHMARKS.md#bounded-attachment-decoding)
includes raw samples and source/binary hashes; this is not a native Access or
whole-query performance comparison.

## Scoped native complex parent and child storage

`read_complex_column_links` now resolves a selected ACE 16-family complex column,
projects its qualified stored parent keys and returns checked raw child memberships.
Parent keys and child identities remain unique in their own scopes; orphan,
ambiguous and NULL child references return typed failures without a partial view.
Positive assigned keys, exact flags/widths and qualified child schemas are required.
NULL parent storage and empty membership retain distinct snapshot shapes, and
child iterators retain flat snapshot order. Encoded attachment Binary stays raw.

CLI `inspect --table NAME --complex-links COLUMN` reports buffered parent ordinals,
keys and raw counts. Ordinary scalar/collection query eligibility is unchanged.
Shared fallible projected-carrier construction and native row traversal retain
budgets/cancellation; feature modules separate encoding, reads, metadata and errors.
Independent original fixtures match nine parent links per family across multivalue,
attachment and history columns. Twelve corrupted disposable copies/profile fail
with no partial stdout; hashes remain unchanged. Synthetic tests cover exact child
rows/order, empty/NULL shape, ambiguity/orphans/identity/NULL failures, profile and
resource gates, and CLI atomicity. [Advanced field scope](features/ACCESS_ADVANCED_FIELDS.md#scoped-complex-parent-and-child-links)
owns the implemented profile. Native Access, query expansion, attachment extraction,
history semantics and ACE writes remain unqualified; milestones 1–9 are ongoing.

All 425 workspace tests/doc tests and workspace Clippy pass, as do ACE all-feature
Clippy and 171 tests/doc tests, release CLI, warnings-denied ACE documentation,
dependency-boundary and local documentation/fixture checks. Explicit stored Int32
calculation comparisons also pass after sharing projected-carrier construction.
No dependency was added.

## Explicit stored Int32 calculation inspection

`read_stored_int32_calculation` and CLI `inspect --table NAME --stored-calculation
COLUMN` now inspect strictly qualified 2016/2019-family calculated Int32 caches.
Matching typed declarations, variable storage, exact flags/capacity and known
wrapper prefixes/lengths/trailers are required even on empty tables. NULL and
Int32 extremes are retained. Unknown/malformed states fail without a partial
result. Output labels stored values and zero-based snapshot-local ordinals;
ordinary scalar queries still reject calculated fields. No expression executes
and no freshness/recalculation guarantee is implied.

The feature reuses bounded native record traversal/count/ownership checks through
a factored private helper; ordinary scalar eligibility remains unchanged.
Independent fixtures deliberately disable expression evaluation and match seven
known caches plus an empty table per family, including a stale result that differs
from the expression. Three corrupted disposable copies/profile fail atomically.
Synthetic CI tests cover profile/type/declaration gates, NULL/extremes, late
failures, ordinary-query rejection and resource/cancellation limits. Original
hashes remain unchanged. [Advanced field scope](features/ACCESS_ADVANCED_FIELDS.md#explicit-stored-int32-calculations)
records the implementation and limitations. All 419 workspace tests/doc tests and
workspace Clippy pass, as do ACE all-feature Clippy and 166 tests/doc tests,
release CLI, warnings-denied ACE documentation, dependency-boundary checks and
local documentation/fixture checks. The complete twelve-file ACE 16 regression
comparison also passes with current binaries. No dependency was added.
Native Access, other result/error
variants, recalculation and ACE writes remain unqualified; the broader pre-UI
milestones remain ongoing.

## Exact Int64 and mixed relationship audits

The bounded one-to-ten-column relationship audit now accepts Int32/Int64 tuples
when each parent/child storage-type pair matches. Keys retain exact full-range
integers without floating conversion or implicit cross-type coercion. Partial
NULLs require identical parent tuples; wholly NULL keys remain exempt. Existing
unique-parent, one-to-one, ordered-index eligibility, cancellation and resource
checks remain in force. CLI checked coverage now says integer key membership.

Original V2016/V2019 fixtures independently match 35 records and six audit
reports each. Generation checks invalid inserts; four corrupted disposable
copies per family fail with typed missing-parent/duplicate errors and no partial
stdout. Existing composite fixtures still match all fifteen records/audits and
corrupt-copy cases. Input hashes remain unchanged. Synthetic mixed self relations,
mismatched types, exact uniqueness, large adjacent values and fixed-seed tuple-set
checks cover the implementation. All 413 workspace tests/doc tests and workspace
Clippy pass, as do ACE all-feature Clippy and 161 tests/doc tests, release CLI,
warnings-denied ACE docs and local documentation/fixture checks. No dependency was
added. Native Access, cascades and physical composite indexes remain unqualified;
the remaining pre-UI milestones are still ongoing.

## Exact ACE 16 Int64 index reads and checks

Complete named index reads now support qualified single-Int64 keys on the
2016/2019 profiles. Width-specific decoding preserves the full signed range,
NULL/duplicate ordering and compressed prefixes in either direction. Existing
ownership, separator, sibling, live-root, count and exact row-key checks still
apply. Explicit table checks also validate unique/required Int64 declarations.
The Int32 range API remains Int32-only; the Access source retains the complete
selected Int64 index for other filters, including NULL. Missing/corrupt indexes
fail without a sequential fallback or partial CLI output.

Original multi-page fixtures independently match 1,212 ordered rows per family,
with real Int64 node roots, complete checks and precision/NULL filters. Earlier
modern-scalar fixtures also pass with BigInt indexes now checked. Input hashes
remain unchanged. Synthetic tests cover exact extremes, adjacent large keys,
duplicate/NULL constraints, shared descending prefixes, strict typed selection,
corruption and budgets/cancellation. [Index scope](NATIVE_INDEXES.md#ace-16-int64-index-qualification)
records the implemented profile. All 410 workspace tests/doc tests and workspace
Clippy pass, as do ACE all-feature Clippy and 158 tests/doc tests, release CLI,
warnings-denied ACE documentation, dependency-boundary checks and local
documentation/fixture checks. No dependency was added; native Access remains
unrun and index mutation remains separate work.

## Owned-object preservation gate

`NativeImage::inspect_object_children` now reports direct catalog containment for
an exact captured object identity, with bounded allocation/work and cancellation.
Unknown or foreign roots fail rather than appearing dependency-free. No object
body or application content executes. This is containment metadata, not a complete
dependency graph or proof of absent table data macros.

Offline edit planning now rejects separately owned catalog objects even when the
table's own Lv/LvModule/LvExtra fields are NULL. Synthetic tests cover known and
opaque child kinds, exact scope, unchanged input, limits and cancellation. All
405 workspace tests/doc tests and workspace Clippy pass, as do ACE all-feature
Clippy and 154 tests/doc tests, release CLI, warnings-denied ACE docs and local
documentation/fixture checks. No dependency was added. ACE writes remain blocked
and the remaining pre-UI milestones remain ongoing.

## Shared range/membership kernels and native rule audits

SQL-independent predicate kernels now serve bound query execution and source-free
scalar evaluation. BETWEEN supports either written bound order; IN retains exact
numeric and NULL/NOT truth without materializing candidate lists. Hosts retain
comparison/collation, work and cancellation policy. Eager evaluation still exposes
late errors after earlier true matches. Empty syntactic lists remain invalid.
The CLI can evaluate these predicates without a database.

Native pure-rule preflight now admits numeric range/membership forms, with shared
limits and field/table scope. Text, calls, subqueries and unknown NULL validation
results remain unqualified, including on empty tables. Existing fixed Int32 edit
plans recheck range/list predicates before/after candidates; rejected values
publish no plan. ACE writes are still blocked. [Expressions](EXPRESSIONS.md) and
[native constraints](NATIVE_CONSTRAINTS.md) own the profiles.

Two original newer-family fixtures independently match six rule reports and
thirteen exact rows each; generation asserts forward/reversed-range and list
write acceptance/rejection. Input hashes remain unchanged. Source-free CLI,
fixed-seed full-i64 intervals, eager failures, precision/NULL, budget/cancellation
and native pre/post-edit tests cover the shared implementation.
All 402 workspace tests/doc tests passed; seven mutation tests including the new
rule-gating case passed separately afterward. Expression minimal/all-feature
Clippy and 19/33 tests/doc tests pass, as do ACE all-feature Clippy and 152 tests/
doc tests, workspace Clippy, release build, warnings-denied workspace docs and
formatting/local documentation/fixture checks. No dependency was added. Native
Access remains unrun; the broader pre-UI milestones remain ongoing.

## Composite Int32 referential-integrity audits

The constraint milestone now checks ordered one-to-ten-column Int32 tuples
against qualified unique/primary parent declarations. Entirely NULL keys remain
exempt; partially NULL keys require an exact parent tuple and participate in
uniqueness. One-to-one child uniqueness follows the same rule. Fixed-width key
buffers, fallible allocation, bounded comparison work and cancellation retain
operation limits. Physical composite index validation/cascades and all existing
native write guards remain separate. [Constraint scope](NATIVE_CONSTRAINTS.md)
records implementation versus native Access qualification.

New original V2016/V2019 fixtures each independently match fifteen exact records
and assert matched/unmatched partial-NULL, all-NULL and one-to-one cases. Four
corrupt disposable copies/profile produce expected typed missing-parent/duplicate
errors without partial stdout. All original hashes remain unchanged. Native
synthetic tests cover eligibility/order, self relations and duplicate tuples;
256-case generated tuple-set checks supplement existing scalar-key tests.

All 393 workspace tests/doc tests and workspace Clippy pass, as do ACE all-feature
Clippy and 150 tests/doc tests, release CLI, warnings-denied ACE docs, formatting,
whitespace and local documentation/fixture checks. The full twelve-fixture ACE 16
regression comparison also passes with composite coverage now checked. No new
dependency was added. Native Access remains unrun; broader constraints and the
remaining pre-UI milestones are ongoing, not completed by these audits.

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
columns remain explicitly unsupported; complete Int64 indexes are covered by the
later qualification above. All ACE writes stay blocked.
[NATIVE_ACE16.md](NATIVE_ACE16.md#modern-scalar-reads) owns details.

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
