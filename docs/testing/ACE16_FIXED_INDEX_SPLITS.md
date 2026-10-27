# ACE 16 exact fixed-scalar index split previews

The optional `ace16-write-preview` feature supplies
`NativeImage::preview_ace16_split_fixed_updates(table, column, requests, budget, cancelled)`.
It accepts the existing `FixedScalarUpdate::new(original_row, expected, value)` and
returns a separate, non-journalable `Ace16WritePreview`. This engineering service
creates a complete candidate; it does not enable normal backend/CLI writes,
execute required cascades or establish Microsoft Access/data-macro qualification.

## Exact admission

Requests must use the selected field's exact non-NULL `Value` carrier: Byte,
Int16, Int32, Int64 or Currency. Byte is unsigned; Currency contains an exact
scaled signed 64-bit integer. Equal physical widths do not make Int64 and
Currency interchangeable. NULL expected/replacement values and transitions are
refused. Unchanged stored NULL peers retain their original masks, payloads and
indexed membership, including through a split. Existing Required, unique and
ignore-NULL policies are preserved; this service does not relax UPDATE's
ignore-NULL refusal.

All existing ACE 16 preview format, expected-value, field/property, row-layout,
index, relationship and final-constraint gates apply. Unknown metadata is not
ignored. Ordinals refer to the captured original's live rows, not durable IDs.
Duplicate requests refuse before carrier conversion or selected-table checking;
all work uses cumulative caller allowances and cancellation.

New index allocation is limited to qualified single-column indexes over these
five integral/Currency kinds. Boolean, float, DATE, GUID, Decimal and composite
allocation are not admitted. Other already-qualified indexes, including
composites, may use existing replacement machinery when their pages fit; this
is not composite split support. Logical aliases share their physical index.

## Geometry and independent preservation

The service uses the [existing recursive split geometry](ACE16_INDEX_SPLITS.md):
root-leaf overflow allocates right then left and retains a single-bound/right-tail
root; non-root overflow allocates a new left sibling while retaining the right.
Internal maxima contain the complete key and physical row reference. Parent
overflow propagates upward, and a growing root retains its ID with two bounded
children and no tail. Sibling chains are maintained at every height.

The common prefix is measured for each actual codec and full row-inclusive
entry. Balanced entry/child-count halves must each fit. An oversized half
refuses atomically; repeated splitting of that half is a remaining capability,
not silently replaced by a different partition policy. In particular, a literal
600-child descending Int64/Currency parent with a MAX replacement produces a
299-bound half requiring 4,785 bytes in a 3,616-byte region and refuses. The same
source with exact integer/scaled value 1 exercises successful parent-only growth.
Byte/Int16 MAX controls and wide ascending halves fit independently.

The private operation binding retains original borrowed requests and selected
names. The finalizer rebinds actual native metadata, replays exact row identity,
field mask/span, expected and replacement bytes, and compares the complete patch
inventory. Declared kind is rederived rather than inferred from width or supplied
keys. The old Int32 facade retains its original borrowed requests, no-op behavior
and preflight order without an early paid conversion to `Value`.

The existing independent index replay derives topology, EOF allocation and map
intents from original keys and measured capacity. It checks complete key-plus-row
streams, bounds, framing, links and exact allocation ownership. Unaffected pages
and original opaque residue stay unchanged; new page padding is zero. Historical
counters and original row references remain unchanged. Full fresh catalog,
field/index and relationship checks run before publication. No journalable native edit
plan or ordinary save path is introduced.

## Engineering consumer

The [working example](../../crates/engines/tablecraft-ace/examples/preview_ace16_split_fixed.rs)
uses eight positional arguments:

```sh
cargo run --locked --offline -p tablecraft-ace --features ace16-write-preview \
  --example preview_ace16_split_fixed -- \
  before.accdb fresh-candidate.accdb ByteKeys KeyValue 0 byte 0 255
```

Kinds are `byte`, `int16`, `int32`, `int64` and `currency-scaled`; Currency input
is the exact scaled integer, without floating-point parsing. Carrier/range and
NULL-request errors are detected before opening the source. The allowance is
256 MiB, 100,000 rows and 100 million steps. The complete candidate is buffered
before fresh `create_new` output, synchronized, and an owned incomplete output
is removed on publication failure. Source and preexisting outputs are never
replaced. Success reports `candidate_changed_rows=N` and the engineering notice.

## Verification and evidence boundaries

The [scoped implementation record](results/ace16-fixed-index-splits-native-2026-10-27.json)
records exact source/log pins and accepted check counts. Independent whole-image
literals cover both ACE codes and directions, each width and declared kind,
physical aliases, full row ties, root/non-root/recursive growth, stored NULL peers,
map authority, stale/duplicate requests, quotas, late cancellation and retry.
Private controls check immutable request authority and old/new preflight ordering.
Consumer controls exercise eager input refusal and real fresh candidate publication.

The initial independent parent-positive loop contained an oversized descending
wide half. Isolated runtime and capacity diagnostics established the existing
atomic refusal; the owner retained that exact negative and added the same-source
value-1 positive. No production change or smaller source was used to satisfy it.
A private test compile used an obsolete staged-key field, and a helper borrow lint
was corrected separately; exact initial sources/logs are retained. Diagnostics
are excluded from accepted function totals.

The separate [source observations](INTEGRAL_INDEX_SPLIT_SOURCE_OBSERVATIONS.md)
and [public MAX reference observations](INTEGRAL_INDEX_SPLIT_OBSERVATIONS.md) establish fresh public-builder inputs,
prior typed-service refusals and observed public split geometry. Public fresh-page
scratch bytes are observations, not the engineering zero-padding golden. Those
actions and static original-image models do not establish a candidate replay
against a committed build of this new facade. Such replay requires its own frozen
artifact, input identities and result receipt.
