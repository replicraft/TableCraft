# Normal EOF DATA growth

The ordinary variable-update route can append one 4096-byte DATA page when
qualified original records cannot fit their current capacity or the available
already-owned DATA pages. Admission requires the native 17-field catalog profile,
a freshly derived `NoRequiredTablePrograms` certificate and the existing complete
Source ownership, declaration, constraint and record checks. This does not extend
positive data-macro execution or infer absence from an empty inventory.

Selection binds every original target and expected value, encodes the requested
records and reserves their placements before constructing a candidate. Automatic
selection keeps TCNI0005 for fitting records and TCNI0006 for already-owned-page
growth. TCNI0007 is selected only when a real reservation uses the single new EOF
page. Explicit EOF planning rejects fitting, no-op and empty requests rather than
appending an unused page. Failed qualification never retries another planner.

The new page must be representable in the original GLOBAL FREE, selected ordinary
owned and candidate maps. Their three distinct qualified cells must initially
describe FREE, not owned and not a candidate. Planning clears the GLOBAL FREE bit
and sets the owned and candidate bits, preserving all other map bytes and existing
hints. Physical map storage, aliases, other owners and live LVAL storage remain
protected. Allocation requires existing representation; it does not create map
storage or expand a bitmap window. AFTER is exactly BEFORE plus 4096 bytes and
must remain within the 64 MiB native image ceiling.

TCNI0007/profile 7 retains the original fitted-group body: table and column names,
snapshot-local rows, exact expected/replacement carriers and the complete BEFORE
length/digest. It stores neither placement decisions nor a certificate. Recovery
requires the sealed `CheckedV2` returned by inspecting the same loaded journal,
verifies the original binding, freshly rebuilds the native candidate and compares
both complete journal images before effects. Pending/interrupted recovery restores
BEFORE, while committed recovery requires exact AFTER. Durable completion
confirmation checks and synchronizes without rewriting or resizing the target.
The fixed V1 `prepare` entry refuses profile 7 before effects; `prepare_update`
selects the real resizable V2 preparation family for profile 7 and retains V1 for
profiles 1–6.

## Independent evidence

The [first EOF sources](../../../tests/reports/storage/ACE16_EOF_ALLOCATION_SOURCES.md) qualify
four original-derived BEFORE/AFTER views through native and independent SDK reads
before the corresponding producers. A separate compound corpus reuses two
already qualified two-row originals and adds two literal AFTERs: one row uses the
last available owned slot and another the new EOF page. Four additional native
and SDK source views precede those producers. Valid source and wire goldens are
unchanged.

Four direct physical controls cover complete images, exact/one-less resource
pools, cancellation, map aliases, unavailable claims and unused-allocation
refusal. Three codec tests retain literal versions 5/6 and strict version 7
framing. Six native commits match complete independent images, including both
compound request orders. Sixteen pending recovery contexts cover partial tails
and mixed old map/root pages, repeated rollback/truncation and write-free
confirmation. Four committed target mismatches and eight self-consistent native
forgeries refuse without writes or resize. The generic
[V2 filesystem checks](../../foundation/IMAGE_JOURNAL.md#sealed-v2-inspection-and-completion)
retain the same descriptor and journal borrow.

The regression batch passes 1,108 ACE units and 83 recovery functions; the joint
host worktree passes 79 adapter and 94 CLI units. Strict combined, journal-only,
preview-only and minimal builds pass. Twelve focused existing owned-page
regressions preserve prior images and recovery. See the
[write record](../../../tests/results/storage/ace16-normal-eof-allocation-writes-2026-11-01.json)
for exact scope, counts and retained first failures. Retained Access/CLI adoption
has a separate acceptance record.

This is a bounded first EOF DATA-allocation route. General map growth, additional
allocation geometries, broader index allocation and broader INSERT remain
separate work. [Normal explicit-row insertion](NORMAL_ACE_INSERT.md) uses existing
owned DATA capacity without allocation. The independent [normal DELETE continuation](NORMAL_ACE_DELETE.md)
retains allocated forwarding storage without reclamation. These source/read and portable journal checks do not establish
Microsoft Access save/reopen conformance or Windows locking/durability behavior;
those comparisons remain unrun.

## Retained Access and CLI adoption

The existing typed editor, SQL session and `query journal-update` command now
consume this automatic route. The session borrows the same retained target's
explicit resize capability and stores the original intent before effects.
Active recovery selects V1 for profiles 1–6 and V2 for profile 7 before native
qualification; mismatched protocols refuse without fallback. Retired profile 7
uses write-denied ordinary views for complete-image confirmation.

Two typed and four SQL saves, four CLI saves/readbacks, eight real restarted
active/retired contexts, two interrupted-extension same-session rollbacks and
two intent6/V2 mismatch refusals pass. Counts, current-image versus historical
receipt facts, namespace retirement, exact restored length and continuous lease
are checked. See the [host record](../../../tests/results/storage/ace16-retained-eof-allocation-2026-11-01.json).
