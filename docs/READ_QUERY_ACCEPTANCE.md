# Local Access read/query acceptance

Reviewed 23 October 2026. This is a current acceptance checklist, not a claim of
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
  decoder-triggered cancellation and bounded interrupted short reads.
- Buffered collection pages now accept explicit native read limits, retaining
  complete parent/child validation and output refusal across `--all` sections.
- Native CLI reads now use shared OS locks, excluding cooperating exclusive
  writers. Offline/stable input remains required; this is not live Access
  coordination or coherent capture during noncooperating changes.

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

DateAdd now exposes its existing successful date carrier for numeric/Boolean/Text
sources during metadata-only preparation, with source nullability preserved.
Named/derived, UNION, Nz and Avg consumers retain this metadata without scans or
policy activation. Existing Extended sources retain their precision; unknown
source families and native result-type qualification remain separate. See
[result metadata](QUERY_RESULT_TYPES.md).

## Remaining acceptance work

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
| P1 | Advanced fields | Native FileData query representation, history query access, full collection semantics and calculated-field evaluation/freshness rules |
| P1 | Storage and scale | Wider column-type/row histories, broader collection sizes and file locality, predictable limits and unchanged originals |
| Qualification | Native Access | Build/locale-qualified values, ordering, errors, saved queries and representative application outcomes on Windows |

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

Native [larger stored payloads](LARGE_PAYLOADS.md) now admit 2 MiB OLE and 1 MiB
decoded Memo/attachment files. Explicit query Text (up to 1 MiB) and buffered
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
preserving CVar and raw string buffers. Byte producers, observations and InStrB
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
