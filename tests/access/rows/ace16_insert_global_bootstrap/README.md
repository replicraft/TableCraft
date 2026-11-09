# Original-file GLOBAL bootstrap proof

Twelve engineering INSERT candidates on four immutable ACE 16 originals match
complete SOURCE/request-derived images. Twelve fresh combined typed-row/map
inspections then pass. A separate public reference phase passes 24 child calls and
twelve transition proofs; all twelve public outputs also match the previously
frozen images. Every actual runtime phase passes first attempt. These independent
comparisons do not qualify Microsoft Access execution or native saves.

The repository-authored helper creates V2016/V2019 originals with raw/compressed
Memo, Items (required Int32 ID, nullable Int32 Group, OLE Blob and Memo), Sentinel
and independent Padding. Fixed public counts 400/382 place all four source EOFs at
512 pages. There is no page patching, count search, source regeneration or tuning.
Actual public deletion leaves one observed orphan per long column; all physical
live/orphan bodies remain protected. The original global row is canonical page 1,
slot 0, 69 bytes/base zero in its owner 1 container. The adjacent opaque 69-byte row 1 is
preserved, not treated as another GlobalFree authority.

| Request on each original | New body/metadata order | Final EOF |
| --- | --- | ---: |
| Single OLE 65 / Memo 66 encoded bytes | LVAL 512, GLOBAL 513, LVAL 514 | 515 |
| Chain OLE 8,145 / Memo 8,146 | Blob 512→514→515, GLOBAL 513; Memo 516→517→518 | 519 |
| 32 rows, each inline OLE 64 + Memo 64 | DATA 512, GLOBAL 513 | 514 |

The inline batch places 21 new records in the original highest DATA page and 11
at 512:0..10. Its ordinary-owned indirect bitmap gains bit 512 and its ordinary
hint rebases to 512. Long-column maps/bodies remain unchanged. External cases
allocate no fresh DATA; column ownership rebases independently and GLOBAL 513 is
excluded from all body/hint roles. Both physical indexes retain fitted leaf roots;
raw key/ROW pairs, aliases and per-physical distinct-family history are checked.
There is no new INDEX page or INDEX-growth qualification in this corpus.

The source-pinned [policy](MODEL_POLICY.md) predates expected images and all AFTERs.
GLOBAL initialization fills bytes [4,4092) with FF and leaves final four bytes zero,
then clears the used frontier/self/new pages. Physical geometry remains 32,736
bits. Promotion deliberately retires the actual old FREE holes as USED: page 83
in V2016; pages 65, 96, 98, 99, 100, 101, 102, 104, 105 in V2019. Every complete old hole
page stays byte-identical. Existing claimed/owned/live pages are proven allocated.
This is the pinned reference writer's frontier policy, not a universal Microsoft
format/reserved-byte claim. Earlier overbroad draft free-membership preservation
wording remains retained with the explicit pre-model correction.

## Phases and saved verification

SOURCE9 is one javac, four creates and four inert inspections. Capture leaves
qualification false; four saved raw metadata/record/map/census gates then pass.
Twelve independent complete images are frozen before the reference phase and
producer build. REF24 runs twelve fresh public appends plus twelve inspections.
The isolated producer build binds commit 7c282f43de9cbaad218f9b1f01b6189364d0d4a0,
a fresh absent target and all 4,587 Git-bound exported files, with actual engine
compilation recorded. Native12 and fresh12 are separately authorized phases;
all twelve complete candidate images pass before reader 1 and are revalidated
before and after every reader. No active document code executes.

```sh
python3 -B docs/testing/ace16_insert_global_bootstrap/verify.py
python3 -B docs/testing/ace16_insert_global_bootstrap/verify.py --verify-git
```

The default starts no subprocess and imports no cached Python implementation. It
checks exact maintained transformations/pins, all four original gates and twelve
reconstructions before interpreting any AFTER, then replays all public/fresh
proofs and the isolated archive/receipt. Optional Git mode runs one metadata-only
batch for 4,587 committed source blobs and the committed build receipt. Neither
mode runs the producer, Java or Cargo. Missing private evidence reports
available:false/status 2; contradictions return status 1, with no success fallback.

Limits remain 8 MiB files, 16 MiB streams, 512 MiB heap and 30 seconds per runtime
call. Only the explicit example requests use `--value-bytes 16384`; its default
stays 1024. This corpus does not qualify block-one global initialization, arbitrary
free-page reuse, missing-pointer large-file behavior, normal editing or native
Access save/reopen. See the [result](../results/ace16-insert-global-bootstrap-2026-10-28.json)
and [owning contract](../ACE16_GLOBAL_MAP_INSERT_GROWTH.md).
