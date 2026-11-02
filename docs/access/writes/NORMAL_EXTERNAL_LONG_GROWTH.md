# Growing the final external chunk

Normal external Memo/OLE UPDATE can expand an original terminal chunk into free
space on its existing LVAL page. It keeps every original reference, descriptor
kind and chain link. The terminal must occupy the physically last slot. Its new
body must fit the checked page free space and native external-row capacity,
including the link word for a chain. Earlier chain partitions stay fixed.

This extends the [external reuse route](NORMAL_EXTERNAL_LONG_UPDATE.md) through
the same Auto preflight, TCNI5 intent and checked V1 recovery. Planning checks
capacity before staging; it does not catch a failed planner and retry another
route. The expanded slot's start and page free count change together with its
payload and original parent length word. Other slots, inactive bytes outside
the newly occupied extent, maps, old orphan rows and EOF remain exact.

Fresh proof encodes requested payloads independently and walks original raw
ROW/link geometry. It reconstructs the expanded terminal and parent length,
then checks complete final Source and the whole image. No staged chunk or patch
is used as proof authority. Complete source/catalog/LVAL ownership and original
expected-value checks still apply to empty and no-op requests.

The [source family](../../testing/ACE16_EXTERNAL_CONTINUATION_WIDE_SPLIT_SOURCES.md)
passed complete native reads and 36 inert independent-reader reports before
matching writers. Its twelve growth outcomes cover single, chained and joint
changes under both header variants and both Memo storage declarations.
[Whole-page clearing/retirement](NORMAL_EXTERNAL_LONG_RETIREMENT.md) now has separate
connected acceptance. [Mixed changes](NORMAL_EXTERNAL_LONG_MIXED.md) now have
their separate composed route. Row relocation and new LVAL
allocation remain work. Native Microsoft Access save/reopen remains unrun.

Twelve changed native commits plus eight original no-op/empty commits, independent
proof and forged-image controls, 72 recovery contexts, measured resources,
sixteen typed/SQL saves with sixteen empty follow-ons, stale-handle checks and
four CLI saves/readbacks pass. Existing reuse commits and retained-session tests
pass with active capacity refusals updated to actual nonfitting requests.
Strict combined lint, journal-only and preview-only builds, formatting and the
focused CLI help check pass. See the
[execution record](../../testing/results/ace16-normal-external-long-growth-writes-2026-11-01.json).
