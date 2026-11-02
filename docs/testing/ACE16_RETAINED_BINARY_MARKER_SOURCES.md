# Stored-field updates beside retained Binary indexes: sources

Two SDK-authored Binary-indexed originals and four independent prospective images
now pass six native read/Actions Source contexts and 48 exact inert reader
reports. The table has an Int32 Id, Binary KeyValue and Int32 Marker, with three
physical indexes and six logical aliases. Marker is not an index member.

One image per header changes two original Marker values, 17 to 29 and 68 to 70.
Another clears only Marker's presence bit for the latter row, preserving its
inactive Int32 bytes. Source laws precede construction. Every byte outside those
literal fixed spans or presence bit remains exact, including the nullable Binary
key row, all INDEX pages, histories, map storage, catalog and EOF. No-op and empty
requests reuse originals rather than creating duplicate images.

Independent native replay checks whole images, logical rows, Marker declarations,
complete absent property inventories, constraints, all aliases and full Actions
Source. Read/Actions qualification grants no canonical UPDATE permission. A
separate initial observer recorded canonical capture refusing both originals;
that historical observation is not a permanent format invariant. The later
qualified implementation now reports `ADMITTED` for both headers without changing
this read gate or any source image.

The 48 independent reports cover full inventory, metadata and six index cursors
for each view. Closed saved verification validates exact input/output bindings,
all 59 source artifacts and typed receipt schemas without starting children.
An external launch-wrapper class-name check was corrected before any reader
child ran; source/controllers remained unchanged and no SDK call was retried.

```sh
TABLECRAFT_RETAINED_BINARY_MARKER_SOURCE_DIR=/absolute/path/to/expected-first \
  cargo test -p tablecraft-ace --all-features --lib \
  retained_binary_marker_sources_have_exact_bytes_and_full_read_actions_coverage \
  -- --ignored
```

See the [source record](results/ace16-retained-binary-marker-sources-2026-11-02.json).
Subsequent [normal fixed UPDATE acceptance](../access/writes/NORMAL_RETAINED_BINARY_MARKER_UPDATE.md)
now passes against these same frozen files; its separate
[write result](results/ace16-retained-binary-marker-writes-2026-11-02.json) records
the matching runs. The linked source record is intentionally unchanged and
describes the earlier source-only phase. Native Microsoft Access save/reopen
and Windows mixed-client qualification remain unrun.
