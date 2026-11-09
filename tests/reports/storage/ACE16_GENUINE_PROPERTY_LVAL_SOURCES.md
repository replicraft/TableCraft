# Genuine property and readonly OLE sources

The unchanged V2016/V2019 originals from the
[fixed-scalar corpus](../../results/rows/ace16-preserved-fixed-scalar-genuine-sources-2026-10-31.json)
contain `Sentinel(ID, Marker, Bytes)`: a manual Int32, Short Text and readonly
inline OLE value. The table has a nonempty 57-byte MR2 `PublishToWeb` property
bag, General text sort versions 1/0 respectively, and relocated catalog records.
Original database bytes and provenance remain with that corpus; no database or
SDK binary is added here.

The independent [model](../../access/storage/genuine_property_lval/model.py) and
[requests](../../access/storage/genuine_property_lval/requests.json) construct twelve complete outcomes
from those originals: same-length/shorter/empty/NULL text, a noop and an empty
group under each header. They preserve the original record capacity, readonly
OLE descriptor/content, property bag and every unrelated file byte. Capacity
overflow and stale expectations have no manufactured outcome. The model imports
only existing independent framing helpers and starts no native writer or process.

Fourteen native views (two originals and twelve outcomes) pass reads, property,
index and long-value checks. The existing pinned Jackcess 5.0.4 getter independently
matches all twelve precomputed inventories. Original and expected inputs remain
unchanged. Source/model preparation and source admission preceded the normal
write tests; readers do not grant mutation permission.

The ignored native source gate is selected explicitly:

```sh
TABLECRAFT_PROPERTY_LVAL_ORIGINALS_DIR=/path/to/originals \
TABLECRAFT_PROPERTY_LVAL_EXPECTED_DIR=/path/to/expected \
cargo test --locked -p tablecraft-ace --all-features --lib \
  genuine_marker_property_and_readonly_ole_views -- --ignored
```

The shared testkit loader pins both originals and all twelve expected images
before returning a case to save tests. These pins bind test evidence, not engine
admission. See the [source record](../../results/storage/ace16-genuine-property-lval-sources-2026-11-01.json)
and [normal fitting contract](../../../docs/access/writes/NORMAL_ACE_FITTED.md).
Microsoft Access save/reopen qualification remains unrun.
