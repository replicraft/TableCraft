# Shared ACE allocation-map authority

The optional engineering previews now share a catalog-wide map-footprint census.
The first working consumer is the existing [Memo/OLE replacement
preview](ACE16_LONG_VALUE_UPDATES.md). Its five selected map owners and original
catalog recertification remain required. This extraction does not allocate map
storage or extend normal backend writes.

## Catalog coverage and explicit selection

Canonical identities distinguish global free metadata, table-definition-owned
ordinary maps and candidate maps, physical-index-owned maps, and declared
long-column owned/candidate maps. Table-definition and native column/physical
index numbers belong to each identity. True logical aliases of a physical index
share one owner. Conflicting references for one owner are malformed.

The collector visits every local catalog table. It captures ordinary usage and
optional candidate references, parsed physical-index owners, and complete current
and historical long-column declaration tails, including tables without surviving
long columns. Unsupported tails remain refused. The existing parser exposes
physical descriptors through logical declarations; it does not return an inventory
of unreferenced descriptors. The census therefore requires every declared physical
slot to have a parsed logical owner. A readable table with an orphan physical
slot now refuses mutation with `Unsupported(RowLayout)` instead of receiving an
incomplete ownership certificate. Raw orphan descriptors are not guessed or
silently omitted.

The caller supplies the exact selected owner slice. Each selected identity must
exist, and repeated selected identities refuse. The shared service has no fixed
role count or automatic mutable-owner predicate. LVAL explicitly selects global
free, the selected ordinary owned/candidate pair, and the selected column
owned/candidate pair. Future consumers must derive their own selected identities
from qualified original metadata; this extraction supplies no index allocator.

Selected claims are checked against all foreign claims for identical references,
overlapping absolute map-record ranges, and bitmap-to-bitmap or bitmap-to-container
collisions. Disjoint map rows sharing a container remain valid. Foreign footprint
inspection retains the existing read-only storage grammar. It grants neither
membership nor writable cells; actual edits still require strict role-qualified
maps and independent event/bit replay.

## Payload and original-image boundaries

The collector performs metadata work. An internal per-table visitor lets the
LVAL consumer separately retain its ordinary/index page protection, declared
LVAL membership and live chunk-reference checks without reparsing schemas or
materializing a second catalog snapshot. Protected map-storage pages are added
separately. A malformed LVAL body can therefore have a valid metadata footprint
while payload validation and the mutation API correctly reject it.

The LVAL finalizer still re-captures authority against the immutable original
catalog. A staged plan from a different valid catalog cannot confer permission
over shared original maps. No authority is inferred from candidate bytes alone.
Original per-table reader ceilings remain; allocation, work and cancellation are
paid. Explicit selection and footprint protection change some charge/check order,
so identical budget or cancellation boundaries are not claimed.

## Scoped verification

The [implementation record](results/ace16-map-authority-native-2026-10-27.json)
records 43 distinct runtime functions: 36 existing integration functions and seven
private functions, including five new controls. They cover required and duplicate
owner selection, record and bitmap collisions, shared containers, bounded work,
cancellation/retry, a healthy readable orphan physical slot, metadata-versus-payload
authority, and immutable finalizer recensus. Existing integration controls retain
aliases, historical declarations, mixed maps, constraints and full candidate
preservation. Three compile-fail docs, scoped feature/default strict linting,
minimal builds and formatting also pass.

A separate temporary read-only harness repeats both Memo-column admissions on
22 unchanged pinned originals: 44 successful admissions, with no staging,
publication, getter/JVM invocation or original mutation. It is excluded from the
43-function runtime total. The diagnostic hook is removed from shipping sources.
The initial type/assembly compile errors and the initial diagnostic manifest-path
failure remain preserved. These checks establish the scoped engineering contract;
they do not establish native Access save compatibility or generic map growth.
