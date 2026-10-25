# Local Access read/query acceptance

Reviewed 24 October 2026. This is a current acceptance checklist, not a claim of
complete compatibility. The [roadmap](ROADMAP.md#remaining-local-file-readquery-work)
retains the full backend scope and implementation order.

The immediate target is reliable offline local-table reads and queries for ACE 16,
ACE 2007 and Jet 4. Other working profiles remain preserved. Windows comparisons
are deferred by the user; independent engineering continues, with native
qualification recorded separately.

## Working baseline

- Table/catalog discovery, system-table filtering, declared row counts, scalar
  pages, explicit progressive TSV rows and bounded native index reads work.
- SELECT, joins, grouping/HAVING, subqueries, UNION, top-level crosstabs and
  qualified saved/named queries execute under shared resource/cancellation limits.
  Contextual named schemas use explicit execution-stage discovery; binding does
  not scan rows or invoke formatting providers.
- Qualified encryption opens and fresh add/remove/change-password copies work.
  The 2 GiB read-source envelope is separate from the 64 MiB complete-image and
  encryption-management limits. Encryption is not an unstarted read milestone.
- Independent comparisons include 20,000-row indexed inputs and 99,110-row
  sequential scalar histories with added columns, Memo growth and deleted roots.
  A separate [mixed scalar history corpus](testing/MIXED_SCALAR_HISTORIES.md)
  checks seventeen-field layouts, null-mask growth, Memo/OLE updates and exact
  native scalar values across 19,902 retained rows per priority profile.
  [Modern ACE 16 histories](testing/MODERN_SCALAR_HISTORIES.md) additionally check
  Large Number and 2019 Extended dates, historical fixed-prefix changes and
  scattered indexed row loading under cumulative controls. [Encrypted modern
  histories](testing/ENCRYPTED_MODERN_HISTORIES.md) now compare both reference-
  and TableCraft-created ciphertexts, exact rows/queries, every decoded data
  page, SID digests and complete independent plaintext recovery.
  Larger explicit multivalue loads include 20,000/400 parents with 48,000/40,000
  children; a separate 20,000-attachment/version corpus checks exact metadata,
  decoded bytes and stored history DATE bits. These do not establish arbitrary
  million-row throughput or native Access equivalence.
  SQL results remain buffered with separate row, byte, work and output ceilings.
  Absolute I/O now rejects reported seek-position mismatches before reading or
  writing; failpoints cover scalar, Memo and indexed snapshot publication.
  [Continuation fault controls](NATIVE_READ_FAILURES.md) now additionally cover
  each remaining read/seek/page-hook callback after a successful prefix,
  decoder-triggered cancellation and bounded interrupted short reads. Sparse-source
  controls also traverse the final two pages of a virtual 2 GiB source, retain
  bounded projected cursors and refuse corrupt/failed high-offset tails; this is
  synthetic address/publication evidence, not native large-file throughput.
- Buffered collection pages now accept explicit native read limits, retaining
  complete parent/child validation and output refusal across `--all` sections.
- Native CLI reads now use shared OS locks, excluding cooperating exclusive
  writers. Offline/stable input remains required; this is not live Access
  coordination or coherent capture during noncooperating changes.
- The engine exposes [inert linked definitions](NATIVE_LINKED_TABLES.md), and
  explicitly supplied Access sources can share [one native allowance](NATIVE_SOURCE_BUDGETS.md).
  Target authorization, validated routing and linked-query execution remain
  unimplemented; neither API follows stored paths or activates providers.

CLI declared parameters now include exact Decimal and finite Single/Double
carriers. They retain explicit syntax and strict carrier matching; see
[parameter input rules](DEVELOPMENT.md). This closes a host-input gap without
completing the value-semantics acceptance work below.

The explicit declared-arithmetic profile now permits unknown aggregate/UNION
categories when the implemented operation cannot vary by category: Mod, fixed
Int64 integral arithmetic/negation, Byte quotients and Byte/Boolean negation.
Unknown provenance remains unknown for later consumers; checked conversion,
overflow, zero-divisor and narrow-width rules remain active. This removes false
declaration refusals without qualifying native aggregate categories. See
[dispatch scope](DECLARED_ARITHMETIC.md#operations-independent-of-widening-categories).

Ordinary Date/numeric predicates now reuse checked CDate conversion, or Decimal
raw-serial selection when Decimal participates, without requiring the engineering
declaration flag. This does not enable strict set-key coercion or qualify native
Access comparison behavior. See [predicate scope](NUMERIC_PREDICATES.md).

Ordinary numeric pairs also share checked effective-type conversion. In
particular, Currency/LongLong predicates now refuse integers outside Currency's
range instead of bypassing conversion through exact key comparison. Source
controls retain eager failures without partial hints. This does not complete
P0 promotion or qualify native comparison behavior; numeric Text and strict
set/group/sort comparison keep their separate contracts.

Prepared arithmetic metadata now has 44,800 binary and 240 unary boundary
controls across current scalar families and NULL. Successful claimed descriptors
remain consistent with runtime carriers; only explicit numerical/conversion
refusals are admitted by the controls. This is cross-layer evidence, not an
independent arithmetic-answer oracle or complete promotion qualification. See
[result-type evidence](QUERY_RESULT_TYPES.md#evidence-and-references).

DateAdd now exposes its existing successful date carrier for numeric/Boolean/Text
sources during metadata-only preparation, with source nullability preserved.
Named/derived, UNION, Nz and Avg consumers retain this metadata without scans or
policy activation. Existing Extended sources retain their precision; unknown
source families and native result-type qualification remain separate. See
[result metadata](QUERY_RESULT_TYPES.md).

Other date functions now retain their existing successful-value NULL contracts
in metadata: component/extraction calls and DateDiff follow their date operands,
while CDate/CVDate and constructors promise non-NULL dates only on success.
Named/derived, UNION and Nz compositions retain these guarantees without source
scans or control evaluation. This closes an inference gap, not broader coercion
or native result-type qualification. See
[date nullability](QUERY_RESULT_TYPES.md#date-value-nullability-and-successful-constructors).

Logical operators now retain raw String operands through eager expression
execution and then perform ordered numeric conversion. Explicit host Empty
selects Integer zero without becoming a stored value. Independent signed-bit and
half-even models cover all six operators, Not, exact LongLong inputs, NULL/zero
peers, raw slices and declaration-sensitive widening. See [logical evidence](INTEGER_LOGIC.md).
This closes an operand/admission gap, not complete Variant promotion or native
Access qualification.

Explicit [cached calculated Decimal reads](CACHED_DECIMAL_CALCULATIONS.md) now
retain the observed wrapped coefficient/scale through stored inspection and
cached row browsing. Eighty-two independent controls pass; ordinary calculated
query execution, freshness and native recalculation remain separate gaps.

Explicit [cached Memo reads](CACHED_MEMO_CALCULATIONS.md) now compose native
long-value resolution, checked calculation wrappers and existing Unicode decoding.
Eighty-six independent controls pass across fresh ACE 2016/2019 inputs.
Authored calculated Hyperlink Memo C0 fields now use the same raw wrapper decoder;
a [Text-family replay](testing/CALCULATED_TEXT_FLAG_FAMILY.md) adds 384 operation
controls and 384 cached-cell comparisons across plain/Hyperlink/RichText caches.
Ordinary query evaluation, broader declarations and freshness remain separate
acceptance work.

## Remaining acceptance work

Explicit [decoded attachment allowances](LARGER_DECODED_ATTACHMENTS.md) now admit
larger file bytes under validated policies and an unchanged header cap. Separate
[encoded-value allowances](LARGER_ENCODED_VALUES.md) now permit up to 32 MiB;
independent native inputs cover 20 MiB OLE and 3 MiB incompressible attachments,
plus 3 MiB compressed and 1.5 MB stored files. Pure decoder controls cover the
16 MiB file ceiling. Metadata pruning, cache policy changes and cumulative budget
refusals remain explicit. [Larger decoded Memo allowances](LARGER_MEMOS.md)
now permit up to 16 MiB independently of encoded storage and query/output pools;
76 four-family getter/byte/query controls cover 3 MiB ASCII and 1.8 MB Unicode
payloads. [Stored FileData queries](STORED_ATTACHMENT_DATA.md) now expose exact
encoded Binary under `--collections`, separately from explicit decoded files.
Native SQL/DAO byte equivalence remains unqualified.

Explicit [stored-history query members](STORED_HISTORY_QUERIES.md) now expose
actual hidden-field names through `--collections`, retaining current Memo,
aligned stored versions, complete caches and native flat-field policies.
The original replay passes 192 independent stored-record/query controls.
Unsupported history candidates remain unavailable without blocking unrelated
supported fields. Ordinary current Memos, including Hyperlink flags and optional
Unicode compression, now retain qualified history discovery and querying; the
expanded focused engine/adapter batch passes 22 tests and a separate
[compressed-Memo replay](testing/COMPRESSED_MEMO_HISTORIES.md) passes 198 controls.
The broader [ordinary Memo family replay](testing/MEMO_HISTORY_FLAG_FAMILY.md)
adds 594 controls across ordinary/Hyperlink/compressed values and inert RichText.
Unknown/calculated flags and unsupported template/flat flags remain refused. Native history SQL/latest-version behavior
and wider advanced-field semantics remain unqualified.

Opt-in [automatic native indexes](AUTOMATIC_INDEXES.md) now choose matching
qualified single-key definitions under existing safe constraints. Automatic
selections preserve sequential encounter order, original predicates and cache
boundaries; chosen failures remain explicit. Independent four-family controls
cover all supported key carriers, default-limit selections and TOP/First/Last.
NULL-only General Text/Short Binary constraints now use the same qualified
selection paths without activating string comparison; 216 independent controls
cover both General versions and all four families. Non-NULL Text/Binary bounds,
cost-based optimization, wider seeks, TOP pushdown and native optimizer
qualification remain open; this does not complete read/query acceptance.

The engine now offers explicit [Short Text equality candidates](NATIVE_TEXT.md#conservative-short-text-equality-candidates)
under the current General model. They conservatively retain primary-prefix
matches and require full residual equality; 824 independent controls protect
candidate sets, ordering, completeness of reference matches and pruning failures.
[Direct Short Text equality](TEXT_EQUALITY_SEEKS.md) now implements explicit
comparer/source agreement, residual evaluation and source-order/cache controls.
Inequalities, compound predicates and broader locale qualification remain open.
This is not native SQL qualification or completion of text predicate pushdown.

[Min/Max and domain extrema](EXPRESSION_AGGREGATES.md#retained-string-extrema)
now retain selected String data through raw-aware consumers, with strict actual
comparison, scalar publication, NULL/type/Error rules and cumulative budgets.
The independent runner covers reordered key vectors and four unchanged native
inputs through sequential/selected-index aggregate and domain calls. Remaining
String consumers, broader coercion and native qualification are still open.

[Retained String predicates](RAW_PREDICATES.md) now preserve NULL truth and empty
quantifier outcomes without Unicode publication. The existing explicit both-known-
Variant String/numeric rule also recognizes raw String identity. Eager failures,
metadata/policies and strict non-NULL coercion remain explicit. LIKE/NOT LIKE now
retain raw String/NULL truth under an explicitly selected wildcard mode. Non-NULL
raw matching also feeds complete units to explicitly selected UTF-16 providers,
retaining policy routing and refusing odd byte lengths. Non-NULL String
comparisons also use an explicitly selected [raw-unit provider](RAW_STRING_COMPARISON.md),
retaining separate SQL space rules and stored field policies. [Raw keys](RAW_STRING_KEYS.md)
now share the comparison boundary for grouping, sets, sorting and TOP ties, with
retained fixed crosstab cells and separate heading publication. Additional consumers,
native locale providers, native matching qualification and P0 semantics remain
unfinished.

Qualified [query cells](RETAINED_STRINGS.md) now retain core's checked String byte
ranges through derived rows, UNION ALL, correlated subqueries and First/Last.
Raw-aware consumers operate without premature Unicode publication; ordinary
scalar services and final results remain strict. Key comparisons now use explicit
providers while preserving strict type rules. Stored Text, source/parameter
contracts and native qualification are unchanged. Remaining raw consumers, wider
Variant/coercion and the rest of P0 remain unfinished.

StrComp binary mode now compares [complete raw UTF-16 units](STRING_COMPARISON.md#lossless-binary-string-operands)
without Unicode publication. [Database mode](STRING_COMPARISON.md#lossless-database-string-operands)
now also accepts raw units through an explicitly selected UTF-16 provider.
NULL/mode ordering, field-policy routing and separate text-mode providers remain
explicit. Native locale providers and Windows qualification remain unfinished.

Nz/Switch/Choose now retain [lossless choice payloads](CONDITIONAL_FUNCTIONS.md#lossless-nz-switch-and-choose-choices)
through their existing eager evaluation and control conversion. Selected NULL,
branch policies/declarations, quotas and failures remain explicit. Raw relational
carriers, general coercion/result promotion and native qualification remain open.

IIf now retains [lossless String branches](CONDITIONAL_FUNCTIONS.md#lossless-iif-branches)
through eager evaluation and selection, allowing observations/inspection and final
publication to happen at their owning consumers. Branch policies/declarations and
operational failures remain explicit; broader conditional/raw relational and native
qualification work remains unfinished.

IsNull and SQL IS NULL/IS NOT NULL now observe raw String identity without Unicode
publication, including odd byte ranges and unpaired units. Operational failures
remain failures. This closes a nullness-consumer gap, not broader comparison,
relational storage or native qualification; see [raw NULL tests](BYTE_STRING_FUNCTIONS.md#null-identity-without-unicode-publication).

Count/DCount now observe [lossless String nullness](EXPRESSION_AGGREGATES.md#counting-lossless-string-expressions)
without Unicode publication. This closes unnecessary decoding in count inputs;
raw relational carriers, other aggregate semantics and native qualification
remain unfinished.

The six non-addition numeric operators now admit lossless String intermediates
before NULL propagation. Non-NULL conversion retains strict Unicode publication,
numeric policies and original declaration categories. Independent arithmetic
controls and focused expression/query/CLI checks cover the shared path; broader
Variant, raw relational and native qualification work remains P0. See
[numeric String admission](ARITHMETIC_TEXT.md#lossless-string-operands-in-numeric-arithmetic).

Len/AscW now inspect [complete raw UTF-16 units](BYTE_STRING_FUNCTIONS.md#complete-utf-16-unit-observations),
including nested ChrW surrogates, without forcing early Unicode publication.
Byte lengths, signed unit codes and String inspection remain distinct from
Binary values. Strict final publication, odd-byte Unicode observations and wider
raw relational/operator/coercion gaps remain explicit.

Nested Left/Right/Mid now use the same lossless path, retaining split surrogate
units under Len/LenB, AscW/AscB and inspection. Checked Long counts clamp before
byte multiplication, and source/control admission retains the ordinary slice
ordering. Direct scalar publication remains strict; qualified intermediate cells
now retain raw Strings, while broader key semantics remain unfinished;
see [nested slicing](BYTE_STRING_FUNCTIONS.md#nested-utf-16-unit-slices).
Direct outer slices can now retain raw-producing sources through selection and
publish a representable final range without publishing an invalid intermediate.
Materialized scalar sources retain their borrowed path and existing budgets.

Ampersand now joins complete raw units without early publication, allowing two
surrogate halves to form valid final Text. Scalar-only pairs retain their borrowed
join, while bound raw intermediates retain runtime text/declaration provenance.
Host-supplied Empty becomes empty Text; Error and Missing remain refused. Odd-byte
concatenation, other raw operators and broader raw key semantics remain explicit
gaps. See [concatenation scope](CONCATENATION.md#lossless-utf-16-concatenation).

Explicit CStr now preserves existing raw string ranges without decoding or
allocating another payload. Scalar conversion states/policies, odd-byte Unicode
observation refusals and strict final publication remain unchanged. This closes
a conversion-path gap, not raw relational carriers or broader Variant/native
semantics. See [CStr string inputs](CSTR.md#lossless-string-inputs).

The String/String branch of plus now retains raw units through joining and
propagates NULL without forced decoding. Mixed raw String/numeric operands still
use strict publication, explicit numeric policies and their original declaration
categories. Scalar/Variant Date overflow and field/subquery text policies remain
covered. This does not complete host Empty, odd-byte joins, general promotion or
native qualification. See [String addition](CONCATENATION.md#lossless-string-addition).

The shared expression API now admits host Empty with operator-specific arithmetic
targets, including rounded Byte exceptions and String identity under plus. It
does not introduce stored/parameter Empty or a SQL constructor. Numeric promotion,
declarations and resource accounting remain in their existing services; broader
Empty subtype/native qualification stays open. See [host Empty scope](EMPTY_ARITHMETIC.md).

The expression API also admits host Empty at relational comparisons, using zero
beside numeric/legacy Date operands and empty Text beside String data. Raw String
comparison retains explicit provider, complete-unit and publication boundaries.
NULL, Error, Missing, work/byte quotas and cancellation keep explicit outcomes.
No stored/parameter Empty or SQL constructor is introduced. See
[comparison admission](RAW_PREDICATES.md#host-empty-comparisons).

Round's digit count now follows the nonnegative Long profile independently of
Decimal storage scale. [Bounded high precision](MATH_FUNCTIONS.md#round-precision-beyond-decimal-storage-scale)
covers tiny IEEE values and extreme counts without an overflowing scale factor or
count-sized allocation. Independent exact-binary/Decimal controls close this
artificial admission limit; native precision/subtype/error qualification and wider
P0 coercion remain unfinished.

RGB now follows the formal Integer channel contract before upper saturation,
closing the earlier Long-envelope admission. Checked fractional/numeric Text
conversion, eager operands and progressive channel validation remain explicit;
native query-wrapper behavior and error precedence remain unqualified. See
[color boundaries](COLOR_FUNCTIONS.md#rgb-integer-channel-boundary).

Shared arithmetic admission now rejects unsupported GUID/Binary/Extended-date and
non-finite floating operands before returning NULL. Supported NULL/Text operands
retain policy-free propagation; eager evaluation failures remain visible through
aggregates and domains. This closes a silent unsupported-carrier-to-NULL path,
without adding those carriers to arithmetic or qualifying native error precedence.
See [arithmetic boundaries](ARITHMETIC_PROMOTION.md).

Attachment metadata queries now use complete projected child snapshots without
loading FileData values or external payload pages. Independent controls cover
valid 3 MiB files across ACE 2007/2016/2019; explicit file-content queries retain
the existing payload limits and complete-read refusals. This closes unnecessary
metadata payload reads, not native FileData query representation or full collection
semantics. See [metadata contracts](NATIVE_TABLES.md#attachment-metadata-projection-snapshots).

| Priority | Area | Completion evidence still needed |
| --- | --- | --- |
| P0 | Value semantics | Declared-expression/Variant result promotion, complete coercion and Error operator/aggregate behavior; independently modeled boundaries and later native comparisons |
| P0 | Native text | Complete mixed-policy precedence, additional locale providers and explicit LCID search implementations; consistent predicates, sets, groups, sort and LIKE behavior |
| P1 | Query composition | Remaining admitted saved/named definitions, heading names and wider native syntax; qualify whether proposed inline TRANSFORM forms are actually accepted by Access before adding grammar |
| P1 | Advanced fields | Native FileData SQL/DAO byte qualification, wider history/collection semantics and calculated-field evaluation/freshness beyond the explicit row-local preview |
| P1 | Storage and scale | Wider column-type/row histories, broader collection sizes and file locality, predictable limits and unchanged originals |
| Qualification | Native Access | Build/locale-qualified values, ordering, errors, saved queries and representative application outcomes on Windows |

[Binary substring searches](STRING_SEARCH.md#retained-binary-string-operands)
now retain raw String units through InStr/InStrRev in both evaluators, including
derived/grouped/correlated inputs and prepared parameters. Shared budgets, eager
failures and provider boundaries remain explicit. [Database-mode raw search](STRING_SEARCH.md#retained-database-string-operands)
now uses a separate opt-in consumed-span contract, implemented by the existing
General models. Text/LCID raw searches still require Unicode publication;
additional consumers, locale providers and native qualification remain acceptance
work.

[Replace](TEXT_REPLACEMENT.md#retained-string-operands-and-results) now retains
complete String units through matching and output, with binary and explicit raw
General providers. Raw-aware observations, derived cells, grouped/correlated
queries and domain retrieval work without premature publication. Eager/control/
state rules, raw byte/work limits and strict final Unicode remain explicit.
Other consumers, complete Variant semantics and native qualification remain open.

[Raw trimming](TEXT_CASING.md#retained-string-trimming) now retains complete
String ranges through LTrim/RTrim/Trim, including unaligned inputs and isolated
units. Only U+0020 edges are removed. Work/cancellation, scalar policies, fresh
result text policy and final Unicode publication stay explicit. Remaining String
consumers, broader Variant/coercion and native qualification are still open.

[String repetition](TEXT_CONSTRUCTION.md#retained-string-construction) now
retains selected complete UTF-16 units through output, with the existing NULL,
count, character/code-page and fresh result-policy rules. Zero results and later
pair repair can publish; nonempty unpaired final output remains refused. This
closes a construction-consumer gap, not broader coercion, native text providers
or P0 qualification. [Retained reversal](TEXT_CONSTRUCTION.md#retained-string-reversal)
now preserves complete String units through observation and repaired publication;
native supplementary behavior remains unqualified.

[Full-width Decimal boundary controls](testing/DECIMAL_ARITHMETIC.md#full-width-rational-boundary-checks)
now independently check 11,560 binary operations over 2,890 operand pairs through
both CLI evaluators. All 23,460 numeric/subtype/failure controls pass, including
all supported scale pairs and signed 96-bit boundaries. The exact Fraction oracle
found no mismatch with the current bounded arithmetic contract. This closes an
independent-evidence gap; declared/Variant promotion, mixed carriers, aggregate
semantics and native Access qualification remain unfinished.

DISTINCT/DISTINCTROW now preserve runtime ORDER BY policy contributors before
discarding duplicate rows. This fixes a concrete sort/TOP propagation defect
without defining native mixed-field precedence; the focused regression batch
passes 84 tests. See [the runtime contract](RUNTIME_TEXT_POLICIES.md).

Literal-controlled IIf/Choose and reachable Switch alternatives now retain
precise successful result descriptors without removing eager branches. Unknown
controls keep conservative metadata; this does not complete native subtype or
Variant promotion. See [selector metadata](CONDITIONAL_FUNCTIONS.md#literal-selection-and-result-metadata).

Dynamic crosstab discovery now retains candidates when runtime policies can
change their equality, then uses the final policy for heading admission and
buckets. Both coarser and stricter late policies work without rescanning or
premature final-width rejection; the focused regression batch passes 120 tests.
Candidate storage remains separately budgeted. See [runtime policies](RUNTIME_TEXT_POLICIES.md).

All-fixed variance/deviation groups now preserve exact differences before their
first binary64 rounding. Four hundred four independent Fraction-model controls
cover large adjacent fixed values, differing scales and unchanged explicit/mixed
floating conversion. This closes a numerical-loss defect, not native precision
or Variant qualification. See [statistics](STATISTICAL_AGGREGATES.md).

GROUP BY now completes runtime policy accumulation per key column before
partitioning when an explicit resolver is used. Late field identities can merge
or separate earlier neutral keys without re-evaluation or extra scans; filtered
rows remain outside the grouping domain. The focused batch passes 133 tests.
See [runtime policies](RUNTIME_TEXT_POLICIES.md).

Uncertain String extrema now select under their complete runtime policy, with
bounded storage and once-only argument evaluation. Six focused regressions and
98 collation/domain/Error tests pass. This closes a provider-contract gap without
qualifying native mixed-policy precedence. See [extrema](EXPRESSION_AGGREGATES.md).

Wider Decimal sums retain exact residuals through cancellation and final rounding
using the existing fixed-total implementation. Twenty-one focused regressions and
125 independent Fraction-model results plus five refusals pass; native Access
precision remains unqualified. See [Decimal sums](DECIMAL_SUMS.md).

Explicit cached query sources now expose qualified stored calculation fields
without formula execution or inferred freshness. Ninety-two independent getter/
query controls pass; ordinary sources keep their prior eligibility. This closes
a cached-read gap without completing formula freshness or native query
qualification. See [cached queries](CACHED_CALCULATION_QUERIES.md).

The consolidated all-feature query regression batch passes 1,041 tests across
190 targets after the current composition/value fixes. The separate
[advanced-field evolution corpus](testing/ADVANCED_FIELD_EVOLUTION.md) passes
60 controls and 18,584 returned-cell comparisons over unchanged original inputs,
including untouched historical roots and changed collections/caches. No reader
fix was needed for this workload. These are offline engineering checks, with
explicit work allowances; the remaining P0/native qualification boundaries below
are still open.

### P0 implementation boundaries reviewed 24 October 2026

| Boundary | Current implementation | Remaining work |
| --- | --- | --- |
| Database text providers | Native General recognition requires LCID 1033, variant zero and version 0 or 1; raw predicates, keys and database StrComp use explicit providers | Additional database orders and native function-specific padding/weight qualification |
| System text / explicit LCID | Opt-in ICU host profiles implement decoded text comparison/search and exact registered LCID dispatch; General database comparison remains separate | Windows NLS/ACE function-specific qualification and broader locale providers; explicit registration does not establish native LCID equivalence |
| Mixed field policies | Runtime policies survive retained cells; distinct required IDs conservatively merge to Conflict | Native precedence rules before relaxing conflicts |
| Value categories | Explicit declaration-sensitive arithmetic now captures decimal integer literal carriers; default preparation is unchanged and unknown provenance is retained | Qualify literal subtypes and complete Variant promotion, Error and result-type matrices before enabling broader defaults |

The [explicit ICU profile](ICU_TEXT_PROFILE.md) has bounded callbacks and 3,032
independent ICU4C controls. It supplies working host functionality while retaining
the native NLS/ACE gaps above; no system locale or numeric LCID alias is inferred.

These are implementation gaps as well as later qualification tasks. Completing
raw String consumers does not complete P0. Windows comparisons remain deferred;
independent implementation can continue without claiming native equivalence.
Live shared-file coherence remains separate from the stable/offline read target.

Composed grouped keys now retain accumulated runtime text policies inside nested
ampersand/plus and CVar operands, including correlated reuse. The string path uses
the same metered result-policy enrichment as scalar cells, so a neutral first row
cannot erase a later contributor. Fresh text-function defaults and explicit global
overrides remain unchanged. Independent model/native-source controls pass across
Jet 4, ACE 2007 and ACE 16-family files; this fixes propagation without establishing
the unresolved native mixed-policy precedence. See [runtime policy scope](RUNTIME_TEXT_POLICIES.md).

The registered application's object-list query now reconstructs its qualified
reserved `msysobjects.Connect` field alongside the original filter and sort keys.
Its VBA calls remain unresolved and explicit execution still fails without
publishing rows. This closes a stored-syntax gap, not the remaining query
composition or native execution acceptance scope.

Stored derived reads now admit a single final standalone terminator through a
lexical prefix and the original embedded-query guard. This closes a source-shape
gap found in the example's report query. Native type-zero parameters now retain
their [VALUE contract](VALUE_PARAMETERS.md), closing the remaining reconstruction
gap in the registered read-query inventory: all 51 read definitions reconstruct
with independent parameter comparisons. This is syntax/metadata evidence, not
execution of missing sources or VBA UDFs. Original query fragments remain unchanged.

[Runtime domain text policies](RUNTIME_TEXT_POLICIES.md) now survive derived/named
outputs and comparison/key consumers. Their conservative merge behavior does not
complete native mixed-policy precedence or supply additional locale providers.

Floating-selected zero/zero now reports Overflow while Decimal division and
integer division/Mod retain DivisionByZero; aggregate/domain consumers propagate
the operational failure. [Floating Sum/DSum](testing/FLOATING_SUMS.md) now retain
cancellation residuals with bounded exact binary totals. Mixed Currency/floating
sums now reuse explicit Double conversion in both encounter orders; floating-selected
integral inputs use the same rounded conversion, retaining
checked fixed prefixes and their documented order limitations. Fixed-point
Avg/DAvg now round directly from their exact rational total to Double, avoiding
an intermediate Decimal quotient. Broader Error/state,
mixed-type promotion and native aggregate qualification remain P0.
Statistical numeric admission now explicitly excludes legacy DATE values before
shared conversion; explicit CDbl serial statistics remain available. Legacy Date [Avg/DAvg](AVERAGES.md#legacy-datetime-means) now average stored
Automation serials with a Double result. Native return-carrier/rounding and
Date/Time Extended average behavior remain unqualified.

Ordinary scalar Len/LenB string-expression sources now use explicit CStr
policies; raw byte lengths and strict Unicode publication retain their boundaries.
This does not complete variable/UDT size overloads or native query coercion.

Explicit numeric expression conversion now admits host Empty as destination zero
or Boolean false. CDec rejects direct Error data according to its distinct
Let-coercion contract; explicit Long code extraction remains available before
Decimal conversion. NULL, Missing, storage admission and query Nz empty-Text rules
remain separate. [Expression-state rules](VARIANT_ERRORS.md#explicit-numeric-empty-and-cdec-error-rules)
record the reviewed specification and remaining native qualification.

Scalar crosstab heading carriers now include Boolean, floating, fixed-point and
both date families under explicit policies. Their native spelling/order is still
unqualified. A built-in marked Partial is not completed by having a callable
implementation; its owning contract retains coercion, locale and evidence gaps.

Coherent reads during concurrent Access modifications, linked providers, wider
ACCDT/package handling and signature verification remain separate backend work.
They are not prerequisites for every offline write experiment, nor are they waived.
VBA execution remains deferred; dependent queries must report the limitation.

Native [larger stored payloads](LARGE_PAYLOADS.md) retain a 2 MiB encoded default;
[explicit encoded reads](LARGER_ENCODED_VALUES.md) reach 32 MiB, with independent
20 MiB OLE/3 MiB incompressible attachment controls. Decoded Memo defaults to
1 MiB, with a separate [explicit 16 MiB policy](LARGER_MEMOS.md); attachment files
also have an independent 16 MiB ceiling. Explicit query Text (up to 16 MiB) and buffered
output (up to 16 MiB) allowances now enable full larger payload projections;
defaults and separate cumulative execution/provider limits remain.

The shared [declared integral API](DECLARED_ARITHMETIC.md) now honors explicitly
supplied scalar/Variant overflow categories. Explicit query declaration provenance now works under `--declared-arithmetic`
(with `--declared-integral` retained as an alias),
including derived/named/correlated outputs, Single-selected and legacy
Date-envelope overflow (including explicit-policy numeric Text operands) and distinct
CDate Scalar/CVDate Variant, reviewed math returns and domain-function Variant
returns. Financial/cashflow Double returns, date constructors/extraction/clocks,
inspection and all currently admitted text signatures now have reviewed
categories. Chr$ retains its distinct Scalar String return identity. Scalar financial calls
now accept explicit-policy numeric Text through shared checked conversions,
retaining typed Euro options and kernel validation order. NPV/IRR/MIRR also
accept explicit-policy Text for scalar rates and supplied guesses; their array
admission and native qualification limits remain explicit. All fifteen math calls
now accept explicit-policy numeric Text at their existing conversion positions;
Round fixed-point retention and Rnd failed-conversion state are preserved.
Hex/Oct/Str also accept explicit-policy numeric Text under their checked
Long/Double targets while preserving exact numeric-source output. Partition
accepts explicit-policy Text in all four controls while retaining its NULL
precheck and ordered Long conversion before range validation. Str now also
consumes standard Error data, host Empty and explicit-policy legacy dates;
query Nz(NULL) retains its existing Text result. Abs/Sgn now use legacy DATE
serials; Int/Fix reconstruct checked Date results with static output metadata.
Hex/Oct also accept legacy DATE and host Empty; Error arguments retain explicit
TypeMismatch instead of implicit code conversion. Math calls now retain Error
data through eager operand evaluation and admit it at each kernel's validation
position; reviewed host Empty zero targets work without adding a SQL constructor.
All eleven scalar financial calls now share ordered state admission, preserving
eager failures, once-only volatile operands and Euro currency/option order.
NPV/IRR/MIRR also retain states through eager control/element evaluation, preserving
strict array admission, period positions and their distinct conversion order.
DateSerial/TimeSerial also preserve eager state operands through ordered Integer
conversion, with existing relative normalization and execution-local year windows.
DateAdd/DateDiff/DatePart likewise preserve states through eager evaluation and
progressive interval/count/week/source admission, retaining NULL and policy rules.
Weekday now validates first-day settings before date-state admission; single-date
conversions and component extraction also retain deliberate Error/Empty/Missing
policies. DateValue/TimeValue host Empty stays explicitly refused.
IsNumeric/IsDate now recognize Error data and host Empty as FALSE after eager
evaluation, retaining operational failures and explicit conversion boundaries.
Numbered Error/Error$ now retain those states through ordered Long admission;
Error data requires explicit code conversion, while host Empty selects zero
without catalog lookup.
Left/Right/Mid now retain eager states through source/position/length admission,
including explicit Empty targets and the existing NULL-source gate.
InStr/InStrRev and Replace now retain states through their distinct control/NULL/
string conversion order while sharing the existing search/provider/output kernels.
StrComp likewise retains states through mode validation, its NULL gate and
progressive string formatting, sharing the original comparison kernel. Full coercion
and native precision/error qualification remain open. Space/String now likewise
retain states through their NULL gate and ordered count/character admission,
including explicit host Empty numeric targets and eager operational failures.
Ten unary string functions now likewise admit host Empty as empty Text and retain
Error/Missing refusal through their existing kernels. Chr/Chr$/ChrW now likewise
retain explicit numeric states; the three named numeric formatters preserve
source NULL, policy and ordered option/source admission without payload copies.
FormatDateTime and calendar-name calls now also retain states through their
selector/ordinal/control/policy stages with explicit host Empty targets.
StrConv now also retains states through flag/LCID/source admission while
preserving CVar and raw string buffers. [Standalone Japanese kana](STRCONV.md#explicit-japanese-kana-conversion)
now converts basic fullwidth letters/iteration marks with explicit LCID 1041;
[Japanese width](STRCONV.md#explicit-japanese-width-conversion) now adds standalone
4/8, ASCII/punctuation/kana pairs and bounded voiced-mark composition; combined
settings, broader width/kana and native NLS equivalence remain open.
Byte producers, observations and InStrB
now also retain explicit states through their control/NULL/progressive source
stages. General Format now retains states through NULL-week, ordered controls,
mask and source admission. Masked host Empty sources remain explicitly
unqualified/refused. Partition and HyperlinkPart now likewise retain states through
their distinct NULL/control/source admission; hyperlink numeric Text selectors
require the explicit number-format policy. Broader raw carriers, Variant semantics and remaining
string/formatting options retain separate state/coercion gaps. CurrentUser,
numbered Error and GUID conversions now also retain reviewed return categories;
provider invocation and session/permission boundaries remain unchanged. Numbered
Error now accepts signed Long codes through a separate checked provider identity;
Error$ retains Scalar String while Error retains Variant. Native messages remain
unqualified. Logical Not/And/Or/
Xor/Eqv/Imp and ordinary relational comparisons preserve those categories through
composition; CVErr retains its formal Variant return. Two known Variant
String/numeric operands now use the reviewed category-order exception. The
explicit profile also selects reviewed effective numeric predicate types,
including checked integral/Text rounding, Single/Double coercion and legacy
Date/numeric selection with Decimal precedence. Extended Date and other owning
comparison profiles retain their existing limits. LIKE/NOT
LIKE, list membership and BETWEEN preserve their result categories; NULL checks
and EXISTS retain Scalar Boolean. Static plan-output categories now compose with left operands for subquery
membership/quantifiers, including empty results. Empty scalar subqueries retain
the same static category; UNION and unreviewed aggregate outputs stay unknown. Len/LenB retain
their formal Variant return category separately from
the Long carrier. Broader static inference and carrier
rules remain P0 work; default queries retain their existing materialized profile.

## Transition to broader writes

Read/query completion does not itself authorize ordinary ACE saves. Existing
Jet 4 fixed Int32 copy updates and ACE 16 non-journalable engineering candidates
are useful foundations, not a general writer. The write milestones remain:

1. Complete applicable field/rule/index/relationship validation and maintenance.
2. Detect required table data macros with qualified presence/absence evidence;
   implement required semantics or block affected writes. Current absence guesses
   cannot grant a write permit.
3. Preserve opaque objects, VBA and unaffected bytes through qualified saves.
4. Implement record resizing, allocation, index splits/merges, INSERT/DELETE and
   remaining relationship actions beyond existing-width previews.
5. Qualify transaction/recovery behavior and native Access save/reopen. Shared-file
   writes additionally need native locking and mixed-client recovery evidence.

Follow [native write limits](NATIVE_WRITES.md),
[ACE engineering previews](testing/ACE16_WRITE_PREVIEWS.md) and
[data-macro requirements](testing/DATA_MACROS.md). Independent work can continue
before Windows is available; production compatibility claims cannot substitute
independent reader results for native Access evidence.


Known fixed-prefix values in deleted-column histories now read through ordinary
native rows, cursors and queries without inventing the deleted storage classes.
Required visible variable tails retain validation; ignored deleted tails and
broader evolution remain unqualified. [History evidence](testing/COLUMN_HISTORIES.md#known-fixed-prefix-qualification)
records immutable original/derivative inputs and corrupted-copy refusals.


[SQL date literals](SQL_DATE_LITERALS.md) now accept U.S./English dates with a
fixed token-year mapping, documented component-order priorities, known month/year
forms and qualified times through pure Jiff-backed recognition. Literal values
remain independent of runtime date policies; explicit reference-year preparation
captures omitted-year dates without scanning. Invalid constants fail metadata
binding even on empty results. Wider time/regional forms and native precision/
error qualification remain open.


[Maximum-width histories](testing/WIDE_TABLE_HISTORIES.md) now independently
exercise 255-column tables, nine schema-width generations, high variable offsets
and 32-byte masks across the four priority layouts. Complete/progressive/projected
reads and sequential/indexed queries retain registered original bytes. This closes
the explicit wide-table coverage gap; wider schema histories and coherent live
shared-file reads remain separate work.


[Malformed wide-row suffixes](NATIVE_READ_FAILURES.md#maximum-width-malformed-suffixes)
now have portable terminal-cursor checks and 280 corrupted-copy CLI controls,
including ID-only/empty projections, complete buffered refusal and exact earlier
streaming publication. This closes the announced wide-row continuation slice;
it does not establish live-file coherence or validate unselected payload values.


QBColor now follows its formal Integer argument boundary before palette range
validation, through the existing shared scalar/Text conversion service. This fixes
Overflow-versus-Domain classification outside Int16 without changing RGB's Long
channel profile. Native query-wrapper and broader coercion qualification remain
open; [the color contract](COLOR_FUNCTIONS.md#qbcolor-integer-argument-boundary)
separates the model from native evidence.


Choose now applies its formal Single argument precision before rounded selection,
including explicit-policy Text and legacy DATE serials. Conversion overflow no
longer becomes NULL; representable out-of-list values retain NULL. Eager failures
and selected carriers remain unchanged. [The selector contract](CONDITIONAL_FUNCTIONS.md#single-precision-of-choose-indexes)
keeps native wrapper/return-promotion qualification separate from this model.


[Query Nz and checked-conversion metadata](QUERY_EXECUTION.md#query-nz-and-checked-conversion-output-metadata)
now reflect empty-Text defaults, known Text ceilings and non-null successful
checked numeric conversions without scans. Explicit fallbacks remove nullability
only when their descriptors establish it. Downstream UNIONs use those descriptors
under existing conversion policies; eager failures and mixed-carrier Dynamic
results remain explicit. Native result typing/Variant promotion is still open.


[Same-carrier selector constraints](QUERY_EXECUTION.md#same-carrier-selector-constraints)
now retain common Text ceilings and a valid Decimal carrier across differently
constrained branches, without converting selected values or scanning records.
Mixed carriers stay Dynamic. This reduces avoidable unknown metadata through
selectors, named/derived outputs and downstream UNIONs; broader native return
promotion and declaration inference remain open.


Calendar-name ordinal/abbreviation/first-day Text controls now share metered
numeric conversion under explicit NumberFormat. Long lexical half-even conversion,
Boolean admission, validation order, request-local settings and buffered failures
are covered through evaluator/query/CLI checks and independent Gregorian/Decimal
controls. [Calendar names](CALENDAR_NAMES.md#numeric-text-controls) remain Partial;
this closes a specific coercion gap without completing P0 Variant/NLS/native
qualification or changing storage/write scope.


Round's typed digit control now shares explicit Long conversion, including raw
legacy DATE serials, before the existing 0–28 range and value admission. This
removes a numeric-only control inconsistency while preserving DATE-value refusal,
eager failures and NULL/Error ordering. [Round controls](MATH_FUNCTIONS.md#round-long-digit-controls)
remain an explicit engineering profile awaiting native qualification.


Logical operators now expose stable successful result carriers during metadata-only
binding. Nullable alternatives are checked explicitly: varying carriers remain
Dynamic, and known admitted NULL-only XOR/EQV results stay NULL-only. Named/derived
and UNION consumers retain the descriptors without record scans or declaration
inference. [Logical metadata](QUERY_RESULT_TYPES.md#logical-result-carriers-and-nullable-alternatives)
closes an inference gap; complete Variant/operator/native semantics remain P0.


Named attachment containers now support direct WHERE IS NULL/IS NOT NULL criteria
using complete membership views, independently of nullable metadata and FileData.
[Presence criteria](COLLECTION_PRESENCE.md) cover parent counts, child display,
joined/derived/grouped/crosstab consumers and explicit failures. This closes a
specific advanced-field query gap; full collection semantics and native Access
qualification remain open.


Runtime domain criteria now reuse typed named-collection presence filters without
multiplying parent records by attachment count. All twelve domain functions retain
their owning aggregate contracts and shared budgets; scalar values, missing-field
NULL behavior and unsupported member syntax remain explicit. [Domain presence](COLLECTION_PRESENCE.md#domain-criteria)
closes a composition gap without widening native payload or write permissions.


Presence operands now compose inside supported scalar WHERE/domain predicates,
using a borrowed current-parent frame and written evaluation order. Comparisons,
selectors, conversions, ranges/lists and optional scalar controls no longer require
a top-level presence atom. Clause/subquery boundaries, eager errors, once-only Rnd,
complete-view validation and shared limits remain explicit. [Composition scope](COLLECTION_PRESENCE.md#scalar-predicate-composition)
still refuses unqualified hidden-member folds, projection and correlated outer
containers; this does not complete native collection or P0 value/text semantics.


Bounded interrupted seek handling now covers native header/image/Agile length
observations and shared absolute reads/writes. Mixed short/interrupted native
cursor controls retain complete output; persistent seek interruption closes
continuation under the original work allowance. [Seek failure evidence](NATIVE_READ_FAILURES.md#interrupted-seeks-and-length-observations)
strengthens storage reliability without qualifying shared-file coherence or new
write behavior.

NULL-only unary math now retains its existing successful-value metadata through
composition and UNION without imposing a numeric conversion on other arms.
Round control validation and eager failures remain active. This closes another
[result metadata gap](QUERY_RESULT_TYPES.md#null-only-math-inputs); native declared
UNION types and full P0 value/text semantics remain unfinished.

Declared unary negation now permits unknown Integer/Long/legacy Date provenance
when the materialized result retains its carrier and both category rules agree.
Unknown widening cases remain refused, and successful results retain unknown
provenance. This removes a false refusal without qualifying native aggregate or
UNION declarations; see [negation agreement](DECLARED_ARITHMETIC.md#unary-negation-with-unknown-provenance).

Explicit declared binary arithmetic now permits unknown provenance where the
existing Scalar/Variant outcomes agree, while retaining unknown output categories.
Widening still requires provenance, and common conversion/arithmetic failures remain
typed errors. The shared numeric/evaluator services avoid repeated operand or Text
conversion work; [agreement scope](DECLARED_ARITHMETIC.md#binary-arithmetic-with-unknown-provenance)
does not establish native declared types or complete P0 compatibility.

Text metadata now retains qualified NULL-only source/count arguments across 23
existing functions, including both search call shapes and raw-byte producers.
Named/derived/UNION composition reuses those facts without scans or providers;
nullable parameters and NULL controls remain distinct. This closes another
[result-inference gap](QUERY_RESULT_TYPES.md#null-only-text-inputs), not native
DAO/UNION typing or complete P0 compatibility.

The [aggregate capture/replay recipe](testing/NATIVE_AGGREGATE_ORACLE.md) now covers
100 populated/empty numeric aggregate shapes with separate native values, runtime
subtypes and DAO field metadata. TableCraft-side inline exercise passes; portable
integrity tests pass. Native capture remains unrun, so existing Count/DCount
carriers and native aggregate qualification remain unchanged.

Stored qualified wildcard projections now reconstruct through a guarded owned
projection parser. The registered application's saved Leszynski join explicitly
executes, with all 933 rows matching independent inert getters and unchanged
original bytes. [Saved-query scope](NATIVE_SAVED_QUERIES.md#registered-qualified-wildcard-query)
retains VBA and native SQL qualification gaps.


Standard [Error relational keys](VARIANT_ERRORS.md#standard-error-relational-keys)
now reuse the existing standard-code comparator for grouping, sets, sorting and
TOP ties, retaining expression carriers across named/domain/crosstab composition.
Opaque codes, Empty/Missing and Error/NULL or mixed-scalar comparisons retain
explicit refusals. This closes a finite key-consumer gap; broader Error aggregate,
Variant promotion and native grouping/order qualification remain unfinished.


## Crosstab grouped cell policy correction

The [runtime group-policy correction](RUNTIME_TEXT_POLICIES.md#crosstab-cell-expressions-and-row-group-policies)
fixes a reproduced wrong result within the admitted crosstab profile: cell
expressions now reuse the complete row group's retained key policies. Bucket
aggregates still use their own rows and operand policies. Named parameter reuse,
correlated keys, NULL/conflicting groups, omitted pivot buckets, explicit provider
controls and no-partial-result failures have focused regression coverage. This
closes an execution-context defect without qualifying native mixed-policy rules.


## Explicit declared mixed-plus operation selection

The [declared mixed String/numeric plus rule](CONCATENATION.md#declared-mixed-string-and-numeric-addition)
now selects addition, concatenation or TypeMismatch from retained Scalar/Variant
categories before conversion. Conservative Dynamic output metadata preserves
plan reuse when execution options change the result family. Default materialized
arithmetic remains unchanged. This closes a missing explicit declaration-profile
family; it does not qualify native ACE expression categories or complete P0.


## Explicit linked native source routing

[One-hop linked reads](NATIVE_LINKED_QUERIES.md) now compose explicitly supplied
already-open native sources, including different concrete reader types. Root and
targets borrow one cumulative native allowance with distinct catalog scopes;
metadata-only setup validates selected link declarations and target base-table
identities. Every Source capability forwards to its target, while named queries
remain in the root namespace. Unmapped links fail only when accessed. This closes
the adapter routing prerequisite without opening stored paths, claiming native
Access linked SQL, supporting chains/providers or granting writes. Broader host,
large-file, policy and native qualification work remains explicit above.


## Mixed Decimal division and profile-specific composition

The explicit declared-arithmetic profile now applies the reviewed operator-specific
Double selection to Decimal paired with Double, Currency, legacy Date or numeric
Text. Default materialized arithmetic remains Decimal-first. [Paired result
metadata](QUERY_RESULT_TYPES.md#runtime-profile-sensitive-division) preserves
static default UNION conversions while permitting the different declared carrier
through nested, named, aggregate and crosstab composition. Metadata inference
remains bounded and reads no source records.

The [independent replay](testing/DECIMAL_DIVISION.md) checks exact IEEE results and
separately verifies 120 unchanged default observations. This closes one finite
P0 operator/composition gap; complete Variant promotion, native result typing,
text policy precedence and Access execution qualification remain unfinished.


## Explicit external database source execution

Read-only FROM IN now uses exact host-authorized selectors and immutable revision
keys, checked before scans and across successful schema-discovery stages.
Actual target schemas, text policies, hints, scalar/collection callbacks and
selected-index failures retain ownership. The CLI opens only explicit
`--external DATABASE_TOKEN PATH` targets under the same native allowance as the
root and mapped links; explicit target indexes use `--external-index`.

[Independent getter/model evidence](testing/EXTERNAL_NATIVE_QUERIES.md) records
350 engineering and 375 CLI controls across five profile pairs, with all 75
pinned hashes unchanged. Nested SELECTs choose their own root/external namespace;
these lexical rules remain engineering behavior pending Access comparison.
Qualified external named targets and stored global remote-source reconstruction
now work. Linked chains, wider stored layouts and providers remain unfinished. This closes a direct-source execution gap
without completing P0 value/text semantics or read/query acceptance.


## Exact mixed floating relational keys

Mixed Single/Double key views now widen Single exactly while retaining Double.
This removes encounter-dependent equivalence classes across groups, sets,
ordering/TOP, extrema and pivot matching. Scalar predicates retain their effective
Single comparison; result carriers, declarations, static UNION conversions and
strict family refusals remain separate. Rounded heading-name collisions still
fail instead of merging distinct buckets. See [the contract](NUMERIC_KEYS.md).

[Independent replay](testing/NUMERIC_KEYS.md) passes 2,910 candidate controls,
including 1,584 native controls over original typed Float/Double fields in four
priority-format databases; all 25 selected hashes remain unchanged. Seventy-eight
focused Rust tests and test-aware strict Clippy pass. This closes a concrete
binary-key inconsistency; broader Variant/subtype/coercion, native heterogeneous
grouping, text semantics and the remaining read/query roadmap remain unfinished.


### Native saved external FROM sources

Qualified stored SELECT/crosstab definitions now reconstruct their global
attribute-4 qualifier into exact owned external tokens. Explicit mapped execution
uses the existing external-source facade, including named root dependencies.
Connections remain inert metadata; the native facade admits only absent
connections. Labelled UNION arms retain their existing independent IN scopes.
Per-table qualifiers, global UNION remote rows and malformed metadata remain
refused. See [the saved-source contract](NATIVE_SAVED_EXTERNAL_QUERIES.md).
This closes one persisted composition gap; broader definitions, full P0 semantics and native Access qualification remain unfinished.


### Power zero-negative error classification

Converted zero raised to a negative exponent now reports Domain consistently
through the shared kernel/evaluator and query consumers. Actual operand division
errors, admission/conversion failures, NULL handling and successful Double values
retain their separate contracts. Focused controls cover prepared rebinding,
runtime domain aggregates and bad constant crosstab headings before row scans.
This closes one operator error-class defect, not complete Variant promotion,
error precedence or native Access qualification. See [arithmetic](EXPRESSIONS.md).


### Scalar String/Variant predicate coercion

The explicit declared profile now formats a known Variant numeric/Boolean/legacy
Date peer when paired with a known Scalar String, then uses the original String
comparison policy. Conversion, provider failures, raw units, eager evaluation
and cumulative allowances remain explicit. Unknown categories fail when they can
change dispatch. Both-Variant ranking and Scalar numeric/Variant String coercion
retain separate paths; strict keys and default execution are unchanged. See
[the declared contract](DECLARED_ARITHMETIC.md#scalar-string-compared-with-variant).
Complete declaration inference, Variant/Error behavior, native text precedence
and Access qualification remain unfinished.


### Explicit external named sources

Qualified target saved SELECT/UNION/crosstab reads now retain their lexical owner
across nested queries, domains, parameter scopes and schema discovery. Scoped
identities distinguish identical names and native IDs across files. Explicit IN
uses only operation-root authorized mappings; no stored path opens implicitly.
Closed Eval and target linked-table refusals remain enforced.

The [feature contract](EXTERNAL_NAMED_QUERIES.md) records 196 passing focused Rust
tests and explicit boundaries. Wider query definitions, native heading semantics,
P0 value/text behavior and native Access comparisons remain unfinished.
