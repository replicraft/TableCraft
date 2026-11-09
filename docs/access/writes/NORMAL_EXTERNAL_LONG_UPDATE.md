# Reusing external Memo and OLE storage

Normal fitting UPDATE can replace qualified nonempty external Memo/OLE payloads
without allocating or retiring storage. Memo uses Text and OLE uses Binary.
Original single-row and chained descriptors retain their kind, references,
links and reserved bytes. The reuse profile fits every retained chunk, including a nonempty terminal
chunk. [Terminal growth](NORMAL_EXTERNAL_LONG_GROWTH.md) can separately expand
the last slot into its checked existing page capacity. A shorter terminal must occupy the physically last
slot on its page; its start and page free count change together. Other row
boundaries, freed prefix bytes and EOF remain source-relative.

Fresh ordinary Source and complete LVAL checks establish actual column ownership,
map membership and catalog-wide exclusivity before selecting this route. Distinct
slots on one same-column page are allowed; shared row references or cross-owner
pages refuse. Existing orphan rows are preserved rather than silently reclaimed.
Empty and no-op groups repeat admission. A separate
[whole-page retirement route](NORMAL_EXTERNAL_LONG_RETIREMENT.md) now admits
qualified NULL and present-empty OLE replacements. Changed fixed/short siblings,
inline transitions and dropped retained chain tails remain outside reuse.

The existing fitting stage supplies page patches through TCNI5 and checked V1
recovery. Independent proof walks original raw descriptors, links and slot spans,
encodes requested values separately and reconstructs the complete expected file.
It consumes no staged chunks or patches. Logical changed-row counts follow
original assignments even when an equal-length payload changes no descriptor.
The shared typed, SQL and CLI services consume this route; SQL tests use Memo
expressions rather than inventing Binary literals.

Source qualification precedes matching writers. The four original databases and
32 complete contexts passed current native reads, and 32 inert independent reader
reports passed. See the [source record](../../../tests/reports/rows/ACE16_RETAINED_REUSE_COMPANION_SOURCES.md).
Native Microsoft Access save/reopen remains unrun.

Twenty-eight literal native commits, 120 recovery contexts, forged/stale and
resource controls, 40 typed/SQL saves with 40 same-lease empty follow-ons and
twelve CLI saves/readbacks pass. The first host test used an unbracketed reserved
field name; corrected `[Memo]` SQL passes with unchanged source and expected
images. See the [execution record](../../../tests/results/rows/ace16-normal-external-long-update-writes-2026-11-01.json).
