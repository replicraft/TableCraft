# Long-value map edit authority

The optional [Memo/OLE replacement preview](ACE16_LONG_VALUE_UPDATES.md) now
checks map identity and storage authority before changing any membership bit,
including when the selected map has no existing data members. Its final
certificate repeats that proof against the immutable original catalog. This
correction does not admit indirect selected-column LVAL maps or enable normal
backend writes.

The [scoped record](results/ace16-lval-map-authority-native-2026-10-27.json)
retains the actual unsafe before candidate, source/log identities, focused tests
and read-only original-admission diagnostics.

## Actual empty-map defect

A literal ACE 16 fixture has selected Items.Memo and unrelated Other.Memo as
inline `A`. Both ordinary readers, table constraints and explicit long-value
storage checks pass. Items has empty owned/candidate maps. Changing only Other's
complete declaration to reference those exact empty map rows leaves both
storage reports at zero owned pages.

Before the correction, previewing Items.Memo → 33 U+0100 units accepted a new
external row at EOF. Items ownership changed from zero pages to one, but so did
unrelated Other ownership through the same map records. Other's displayed value
still read `A`; value equality and existing data-page claims did not establish
authority over the aliased metadata. The disjoint declaration control succeeds
without changing Other. The exact before image, accepted unsafe image and failing
assertion are preserved; no original-file candidate or JVM action was needed.

## Role and footprint proof

A paid catalog census records canonical owners for global free metadata,
ordinary-table owned/candidate maps, physical index-owned maps, and every current
or historical long-column owned/candidate declaration. Native definition and
logical column/physical index numbers form the owner identity. True logical
index aliases deduplicate by physical data number.

For one replacement operation, five prospective mutable roles must be present:
global free, selected ordinary owned/candidate, and selected column
owned/candidate. Each is checked against all captured local catalog claims,
before iterating selected existing data pages. Empty/no-op requests retain this
admission proof. A different canonical owner claiming the same packed map row
refuses with `Unsupported(RowLayout)`.

Claims also retain the exact absolute map-record byte range and its container
and indirect bitmap pages. Overlapping record ranges refuse even when malformed
container metadata uses different packed references. Bitmap-versus-bitmap and
bitmap-versus-container overlap also refuse across different owners. Distinct,
nonoverlapping rows on one ordinary metadata container remain valid. Protecting
the entire container as exclusively owned would incorrectly reject real shared
container layouts.

The internal `read_storage` helper returns only a footprint. Foreign census
inspection retains read-owned container grammar, including the existing healthy
owner1 system-map fixture; it grants no writable map object, membership proof or
cell. Global free metadata retains its separate fixed-container qualification.
Unknown kinds/flags remain explicit unsupported profiles, malformed/truncated
read prefixes keep their own errors, and work/allocation/cancellation remain
bounded. Every actual writable `MapCell` still comes through unchanged strict
`RepresentedMap::read` role provenance, selected map checks and final map replay.
No owner bytes or field flags are masked to manufacture eligibility.

Both ordinary staging and the immutable-operation final authority consume the
same census. An independently valid staged plan from a disjoint catalog cannot
certify against an original catalog whose empty maps are shared. Existing
owned/live-data overlap checks and map-bit replay remain required; footprint
identity is additional authority, not a replacement for those proofs.

## Verification and boundaries

Six independent integration functions exercise owned-only, candidate-only,
paired and cross-role empty aliases; historical declarations; ordinary roles;
shared indirect global bitmap storage; legitimate shared containers and real
physical index aliases; and limits/cancellation/retry. The indirect case concerns
existing global/ordinary metadata and leaves selected LVAL maps inline. A healthy
active index cannot simply reuse an empty LVAL owned map, so tests do not relabel
an unreadable index as a healthy causal fixture.

Two focused private functions test overlapping versus disjoint record ranges
and immutable finalizer recensus. The pure role-claim overlap case deliberately
isolates malformed-container authority; it does not claim to be a native valid
page fixture. Physical index role identity is also checked directly there.

The accepted scoped batch covers 85 distinct runtime functions, including eight
new functions, plus three compile-fail doc tests. Feature/default strict checks,
minimal default build and scoped formatting pass. Twelve default-build function
executions repeat checks also run with the preview feature and are counted once
in the distinct total.

A separate bounded read-only diagnostic checks both Memo columns on all 22
unchanged sources from the [148 compressed/raw controls](COMPRESSED_MEMO_ACCEPTANCE_OBSERVATIONS.md):
44 successful map-admission probes under the existing 256 MiB/100,000-row/
100-million-step allowance, with all source hashes unchanged. It creates no
staged plan, invokes no finalizer, publishes no candidate and runs no native or
JVM command. The temporary diagnostic and initial failing probe were archived
and removed from the library/test inputs. These diagnostics do not rerun or
replace the preceding frozen original-file acceptance.

An initial library check named a nonexistent storage helper; its failure is
retained. The first broad claim implementation then imposed writable owner0 on
an unrelated read-valid owner1 system map. That disjoint-positive failure is also
retained. The correction extracts only read-side storage identity while leaving
writable provenance strict; no fixture normalization or blanket fallback was
used. The six independent integration functions passed on their first execution.

Indirect selected-LVAL admission, bitmap/map growth, arbitrary in-file page reuse,
reclamation and native Access save/reopen qualification remain separate required
work toward full ACE 16 compatibility.
