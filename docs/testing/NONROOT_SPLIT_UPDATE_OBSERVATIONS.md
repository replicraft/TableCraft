# Original-file Int32 non-root leaf split acceptance

Both fixed V2016/V2019 candidates passed on their **first attempt**: two native
calls and four read-only getter/map calls, six commands total, terminal status 0.
There were no retries, disagreements or source changes. The
[compact receipt](results/ace16-nonroot-split-originals-2026-10-27.json) pins the
exact executed private programs, frozen requests, artifact, raw report and log.
This is separate engineering-preview evidence, not a normal save or Microsoft
Access/data-macro qualification.

## Unchanged inputs and exact requests

The registered `DeleteTreeFixture` originals each contain 1,000 Tree rows with
nullable/tied peer keys, real aliases, five node roots, long Memo/OLE values,
unindexed rows and a sentinel. Each request changes `Tree.ID` at original live
ordinal **27**, value **1000**, to **2147483647**. The physical row references stay
`0000691b` for V2016 and `00007a1b` for V2019; ordinals remain snapshot-local.
No rows were added and no header, property, index or map flags were altered.

The original affected leaf has 496 entries, prefix 3 and capacity 3,616 bytes.
The replacement reduces its best prefix to 1 and needs 3,969 bytes. Its parent
has room for another separator. The tested
[split service](ACE16_INDEX_SPLITS.md) allocates one LEFT leaf at EOF and retains
the original leaf as RIGHT; every resulting leaf and parent must fit.

| Profile | Root / old RIGHT / previous leaf | New LEFT | Output bytes | Output SHA-256 |
| --- | --- | ---: | ---: | --- |
| V2016 | 91 / 143 / 144 | 166 | 684,032 | `edb08a0883984d4c01a9144e39856a3fae755b0ff0ffeb0103fe417c0dd11ab0` |
| V2019 | 108 / 160 / 161 | 184 | 757,760 | `797016160beae16e29f4349c165870d551853c9033586c2a79fd7421a39938f1` |

The artifact was compiled from committed revision
`d8bdf6c3ec015afdb70ffa4668ac10974cf7dbb6`, SHA-256
`d9facf656f8cb51e5e235223b5532ee64624695e0f1a165fee3b8c6a4ec614c3`.
Its [build receipt](results/ace16-nonroot-split-example-build-2026-10-27.json)
pins 153 selected Git objects and the isolated exported source snapshot.
Later working-tree Memo/OLE code is not the tested binary.

## Complete expected-image proof

The independent whole-image plan was frozen before calls. The old 496-entry
leaf becomes two 248-entry leaves. The original previous leaf points to the new
LEFT; LEFT points back to that previous leaf and forward to retained RIGHT;
RIGHT points back to LEFT. The original root remains height one, retains its
right tail and gains the complete LEFT maximum **key and row reference**.

Both actual images equal the independent expected bytes exactly. Only the
intended four-byte ID replacement, required index headers/bitmaps/entries,
neighbor link, one initialized EOF page and two map bytes are changed. Original
root/RIGHT residue remains intact; new LEFT padding is zero. All 1,000 original
row references, peer indexes, other record bodies, counters and opaque payloads
are preserved.

V2016 index-owned byte 372543 changes `00` to `08`; global-free byte 8148 changes
`c0` to `80`. V2019 ownership byte 442175 changes `00` to `10`; global-free byte
8151 changes `ff` to `fe`. No table-data, candidate-hint or LVAL map changes are
authorized by this operation.

Complete typed rows, Memo/OLE lengths/digests, row locations, logical index
metadata/orders and full physical key-plus-row streams match the unchanged
[public references](NONROOT_INDEX_SPLIT_OBSERVATIONS.md). Those public updates
copied the selected row's unchanged long values into nine fresh LVAL pages before
allocating the new index leaf. They grew by ten pages; these candidates grow by
**one** page and preserve original descriptors and long-value pages exactly.
Public redistribution/canonicalization is comparison evidence, not the byte
golden. The earlier public-action verifier failure and its read-only correction
remain separate, immutable evidence; no action was rerun to obtain this result.

## Maintenance and read-only verification

[compare_nonroot_split_updates.py](compare_nonroot_split_updates.py) and
[nonroot_split_preview_originals](nonroot_split_preview_originals/replay.py)
are maintained public helpers. They are distinct from the exact private runner
executed for this result; the public proof is byte-identical to its executed
private source. The recipe requires the exact frozen plan and explicit committed
artifact/hash/revision/receipt plus fresh outputs before `--execute`. It never
issues a new public UPDATE action.

The saved images and cached getters/maps can be rechecked without native or JVM
calls:

```sh
python3 docs/testing/compare_nonroot_split_updates.py \
  --verify-existing .cache/ace16-nonroot-split-acceptance-first-20261027/report.json \
  --output .cache/nonroot-split-new-verification.json
```

This verification passed for both images and all 153 compiled Git source pins.
Expanded key arrays, raw command argv and getter output remain ignored and
hash-pinned. Compact command receipts use explicitly labeled portable path
renderings; exact executed argv remains in the immutable raw report. Recursive
internal-node splitting, map growth, ordinary saves and native Access behavior
remain explicit unfinished qualifications.
