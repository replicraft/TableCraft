# Cell updates across retained table storage

## Empty LONG actions

`TCNI0024`/profile 24 is a distinct original-request language for a nonempty list
of named ordinary unindexed LONG fixed groups, each containing zero row
assignments. It uses the complete retained-row source certificate and validates
actual target declarations, every original record, ownership, INDEX/LVAL roles,
constraints and required programs even when no row matched. It does not admit
variable groups, generated/indexed targets, nonempty replacements or opaque-row
sources. TCNI17–23 retain their existing languages.

The original binds the complete native-sized BEFORE image and retains column
names and fixed-group categories. Fresh native reconstruction repeats source and
target admission and requires whole-image identity with changed rows zero. The
adapter chooses this route from actual empty LONG requests before consuming any
cell planner; it never retries a failed TCNI17 request or invents a matched row.
SQL still evaluates predicates and eager failures before backend finish.

Only checked V3 preparation and retained recovery accept this exact family. Equal
BEFORE/AFTER does not erase journal state: pending recovery rolls back, committed
recovery completes, and settled confirmation uses the same qualified session.
Old V1/V2 APIs refuse the new family before effects. The data codec is available
with `journal`; constructive planning and fresh native reconstruction require
`journal` plus `ace16-create`, matching the actual retained source closure.
Journal-only decode and a typed unsupported-source refusal do not qualify native
construction. This narrow ordinary LONG operation does not qualify general
zero-match writes across other field types or native profiles.


The normal reviewed cell editor can update qualified unindexed Boolean, Byte,
Integer, LONG, Large Number, Single, Double, Currency, legacy DateTime, GUID,
Decimal and Short Text fields after table replacement, subsequent table creation and retained row
insertion. This separate ACE16 source family uses actual catalog/allocation
ownership rather than the original eleven-page, definition-7 geometry. Desktop
and MCP consume the shared runtime and Access adapter; no native editing logic
was added to the desktop host.

## Historically absent Boolean bits

`TCNI0023`/profile 23 is a separate original-request language for an eligible
Boolean field whose native number is beyond the selected record's stored column
count. Its observed value is False, not NULL and not an ignored byte from a
retired field. Source qualification checks both complete present-bit records and
unambiguously framed historical records before exposing the Boolean capability.
The selected operation still rechecks its original typed Boolean expected value,
actual field declaration, programs, constraints, ownership and record framing.

False-to-True reconstructs the complete current record: native high-water count,
full retained variable footer, expanded mask and the selected field's actual
Boolean bit. Other raw fixed/variable payloads, mask bits and opaque padding are
preserved under the separate reconstruction/placement proof. False-to-False
retains the complete original image as a fully admitted no-op. NULL is not a
Boolean carrier. Unknown or ambiguous old framing refuses this action.

Metadata and actual original record presence select 23 before planning; there is
no failed TCNI20 retry. A record with a stored selected bit retains TCNI20's
present-Boolean language. TCNI23 retains exact Boolean expected/replacement data
and whole BEFORE binding, uses checked V3 preparation/recovery, and does not widen
TCNI17–22 or grant general historical materialization to other scalar kinds.

## Exact GUID and Decimal edits

`TCNI0022`/profile 22 adds separate GUID and Decimal actions over the complete
retained-row source. A GUID target must be a fixed, updatable native GUID with
exactly 16 payload bytes. A Decimal target must be an updatable, non-complex
variable NUMERIC field with exactly 17 payload bytes and valid declared precision
1–28 and scale 0–precision. Both require exact native flags and no extended flags;
indexed, generated and calculated targets remain refused. An unsupported target
never grants a different action permission on the same table.

GUID values retain their canonical identity and native mixed-endian field bytes.
This supplies no GUID generation, replication or GUID-default authoring. Decimal
input is parsed exactly, rescaled only without changing its value, and checked
against the actual field's precision and scale. The signed coefficient never
passes through floating point, Currency or display formatting. Microsoft's
[Access SQL type contract](https://learn.microsoft.com/en-us/office/client-developer/access/desktop-database-reference/sql-data-types)
defines the precision/scale range; [Number field sizes](https://support.microsoft.com/en-us/access/set-the-field-size)
separately identifies Replication ID as GUID storage. The native NUMERIC record
carrier is distinct from the API-level storage-size descriptions in those references.

GUID NULL clears the selected presence bit while retaining ignored payload bytes.
Decimal NULL removes only the selected variable payload and rebuilds the footer,
retaining the old physical footprint, unrelated segments and padding. Refilling
can grow or forward the record while retaining that padding. Source qualification,
expected-value equality, complete ownership and preservation checks still run for
NULL-to-NULL and exact no-ops; a no-op preserves the complete image. Ambiguous
retired tails do not acquire admission, and this section does not qualify all
historical short-record materialization cases.

The original request uses explicit `encode_retained_wide_values` and
`decode_retained_wide_values` framing. It retains the actual GUID or Decimal
expected/replacement carriers and whole BEFORE binding, not a patch or permission.
The consuming plan and fresh recovery reconstruction use checked V3 storage;
older wire languages and V1/V2 preparation remain separate. Action dispatch
selects 22 from the actual target metadata before planning, alongside 21 for the
six fixed scalars, 20 for Boolean, 19 for Currency and 17 for LONG/Text.

## Exact fixed scalars

`TCNI0021`/profile 21 owns one explicit fixed-scalar language: Byte, Int16, Int64,
Single, Double and legacy DateTime, each with its exact native declaration and
carrier. The selected field must be fixed and updatable, with the exact native
kind and width and no extended flags. Generated, calculated and indexed targets
remain refused. The complete retained-row source must qualify every selected
record; unknown retired-tail geometry stays outside this action. Single and Double expected and
replacement values must be finite; their IEEE bit patterns are retained without
converting through another numeric type, including finite subnormals and signed
zero. Int64 values do not pass through f64.
Legacy DateTime retains its checked OLE date bits and range, without time-zone
conversion. NULL is distinct from every scalar value and preserves ignored
payload bytes. Defaults are not reevaluated by UPDATE.

The action reuses the complete retained-row program-absence certificate, dynamic
ownership census, record placement and full-image proof. It introduces no new
source-purpose certificate, INSERT encoder or UI parser. Each expected carrier
must match the original cell. Source/program/constraint checks still run for
no-ops, NULL-to-NULL requests and an empty request group. A stored control,
formatted value or a readable declaration alone grants no update permission.
The native producer and independent proof rebuild exact selected payload/mask
changes while preserving sibling cells, counters, indexes, history and all bytes
outside the admitted record-placement scope.

A complete historical record may omit later native fields. Its stored field
count and presence mask make such a selected field NULL, independently of ignored
payload bytes. A non-NULL replacement reconstructs the retained schema count,
fixed span and any completely parsed footer; other absent fields remain NULL
rather than receiving defaults. The original prefix remains preserved outside
the selected declared fixed range. A NULL-to-NULL no-op keeps the entire shorter
record unchanged. This complete-frame reconstruction does not infer deleted
field types or admit the ambiguous tails handled separately by TCNI18.

The original request retains fixed groups only, with explicit column spellings
and logical row ordinals. `NativeFittedIntentGroup::new_retained_fixed_scalars`
and `NativeFittedEditIntent::new_retained_fixed_scalars` validate this distinct
data grammar. Variable groups, including empty ones, and other scalar carriers
are refused. Counts and lengths retain their actual u32 representation; the
whole BEFORE binding uses native page geometry up to 2 GiB, with fallible
allocation, cancellation and cumulative caller budgets.

| Carrier | Tag | Width | Exact payload |
| --- | ---: | ---: | --- |
| NULL | 0 | 0 | Empty |
| Int64 (Large Number) | 3 | 8 bytes | Signed little-endian i64 |
| Legacy DateTime | 5 | 8 bytes | Checked OLE f64 bits, little-endian u64 |
| Byte | 8 | 1 byte | One u8 |
| Int16 (Integer) | 9 | 2 bytes | Signed little-endian i16 |
| Single | 10 | 4 bytes | Finite IEEE f32 bits, little-endian u32 |
| Double | 11 | 8 bytes | Finite IEEE f64 bits, little-endian u64 |

`decode_retained_fixed_scalars` accepts only TCNI21. Generic intent dispatch
selects that exact marker before native replay. TCNI17–20 retain their original
wire grammars, native target languages and reconstruction paths; a failed older
action is never retried as 21. `prepare_retained` and
`qualify_retained_recovery` use the same checked V3 owner. Recovery verifies the
complete original binding, requalifies the selected native action in a fresh
scope, and compares complete BEFORE and AFTER images. Old V1/V2 preparation and
qualification APIs refuse 21 before effects. Journal-only builds decode its
original data but return typed unsupported-source refusal for native recovery.

The Access capability is a union of eligible actions over one qualified source.
Actual target metadata selects 22 for GUID/Decimal, 21 for these six types,
20 for Boolean, 19 for
Currency and 17 for LONG/Text, before original-group construction or planning.
An ineligible target is not redirected to another family. A reviewed SQL Byte
assignment followed by a LONG assignment on the same retained table keeps the
21 and 17 profiles respectively, with independent complete AFTER images and
same-file offline saves. Exact carrier, finite/range, stale-witness and protected
image checks remain shared with typed runtime edits. SQL assignment conversion
rules are unchanged.

Local native/wire/recovery and reviewed SQL checks cover all six scalar carriers,
NULL/no-op behavior, Int64 above 2^53, exact Single/Double/date bits, forged AFTER,
wrong protocols and journal-only refusal. These checks are implementation
evidence; they do not establish Windows or Microsoft Access save/reopen parity.
The application workflow record belongs to the
[runtime contract](../../application/RUNTIME.md).

[Microsoft's Access field-type reference](https://support.microsoft.com/en-au/access/data-types-for-access-desktop-databases)
distinguishes Number sizes, eight-byte Large Number and eight-byte Date/Time from
Date/Time Extended. The [provider data-type contract](https://learn.microsoft.com/en-us/office/client-developer/access/desktop-database-reference/datatypeenum)
identifies the integer and floating carriers and describes Automation DATE as a
double containing days from 30 December 1899 plus a fractional day. Provider enum
values are not ACE storage tags. These references support representation facts;
they do not establish Microsoft Access save/reopen parity.

Local macOS ARM64 headless verification passes the selected 135 native tests and
348 runtime tests, plus three journal-only request/refusal cases. The focused
native controls compare independent full images for all six kinds, exact scalar
extremes, NULL/no-op/refill, stale values, long user/catalog forwarding chains,
wrong carriers, nonfinite floats, original-family refusal and same-table LONG
compatibility. Seven-consumer API and feature/documentation checks pass. These
are implementation observations, separate from Windows or Microsoft Access
execution evidence.

## Source scopes and mutation certificates

Retained DATA actions observe represented catalog property maps, including
indirect candidate maps, through read-only scopes. The observation proves actual
allocated storage, membership, per-hop ownership, cycle/alias exclusion and
protected roles. It has no map-edit, allocation or schema-write methods. A scope
observation cannot stand in for a source-bound INSERT, UPDATE or DELETE
certificate; each consuming action separately rechecks its programs, constraints,
record grammar and complete original witness. Writable LVAL map factories and
schema/property mutation admission retain their separate restrictions.

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

The current retained-cell capability combines the eligible scalar actions
from one qualified source. The actual target selects TCNI22, TCNI21, TCNI20, TCNI19 or TCNI17;
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

The shared source capability check includes a genuinely qualified Currency
editor without replacing the other scalar actions. Planning selects the action
from the actual edited column: Currency uses TCNI19, while LONG/Text keep TCNI17. It never retries
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
indexed Currency edits and generic imported schema editing
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

Each operation changes one qualified unindexed cell from its selected action
language. Native field numbers, fixed offsets, variable-slot history and null
masks remain distinct from visible column positions. Supplied NULL and empty text are separate; defaults
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
cancellation and cumulative caller budgets. Native file geometry, field kinds
and physical storage restrictions are checked independently of inert request data.

Retained and legacy request grammars remain explicit. The generic update decoder
selects TCNI17–22 from exact framing; older direct decoders and encode methods cannot reinterpret
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
Indexed-cell changes, other target carriers, new-page allocation and
required-program execution remain separate work. Table and field changes retain
their own source and action proofs.
