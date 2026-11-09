# Genuine SDK-created NODE originals

Two fresh ACE16 files created through the pinned read/write SDK now provide
healthy non-leaf Int32 indexes for native read qualification and current normal
INSERT/DELETE refusal controls. This is source evidence, not an admitted NODE
mutation or a native Access save/reopen comparison.

The maintained [builder](../../../scripts/compatibility/genuine_node_originals/BuildNodeIndexOriginal.java)
and [literal recipe](../../../scripts/compatibility/genuine_node_originals/recipe.json)
create Items with three manual LONG fields, two physical indexes and four
logical aliases. Positions 0..799 supply `(position+1, position-400, position%7)`.
Sentinel has one manual Marker value 999. Public builders use an exclusive
CREATE_NEW bounded channel, disable expression evaluation and deny external
resolution. No TableCraft writer, existing AFTER or raw-byte patcher creates
these originals.

| Header | Bytes | TD | Primary root | Guard root |
| --- | ---: | ---: | ---: | ---: |
| 5 | 434176 | 89 | 91 | 92 |
| 6 | 503808 | 106 | 108 | 109 |

Both actual physical roots were observed as NODE pages. Their child order is
structural, not numeric: header5 Primary [98,97], Guard [101,100]; header6 Primary
[115,114], Guard [118,117]. Original leaf counts are Primary302/498 and
Guard395/405. The full Guard bound includes a ROW suffix because equal Guard
values occur in both children. Row count alone never established tree shape.

One compile, two original creations and twelve bounded read-only SDK calls
passed on their first attempts. The unchanged complete inventory observer,
metadata oracle and four alias cursors verified all 800 rows/2400 cells,
actual raw row roots, physical descriptors and every PAGE hash. All original
and tool hashes remained unchanged. The two native test functions then passed:
complete ordinary Source plus retained owned-tree membership/all aliases, and
precise current NODE refusals for normal INSERT/DELETE/witness preparation with
empty and nonempty requests. Source checks took 0.11s and refusal checks 0.58s.
The initial source build was blocked by an unrelated DESC-NULL gate's undeclared
serde_json use; root corrected it to literal pins before the successful runs.
That first failure remains recorded, with no NODE source change.

The [result record](../../results/rows/ace16-genuine-node-originals-2026-11-01.json) pins both
complete originals, recipe/builder/tool identity, all fifteen SDK child receipts
and native logs. It contains no class/JAR binaries or machine paths. Whole-file
hashes belong to this frozen creation, including SDK system metadata; a fresh
creation needs a new freeze and read qualification rather than substituted hashes.

Reproduction uses the maintained
[bounded channel](../../access/indexes/parent_index_split_originals/BoundedFileChannel.java) and a
separately supplied pinned SDK JAR. Compile into a fresh directory, then invoke
`BuildNodeIndexOriginal fresh.accdb V2016` or `V2019`. Creation must fail when
the target already exists. Freeze the generator/tool closure before creation,
and require observed NODE roots and complete read-only qualification afterward.
Keep any first failure; do not increase the population in place to force a pass.

The ignored engine gates use `TABLECRAFT_NODE_ORIGINAL_SOURCE_DIR` and literal
hashes in `normal_delete/node_sources.rs`. The source gate must pass before the
separate refusal gate. No AFTER or accepted native mutation candidate was produced. The refusal gate
made eight normal planner attempts and four witness-preparation attempts; all
refused before staging and durable writes. Topology-preserving NODE mutation, contraction,
allocation and native Access conformance remain separate acceptance work.
