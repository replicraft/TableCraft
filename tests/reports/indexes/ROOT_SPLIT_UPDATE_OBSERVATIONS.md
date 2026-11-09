# Original-file Int32 root-leaf split acceptance

Both fixed V2016/V2019 candidates passed on the **first attempt**: two native
calls and four read-only getter/map calls, six commands total, terminal status 0.
There were no candidate retries or disagreements. The
[compact receipt](results/ace16-root-split-originals-2026-10-27.json) pins the raw
report, log, exact executed private sources, requests, outputs and frozen build.

This qualifies the separate engineering preview on these original files.
Microsoft Access execution/save, data macros, general splits and normal backend
writes remain unqualified. The [implementation contract](ACE16_INDEX_SPLITS.md)
continues to expose non-root/internal overflow as an atomic refusal.

## Fixed inputs and compiled service

The unchanged registered `BatchDeleteFixture` originals contain 500 parents,
500 children and a protected long-value sentinel. Each request changes the
original live ordinal 0 in `Children.ID` from 1 to 2147483647. ParentID and all
relationship associations remain unchanged. Every input, public reference,
getter, helper and JAR retains its recorded identity.

The consumer was compiled from committed revision
`0c0dc6d904bd112236a383c74b04192ee6c5f90f`, artifact SHA-256
`8245c35b6b533dbdc8f6dec5fed74aab0c0fdaf0da73e5df529dd3053eae4709`.
The [build receipt](results/ace16-root-split-example-build-2026-10-27.json)
records 143 selected immutable Git objects and the exported compiler-source
snapshot. Subsequent working-tree writer code is not the tested binary.

| Profile | Retained root | Right / left EOF pages | Output bytes | Output SHA-256 |
| --- | ---: | --- | ---: | --- |
| V2016 | 95 | 241 / 242 | 995,328 | `a8647884b25bdf5c84e672e22177f5efc98be4e05d4ead755d433a8b2676adc8` |
| V2019 | 112 | 258 / 259 | 1,064,960 | `6e37605b34950d678ca61aae0b97cd641f5e42f49808c6b2e05f219f5f617100` |

Both retained roots become height-one nodes with two 250-entry leaves. The
separator contains the complete left maximum key **and original row reference**.
All 500 original references survive, leaf siblings are reciprocal, and index
orders and historical counters remain correct.

## Independent byte proof and public comparison

The whole-image expected model was frozen before calls. It permits exactly the
intended four-byte ID replacement, the retained-root framing/entry changes, two
initialized EOF leaves and two existing map-byte edits per file. Root residue
outside the new entry is retained; new leaf padding is zero. Every other original
byte, including all Wide index pages and long payloads, remains identical.

V2016 index ownership byte 388936 changes `00` to `0c`; global-free byte 8158
changes `fe` to `f8`. V2019 ownership byte 458568 changes `00` to `0c`;
global-free byte 8160 changes `fc` to `f0`. Data ownership and candidate hints
remain unchanged. Both complete candidate images exactly equal the independent
expected images, rather than merely decoding to the requested value.

Typed values, row locations, logical index metadata/orders and full physical
key-plus-row streams agree with the two previously captured public updates.
The public writer also redistributed an unchanged Wide index; that rewrite is
not the candidate byte golden. The older
[108-object refusal/public-reference record](INDEX_SPLIT_OBSERVATIONS.md) remains
separate and unchanged.

A read-only preflight initially asserted that the plan's `expected` facts object
was the scalar 1. The scalar request is stored in the frozen argv; checking that
argv resolved the authoring mistake before any candidate call. No selector,
model, program, source database or budget changed. The successful first native
attempt and its complete evidence remain intact.

## Replay and read-only verification

[compare_root_split_updates.py](compare_root_split_updates.py) and
[root_split_preview_originals](root_split_preview_originals/replay.py) are public
maintenance helpers. They are distinct from the pinned private runner actually
executed. The public proof is byte-identical to the executed proof. Replay is
inert without `--execute`, requires the exact frozen plan and explicit artifact,
SHA-256, revision, build receipt and fresh output directory, and preserves failed
outputs. It performs no new public UPDATE actions.

The saved images, cached getters/maps and immutable Git objects can be rechecked
without native or JVM calls:

```sh
python3 docs/testing/compare_root_split_updates.py \
  --verify-existing .cache/ace16-index-split-acceptance-first-20261027/run/report.json \
  --output .cache/root-split-new-verification.json
```

This read-only verifier passed against both accepted images. Expanded getter
outputs and key arrays remain ignored and hash-pinned. The tracked receipt stores
compact outcomes, command receipts and essential framing/map facts.
