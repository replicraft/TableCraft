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
