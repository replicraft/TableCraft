# Native ACE 16 database-creation qualification

A production database creator is not implemented. The source-first work below
establishes independent complete-file inputs before introducing one. It uses
originally authored bytes, not a bundled SDK database or an opaque template.

### Authored header-5 source qualification, version 1

Three independently authored, 36,864-byte files exercise an empty Items table,
(1, 10), and the signed Int32 endpoints. Their complete page and record ledgers
were frozen before reads. Jackcess 5.0.4 opened all three read-only as V2016;
file-format, database, summary and user-defined metadata calls succeeded, with
all three property maps explicitly empty. Saved verification matched the full
names, 29 column declarations, eight catalog rows, dates, physical row IDs and
Items rows for every file. All original hashes remained unchanged.

Native catalog inspection, three table checks and three row reads passed. A
separate read-only helper passed all three files through ordinary source gates
and the complete Metadata/GLOBAL/owned/candidate-map census. It invokes no
mutation API. These observations qualify the recorded read paths on these three inputs;
they do not establish a production creator or normal ACE write permission.

Reader acceptance does not erase the first model's constructive omissions:
column-descriptor magic and column IDs are zero, variable descriptor flags omit
the constructive updatable bit, and four catalog OLE declarations have no LVAL
map pairs before the final terminator. TEXT sort descriptors are also zero;
variable-slot packing and unused descriptor fields differ from the selected SDK
constructor layout. Unresolved header ranges contain chosen
zero bytes; they are not proven reserved. Empty public property maps do not
establish that missing database-property objects or the system/ACL envelope are
sufficient for native Access. A corrective source version keeps these original
files and observations immutable. Native Access creation/open/save/reopen,
Windows behavior, native required-event absence, normal ACE editing/recovery and
broader schemas remain unqualified. No production creator was invoked.

The [version-one record](results/ace16-canonical-creation-source-v1-2026-10-31.json)
pins original images, models, readers, exact saved comparisons and remaining gaps.

### Corrective generic source qualification, version 2

The second independently authored source set retains the same nine-page,
header-5 envelope and corrects the known constructive omissions in version 1:
column-descriptor magic and column IDs, updatable variable descriptors, selected
text sort metadata, ordinary-variable/LVAL slot order, the mask-only Boolean
fixed offset, and four explicit LVAL owned/candidate map pairs. Six immutable
36,864-byte sources cover empty, one-row, signed-extrema and 64-row Items tables;
People Stats with Person Id and Score Δ; and Wide Rows with 16 columns and 58
rows. The wide DATA page has 22 free bytes and remains owned while its candidate
map is empty. The selected 64-row ceiling is separate from physical fit.

All six first read-only SDK calls passed as V2016. Each attempted all four
metadata APIs and reported empty database/summary/user-defined property maps.
Saved verification compared complete names, schemas, catalog rows, scalar cells,
dates and physical row IDs: 188 column declarations, 48 catalog rows and 126 user
rows across the six reports. All source hashes remained unchanged. Twelve
native check/row calls passed; the strongest read-only Metadata helper passed
all six originals, including GLOBAL and all 16 map records, with no mutation
writer or production creator call.

The independent Rust testkit fixtures matched all six frozen Python images and
SHA256 identities on their first source-gate execution, before native Metadata
read admission. Their nonignored six-hash CI anchor also passed. ACE and testkit
all-target, all-feature strict checks and workspace formatting passed. Database
creation is not implemented or qualified by this source phase. These are closed read-profile observations, not native Access creation,
open/save/reopen, Windows evidence or normal ACE write permission. Unresolved
header fields remain chosen zeros, empty public property maps do not establish
native system/ACL sufficiency, and native required-event absence remains
unqualified. Version 1 sources and their accepted omissions remain immutable
historical evidence.

The [version-two record](results/ace16-canonical-creation-source-v2-2026-10-31.json)
pins the six sources, independent Rust fixtures and observed read checks.
