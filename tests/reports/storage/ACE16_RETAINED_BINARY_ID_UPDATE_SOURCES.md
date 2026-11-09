# Int32 key updates beside retained Binary indexes: sources

Four independent source views now pass native reads, all six index cursors,
constraints, canonical UPDATE Source and Actions qualification. Two SDK-authored
BinaryKeys originals have an Int32 Id, nullable Binary KeyValue and Int32 Marker,
three physical indexes and six logical aliases. One prospective image per header
changes original physical ROW0 Id from 1 to 14, retaining Marker 17 and Binary NULL.

The primary cursor now orders original rows 1 through 12 followed by original row
0; physical table order and both Binary key populations stay original. The four
Id bytes and primary 117 active entry bytes are the only permitted edit spans.
Primary bitmap/free/links/protected tail, every Binary index byte, histories
13/12/12, DATA framing/masks, aliases/maps/properties/catalog and EOF remain exact.
Independent laws precede construction; a separate raw-role verifier derives
signed I32 components and ROW identities without using the image model. Original
reader reports must round-trip before any prospective file is created.

Native replay independently reconstructs those literal changes, checks complete
rows/cursors/counters and captures full Source. It calls no mutation planner.
Thirty-two independent reader observations match complete frozen reports. This
comprises thirty fresh corrected reads and two reused successful metadata reads;
thirty-three actual calls include one excluded historical failure. No SDK writer,
compiler, generator or stored expression runs. Saved verification starts no child
and checks exactly 66 concrete regular files with pinned stream provenance.

The [source record](../../results/storage/ace16-retained-binary-id-update-sources-2026-11-02.json)
preserves three distinct corrections. A raw bitmap-offset assumption was fixed
before laws/models existed. Native test registration then changed one historical
Rust reference; a narrow binding checks the old snapshot plus the exact added
registration without changing executable inputs. Finally the first cursor read
requested nonexistent `Binary Id` instead of `Marker`; only that final field
argument changed in 24 cursor commands. Its failed output remains excluded, the
two correct metadata reads are explicitly reused, and source images/models/report
expectations remain unchanged.

Subsequent [actual save qualification](../../../docs/access/writes/NORMAL_RETAINED_BINARY_ID_UPDATE.md)
uses these unchanged images for native commits, recovery and retained typed/SQL/CLI
saves. The [write result](../../results/storage/ace16-retained-binary-id-updates-2026-11-02.json)
is separate from this preserved source record. This source gate never asserts
historical mutation refusal as a permanent format rule. Native Microsoft Access
save/reopen and Windows mixed-client behavior remain unrun.

```sh
TABLECRAFT_BINARY_ID_UPDATE_SOURCE_DIR=/absolute/path/to/expected-first \
  cargo test -p tablecraft-ace --all-features --lib \
  table::mutation::normal_ordinary_sources::binary_id_update::binary_id_sources_have_exact_primary_membership_and_full_source \
  -- --ignored --exact
```
