# Journaled ACE BeforeChange updates

The optional `journal` engine profile now supports normal recoverable updates for
one narrow ACE 16 table profile. The Access adapter exposes it through
`NativeJournalEditor` and the existing retained `OfflineWriteSession`;
[`query journal-update`](../../cli/JOURNAL_EDITING.md) supplies the CLI host.
Ordinary fresh-copy `query update` remains Jet 4 only. Engineering candidates
cannot be converted into normal plans or recovery permission.

## Admission

The source must be an unencoded, standard-capture ACE 2016/2019 layout with
subversion zero. The selected ordinary table must have supported fixed Int32
fields, complete native event coverage and a positive pure BeforeChange program.
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
For indexed tables, every field of every original row must be ordinary non-NULL
Int32, including unselected rows and non-key fields.

Relationships, ignore-NULL indexes, splits, allocation/map changes, mixed scalar
schemas, original or final NULL fields, unknown properties, opaque or incomplete
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
