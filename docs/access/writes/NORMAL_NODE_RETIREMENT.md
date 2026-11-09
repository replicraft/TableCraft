# Normal Int32 NODE child retirement

Normal DELETE now retires empty children in the qualified height-one ascending
Int32 profile. It consumes the existing contraction/retirement services through
original requests and unchanged TCND1/CheckedV1 recovery; no preview conversion or
caller-provided page authority is introduced.

## Source and representation

The original has a retained root and two nonempty leaf children with present
Int32 keys, zero prefix and a right tail. Removal derives exact children from
original full membership. A sole surviving child becomes a complete bounded
key/ROW/child entry with zero tail. If all children empty, the retained root
becomes an empty leaf. No nonempty child is promoted into the root.

Only empty children retire. Original catalog/census, GLOBAL and INDEX map rewrite
authority, all local member roles and live LVAL ownership remain checked. Exact
map bits change and retired pages gain four invalid-header bytes; residual bodies,
DATA payload/capacity, histories, logical aliases and EOF remain preserved.
Shared-byte masks compose against original cells. The preview consumer retains
its previous contraction representation.

## Independent proof and consumers

Fresh proof separately derives original populations, surviving partitions, full
root bounds, retired children, raw map XOR masks and complete image. It consumes
no producer patches or staged arrays. Existing audits are direct proof consumers,
and complete final Source/constraints remain mandatory before publication.

Native, typed/SQL retained-session and CLI consumers share this plan. Snapshots
refresh after saves; old handles refuse while retaining the completed receipt
and held lease. Empty follow-ons preserve exact images and retire their journal
namespace. Current actual Source is still required for every operation.

The [source phase](../../../tests/reports/indexes/ACE16_NODE_RETIREMENT_SOURCES.md) passed eight
native views and 48 SDK observations before matching producers. Direct literal
proofs/forgeries, twelve native commits, mixed/pending/committed recovery,
twelve full-public resource/cancellation contexts, eighteen typed/SQL saves and
empty follow-ons, and twelve CLI saves/readbacks pass. Earlier non-retiring NODE
regressions pass; the old child-emptying refusal now checks the separately
qualified LEFT/ALL images and keeps its reserved-stamp denial.

The separate [height-two profile](NORMAL_RECURSIVE_NODE_RETIREMENT.md) now
supports qualified deeper retirement. NULL/DESC retirement, DATA-page retirement, nonempty root
promotion and a second nonempty retirement from an already single-child source
remain work. The sole-child AFTER is readable and accepts empty plans. Native
Access save/reopen and Windows qualification remain unrun.

The [combined execution record](../../../tests/results/schema/ace16-constant-defaults-and-node-retirement-writes-2026-11-01.json)
retains focused/strict checks and the first broad regression. One stale unit-test
error expectation was corrected and replayed; the 1,164 other engine units passed
the first run. Adapter/CLI/recovery checks passed separately.

The later [recursive source gate](../../../tests/results/general/ace16-recursive-retirement-sources-2026-11-01.json)
passes six independently modeled height-two views and twelve read-only SDK
observations before matching recursive producers. It checks original partitions,
fences, sibling chains, exact retirement cells and current complete native Source
admission. Matching mutation is now qualified separately in the height-two
contract above.
