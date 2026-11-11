# Retained LONG secondary-index candidates

`NativeImage::prepare_retained_long_index_creation` builds a separate,
source-bound `NativeLongIndexCandidate` for a qualified ACE 16 retained table.
It creates one ordinary, nullable, nonunique LONG secondary index in ascending
or descending order, including NULL keys and duplicate values. Duplicate ROW
ties remain ascending in either direction.

This is candidate construction and independent checking, not a durable edit
plan. It does not itself grant journal, CLI SQL, runtime or GUI CREATE INDEX capability.
The distinct original-intent route below consumes its own checked recoverable plan.
The existing engineering-preview request paths and admission remain separate.
ACE 16 evidence does not qualify writing other Jet/ACE profiles or native
Windows/Access save behavior.

## Source and construction

The source must have the actual retained catalog cohort, complete inventory and
retained ordinary-schema/program/constraint qualification. The selected field
must be an ordinary unindexed native LONG with its exact fixed/updatable flags,
zero extended flags and four-byte storage. Generated, calculated, already
indexed and other scalar fields remain refused. Existing index-name conflicts,
logical-identity collisions, dependencies, malformed ownership and actual
native definition/allocation capacity remain checked.

The target definition currently occupies one physical page. The writer inserts
its historical counter, physical descriptor, logical descriptor and UTF-16 name
without replacing original field declarations or existing index aliases. It
appends leaf pages followed by successive parent levels, with the root last.
Parent fences retain the complete maximum KEY plus ROW; sibling links and the
actual subtree height are checked. Maximal fitting groups and prefix encoding
use physical INDEX-page capacity rather than a row-count quota.

A new DATA usage-map host follows the INDEX pages. Its bitmap represents only
those INDEX pages. The implementation constructs inline storage when it fits,
and represented indirect storage otherwise; all appended map-storage roles are
separately allocated. The current independent complete-image fixtures exercise
inline maps, including a genuine three-level tree; they do not independently
qualify every indirect-map construction geometry.

## Independent proof and evidence

The proof recaptures the original source and ROW/value membership, checks the
complete AFTER inventory and every qualified index, verifies one-to-one ordered
membership and the historical distinct counter, and independently reconstructs
the target definition and appended page framing. Every original byte outside
that definition and the sealed allocator ranges must remain exact. Existing
DATA, primary INDEX, catalog, LVAL and companion fields remain protected.

Preserved-index membership lookup classifies the borrowed expected ROW sequence
before walking the index. Ordered sequences use a paid, cancellable lower-bound
lookup returning the first equal ROW; unordered sequences retain first-match
linear lookup. Duplicate-reference, missing-reference, key, seen-entry and
constraint checks remain unchanged. Neither lookup copies payloads nor retries
a failed native read.

The [qualification record](../../../tests/results/indexes/normal-secondary-index-candidates-2026-11-11.json)
retains the independent source and candidate results, explicit test allowances
and the initial raw-fixture and work-budget failures. Empty, nullable/duplicate,
3,000-row and 200,000-row populations cover both directions; the largest has
more than two leaves and at least three tree levels. A separate generated-primary
source protects Currency, Unicode Text, NULL/empty values and sibling storage.
Corruption and request-refusal controls are distinct from positive construction.

## Reviewed native index creation

The distinct `NativeIndexCreateIntent` uses exact TCIC0001 framing and original
source binding. Its recoverable plan reconstructs one qualified index and checks
the complete BEFORE and AFTER images before preparation. Preparation and cold
recovery use the resizable V3 journal; no V1/V2 route or legacy decoder retry is
provided. Pending, committed and settled recovery retain the same native proof.

With creation and offline editing enabled, shared runtime schema review accepts
`CREATE INDEX name ON table (field ASC|DESC)` for this qualified LONG profile.
UNIQUE, PRIMARY, compound keys and explicit NULL-policy clauses remain refused.
An ordinary unindexed native field0 is eligible; generation remains refused.
Source programs, dependencies, ownership, names and native capacity still apply.

Desktop SQL Run shows the actual table, index, field, direction and existing
population before Yes/No. Existing population is descriptive metadata, not an
affected-row count. No releases the review without a write. Yes consumes the
review through one exclusive offline save and exact guarded AFTER reopen,
without reevaluating SQL. The receipt is `IndexCreated` with no matched/changed
row counts. Existing source rows, NULL values and duplicate values remain exact.

The MCP service currently does not expose the shared `ReviewSchemaSql` endpoint;
its explicit unsupported response is not SQL DDL parity. This workflow does not
establish a CLI schema command, other native key families, older Jet/ACE writes
or Windows/Access parity. The timestamp boundary below remains unchanged.

The separate [reviewed-host checkpoint](../../../tests/results/indexes/reviewed-create-index-hosts-2026-11-11.json)
records native original-intent feature gates, actual adapter cold recovery,
Runtime and desktop workflows, strict checks and four directly inspected
English/German light/dark offscreen frames. It retains initial failures and
repairs separately. The final preview depicts an ordinary LONG field; no matching
native Access SQL warning capture exists.

## Preserved logical identities after deletion

`prepare_retained_fresh_long_index_creation` has a separate construction policy
for retained definitions whose surviving logical IDs can differ from their
positions. It uses the current count when that identity is free; otherwise it
uses checked maximum logical ID plus one. Surviving logical IDs are never
renumbered. Physical descriptor numbering remains dense and independent.
This is an explicit construction policy, not an observed Access allocation rule.
The fresh route currently requires an ASCII index-name namespace; the original
count-based producer retains its existing name profile.

`NativeFreshIndexCreateIntent` records both allocated identities in the distinct
TCIC0002/profile2 request. Complete source binding, fresh source qualification,
identity checks and complete AFTER comparison precede the checked V3 protocol.
The original TCIC0001 producer and reconstruction language remain separate;
a fresh candidate cannot be captured as an original request merely because its
result bytes coincide.

The [fresh-identity checkpoint](../../../tests/results/indexes/fresh-index-identities-2026-11-11.json)
records raw post-deletion histories, native identity/recovery checks, journal-only
refusal and minimal-feature builds. It does not qualify application selection
of that new route or native Access allocation and timestamp behavior.

## Design-timestamp parity gap

Current index construction preserves the original `MSysObjects.DateUpdate` and
other catalog bytes. This byte preservation does not qualify native Access
`TableDef.LastUpdated` or Navigation Pane Modified behavior. Jackcess explicitly
[does not automatically update table modification dates](https://jackcess.sourceforge.io/apidocs/com/healthmarketscience/jackcess/Table.html#getUpdatedDate());
its readable output and unchanged timestamps cannot establish Microsoft parity.

Microsoft documents [LastUpdated as a table design-change stamp](https://learn.microsoft.com/en-us/office/vba/access/concepts/data-access-objects/track-design-changes-to-a-table-type-dao-recordset),
separate from record changes, and [Modified as the same property for tables](https://learn.microsoft.com/en-us/office/vba/access/concepts/miscellaneous/object-properties).
A stamp change on persisted index creation is an inference from that general
contract. Exact SQL/DAO `CREATE INDEX` chronology, physical catalog updates and
precision have not been qualified here. This remains part of the Access parity
goal, alongside the functional index work. A future stamp implementation must
bind its captured time to the versioned original request and exact catalog edit
proof; cold reconstruction must not consult a clock.
