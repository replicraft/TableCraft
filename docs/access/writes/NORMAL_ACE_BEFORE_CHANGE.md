# Journaled ACE BeforeChange updates

The optional `journal` engine profile now supports normal recoverable updates for
bounded ACE 16 table profiles. The Access adapter exposes it through
`NativeJournalEditor` and the existing retained `OfflineWriteSession`;
[`query journal-update`](../../cli/JOURNAL_EDITING.md) supplies the CLI host.
Ordinary fresh-copy `query update` remains Jet 4 only. Engineering candidates
cannot be converted into normal plans or recovery permission.

## Admission

The source must be an unencoded, standard-capture ACE 2016/2019 layout with
subversion zero. The selected table must have a supported ordinary fixed Int32 target, complete
native event coverage and a positive pure BeforeChange program.
Native catalog/Extra references select the actual XML event, rather than a
caller-selected property alias. Complete dormant BeforeDelete definitions are
compiled but do not execute during UPDATE.

The indexed extension admits checked ordinary Int32 indexes with complete
non-NULL row membership and qualified existing page topology. Compressed
prefixes and interior nodes are supported without changing page identities,
child links, owned maps or each leaf's original entry count. Both separate-tail
nodes and nodes with an explicit final child are retained. Ascending and
descending components, composites,
consistent logical aliases and final unique-key checks are supported. Aliases
share one physical rewrite. Final uniqueness is checked over the complete result,
so an atomic key swap need not satisfy an intermediate assignment order.
For the all-ordinary-Int32 index-rewrite route, every field of every original
row must be ordinary non-NULL Int32, including unselected rows and non-key fields.

Mixed schemas use real scalar projections, retaining unavailable declarations
without inventing NULL cells. Ordinary complete scalar tables can now maintain
checked fixed-Int32 indexes, including unchanged NULL components. Readonly Int32
AutoNumber keys may participate, while user assignments and dormant SetField
targets still cannot write generated fields. Final user and macro changes compose
into one fitting key update, including descending composites, unique swaps and
retained compressed trees. Every affected INDEX page is excluded from declared
and live long-value claims before staging.

The unindexed route continues to preserve admitted scalar and rich siblings.
Original key-presence facts and the union of native static SetField targets plus
the supplied user target select the route before new INDEX ownership parsing or
event-row evaluation. A nullable I32 index with no potential indexed target keeps
the previous unindexed qualifier and default compiler policy. Present indexed
targets use the checked route, preserving untouched NULL components and their
inactive payloads. Neither route grants NULL transitions, catches a failed
planner or encoder, or retries another implementation.
Rich/calculated/unavailable siblings retain
unindexed admission only; header-5 native-20 siblings remain unsupported.

The independent index proof reconstructs variable-width NULL/present components
before checking exact ROW/child trailers, retained node bounds and final keys.
All-NULL keys retain their physical ROW suffixes; partial-NULL tuples retain
their non-NULL identity. The authored all-NULL unique pair is a read hypothesis,
not native Access uniqueness or save qualification. See the
[frozen source contract](../../testing/ACE16_NULLABLE_KEY_BEFORE_CHANGE_SOURCES.md).

Non-Int32 field operands refuse. Changed fields must remain outside calculation/
collection dependencies and have present original/final Int32 values. Native
field numbers must currently match the complete schema order. Unchanged NULL and
present-empty sibling values retain their bytes. Ignore-NULL key rewriting,
non-I32 indexed targets, record resizing and index allocation remain unsupported.

Relationships, splits, allocation/map changes, unknown properties, opaque or incomplete
macro carriers, unrepresented event names, body/module content, unsupported
actions and required after-effects refuse. Tables without a qualified positive
event are not admitted: successful creation or an empty macro inventory is not an absence certificate.

All supplied expected values must match the original snapshot. Repeated or
missing row ordinals refuse. Any supplied equal expected/replacement assignment
refuses with `EventTiming`, including equal assignments within a mixed batch.
Native equal-assignment event timing has not been established. A truly empty
request still completes native discovery and program preflight. A non-equal
request whose program restores the original row is allowed and reports zero
changed rows.

## Planning and recovery

Original requests enter the existing pure field-program preparation service.
Sequential SetField assignments see the current row; Old retains the original.
RaiseError rejects the complete operation before journal creation. No program
may invoke external effects. All resulting field patches compose into one plan;
final constraints validate the complete result.

A fresh catalog/map census protects definition pages, other table DATA,
long-value and allocation-map storage. Selected DATA and admitted INDEX pages
must have distinct qualified owned and GLOBAL allocation roles. Staging derives
full key changes from final field values, including macro-only targets, and
rewrites each affected physical index once. Existing-page fitting serialization
is selected before preparation; a failed encoder never triggers another profile
or a split. A prefix may shrink when the complete entries still fit the same
page. Nodes receive separators derived from each final child's complete maximum
key and row reference. Entirely unchanged indexes preserve their original bytes,
including valid retained upper bounds.

The independent proof recaptures BEFORE, rediscovers and evaluates the original
requests, reconstructs permitted Int32 payloads and complete tree keys, and
compares the entire AFTER image and changed-row count. It independently checks original and
final complete row membership, key directions, aliases and final uniqueness.
The proof checks complete original owned-page reachability, child bounds and
sibling links, then independently partitions final keys by original leaf counts
and reconstructs compressed entries and node separators. Headers, topology,
map/counter storage and unused or vacated bytes remain protected.
Staged patch lists and staged key deltas are not proof inputs.

`NativeUpdateIntent` uses closed version/profile dispatch: existing Jet 4 records
retain byte-identical `TCNI0001`/profile 1; this ACE route uses `TCNI0002`/profile 2.
The record retains the original source digest and user requests, including the
user replacement when a program later restores the original. It contains no
final rows, caller macro selectors or absence permission. Both active recovery
and retired confirmation reconstruct the selected native plan and compare both
complete images before accepting the journal. Unknown versions/profile pairs
refuse without trying another validator.

The same exclusive offline target survives planning, commit, recovery and
successive edits. Existing explicit journal ownership, durable namespace setup,
retirement and known-outcome reporting remain required. Filesystem integration
currently targets macOS and Linux; this is not Access shared-file coordination.

## Shared mixed source and independent proof

Shared projected source validation, logical record roots and long-value ownership
now live below the preview-only mutation modules. Engineering consumers use those
owners directly; there are no forwarding compatibility re-exports. The normal
`journal` feature does not require `ace16-write-preview`. Existing engineering
source admission remains distinct, and the public complete-row BeforeChange
preparation API keeps its earlier strict profile.

Mixed planning compiles all native-selected definitions before supplied assignment
timing checks. It stages only eligible original fixed payloads, then replays the
original requests against a fresh source. The proof independently binds readable
Int32 operands to original raw rows, overlays literal little-endian values and
checks final original-property constraints, changed-row count and the complete
image. Index pages, counters, variable records, masks, long values and unrelated
objects remain exact. Recovery retains the same original TCNI0002 request record;
it obtains no caller-selected programs or final-row authority.

## Evidence and limits

[Original sources and independently read modeled outcomes](../../testing/ACE16_NORMAL_BEFORE_CHANGE_SOURCES.md)
were frozen before normal writer calls. Ten admitted complete images pass both
native planning and real offline SQL session commits. Native tests additionally
exercise framing states, restart, intent/image tampering, resource exhaustion
and cancellation. Separate direct-proof tests reject mutations outside permitted
payloads. The [execution record](../../testing/results/ace16-normal-before-change-2026-10-31.json)
separates those checks from retained legacy regressions.

These are bounded engineering results. Read-only independent SDK inventories do
not execute the programs or establish Access event timing. Native Access
execution/save/reopen, power-loss behavior, encrypted editing and broader normal
ACE UPDATE/INSERT/DELETE remain unqualified or unfinished.

The [indexed execution record](../../testing/results/ace16-normal-indexed-before-change-2026-10-31.json)
adds eighteen in-memory journal commits, four independently qualified tie images,
32 typed/SQL offline saves and sixteen complete CLI saves. Direct proof checks
cover index-only forgery, original ownership contradictions and final alias
constraints. A generated signed-tuple ordering law independently checks the key
wire across numeric directions. Existing unindexed and Jet 4 regressions pass;
the retained final lint failure was disk exhaustion, followed by a passing retry.

The [retained-tree execution record](../../testing/results/ace16-normal-tree-before-change-2026-10-31.json)
adds eighteen complete tree/prefix images through native commits and recovery,
36 typed/SQL full-image saves and eighteen matching CLI files. Same-lease
continuation tests rebind Old and row handles across successive saves. Separate
controls retain a loose index bound while a non-key field changes. Source
qualification includes 108 independent reads before expanded writer calls;
observed trees have height one, while recursive code retains explicit depth,
resource, ownership and original-topology checks.

The compressed/tree source continuation was independently qualified before its
producer calls: eighteen originals and eighteen modeled images cover retained
prefixes, fitting prefix loss, height-one nodes, both final-child forms and an
unchanged loose upper bound. All 108 read-only SDK observations and complete
saved comparisons pass. The implementation traverses bounded trees recursively;
these observations do not qualify every historical tree variant or deeper
layout. Read-only source comparisons do not establish Access event timing.

### Mixed-schema continuation

The [frozen mixed sources](../../testing/ACE16_MIXED_BEFORE_CHANGE_SOURCES.md)
provide sixteen complete ordinary outcomes and two separately observed rich
projection outcomes. Seven native/journal tests and four direct independent-proof
tests pass. Offline sessions pass sixteen typed and twelve SQL complete-image
saves, plus stale/foreign handle controls. The four fixed signed-zero cases use
typed ordinal handles because supported SQL predicates cannot distinguish their
two rows. Two explicit rich journal-editor plans pass without enabling preview
writers. The rich SDK catalog-open refusal remains separate from those native
observations. Twelve CLI commits and twelve complete TSV readbacks also pass on the unchanged
ordinary variable/counter sources. General mixed writes and native Access
qualification remain open.

The [mixed execution record](../../testing/results/ace16-normal-mixed-before-change-2026-10-31.json)
retains feature-boundary compilation corrections separately from first passing
native/session outcomes and exact CLI/save/readback qualification.

### Indexed mixed-schema continuation

The [indexed mixed sources](../../testing/ACE16_INDEXED_MIXED_BEFORE_CHANGE_SOURCES.md)
freeze twelve complete outcomes before producer execution. Six native/journal
and four independent proof tests pass; real typed/SQL sessions match all twenty-
four saves. Twelve complete CLI saves and twelve exact TSV readbacks also pass.
An independently derived nullable-index control demonstrates prior
unindexed proof admission, reproduces the initial routing regression and passes
after original-key classification. Four real nullable-index saves preserve the
NULL row, inactive payload and complete old index; indexed Id updates refuse.
The existing TCNI0002 recovery route rederives the same selection from BEFORE.
Native Access comparisons remain unrun.

The [indexed mixed execution record](../../testing/results/ace16-normal-indexed-mixed-before-change-2026-10-31.json)
separates source qualification, initial positive saves, the reproduced nullable
routing regression and its retained compatibility controls from native Access
qualification.
