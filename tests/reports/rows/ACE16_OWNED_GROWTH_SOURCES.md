# Genuine owned-page growth sources

The unchanged V2016/V2019
[fixed-scalar originals](results/ace16-preserved-fixed-scalar-genuine-sources-2026-10-31.json)
contain `Sentinel.Marker`, an uncompressed Short Text field beside an Int32 ID
and readonly inline OLE. Its nonempty PublishToWeb property remains unchanged.

The independent [original facts](owned_growth_originals/original-observations.json)
and [model](owned_growth_originals/model.py) replace the original eighteen-character
Marker with nineteen `a` characters. Raw UTF-16 plus the original OLE descriptor
needs 53 variable bytes, exceeding the original record's 51-byte payload capacity.
The expected image appends a 68-byte hidden terminal within the same owned DATA
page and retains the logical root through a forwarding pointer. Slot/free counts
change; EOF, maps, catalog, properties and every unrelated page remain exact.
The model uses existing independently qualified forwarding laws, not a new
producer's output.

A separately frozen [second model](owned_growth_originals/second_model.py) takes
those authored growth images as inputs and changes nineteen `a` characters to
nineteen `b` characters. Only the hidden terminal's 38 Text bytes change. Root
pointer, terminal geometry, ID, OLE and all other bytes remain exact. Original,
first and second expected images are immutable and distinct from executed saves.

Two native source tests pass eight views over six unique files, checking exact
rows, properties, long values and complete expected overlays. Four fresh bounded
reads through the existing pinned Jackcess getter match every expected inventory.
Inputs remain unchanged. No source constructor invokes a native producer or
child process. See the [source record](results/ace16-owned-growth-sources-2026-11-01.json).

The source gates are ignored unless selected with explicit corpus directories:

```sh
TABLECRAFT_PROPERTY_LVAL_ORIGINALS_DIR=/path/to/originals \
TABLECRAFT_OWNED_GROWTH_EXPECTED_DIR=/path/to/first-expected \
TABLECRAFT_OWNED_GROWTH_SECOND_EXPECTED_DIR=/path/to/second-expected \
cargo test --locked -p tablecraft-ace --all-features --lib \
  normal_ordinary_sources::owned_growth -- --ignored
```

The shared testkit loader pins all six images before returning a case to a write
test. These are evidence bindings, not engine permission. Native write admission,
recovery and Microsoft Access save/reopen remain separate qualification gates.

Separate [joint sources](results/ace16-joint-owned-growth-sources-2026-11-01.json)
add four original-derived outcomes combining Marker growth with ID NULL or 24.
Their [model](owned_growth_originals/joint_model.py) and
[requests](owned_growth_originals/joint_requests.json) preserve the original ID
payload for NULL and clear its bit at the new mask offset 67. Six native views
and four fresh SDK inventories pass before their producer calls. The first six
images and their original evidence remain unchanged.

## Already-owned alternate destinations

The [alternate model](owned_growth_originals/alternate_model.py) and its
[original requests](owned_growth_originals/alternate_requests.json) derive two
BEFORE files from the same immutable SDK originals. They saturate the original
root directory and add an empty, already-owned DATA destination before editing.
Their literal first outcomes change only the root forwarding capsule and that
existing destination. Maps and EOF remain exact during the edit.

A separate [second model](owned_growth_originals/alternate_second_model.py) uses
those independently authored first outcomes as its originals and changes only
the forwarded terminal's same-length text payload. Both original files,
both first outcomes and both second outcomes pass native read gates; six
independent SDK reads match complete predicted reports. See the
[source record](results/ace16-alternate-owned-sources-2026-11-01.json).
This is authored placement evidence, not native Access allocation policy or
write-time page allocation.

The separate [two-row source qualification](ACE16_MULTIROW_OWNED_GROWTH_SOURCES.md) covers competing last-slot reservations and mixed same-page/cross-page events, with ten additional independently read source images.
