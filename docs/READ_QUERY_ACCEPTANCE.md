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
- Independent comparisons include 20,000-row sequential/indexed inputs. They do
  not establish arbitrary million-row throughput or native Access equivalence.
  SQL results remain buffered with separate row, byte, work and output ceilings.

## Remaining acceptance work

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
sums now reuse explicit Double conversion in both encounter orders, retaining
checked fixed prefixes and their documented order limitations. Fixed-point
Avg/DAvg now round directly from their exact rational total to Double, avoiding
an intermediate Decimal quotient. Broader Error/state,
mixed-type promotion and native aggregate qualification remain P0.

Ordinary scalar Len/LenB string-expression sources now use explicit CStr
policies; raw byte lengths and strict Unicode publication retain their boundaries.
This does not complete variable/UDT size overloads or native query coercion.

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
including derived/named/correlated outputs, Single-selected overflow and distinct
CDate Scalar/CVDate Variant, reviewed math returns and domain-function Variant
returns. Financial/cashflow Double returns, date constructors/extraction/clocks
and inspection signatures now also have reviewed categories. Logical Not/And/Or/
Xor/Eqv/Imp and ordinary relational comparisons preserve those categories through
composition; CVErr retains its formal Variant return. Two known Variant
String/numeric operands now use the reviewed category-order exception. LIKE/NOT
LIKE, list membership and BETWEEN preserve their result categories; NULL checks
and EXISTS retain Scalar Boolean. Known Variant left operands also establish
Variant for subquery membership/quantifiers without examining returned rows. Len/LenB retain
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
