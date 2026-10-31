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

The indexed extension admits checked ordinary Int32 indexes only when each
physical index occupies one owned root leaf with prefix zero and complete
non-NULL row membership. Ascending and descending components, composites,
consistent logical aliases and final unique-key checks are supported. Aliases
share one physical rewrite. Final uniqueness is checked over the complete result,
so an atomic key swap need not satisfy an intermediate assignment order.
For indexed tables, every field of every original row must be ordinary non-NULL
Int32, including unselected rows and non-key fields.

Relationships, ignore-NULL indexes, interior nodes, nonzero prefixes, splits,
allocation/map changes, final NULL fields, unknown properties, opaque or incomplete
macro carriers, unrepresented event names, body/module content, unsupported
actions and required after-effects refuse. Tables without a qualified positive
event are not admitted: successful creation or an
empty macro inventory is not an absence certificate.

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
long-value and allocation-map storage. Selected DATA and admitted INDEX leaves
must have distinct qualified owned and GLOBAL allocation roles. Staging derives
full key changes from final field values, including macro-only targets, and
rewrites each physical index once.

The independent proof recaptures BEFORE, rediscovers and evaluates the original
requests, reconstructs permitted Int32 payloads and leaf keys, and compares the
entire AFTER image and changed-row count. It independently checks original and
final complete row membership, key directions, aliases and final uniqueness.
Headers, topology, map/counter storage and unused leaf bytes remain protected.
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
