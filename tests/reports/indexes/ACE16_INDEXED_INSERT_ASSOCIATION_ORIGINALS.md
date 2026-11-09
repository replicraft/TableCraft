# Cross-page insertion association originals

The [SDK generator](../../access/indexes/indexed_insert_association/BuildAssociationOriginal.java) and
[frozen recipe](../../access/indexes/indexed_insert_association/recipe.json) create Items with three
manual Int32 fields, two indexes/four aliases and an uncompressed nonkey Text
field. Nine 230-unit labels yield two observed DATA pages: earlier free218 with
eight slots, later free3599 with one. Jackcess naturally drops the earlier free
hint on a failed fit; the generator explicitly restores that candidate hint via
its UsageMap API. Ownership, GLOBAL allocation and rows are unchanged by that
separate documented hint operation.

One bounded compile, two independent SDK creations and 12 read observations pass,
with frozen inputs unchanged. Native original admission also passes both images:
exact rows/schema, all aliases/histories, complete checks, full Source capture and
both pages eligible under existing protection. No TableCraft writer or expected
AFTER participates. Generated binaries/classes remain outside Git.

A later long request needs423 bytes and must use the later page; the following
short request needs29 and fits earlier. Their independent ROW association must
follow request placement, even when table encounter order differs. The
[original record](../../results/indexes/ace16-indexed-insert-association-originals-2026-11-01.json)
retains actual hashes, geometry and execution scope. AFTER modeling and producer
qualification are separate steps; native Access comparisons remain unrun.
