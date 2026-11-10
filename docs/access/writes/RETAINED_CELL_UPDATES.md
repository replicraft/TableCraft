# Cell updates across retained table storage

The normal reviewed cell editor can update a qualified unindexed LONG, Short
Text or Currency field after table replacement, subsequent table creation and retained row
insertion. This separate ACE16 source family uses actual catalog/allocation
ownership rather than the original eleven-page, definition-7 geometry. Desktop
and MCP consume the shared runtime and Access adapter; no native editing logic
was added to the desktop host.

## Boolean value-bit edits

TCNI20 adds an exact Boolean action over the existing complete retained-row
source certificate. The selected Boolean must already have a stored bit in the
record's declared native column count. A reader's default for an absent later
field is not permission to materialize it through this action. NULL and other
scalar carriers are refused.

Boolean occupies no fixed payload bytes. The native field number identifies its
value bit in the record mask; false clears that bit and true sets it. Its unused
fixed-offset metadata does not extend payload geometry. Planning and independent
proof preserve the original column count, footer, payload, slack, every other
mask bit and the complete protected image complement. Non-Boolean fixed offsets
retain their ordinary overlap and extent checks.

The current retained-cell capability combines Boolean, Currency, LONG and Text
from one qualified source. The actual target selects TCNI20, TCNI19 or TCNI17;
TCNI18 keeps its separate opaque Int32 materialization scope. No failed action
is retried as another family. The original Boolean wire carrier remains canonical
0/1 with an explicit TCNI20 discriminator; V3 recovery performs fresh native
reconstruction. Older preparation APIs refuse it, and journal-only decoding
provides no native write authority.

The shared UI proposes checkbox changes through existing runtime review/save
services. Clicking, Space and accessibility Click do not mutate the displayed
source or apply a save. See [the desktop contract](../../ui/DESKTOP.md#native-yesno-display-controls).

## Exact Currency edits and mixed tables

TCNI0019/profile19 supplies a separate single-Currency-cell action for complete
retained row frames. It reuses the existing retained-row source certificate,
actual catalog/allocation ownership and complete-record admission. No additional
program-absence purpose, INSERT encoder or crate is introduced. Unknown required
programs, indexed targets and opaque record geometry remain refused.

A source capability check admits a genuinely qualified LONG, Text or Currency
editor from one captured source. Planning selects the action from the actual
edited column: Currency uses TCNI19, while LONG/Text keep TCNI17. It never retries
a failed proof under another family. TCNI18 retains its separate Int32-only opaque
tail scope. Mixed tables therefore keep existing Number/Text edits when Currency
becomes available.

Currency writes use the existing exact scaled-integer codec; the independent
proof reconstructs the selected eight bytes separately. NULL preserves ignored
payload bytes, and no-ops retain the complete original image after full admission.
Native identity, mask, variable footer, unrelated cells, siblings, indexes,
counters and the complete protected complement remain checked. Display rounding
never becomes edit input or changes stored precision.

The retained original-request grammar already carries exact Currency values.
TCNI19 adds an exact profile discriminator and native reconstruction, without
changing TCNI17/18 decoding or action admission. The checked V3 owner replays the
explicit family before recovery or settled confirmation. V1/V2 preparation refuse
it; journal-only builds decode original data but cannot enable native mutation.

Actual GUI coverage distinguishes two catalog dialects. A newly created packed
catalog without `MSysDb` already uses the ordinary scalar route, even after adding
a sibling. The new retained case creates `MoneyTable` on a settled `MSysDb` cohort,
then inserts default/supplied Currency, edits, sets NULL, refills and reopens. A
mixed appended table edits Currency, LONG and growing Text in sequence. Every
save is compared with an independent complete image; typed four-decimal values,
siblings and scratch SQL survive. A page number or sibling alone does not prove
which native family was selected.

### Representation references

[Microsoft's Access field types](https://support.microsoft.com/en-au/access/data-types-for-access-desktop-databases)
describe eight-byte Currency storage with four decimal places. The
[provider Currency contract](https://learn.microsoft.com/en-us/office/client-developer/access/desktop-database-reference/datatypeenum)
describes the signed integer scaled by 10,000; provider enumeration values are
not ACE storage tags. [Jackcess ColumnImpl](https://raw.githubusercontent.com/jahlborn/jackcess/master/src/main/java/com/healthmarketscience/jackcess/impl/ColumnImpl.java)
independently reads/writes a long at scale four. These are format references;
no third-party implementation or fixture was copied. The native codec and core
Currency carrier already existed.

SQL examples use the existing typed assignment contract, for example
`UPDATE MoneyTable SET Amount=CCur(-2.0001) WHERE ID=2`. Bare fractional literals
produce Double carriers and remain outside current implicit assignment support;
see [SQL stored assignment](../../query/QUERY_EXECUTION.md#sql-stored-assignment).
That remaining query compatibility gap is separate from Currency persistence.

### Local verification

The selected native/request/recovery batch passes 43 tests. The three focused
native Currency tests also pass after the independent mixed-property dictionary
order correction, covering exact scaled extremes, NULL/no-op/conflicts, twenty
user/catalog forwarding hops, whole-image protection and same-table interleaving.
Two real SQL save-session tests pass six save/acknowledgement transitions. Three
connected Currency GUI journeys and two existing Currency view controls pass;
eight retained schema/row GUI regressions passed in the same initial batch.

Three journal-only data/refusal tests, minimal journal-only Access/runtime and
creation-only ACE checks pass. Strict all-target/all-feature Clippy for ACE,
Access, runtime, UI, desktop and MCP, ACE rustdoc and repository guards pass.
These are local macOS ARM64 headless and independent-byte results; they do not
qualify Windows or Microsoft Access save/reopen parity. Saved-field Currency ADD,
indexed Currency edits, other scalar actions and generic imported schema editing
remain separate work.

## Opaque retired tails

After deleting the last non-key fields and adding a new Number field, an old
record can retain a tail whose former field types are no longer known. Reading
its surviving fixed values does not qualify the complete-record serializer.
TCNI0018/profile18 supplies a distinct LONG materialization route for this case;
TCNI17 remains closed to ambiguous tails. Indexed targets and other target
carriers remain unsupported by this route.

Selection positively examines paid original row geometry before choosing the
route. A separate required-program/source certificate checks the complete
inventory, native declarations, constraints, indexes and original records. The
new route requires an actual ambiguous record; it never retries a failed TCNI17
qualification. Explicit unsupported-source results keep the editor unavailable;
malformed data, cancellation and exhausted caller budgets still propagate.

For an ambiguous selected record, the producer retains the entire original
prefix before its null mask. It changes only the native logical count and the
selected declared four-byte LONG range, and appends a complete retained-slot
trailer and native-number mask. Other original prefix bytes, including retired
footer bytes, remain opaque and preserved. No deleted type or old footer is
inferred. Checked native offsets cannot overlap a surviving field, and the new
record must fit actual DATA-page geometry. NULL does not invent a default;
exact no-ops retain the complete original image.

Independent append-based reconstruction, whole-image comparison, fresh visible
values and the existing placement/index/sibling protections qualify the result.
Complete selected records retain their existing serializer when other records
remain ambiguous. After materializing the final ambiguous record, subsequent
operations can select TCNI17 through its own positive admission.

The original-only request has exact TCNI18 framing and the native file envelope.
Checked V3 preparation/recovery reconstructs this family explicitly; old direct
TCNI17 decoders and V1/V2 preparation cannot reinterpret it. GUI, MCP and SQL
consumers use shared services; desktop implements no native byte mutation.
Insertion while other original records remain ambiguous uses the distinct
[TCNA9 source proof](NORMAL_ACE_INSERT.md#insert-after-opaque-retired-tails),
leaving the older INSERT source languages unchanged.

Local macOS ARM64 verification passes 139 selected retained/current-row native
and recovery tests, with twelve existing ignored cases unqualified. This includes
three opaque-row native cases and five exact request/recovery cases, covering
fixed and variable retired histories, long forwarding chains, NULL/no-op/stale
witnesses, protected prefix/footer bytes and the transition back to complete rows.
Two real offline SQL/save/recovery session tests and one typed unavailable-source
control pass. Three journal-only request/refusal tests pass without native
creation support. Strict Clippy across ACE, Access, runtime, UI, desktop and MCP,
ACE rustdoc and creation-only compilation also pass. These checks establish local
implementation behavior, not Microsoft Access save/reopen parity.

## Source and preservation

Original metadata selects retained image capture before whole-image allocation.
Selection supplies no write permission. The current source is an unencoded
ACE2016/subversion-zero file with the qualified system roles, local tables and
explicit stored Name AutoCorrect OFF. Unsupported application objects, programs,
nonempty permission/relationship roles and broader storage declarations refuse;
general imported-application writes remain unqualified. The consuming planner
requires a complete source-bound storage inventory and a distinct retained-row
program-absence certificate. It checks actual target properties, constraints, declarations,
indexes, row ownership and expected original values. Missing/unknown required
programs are not treated as absent. Existing scalar/read compatibility does not
authorize a mutation.

The operation currently changes one unindexed LONG, Short Text or Currency cell. Native
field numbers, fixed offsets, variable-slot history and null masks remain distinct
from visible column positions. Supplied NULL and empty text are separate; defaults
are not applied to UPDATE. No-ops preserve original representations, including
opaque bytes behind NULL. An expected-value mismatch refuses rather than editing
a different record or accepting the latest value silently.

The existing serializer and placement proof rebuild the requested cell and its
record. Growth may use proven existing DATA capacity and a hidden forwarding
target. It does not allocate another page. Independent proof compares the full
replacement image, preserving sibling cells/tables, retired records, previous
hidden targets, property history, allocation maps, indexes and generated counters.
No page retirement or compaction is implied.

Retained row, catalog, LVAL and property-ledger traversal uses paid dynamic cycle
checks and per-hop owner/scope validation. It has no separate fixed forwarding-hop
quota. Catalog terminal aliases and LVAL/data overlap remain checked. This new
proof path does not silently widen older mutation families or turn ordinary
read observation into write authority.

## Original requests and recovery

TCNI0017/profile17 records original column spellings, fixed/variable families,
logical row ordinals and exact expected/replacement carriers, bound to the whole
BEFORE image. It records no final patch, generated SQL or execution permission.
Its counts and lengths use actual wire widths, with fallible allocation,
cancellation and cumulative caller budgets. It does not inherit the older 16K
assignment, 8 KiB carrier/name or 64 MiB image/request ceilings. Native field and
physical storage restrictions are checked independently of inert request data.

Retained and legacy request grammars remain explicit. The generic update decoder
selects TCNI17/18/19 from exact framing; older direct decoders and encode methods cannot reinterpret
its retained body as their own grammar. Fresh reviewed requalification preserves
the selected family and reconstructs the complete BEFORE/AFTER under separate
native/proof allowances.

Explicit `prepare_retained` uses the V3 journal. Old V1/V2 preparation and recovery
routes refuse this family. Retained recovery owns the same checked V3 session,
verifies the original binding, rebuilds the native operation and compares both
complete images before recovery or settled-image confirmation. Namespace setup,
exclusive offline ownership, retirement and acknowledgement remain with the host.

## Verification and remaining work

Four native retained-update tests and twenty-four fitted-intent/recovery tests
pass on macOS ARM64, including seven new TCNI17 data/recovery controls. Five
LVAL/declaration ownership regressions, three original current-row UPDATE tests
and nine append/inventory/selector regressions also pass. These controls cover
Number/Text values, NULL, empty text, shrink,
growth, repeated forwarding, stale witnesses, no-ops and opaque NULL preservation.
Native long-chain cases cover twenty added user/catalog forwarding hops, separately
and together, plus cycle/owner refusals. Older fixed-depth physical APIs remain
separate and are not used as the new route's compatibility ceiling.

Three headless desktop journeys use actual review/apply/acknowledgement and fresh
reopen, compare complete saved files with independent raw images, and preserve
siblings and scratch SQL. Thirty-nine existing GUI tests, eleven Access read/query
tests and twenty-nine Access update/recovery regressions pass. These are local
implementation and independent-byte checks; native Windows and Microsoft Access
save/reopen comparisons remain unqualified. Strict all-target/all-feature checks
pass for ACE, Access, runtime, desktop and MCP, along with journal-only minimal
ACE/Access feature checks and repository formatting/layout/dependency/link guards.

A separate [retained DELETE route](NORMAL_ACE_DELETE.md#delete-across-retained-tables)
now handles qualified row removal and subsequent insertion without resetting IDs.
Table/field changes, indexed-cell changes, other target carriers, new-page
allocation and required-program execution remain separate work.
