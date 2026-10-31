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

This initial profile refuses indexes, relationships, final NULL fields, unknown
properties, opaque or incomplete macro carriers, unrepresented event names,
body/module content, unsupported actions and required after-effects. Tables
without a qualified positive event are not admitted: successful creation or an
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

A fresh catalog/map census excludes definition pages, other table DATA, index,
long-value and allocation-map storage. Selected DATA pages must have qualified
owned and GLOBAL allocation roles. The independent proof recaptures BEFORE,
rediscovers and evaluates the original requests, overlays only permitted Int32
payload bytes and compares the entire AFTER image and changed-row count.
Staged patch lists are not proof inputs.

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
