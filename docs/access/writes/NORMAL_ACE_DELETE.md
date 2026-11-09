# Normal ACE row deletion

Normal DELETE uses a source-bound engine plan through typed Access editors,
retained offline sessions and `query journal-delete`. Engineering deletion copies
retain their separate admission and cannot become normal journal plans.

```sh
tablecraft query journal-delete 'DELETE FROM Items WHERE [Id]=1' \
  --database offline.accdb --journal fresh-delete-journal
```

## Admission

The route requires the qualified native seventeen-field catalog family in
unencoded ACE2016/2019 subversion-zero images. Every operation freshly derives
[target-local program absence](NATIVE_TARGET_PROGRAM_ABSENCE.md), strict selected
declarations, qualified properties and complete Source protection. Unknown
code/property carriers, linked targets and incomplete ownership refuse. Required
programs and any relationship involving the selected table remain unsupported,
including empty selections. Unrelated objects remain preserved. Positive
BeforeChange UPDATE admission grants no DELETE permission.

Unindexed direct and qualified forwarded deletion keep their existing profile,
including readonly Memo/OLE preservation. The fifteen-field global data-only
family remains outside this deletion route.

Indexed deletion additionally requires a complete ordinary scalar table check
and checked single-column Int32 physical indexes with a qualified root leaf or
a retained tree whose child pages remain nonempty. Every logical alias and physical declaration must
agree. Ascending/descending and stored nullable Int32 key grammar are handled
without reading inactive numeric payloads; required/unique constraints retain
their existing checks. Ignored-NULL indexes, composite/non-I32 keys, foreign
index references and unqualified table/field shapes refuse. Retained trees
refuse requests that would require page retirement or contraction.
This operation removes whole rows and introduces no NULL assignment or generation
policy. The initial literal tranche covers ascending present keys. A separate follow-on
now qualifies descending Guard keys with an actual stored NULL member.

The indexed path proves catalog-wide member and map-storage ownership, excludes
other DATA/INDEX/metadata roles and all live or declared LVAL references, and
groups compatible logical aliases by physical index. It never catches a failed
mutation to choose another profile. Empty requests still qualify every source,
index and ownership obligation.

## Original roots, keys and preservation

`NativeImage::prepare_ordinary_row_deletes` captures exact resolved original
record witnesses for snapshot-local ordinals. Duplicates, missing rows, stale
raw records and ambiguous terminal ownership refuse. Preparation grants no save
permission; planning and recovery repeat admission from the original image.

Only selected logical root slot flags and the table definition's live-row count
change in DATA/TD storage. Qualified forwarding capsules, hidden terminals,
record payloads, readonly long values, other slots and DATA free/slot counters
remain intact. Indexed deletion removes each selected original ROW reference
from every physical root leaf exactly once, retaining the original key/ROW order
of survivors. Result ordinals may compact; physical ROW suffixes do not.

Leaf free space, entry-end bitmap, retained prefix and active encoded entries
are reconstructed within their existing pages. Logical aliases, historical
counters and all unused or newly vacated leaf bytes remain original. An emptied
index retains its allocated root as an empty leaf. Maps and EOF remain exact.
There is no payload erasure, compaction, cascade, LVAL reclamation, page
allocation/retirement or tree contraction.

The fresh proof captures BEFORE again, resolves roots, binds raw witnesses and
manually reconstructs root/count changes. For indexes it independently reads
raw presence/I32 cells, encodes original key+ROW membership, validates original
compressed leaves, filters selected identities and reconstructs complete leaf
bytes. It consumes neither staged patches/keys nor the staging serializer.
Fresh final Source, constraints and index membership must agree with the reduced
row count; the entire candidate must equal the original-derived expected image.
UPDATE's equal-cardinality validator is not substituted.

## Journal and shared consumers

`NativeDeleteIntent` retains ordered original ordinals, complete raw witnesses,
table name and BEFORE binding in unchanged `TCND0001` framing. It stores no SQL,
final patches or absence permission. Existing UPDATE and INSERT action formats
retain their separate dispatch and eligibility.

The recoverable DELETE plan uses the fixed-length V1 journal. Recovery verifies
the original binding, rederives native admission, replans the original requests
and compares both complete journal images before effects. Settled confirmation
checks current bytes without replay. Stored AFTER or decoded intent alone never
authorizes recovery.

`NativeJournalEditor` and `OfflineTableEditor` use the shared `DeletableTable`
contract. Handles belong to one Snapshot; refresh after deletion issues new
handles. `StagedDelete` exposes matched/deleted counts before journal creation
and uses the existing lease, apply, receipt, retirement and cleanup lifecycle.
Failures preserve resources and known outcomes.

`OfflineWriteSession::plan_delete` evaluates all original SQL predicates through
the shared service before consuming the backend once. Parameters, external
sources, joins and hidden providers refuse. CLI reports Transaction, Matched
rows, Deleted rows and Journal namespace after completion/release; output is
allocated before mutation. Existing `recover-update` and `finalize-update`
recognize the separate TCND action without SQL replay or fallback. See
[offline sessions](OFFLINE_WRITE_SESSIONS.md) for platform and namespace duties.

## Evidence and remaining limits

Earlier unindexed direct/one-hop forwarded source and write results remain
recorded in the [native DELETE record](../../../tests/results/rows/ace16-normal-delete-writes-2026-11-01.json)
and [retained host record](../../../tests/results/rows/ace16-retained-normal-delete-2026-11-01.json).
Their property/OLE/forwarding preservation and first failures remain unchanged.

The indexed source tranche is committed as `7fe205f6`: sixteen native read views
and eighty exact independent SDK reports pass for two unchanged genuine originals
and eight literal AFTERs. It covers first/middle/exterior/all-row removal from
both physical indexes and four logical aliases, preserving histories, tails,
maps and EOF. Both exterior request orders share one independently specified
image. See the [indexed source record](../../../tests/results/indexes/ace16-normal-indexed-delete-sources-2026-11-01.json).
The initial broad-selector setup failure is retained: unrelated legacy exporters
were selected without their fresh-output setup; exact source selectors passed
without source/admission changes.

Root's first focused runtime batch passes all seven indexed DELETE native
functions, two Access functions and one CLI function, together with the two
shared leaf cancellation tests. DELETE-specific checks cover:

- Ten complete-image commits, retaining both exterior request orders in TCND.
- Twenty-two recovery contexts, eight committed mixed-target mismatch refusals,
  sixty-four protected-AFTER forgery refusals and eight original-witness refusals.
- Four full recoverable-planner resource contexts (exterior/empty across both
  headers), with exact and one-less Bytes/Rows/Steps budgets and entry/middle/last
  cancellation. Empty admission and malformed flags/physical ownership/leaf
  metadata controls also pass.
- Eighteen principal typed/SQL saves and eighteen empty follow-on saves under
  retained leases, eighteen stale-handle refusals and eight active/retired restart
  contexts.
- Eight CLI deletions, eight zero-match follow-on saves and sixteen exact TSV
  readbacks.

Complete images retain both physical trees, four aliases, historical counters,
inactive tails, maps and EOF exactly as the independently authored outcomes.
Broader regression and feature checks are recorded separately in the
[indexed write record](../../../tests/results/indexes/ace16-normal-indexed-writes-2026-11-01.json). The initial
static extraction review corrected the old preview's two-poll cancellation
cadence before producer execution; earlier private checkpoints remain intact.

Microsoft Access save/reopen and Windows locking/durability comparisons remain
unrun. Broader trees, key families, relationships/programs and reclamation remain
separate work toward ACE16 compatibility. The separate
[normal explicit-row INSERT route](NORMAL_ACE_INSERT.md) retains its own policy.

## Descending and stored-NULL follow-on

Sixteen independent native source views and 80 SDK reports now cover actual
DESC Int32 leaves containing a five-byte NULL entry beside present nine-byte
entries. Ten normal commits match the literal full images, including both
exterior request orders and empty resulting roots. Forty recovery contexts,
eight committed mixed-target refusals, 44 protected-AFTER forgeries and two
original inactive-NULL witness recovery refusals pass. Logical NULL does not
authorize changes to inactive payloads, original masks or vacated entry bytes.
See the [follow-on record](../../../tests/results/indexes/ace16-indexed-write-followons-2026-11-01.json).
No interior-tree or wider-key mutation is established by these results.

## Retained NODE trees

Normal DELETE now uses the existing deletion kernel for fitting retained trees.
It consumes original leaf partitions, removes exact ROW identities and rebuilds
both full key and ROW parent fences in original child order. No surviving row
is redistributed to another leaf. Independent proof uses the separate original
ownership graph and manual page writer, with decreasing-cardinality replay.
Every INDEX descendant receives ownership and live LVAL exclusion checks. Empty
operations preserve original valid fences. Requests emptying a non-root leaf
refuse until retirement is qualified.

Four source views and 24 SDK observations precede four complete native commits
in both request orders. Twelve recovery contexts, 42 protected-image refusals,
full resource/cancellation checks, six retained typed/SQL saves plus six empty
follow-ons, and four CLI saves/readbacks pass. The observed sources have
height-one trees; broader heights and native Access qualification remain
separate evidence. See the [source contract](../../../tests/reports/rows/GENUINE_RETAINED_NODE_DELETE_SOURCE.md)
and [write record](../../../tests/results/rows/ace16-normal-retained-node-delete-writes-2026-11-01.json).


## Int64 and Currency secondary root leaves

Qualified manual Int64 and Currency secondary indexes now compose with normal
INSERT and DELETE. Exact native/scalar declarations, eight-byte storage, dense
field ordinals, nullable/directional wire grammar and complete physical aliases
are checked. Wide keys retain one root leaf; primary-key insertion keeps its
Int32 profile. Ownership, live LVAL exclusions, original row witnesses and full
source/protected-byte proofs remain mandatory. Existing Int32 tree behavior and
UPDATE admission are unchanged.

The action-specific read-fact capture accepts only the checked numeric key
families. It grants no writable-field or journal permission. Independent proofs
reconstruct raw masks, signed key bytes, request-aligned ROW references and
historical distinct-key changes without producer keys or staged patches.

Thirty-four literal source views and 284 exact SDK reports preceded producers.
Thirty-six native whole-image commits, separate direct INSERT/DELETE proofs,
recovery, forged-image and resource controls pass. Typed/SQL sessions preserve
48 literal results and 32 restart contexts; CLI checks cover 20 saves/readbacks
and eight refusals. SQL positives use source-qualified NULL keys; non-NULL wide
values are covered through exact typed/native carriers. These results do not
establish implicit SQL numeric coercions or native Access save conformance.

The [execution record](../../../tests/results/indexes/ace16-normal-integral-indexed-writes-2026-11-01.json)
retains the initial missed action-admission guard and its narrow correction,
complete receipts and strict/broad regression results. Empty/all-NULL, stable-ID
gaps and sparse native ordinals are separate follow-on source work.
