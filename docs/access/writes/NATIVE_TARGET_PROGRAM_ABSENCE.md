# Native target-table program absence profile

Normal ordinary fixed-field planning now has a distinct, bounded admission for
an actual seventeen-field native catalog. It qualifies the selected local table
without requiring every unrelated application object to match the initial
synthetic data-only catalog grammar. This is an inferred supported profile, not
an exhaustive ACE storage theorem or observed Microsoft Access save/reopen
conformance.

## Required conjunction

The original image must have a standard, unencoded ACE 2016/2019 capture with
subversion zero. `NoRequiredTablePrograms` then requires:

- The exact seventeen-field MSysObjects declaration, with all native field
  identities, types, flags, offsets and variable slots checked. Every live
  catalog record has complete field count, footer, mask and variable extents,
  and agrees with its actual captured object and physical origin.
- A local Type=1/Flags=0 target with its full native identity, actual TD page and
  Tables parent. All direct children refuse. Cross-kind object-ID aliases and
  ambiguous parent references sharing the selected TD's low24 bits refuse.
- Four physically present OLE declarations, `Lv`, `LvProp`, `LvModule` and
  `LvExtra`, whose target cells are NULL with empty raw spans. The five remote
  fields, `Database`, `Connect`, `ForeignName`, `RmtInfoShort` and `RmtInfoLong`,
  must satisfy the same NULL/empty-span condition. Present-empty, opaque,
  omitted and short carriers do not count as absence.
- Complete selected user TD framing through the shared declaration validator:
  header, column descriptors, names, index count/history blocks, physical and
  logical descriptors, index names and exact declaration terminator/tail.
  Every local table's Memo/OLE declaration must have a complete, unique matching
  owned/candidate map pair. A parser's `complete` flag alone is insufficient.
- The existing full normal Source checks for original properties and
  constraints, dependencies, relationships, row roots, index membership, map
  ownership, live LVAL references and physical overlap protection. Relationships
  involving the selected table and unsupported target constraints still refuse.

Catalog slots with flags normally refuse. The sole exception is a bounded
zero-length deleted tombstone, optionally also carrying the relocated bit. Its
start and end must coincide outside the complete slot directory and within the
page; it contains no record or payload reference. Unsupported flag bits,
relocated-only slots and every nonempty flagged catalog record refuse. This
exception preserves the observed native empty slot; it does not skip deleted
record contents when establishing child coverage.

Unknown object storage kinds refuse, except the explicitly recognized native
database/property record kinds. Linked and ODBC table origins also refuse until
complete linked ownership is qualified. Known unrelated forms, reports, macros,
modules and other application objects can remain opaque: their stored bytes and
referenced storage are protected, and nothing is executed. Their existence grants
no authority to execute them or to omit a selected-table hook.

## Evidence and claim boundary

A maintained native Access consumer provides concrete locator evidence:
[`clsDbTableDataMacro`](https://github.com/joyfullservice/msaccess-vcs-addin/blob/main/Version%20Control.accda.src/modules/Components/clsDbTableDataMacro.cls)
selects local Type=1 catalog rows with non-NULL LvExtra and invokes actual
`acTableDataMacro` export. It also imports and removes table macros through that
native object kind. Its export handles possible locator false positives. This
supports the carrier inference; it does not establish a universal converse.

The project's
[compatibility record](https://github.com/joyfullservice/msaccess-vcs-addin/wiki/Supported-Objects)
lists table-data-macro import/export testing at `tblSaveXML` and targets Access
2010 onward. That is upstream recorded coverage, not a newly executed TableCraft
comparison. Public three-carrier coverage, XML semantics, an empty selected
program list and SDK property counts are each insufficient on their own.

Two unchanged independently generated originals and ten independently modeled
AFTER views passed native read qualification and sixty exact independent SDK
observations before genuine producer calls. The first CLI matrix then passed ten
journal saves and ten exact TSV readbacks; each complete file matched its frozen
expected image and both originals remained unchanged. The observed selected
table has three ordinary Int32 fields and two physical indexes with four logical
aliases. These observations do not qualify arbitrary genuine schemas.

Source-only controls exercise the full normal Source path, carrier refusals,
identity aliases, declaration closure and eighteen NULL cells with nonempty raw
spans. Five genuine planning/recovery tests pass, including mixed DATA/INDEX
rollback and write-free confirmation. All 1,082 ACE unit tests and 79 existing
recovery tests pass; the typed/SQL session and CLI checks retain whole-image
expectations. See the [write record](../../testing/results/ace16-genuine-ordinary-target-writes-2026-10-31.json). Native Microsoft Access write/reopen,
Windows, mixed-client and physical power-loss conformance remain unrun. Existing
positive-program and initial data-only evidence keeps its separate scope.

## Planning and recovery boundary

Catalog structure chooses the native seventeen-field or initial global family
before planning. A failed admission never retries the other family. The private
certificate borrows the exact image/table/origin, has no public constructor and
uses a distinct normal Source payload-admission case. It is neither an empty
program selection nor an engineering-preview conversion.

The existing fixed DATA and fitting manual-Int32 index rules remain those in
[ordinary fixed writes](NORMAL_ACE_ORDINARY_FIXED.md). Independent proof freshly
recaptures BEFORE, repeats the admission conjunction and compares the complete
expected image and changed-row count. All bytes outside qualified changes remain
exact, including unrelated object payloads and empty tombstone directory words.
No allocation, split, record growth or EOF change is introduced.

`TCNI0004` retains original typed requests and the BEFORE digest, never an absence
flag or serialized certificate. Recovery uses the same fresh native admission
and complete-image reconstruction before effects. The genuine recovery checks exercise this contract independently of the earlier
source-only observations.
