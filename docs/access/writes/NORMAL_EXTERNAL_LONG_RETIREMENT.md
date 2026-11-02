# Clearing external Memo and OLE values

Normal fitted UPDATE can replace qualified external Memo/OLE values with NULL,
or an OLE value with present-empty Binary, while retiring complete LVAL pages.
The existing typed, SQL and CLI journal services use TCNI5 and checked V1 recovery.
This is separate from the existing reuse and terminal-growth branches.

Every live chunk on a page to be retired must belong to the selected original
values of the same column. Complete Source, catalog, map-record and LVAL ownership
checks precede staging. An unselected live slot, another owner or conflicting
map role refuses retirement. Existing unrelated orphan pages are preserved;
this operation is not an implicit orphan collector.

Parent records are rebuilt within their original fitted capacities. NULL and
present-empty OLE keep their distinct masks and descriptors. Retirement clears
the checked column-owned/candidate membership and releases the actual represented
GLOBAL bit. Only the four-byte invalid prefix changes on a retired page; every
remaining page byte stays source-relative. Other rows, residual payloads,
unrelated maps, table counts and EOF remain exact.

Fresh proof walks original descriptors and chains independently, reconstructs
parent records and original map-bit effects, then compares the complete file and
recaptures final Source. It consumes no staged descriptor, retirement patch or
producer trace. The shared original-cell mask helper has actual INDEX and LVAL
consumers; each supplies its own checked ownership and role authority.

The [source family](../../testing/ACE16_EXTERNAL_CONTINUATION_WIDE_SPLIT_SOURCES.md)
passed 36 native views and 36 inert independent-reader reports before matching
writers. Twenty changed commits and eight no-op/empty commits, independent proof,
forged-image and journal controls, 120 recovery contexts and measured public
resource/cancellation checks pass. Existing reuse, terminal-growth and INDEX
retirement ownership-audit checks pass. Old refusal tests now select an actual shared page
with an unselected slot; historical source laws and images remain unchanged.

Twenty typed and sixteen SQL saves, retained-lease/receipt and stale-predecessor
checks, and sixteen CLI saves/readbacks pass. Empty Binary is typed coverage;
SQL and CLI use supported NULL assignments rather than invented Binary literals.
Default, journal-only and preview-only compilation, strict lint and formatting
pass. See the [execution record](../../testing/results/ace16-external-long-retirement-writes-2026-11-02.json).

Mixed retirement with changed retained descriptors, shared-page partial removal,
relocation, new external allocation and native Microsoft Access save/reopen
remain active work. Independent reader agreement does not establish native save
compatibility.
