# Generated preserved joint fixed-field copies

This repository-authored corpus qualifies joint engineering updates on the exact
unchanged V2016/V2019 originals from the
[eight-byte scalar corpus](../preserved_fixed_scalars_genuine/README.md).
It reuses that creation recipe, pinned Jackcess 5.0.4 runtime, compiled classes
and read-only getter. Generated databases, classes, JARs and raw execution logs
remain private; the [result record](../results/ace16-preserved-joint-fixed-genuine-images-2026-10-31.json)
pins their identities. These are independent-reader observations, not Microsoft
Access save/reopen qualification.

## Cases and independent authority

Six requests run against each original:

| Request | Transport | Changed roots per original |
| --- | --- | ---: |
| Same-row integer/Currency endpoints, DATE negative zero, Time and Boolean | Typed batch | 1 |
| Promote four numeric NULLs and clear Boolean | Typed batch | 1 |
| Clear four numeric values and set Boolean | Typed batch | 1 |
| Overlapping field groups across three original rows | Typed batch | 3 |
| Exact original values and NULL no-ops | Typed batch | 0 |
| False SQL selection with five genuinely empty groups | SQL UPDATE | 0 |

The pure [model](model.py) reconstructs expected images from original raw framing
and [frozen requests](recipes.py), without candidate bytes or writer calls.
Fixed payloads use explicit little-endian encodings. NULL changes preserve
inactive payloads; independently accumulated selected mask bits preserve every
other field. Boolean's declared width is one but its physical payload width is
zero: it changes a mask bit only. DATE expectations retain raw bits, including
negative zero, even when the independent reader exposes an equivalent date/time.
Changed rows are the union of original logical roots, not the sum of changed
cells. All unrelated bytes and the original EOF must remain exact.

Complete native Source admission of the twelve expectations preceded candidate
calls. That expectation tranche was committed as `2f436c5a`. A fresh isolated
build of producer `74b6b112` then matched all twelve expected files, followed by
twelve fresh unchanged read-only SDK getter processes. Full getter observations
include preserved stale calculated caches, Memo/OLE, attachments and Sentinel.
Eight images change and four are exact no-ops; the total changed-root union count
across all images is twelve.

## Replay retained evidence

With the pinned private artifacts restored in their recorded repository-relative
locations, run:

```sh
python3 docs/testing/preserved_joint_fixed_genuine/verify.py --root .
```

The saved-only verifier launches no build, JVM, database writer or reader. It
checks retained source/tool identities, complete archived producer source and
fresh compiler events, reconstructs every expectation independently, and checks
all candidate bytes, exact command reports and full reader inventories. Its
first replay passed for twelve images and twelve SDK reads.

This corpus grants no ordinary ACE save, journal eligibility, required-data-macro
absence or native Microsoft Access compatibility claim. Int32 combinations have
separate authored-source coverage; these generated cases use the originals'
actual Int64 field and do not relabel it.
