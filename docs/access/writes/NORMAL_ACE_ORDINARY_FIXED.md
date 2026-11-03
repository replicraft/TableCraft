# Ordinary ACE fixed-field writes

`NativeImage::plan_recoverable_ordinary_fixed_updates` accepts original fixed-field
groups only after fresh native content and physical preservation validation. It is a separate normal
engine route from positive BeforeChange updates and engineering previews.
`NativeJournalEditor`, the retained `OfflineWriteSession` and
`query journal-update` now consume it through their existing lifecycle.
The separate [exact IEEE continuation](NORMAL_IEEE_FIXED_UPDATE.md) admits ordinary
unindexed Single/Double targets under the same complete native Source checks and
a distinct original-request record.

The [Byte/Int16 continuation](NORMAL_BYTE_INT16_FIXED_UPDATE.md) uses the same
ordinary service for exact unindexed small-integer targets. Original named targets
select TCNI10 for [GUID targets](NORMAL_GUID_FIXED_UPDATE.md), TCNI9 for
Byte/Int16, TCNI8 for IEEE, and TCNI4 for legacy fields; old records
retain their own target and carrier checks even for named-empty requests.

## Initial global data-only family

The initial family is unencoded ACE header 5/6, with the known clear header
fields, code page 1252 and General sort declaration. The standard 64 MiB captured
image envelope and existing read/work limits remain. The earlier 64-page map
ceiling is removed. Definitions fit one page; allocation uses qualified variable
inline-map windows and the sole GLOBAL record. Indirect maps still refuse.
Direct live DATA slots, free-space
counts, catalog layouts and map ownership must be complete. Unknown fields,
deleted/forwarded slots, unexplained pages or nonzero free bytes refuse.

The entire catalog must resolve four standard containers, three qualified system
tables and ordinary user tables. User names, values, table counts and physical
locations are data, not creator identities. All fifteen catalog fields and nine
variable slots are checked. Property/body/module/external-source carriers must
be absent, catalog LVAL map pairs must be complete and empty, and the relationship
and ACE system tables must be empty. Calculated/complex fields, user Memo/OLE
storage, wider headers and other objects remain outside this family.

One ordinary Int32 AutoNumber declaration may be retained as a read-only field.
Its exact native flags, width and extended metadata must qualify. Its stored LAST
value is historical data, preserved independently of the live IDs; this route
does not generate values or assign the counter field.

Existing local ordinary/primary indexes qualify when each physical index
has one ordinary Int32 key and complete original membership. Untouched NULL keys
are supported where the original required/unique constraints permit them.
Compatible logical aliases share their actual physical ownership. Foreign and
ignore-NULL index profiles remain refused. Manual indexed Int32 fields may now
be updated through fitting serialization on their existing pages. Present/NULL
transitions and atomic final UNIQUE swaps are supported; primary/required keys
still reject NULL and final duplicate keys refuse. Include-NULL indexes retain
the NULL marker and each original ROW suffix. Generated Id fields remain
read-only, including supplied empty groups.

Ordinary stored scalars, Short Text/Binary and Decimal can be preserved. Writable
targets are the existing Boolean, Int32, Int64, Currency and legacy DATE fixed
carriers; numeric NULL transitions retain inactive bytes. Ordinary fitting
[Short Text/Binary batches](NORMAL_ACE_FITTED.md) use their own TCNI0005 route.
Indexed targets use
the manual Int32 profile above. No record resizing, page allocation, index split,
map growth or EOF change is added. A key rewrite that cannot fit refuses before
journal setup. These limits describe this route,
not the long-term compatibility target or the broader reader.

## Native seventeen-field target family

This selected-table family also supports unindexed updates beside
[unchanged Byte/Int16/Int64/Currency indexes](NORMAL_ACE_RETAINED_NUMERIC.md),
using complete-owned generic checks and actual catalog member-role exclusion.
Its wider readonly index policy does not expand the global closed family or
permit those indexed fields as mutation targets. A separate
[qualified numeric UPDATE route](NORMAL_WIDE_INDEX_UPDATE.md) now maintains
fitting Int64/Currency root leaves under its own complete schema/index checks.

Manual single-GUID indexes also have a separate
[readonly Source continuation](NORMAL_RETAINED_GUID_SOURCE.md), with exact
unrelated I32 Score plans. This supplies no GUID target or save qualification.

A separate [target-table program absence profile](NATIVE_TARGET_PROGRAM_ABSENCE.md)
now admits the exact native seventeen-field catalog through a strict conjunction:
three NULL code carriers and either NULL/empty or strictly qualified nonempty
LvProp, five NULL remote carriers, complete resolved catalog rows and
child/identity census, shared selected TD/index declaration closure, all-local
LVAL declaration bijection and full normal Source protection. Catalog forwarding
requires owned pages, unique hidden-slot ownership and complete resolved bodies;
orphan/shared hidden records and unsupported flag bits refuse. Bounded empty
deleted tombstones remain inert. Selected readonly Memo/OLE declarations now
qualify under exact tail/map checks; those fields are never fixed/fitted targets. Unknown storage kinds and linked table origins
refuse. Known unrelated application objects remain opaque and byte-preserved,
without execution.

This family is selected structurally before planning; a failed check never falls
back to the initial global grammar. It retains the fixed-target, constraint,
index fitting and no-growth limits below. The native VCS locator evidence supports
an explicitly inferred profile, not universal macro absence or native Access
save/reopen conformance. The [write record](../../testing/results/ace16-genuine-ordinary-target-writes-2026-10-31.json)
records genuine native recovery, twenty principal typed/SQL session saves, ten CLI
saves/readbacks and full ACE unit/recovery regressions. Mixed DATA/INDEX rollback
and exact refusal diagnostics are tested separately from source admission.

The selected seventeen-field family also has a fitting
[single-Int32 IgnoreNULL continuation](NORMAL_IGNORE_NULL_INDEX_UPDATE.md).
It checks complete logical membership before omitting NULL secondary keys,
retains Required-before-omission and final UNIQUE constraints, and independently
derives bounded historical increments. This does not widen the initial global
family or authorize ignored primary, tuple or NODE mutations.

## Authority and physical proof

For the initial global family, `ordinary_content` accounts for the header, full catalog graph, declarations,
every live record, all referenced map cells and physical EOF. It rejects duplicate
ownership and accounts for free pages. Actual scalar reads independently validate
the admitted user records. XML-looking names, Text/Binary values and inactive
payloads remain ordinary data under their declared roles.

For retained indexes, actual complete table checks and original row references
and values establish key membership. Every physical declaration needs a real
logical alias, and each physical map and INDEX page has exclusive ownership.
Historical index counters are preserved as stored, except for the independently
derived increments in the qualified IgnoreNULL continuation; they are never
replaced by a live-row count. Full count, physical and logical declarations remain checked independently
of the semantic reader.

Existing compressed prefixes and interior nodes are validated with their
original bounds, row suffixes, links, heights, entry extents and free counts.
Inactive INDEX capacity may retain bytes such as A7 under that explicitly checked
page role. Unaffected indexes remain byte-exact. An affected physical index is
rewritten once even when it has multiple logical aliases, using fitting encoding
selected before staging. Page identities and child topology stay fixed. Per-leaf entry counts stay fixed
for the original include-NULL profile; the qualified IgnoreNULL root leaf may
change its emitted entry count. Prefixes, active extents, free counts and node bounds may change to
describe the final key population. Entry-storage bytes beyond the new active
extent retain their original contents. Equal assignments and empty groups
preserve whole images.

Only a complete validation returns a private image-borrowed certificate. It binds
the actual image instance and user-table identity. Neither an empty inventory,
caller flag, creator marker nor matching digest can construct it. Preserved source
capture consumes a distinct certificate case; no empty program selection is
manufactured. Generic record protection has one shared owner, used by both this
route and existing BeforeChange consumers.

Every requested declaration/carrier/duplicate check precedes stale-value checks.
Equal assignments are ordinary no-ops in this proved program-free context.
Image-bound `CheckedIndexes` supplies actual scalar/index metadata independently
of event admission. Ordinary callers do not construct a program selection;
BeforeChange callers retain their separate native program checks. These neutral
facts alone do not certify ordinary content or make a generated field writable.

The shared fixed-record stage derives DATA patches, and the existing fitting
index writer consumes all original-to-final key changes together. A fresh proof
recaptures BEFORE, repeats complete content validation, manually reconstructs
payloads/masks and independently rebuilds the index population and wire bytes.
The nullable population callback visits actual absent keys as NULL and never
decodes their inactive payloads. Every AFTER byte, final constraint result and
changed-root count must agree. Producer patches, encoded keys and counts are not
proof authority. No failed encoder triggers another writer.

## Journal and verification

`TCNI0004`/profile 4 retains the same bounded original fixed-request body, exact
carriers and BEFORE digest. No SQL, absence certificate, creator identity or final
assignments are serialized. Record-selected recovery freshly rebuilds the full
ordinary plan and compares both complete images before effects. Unknown records
and cross-profile combinations refuse; existing TCNI0001/2/3 bytes are unchanged.
Requests naming a Single/Double target use the separate `TCNI0008`/profile 8
[IEEE record](NORMAL_IEEE_FIXED_UPDATE.md), including named-empty groups. Legacy
carrier-only requests and their TCNI0004 bytes remain unchanged; old record
validators do not acquire IEEE tags.

The [independent sources](../../testing/ACE16_ORDINARY_CONTENT_SOURCES.md) passed
native reads and 24 exact SDK observations before mutation calls. Three content
tests cover 24 accepted and 34 refused views, image/table binding and resource
controls. Four planner/proof tests and five public planning/recovery tests pass,
including 22 literal whole-image commits. The initial ordinary fixed-field unit
and recovery batches passed. First compile/test/lint diagnostics remain in the
[execution record](../../testing/results/ace16-normal-ordinary-fixed-2026-10-31.json).
Native Microsoft Access save/reopen, Windows, power-loss and mixed-client
qualification remain separate and unrun.

## Dynamic inline-map coverage

Inline maps borrow their original variable-length records. Checked base-relative
members determine ownership and candidate subsets even when their windows differ.
Empty windows establish no membership. The ledger allocates roles for actual
captured EOF under the caller budget; it does not derive file size from a bitmap.
Every actual page needs a represented GLOBAL bit and a complete allocated role or
zero-filled free-page check. Future GLOBAL-free bits beyond EOF are valid capacity
and never create physical roles. Owned/candidate members beyond EOF refuse.
Physical map-cell uniqueness, allocated map storage and full record/byte closure
remain mandatory.

Ten 96-page originals pass native reads and exact independent SDK observations
before mutation. Dynamic content controls include a file whose high pages all
have valid DATA roles but lack GLOBAL cells: ordinary reading succeeds, while
certification refuses. Native, retained-session and CLI consumers compare whole
images including all maps and free holes, with unchanged TCNI0004 framing and
recovery. See the [dynamic execution record](../../testing/results/ace16-ordinary-inline-writes-2026-10-31.json).

## Shared sessions and CLI

The earlier counter/index preservation acceptance, including unchanged physical
indexes, historical counters, recovery and retained-session saves, is recorded
in the
[counter/index write record](../../testing/results/ace16-ordinary-counter-index-writes-2026-10-31.json).

The [indexed-target sources](../../testing/ACE16_ORDINARY_INDEXED_TARGET_SOURCES.md)
qualify 32 original and 24 literal AFTER views through native checks and 56 exact
independent SDK observations before indexed producer calls. Native indexed-target
tests pass all 24 literal outcomes, eight refusal contexts and causal proof
forgeries. The public recovery batch passes 79 tests, including four new tests
with 24 literal commits, two reverse-order swap plans, pending/partial/committed
requalification and original-intent/image forgeries.

Five retained-session tests pass: the new cases perform 16 representative typed
saves and two predecessor commits followed by stale-handle refusals; existing
coverage retains 36 typed/SQL counter-preserving saves and four newly admitted
manual empty-group saves. Four CLI tests pass, including 24 new literal saves,
24 exact row readbacks and eight SQL refusal controls, plus the earlier eighteen
saves/readbacks and manual Id migration. Explicit stale expected-value forgeries
remain native API tests; SQL uses actual snapshot expectations. Generated-field
refusals remain separate from newly admitted manual indexed fields. See the
[indexed-target write record](../../testing/results/ace16-ordinary-indexed-target-writes-2026-10-31.json).
The full ACE batch passes 1,075 tests (32 ignored), including exact indexed
planning/recovery pools and entry/middle/terminal cancellation. Strict checks
pass for all-feature consumers and journal-only/preview-only engine libraries.
Native Microsoft Access write/reopen qualification remains unrun.

Inert metadata chooses which family to attempt; it never proves absence. The
ordinary candidate must pass complete content/source/target checking before the
adapter advertises `StoredScalar`. A failed check does not try another writer.
The private retained route keeps ordinary legacy-carrier single Int32, empty and
batch requests on TCNI0004. IEEE-target requests retain their separate TCNI0008
record even when equal or empty. Existing Jet4/positive Int32/positive fixed records remain 1/2/3.
Every subsequent actual plan and recovery repeats native qualification.

Seven adapter tests pass, including typed/SQL literal saves, equal/zero-match
requests, empty tables, stale handles, unknown-content refusals and the older
record routes. The CLI passes 22 literal complete-image saves, four equal/empty
retirements and a real create/update/TSV-readback workflow. It adds no absence
override or alternate publication lifecycle. See the
[host execution record](../../testing/results/ace16-normal-ordinary-host-2026-10-31.json).
