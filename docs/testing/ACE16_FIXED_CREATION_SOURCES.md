# Fixed-scalar creation source qualification

Five independently authored header-5 source files extend the committed canonical
source envelope with ten ordinary fixed fields: Boolean, Byte, Int16, Int32,
Int64, Currency, Single, Double, legacy DATE and GUID. The pure testkit owner
serializes their declarations and rows without a production creator or codec.
The existing header, catalog, system tables and allocation envelope remain exact;
only the user declaration, DATA records and its candidate-map bit change.

The source cases contain no rows, numeric/calendar endpoints, nullable cells,
signed-zero representations, and a 62-row boundary: 70 rows in total. A record
has 59 payload bytes, a two-byte field count and a two-byte mask, with deliberately
unaligned offsets and no variable footer. Boolean uses its mask bit as its value
and has no NULL carrier. Other NULL fields retain zero inactive payloads. Float
and DATE zero signs remain distinct bits; GUIDs exercise the 32/16/16-bit native
word order. The boundary leaves 52 DATA bytes, so its owned map retains page 8
while the candidate map is empty.

The first native source gate passed all five files, complete scalar observations,
field metadata, ordinary structural gates and source-bound Metadata/map capture.
A separate first export wrote five fresh, no-overwrite originals and froze their
complete hashes before any generic fixed creator execution. Source strict checks
and formatting passed. The nonignored five-hash CI anchor passed in the
preview-only configuration without requiring journal. All five first read-only
Jackcess 5.0.4 calls then passed, including all four metadata APIs, complete system/user rows, exact Currency/GUID
observations and
original raw float/DATE bits alongside calendar values. A separate saved-only
comparison reconstructed all five complete source images and matched every SDK
stdout byte. Sources and closed input/class pins remained unchanged. The new
scalar producer has not been executed. Final source strict and formatting checks
also passed.

This is source qualification, not a normal ACE save permission or native Access
creation/open/save/reopen comparison. The inherited unresolved header and system
semantics remain explicit limitations; empty opaque carriers do not establish
required-event absence for arbitrary imported files. The committed canonical
source qualification record owns that envelope's provenance and earlier SDK
observations; the new scalar payloads now have their own separate observations.

The [source result](results/ace16-fixed-creation-source-2026-10-31.json) records
the exact original hashes, read observations, CI anchors and qualification limits.
