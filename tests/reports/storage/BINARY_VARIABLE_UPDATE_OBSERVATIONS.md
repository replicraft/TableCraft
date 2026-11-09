# ShortBinary variable-row candidates on original files

The first frozen Binary8 replay passes **eight candidates with zero
disagreements**, using 32 commands: eight native calls, twelve read-only getters
and twelve read-only map probes. Each shrink consumes its paired native growth
candidate, never a public-writer substitute. The [compact receipt](results/ace16-binary-variable-originals-2026-10-26.json)
pins the immutable plan, private executed sources, requests, images, raw logs and
current public verification. There was no native retry or changed expectation.

The separate `--variable-updates` consumer was compiled at `87c35d43`, frozen at
17,512,496 bytes with SHA-256
`2d2b173617e115a3bca3b10018086dab1c5d8f3d72b536fe3825d21e19cb4a5f`.
Its [build receipt](results/ace16-variable-example-build-2026-10-26.json) identifies
the historical source objects. Later working code is not qualified by this run.
Existing Slot and PAGE consumers/evidence remain unchanged.

## Actual sources and outcomes

The [Binary34 public observations](BINARY_ALLOCATION_OBSERVATIONS.md) supply
separate after-12 and after-13 INSERT checkpoints. Those INSERTs only construct
the fixtures; they are not native INSERT candidates. The selected field is the
unindexed ShortBinary `BinaryStorage.Bytes`, ordinary flag 2/ext 0, with its
required ID primary retained. ID1 changes from NULL to 255 exact bytes, then to
seven exact bytes on the actual native growth output.

| Profile / checkpoint | Stable root | Growth target | File growth |
| --- | --- | --- | ---: |
| V2016 / after 12 | 200:0 | new EOF hidden 255:0 | 4096 bytes |
| V2016 / after 13 | 200:0 | existing owned hidden 255:1 | 0 |
| V2019 / after 12 | 217:0 | new EOF hidden 272:0 | 4096 bytes |
| V2019 / after 13 | 217:0 | existing owned hidden 272:1 | 0 |

All eight targets retain 268-byte physical capacity. The four shrinks fit that
capacity without changing root/target identity, file length or map cells. Logical
row counts and historical index counters remain 22 or 23, according to their
actual source. All typed rows and logical index declarations/orders match the
public references; every preexisting raw index-signature page stays byte-identical.

All four growth images are byte-identical to the public growth references. The
four shrink images intentionally preserve prior opaque Binary bytes beyond the
new seven-byte span, while the public writer zeroes that padding. Their full
logical/index agreement is not presented as eight-image byte identity.

## Independent proof and preserved execution

The fixed plan derives eight selectors from actual public getters and map cells.
It was frozen before calls. The independent proof reconstructs the complete
268-byte record, literal new data page where applicable, root pointer/stub,
hidden-slot directory words, count/free words and exact global/owned/hint byte
changes. It checks every other file byte, all old index pages and unchanged
counters. The four public growth references were independently reconstructed
byte-exactly before native calls.

The raw report retains all command receipts, cached complete getters/maps and
all eight outputs. Private plan `95ce3a6b`, proof and runner remain archived
unchanged. The [public plan](binary_preview_originals/plan.py) reproduces that
same plan byte for byte. The [public proof](binary_preview_originals/proof.py) is
byte-identical to the executed private proof. The [public runner](binary_preview_originals/replay.py)
is a new wrapper with a separate read-only mode, not the historical executable
script. Its initial import-spelling and JSON page-key adapter errors are preserved;
correcting those wrapper errors changed no native image, selector or golden.

## Read-only verification

The [entry point](compare_binary_variable_updates.py) verifies all eight accepted
images, actual candidate-source dependencies, cached getter/map receipts, decoded
map cells, header counters, historical source objects and whole-image proofs
without launching a native process or JVM:

```sh
python3 docs/testing/compare_binary_variable_updates.py \
  --plan .cache/ace16-binary-variable-originals-next-20261026/plan.json \
  --plan-sha256 95ce3a6b064be0cc815fba5d59e8ef2cb6e36ef902dc62e82faab1596f602fb2 \
  --verify-existing .cache/ace16-binary-variable-candidates-initial-20261026/report.json \
  --verify-existing-sha256 4810a112a6bde3ec25e15e3005146cf96b61b58517b45000df29f37ea9930fd6 \
  --workdir .cache/binary-existing-fresh-verification
```

A live replay requires explicit `--execute`, frozen artifact/hash/revision/build
receipt, reviewed JAR/classes and a fresh ignored output directory. No implicit
build, mutable binary selection, source regeneration or automatic retry occurs.
The new public read-only verifier adds zero candidate/JVM calls to the historical
counts. This bounded engineering evidence does not establish Microsoft Access
execution/native saves, data macros, arbitrary free-page reuse, new map allocation
or general variable-row compatibility.
