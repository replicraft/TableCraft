# Local Access read/query acceptance

Reviewed 22 October 2026. This is a current acceptance checklist, not a claim of
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
| P1 | Storage and scale | Unqualified/ambiguous row histories, broader collection sizes and file locality, predictable limits and unchanged originals |
| Qualification | Native Access | Build/locale-qualified values, ordering, errors, saved queries and representative application outcomes on Windows |

Scalar crosstab heading carriers now include Boolean, floating, fixed-point and
both date families under explicit policies. Their native spelling/order is still
unqualified. A built-in marked Partial is not completed by having a callable
implementation; its owning contract retains coercion, locale and evidence gaps.

Coherent reads during concurrent Access modifications, linked providers, wider
ACCDT/package handling and signature verification remain separate backend work.
They are not prerequisites for every offline write experiment, nor are they waived.
VBA execution remains deferred; dependent queries must report the limitation.

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
