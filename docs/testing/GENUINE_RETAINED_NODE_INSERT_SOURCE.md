# Source-first fitting NODE INSERT

This source tranche adds the complete manual row `(801,400,1)` to the two
[SDK-authored 800-row originals](GENUINE_NODE_ORIGINALS.md). The original builder
and recipe remain unchanged. Expected images were constructed from frozen
original bytes and explicit raw rules before any matching normal INSERT writer
was called. This qualifies independent reads and physical expected images; it
does not establish native Access save conformance. All four native source views
also pass; normal NODE INSERT producer and host acceptance remain pending.

The genuine originals contain two height-one physical Int32 index trees and four
logical aliases. Earlier leaves retain their original entry counts and the final
leaf takes the added entry. Primary partitions 302/498 become 302/499. Guard
partitions 395/405 become 395/406: new Guard 1 enters the left leaf, moving old ID 354
to the right. The complete Guard fence changes its ROW suffix from ID 354 to ID 347
while its value remains 3. The primary fence remains ID 302. This is an explicit
population redistribution law, distinct from the qualified NODE DELETE law.

The original final DATA page admits a 15-byte direct record at 2881..2896, slot 80.
Its count 80/free 2722 become 81/2705. Original earlier DATA pages cannot fit this
row. Table count 800 becomes 801; primary history 800 becomes 801; Guard history 7
and LAST 0 remain unchanged. All roots, child order, six INDEX pages, heights,
links, ownership/candidate maps, EOF, unused bytes and unrelated content remain
exact. Child topology order is deliberately independent of numeric page order.

The source closure contains four complete images: two original and two literal
AFTER views. Pure construction passed first attempt. Twelve bounded read-only
SDK observations on AFTER passed first attempt: full inventory, complete index
metadata and four alias cursors per image. The original twelve observations
supply the other half of the 24-observation closure. A saved comparison passed
without children; all original, class and tool hashes remained fixed. The result
record pins the complete images, source models, requests and first receipts.
Native source-gate and producer results are recorded separately when available.
No matching producer or native Access runtime was invoked by this source package.

Maintained pure artifacts live in `scripts/compatibility/genuine_node_insert/`.
`requests.json` is the immutable pre-model request snapshot; its zero constructed
images/producer calls describe that freeze time. `model.py` and `reports.py` are
byte-identical copies of the executed qualification owners. The small maintained
reproduction wrapper is composition-only and has not itself been executed in
this tranche. It accepts the original builder's two image files, uses exclusive
output creation, and imports neither an engine writer nor a JVM:

```sh
python3 scripts/compatibility/genuine_node_insert/create_expected.py \
  --originals /absolute/qualified-node-originals \
  --output /absolute/fresh-node-insert-expected
```

This source does not qualify splits, new index pages, topology changes, composite
keys, generated identities, defaults, relationships, non-Int32 index components,
or arbitrary tree heights. Those remain distinct acceptance work; the long-term
ACE 16 compatibility target is unchanged.
