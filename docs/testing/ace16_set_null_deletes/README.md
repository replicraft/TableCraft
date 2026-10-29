# Original-file DELETE and SET NULL proof

Two engineering candidates on immutable public-created V2016 originals match
complete SOURCE/request-derived images. Two separately bound fresh inspections
pass. The basic case deletes one parent and clears two surviving foreign keys;
the mixed case also deletes one child through a separate CASCADE relationship.
The final snapshots contain 18 user rows and 30 physical key/ROW pairs. NULL edits
are excluded from the total of three deleted logical roots.

This is independent original-file evidence for the committed engineering preview,
not Microsoft Access execution, native save/reopen or required data-macro
qualification. Each public reference performs one parent deletion. It does not
establish multi-call transaction atomicity or crash recovery.

## Genuine sources and independent construction

The repository-authored [helper](SetNullDeleteFixture.java) uses the previously
reviewed Jackcess 5.0.4 public builders. Parents have required manual Int32 primary
keys; Children have required manual IDs, nullable ParentID, two matching rows,
two nonmatching rows and an existing NULL. Marker values and a separate Sentinel
must remain unchanged. The mixed original adds a distinct CASCADE child table
with matching, nonmatching and NULL records. No original bytes, relationships,
counts or indexes are patched, and generation does not search for a layout.

Microsoft's [Access CONSTRAINT clause](https://support.microsoft.com/en-gb/access/constraint-clause)
describes ON DELETE SET NULL directly. The pinned public Jackcess API is
`setReferentialIntegrity().setCascadeNullOnDelete().toRelationship(database)`.
Source qualification separately binds raw relationship flags, reciprocal foreign
index declarations, genuine required primary authority, fields/properties/masks,
full key/ROW membership, historical counters, maps and the catalog LVAL census.
A relationship getter or an unchecked report does not satisfy these gates.

SOURCE5 is one compile, two creates and two inert inspections. Capture keeps
qualification false; both saved raw gates pass before constructor execution.
The [model policy](MODEL_POLICY.md), closed [requests](requests.json) and complete
constructor closure are frozen before either image is derived, and both complete
images precede public AFTERs, producer export/build and native candidates.
The model uses original associations, retains opaque fixed payloads when clearing
NULL bits, tombstones deleted roots and handles each physical index once. It
preserves maps, history, unrelated records and inactive index residue.

## Separate phases and observed differences

Two inert expected-image reads pass before REF4. Four public reference calls then
pass two source/request transition proofs. Each public file differs from its
earlier engineering image in exactly five bytes:

- Two low bytes in newly NULL foreign-key payloads: the public writer stores zero,
  while the engineering policy retains the old opaque bytes behind cleared bits.
- Three bytes in vacated Child FK leaf wire storage, outside the final active
  stream. Live key/ROW streams, masks, values and historical counters agree.

These observations do not change the model or expected images. The isolated build
uses committed `13c075bb` and an initially absent target, records actual owning
engine compilation, and binds all 4,667 exported Git files and the copied artifact.
Both native complete files must match the earlier images before reader one. Both
images are checked again before and after each separate fresh inspection.

SOURCE5, expected-read2, REF4, native2 and fresh2 each pass their first runtime
invocation. Earlier pure failures remain separate: the first saved gate expected
a nonexistent map `reference` key, and the first constructor used a stale table
binding in its final preservation check. Their original programs/freezes and the
first derive input barrier remain retained. Corrections derive the map reference
from verified page/slot fields and add one loop-local table binding. Neither
correction uses an AFTER image. The exceptions were observed in root tool output;
no standalone raw traceback log was retained, and none is presented as a pin.

## Repeatable saved replay

```sh
python3 -B docs/testing/ace16_set_null_deletes/verify.py
python3 -B docs/testing/ace16_set_null_deletes/verify.py --verify-git
```

Default replay writes no files and starts no subprocess. It verifies exact
text/function transformations from the executed source, constructor and public
proof; requalifies both originals; and rederives both complete images before
interpreting AFTERs. It then checks all finite phase receipts, public differences,
archive/build provenance, native image barriers and fresh proofs.

The first default passes 2,594 pin occurrences across 263 paths. It also passes
with `subprocess.run` and `subprocess.Popen` replaced by rejecting guards. Optional
Git mode passes one metadata-only batch for 4,667 source blobs and the receipt;
it never runs Cargo, Java or a producer. Missing private evidence returns
`available: false` and status 2. No fixture binaries or compiled tools are bundled.

Limits remain 8MiB files, 16MiB per stream, 512MiB Java heap and 30 seconds per
child. Native example allowances remain unchanged. The creator and readers disable
expressions, reject linked opens and select UTC/LOCAL_DATE_TIME explicitly.
V2019, tuples, wider/internal trees, mixed explicit seeds, UPDATE propagation and
failure/recovery require their separate evidence; they are not inferred here.
See the [result](../results/ace16-set-null-delete-originals-2026-10-28.json),
[committed build receipt](../results/ace16-set-null-delete-example-build-2026-10-28.json)
and [engineering contract](../ACE16_SET_NULL_DELETES.md).
