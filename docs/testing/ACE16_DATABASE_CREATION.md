# Fresh ACE 16 fixed-scalar construction

The optional standalone `ace16-create` feature constructs a complete header-5
database in memory with one ordinary table. `create_ace16_database` accepts named
`CreateColumn` declarations and borrowed `Value` rows. It supports Boolean, Byte,
Int16, Int32, Int64, Currency, Single, Double, legacy DateTime and Guid.
`create_ace16_int32_database` remains the convenience API for borrowed Int32 rows.
The Access creation service accepts these typed declarations through
`prepare_typed`; the CLI creation command remains Int32-only.

The profile admits 1–16 columns and 0–64 rows, also requiring the complete records
and row directory to fit one DATA page. Names retain their spelling under the
existing native name checks. Declarations are validated even for empty tables or
all-NULL input. Present values must have the exact declared carrier; there is no
numeric conversion. Boolean uses its value bit without payload bytes and refuses
NULL. Other admitted fields allow NULL with zeroed inactive payload bytes.
Decimal, variable fields and extended DateTime declarations remain unsupported.

Catalog table names use a prevalidated stored-text encoding plan. Leading U+FEFF
is escaped where the qualified representation is reversible; unsupported
representations return `TextEncoding` before image allocation. Column-definition
names retain raw UTF-16. The [catalog-name repair](ACE16_CATALOG_NAME_ENCODING.md)
records the independent whole-image checks and actual CLI reproduction.

Single and Double preserve their raw representations, including signed zero,
subnormals, infinities and quiet/signalling NaN payloads. Legacy DATE preserves
its raw bits without calendar conversion; Currency stores its scaled Int64 and
Guid uses the native mixed-endian representation. These storage results do not
qualify arithmetic or expression behavior.

Construction validates borrowed inputs before reserving one complete 36,864-byte
image. Bytes, Rows and Steps remain independent cumulative allowances, with
initial and final cancellation checks. The returned `CreatedDatabase` exposes
immutable bytes and counts. It performs no filesystem operation and supplies no
existing-file edit, journal or normal-save authority.

The first standalone producer run passed all eleven tests and 28 complete-image
comparisons: five fixed-family sources, five IEEE sources, six Int32 sources
through both APIs, and the six retained convenience comparisons. The independent
fixed and IEEE source records were committed before this producer execution
(`fcadc73d` and `d8c7509c`). Their SDK observations establish reader results for
the source goldens; no new SDK reads of generic producer output are claimed.
Native Access and Windows creation/open/save comparisons remain unqualified.
The authored nine-page system envelope and unresolved header choices retain
their existing limits; these results do not establish general system/ACL,
macro-absence or imported ACE save compatibility.

The existing fixed mutation regressions passed 116 tests (seven ignored),
record composition passed seventeen, and the preserved scalar contract passed
54 (two ignored). Strict checks passed for minimal and all-feature targets;
the dependency guard also passed.

The [generic constructor result](results/ace16-native-fixed-creation-2026-10-31.json)
records the first execution and encoder regressions. The
[Int32 constructor result](results/ace16-native-int32-creation-2026-10-31.json)
retains the earlier implementation phase.

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

### Fixed-scalar source continuation

Five [fixed-scalar sources](ACE16_FIXED_CREATION_SOURCES.md) now have independent
complete-image and SDK read qualification for ten field families, NULL masks,
signed zero, GUID byte order and physical page fit. Their frozen originals and
CI hash anchor precede execution of a generic fixed-scalar creator. The current
generic constructor consumes those qualified fixed-scalar declarations;
the CLI remains restricted to Int32.

## Shared publication boundary

The [Access creation service](../access/writes/FRESH_DATABASE_CREATION.md)
forwards typed declarations and exact Value rows through `prepare_typed`, then
uses the existing required fresh publisher and receipt. Its ten fixed/IEEE
whole-file publications pass. The [CLI](../cli/NATIVE_CREATION.md) retains its
Int32 input syntax; this library method adds no imported-file write permission.

### Stored-text and variable-field source qualification

[Variable-field sources](ACE16_VARIABLE_CREATION_SOURCES.md) now qualify exact
Text/Binary/Decimal declarations, NULL/empty masks, UTF-16 and retained Decimal
scale before adding those families to the creator. Separately,
[catalog-name sources](ACE16_CATALOG_NAME_SOURCES.md) qualify escaped stored
names and raw definition names before repairing the leading-BOM name bug.
