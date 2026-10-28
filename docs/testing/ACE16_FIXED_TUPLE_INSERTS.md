# Fixed GUID and tuple INSERT engineering preview

The separate `NativeImage::preview_ace16_insert_rows` service now maintains
single GUID secondary indexes and qualified two-to-ten-component fixed-key
indexes. Components may use Boolean, Byte, Int16, Int32, Int64, Currency, Single,
Double, legacy DateTime or GUID, with an independent direction per component.
The required unique ascending Int32 primary remains mandatory and may occupy
any physical index slot. This does not enable normal saves, journalable ACE
edits, schema changes or required data-macro execution.

The [ordinary record contract](ACE16_ORDINARY_INSERTS.md) still governs all
supplied fields, including fields outside an index. Its contiguous-column,
explicit-value, default/generated-field and payload restrictions remain intact.
Text, Binary and other unqualified index profiles remain refused;
being able to store a value does not qualify an index on it. Foreign indexes and
unsupported relationship or validation semantics retain their existing gates.

## Typed ownership and allocation

Each physical owner prepares complete immutable request rows through its actual
native profile. Declaration positions are resolved from native column descriptors;
no table/field name or first scalar field selects the key. A `PreparedIndex`
retains typed keys and profiles while exposing its original page inventory.
The composer checks disjoint ownership against earlier physical trees before
INDEX allocation. Logical aliases share one physical plan and counter.

Continuation consumes the same DATA-first allocation state. Only after that
owner finishes does the plan retain owned encoded-key/ROW facts for comparison.
The final audit independently derives those facts; it does not accept the
planner's component, NULL or source-membership declarations as authority.
Existing source topology, original bounds, seed/residue rules, recursive
normalization and retirement/allocation exclusions remain in force.

GUID field storage uses its native little-endian segments; physical index keys
use canonical bytes and two fixed segments. Descending encoding complements the
value bytes and final length tag while retaining continuation tag `09`. The
existing native codec constructs keys, and the independent audit owns a separate
GUID wire validator and literal goldens. Numeric and DATE components retain
their established exact carriers and physical ordering, without SQL coercion.

## Complete tuple rules

Required rejects a NULL in any selected component. Unique exempts only an
entirely NULL tuple; repeated partially NULL keys still conflict. IgnoreNULL
omits only tuples whose selected components are all NULL. A field outside the
index does not affect that decision. Required+IgnoreNULL and primary IgnoreNULL
combinations remain refused. Boolean false remains a non-NULL mask value.

Historical distinct counters compare complete key-only families, excluding ROW
references. Repeated families count once, existing families count zero and an
omitted all-NULL key contributes zero. Table and primary cardinality include
every inserted row. An owner receiving no eligible keys retains its entire tree,
map and history, while other owners may change shared global allocation bits.

Ignored original membership is independently reconstructed from the actual
primary group's complete logical ROW universe, including forwarding roots.
Every selected stored field is decoded. The decoder emits declaration order;
the audit remaps those values before encoding the index's written component
order. This preserves reversed keys and prevents same-count or ordinal-only
membership substitution. Short audit wires stay inline; wider keys and copied
branch maxima allocate only after explicit budget charges.

## Current verification

All 171 INSERT regression functions pass. Eight end-to-end GUID/tuple functions
construct real metadata, complete records, streams and expected whole images
before invoking the preview. Both ACE 16 layouts cover independent directions,
reversed key order, aliases, physical primary swaps, exact GUID carriers,
all/partial NULLs, uniqueness, historical deltas, quotas and terminal cancellation.

A full single-GUID leaf grows from 157 non-NULL entries plus one NULL entry:
3,616 active bytes. Ten-GUID tuples additionally exercise eleven fixed fields,
two-byte masks and 168-byte records. Their 194-byte leaf entries grow an
ordinary 3,506-byte source or ignored 3,492-byte source into two leaves and a
real 198-byte node bound. DATA still fits its original page; exact EOF ordering,
map bits, counters, old NULL payloads, original root residue and fresh padding
are checked. A fitted unique case differing only in the last written component
checks complete-key comparison.

The full ACE unit batch passes 468 functions with one existing ignored fixture
generator. Strict ACE Clippy across all targets/features, minimal compilation,
formatting and dependency/documentation guards pass. The first recovered GUID batch passed five functions and exposed two
reversed ignored-tuple audit failures. The projection-order correction preserves
the original and expected bytes; a causal source control additionally rejects
canonical same-count membership with the wrong stored field value. An earlier
attempt stopped on exhausted Cargo-cache disk space before tests ran; only
rebuildable incremental cache was cleared. These failures remain separate from
the subsequent passing batch.

The [GUID/tuple file corpus](ace16_insert_guid_tuple_indexes/README.md) now
compares 32 source/request-only expected images with the first 32 committed-source
producer outputs, followed by 64 fresh independent getter/map calls. All 2,768
old rows remain and 80 rows are added, with 5,684 physical key/ROW pairs. The
separate public-writer source/reference protocol passes all 193 commands.
Independent file coverage is single GUID and two-component GUID/Int32; the
ten-component cases above remain literal engineering controls. The
[recorded producer build](results/ace16-insert-guid-tuple-index-example-build-2026-10-28.json)
and [result receipt](results/ace16-insert-guid-tuple-index-native-2026-10-28.json)
retain the exact source and evidence pins.
Native Microsoft Access save/reopen, data macros, shared-file behavior and
production write admission remain unqualified.


## Actual-width comparison work

Wide audit keys retain their actual encoded length while numeric keys stay
inline. Sorting and final eligible-membership comparison now charge for that
actual width, with the existing in-memory wire size as the minimum. This closes
a conservative-work gap introduced by wider keys without changing encoded bytes,
ordering or image construction. The paid scan checks cancellation before sorting;
failed precharge leaves the collection untouched.

The wide-key control uses sixteen valid 194-byte GUID-tuple entries with a long
common prefix. An allowance sufficient for the former struct-width estimate is
refused, while the complete width-aware allowance succeeds. A second control
checks that inline numeric keys still fit their smaller allowance and that
cancellation refuses explicitly. All 173
INSERT regression functions pass; the preceding full 468-function ACE batch is
recorded separately above. Strict ACE library/test Clippy, minimal compilation
and formatting pass after this work-accounting correction.

## Exact Decimal index continuation

The separate INSERT preview also maintains single Decimal indexes and fixed
tuples containing Decimal, including GUID and independently declared Decimal
components. Each Decimal field keeps its own precision (1–28), scale (0 through
precision) and direction. Requests require the exact Decimal carrier and declared
scale with magnitude below 10^precision; insertion never rescales, rounds or
coerces another numeric carrier. The existing 17-byte variable FIELD storage is
unchanged. A new NULL has an empty variable payload; opaque old NULL bytes remain
preserved.

A non-NULL Decimal index component has an 18-byte marker/sign/magnitude encoding,
separate from FIELD storage. Scalar leaf entries are 22 bytes and node entries
26 bytes; a Decimal/GUID leaf entry is 41 bytes with a 45-byte node bound.
The producer uses the native Decimal profile; the final audit independently
frames and validates exact sign, magnitude, direction, precision and NULL bytes.
All-NULL unique exemptions, partial-NULL conflicts, required and IgnoreNULL rules
remain those of the complete selected tuple.

Eight complete-image regression functions cover both layouts/directions, declared
precision/scale extremes, mixed/reversed GUID keys, two Decimal fields with
different declarations, root growth, aliases, historical counters, NULL rules,
invalid carriers, quotas and calibrated terminal cancellation. Two independent
audit functions check literal wire bytes and reject invalid signs, negative zero
and out-of-precision values. All 183 INSERT regression functions pass, together
with strict all-target/all-feature ACE Clippy, minimal ACE compilation, formatting
and documentation/dependency guards. These are a later batch than the full
468-function checkpoint above. Independent original-file Decimal INSERT
comparisons are being prepared; native Access save/reopen and production write
admission remain unqualified.
