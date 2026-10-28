# Saved INSERT candidate proof

`verify.py` replays the four saved ACE 16 engineering INSERT cases: the V2016 and
V2019 originals, and their fixed twelve-INSERT predecessors. Each request adds
one explicit Int32 ID and a 255-byte Binary value. Two fit the original hinted
DATA page; two append one visible DATA page. This is saved-evidence verification,
not a fixture generator or database writer.

```sh
python3 docs/testing/ace16_insert_candidates/verify.py
python3 docs/testing/ace16_insert_candidates/verify.py --verify-git
```

The verifier uses Python 3.11+ and the existing shared
[`ArtifactStore`](../../../scripts/compat_artifacts.py). Required databases,
expected images, getter streams, classes, tools and raw reports remain private
under `.cache/`; they are not included in this folder. Missing evidence reports
`available: false` and exits 2. A contradiction exits 1; a complete verification
exits 0. A checkout or CI build without the private closure is not a passing
compatibility result.

`model.py` is the exact repository-authored independent model frozen before the
INSERT consumer existed (SHA-256
`e25461040e1315054d2ce30771481262e0f59fa9b703b9cf5a8c294a942680be`). It derives
complete images from original bytes, original getter metadata and explicit
request values. It imports no Rust producer or serializer and consults no
candidate to construct its expected bytes. No third-party implementation,
database, class file or raw getter facts are copied into this maintained folder.

The verifier checks the original 175-input freeze, four independent expectations,
producer and getter protocol closures, before/after pin results, recorded argv,
limits, statuses and raw streams, then rehashes all checked artifacts again at
the end. It derives each complete image before
interpreting the fresh getter results, then checks the candidate bytes, visible
ROW, new record, retained old rows, primary entries, original-seeded framing,
table/historical counters and owned/hint/global map effects. The actual saved
controller reports also record complete-image equality before their getter calls.
All saved getter and map streams must match the independently selected public
reference observations. The parser is the existing maintained
[`slot_preview_originals.facts`](../slot_preview_originals/facts.py).

The [consumer build receipt](../results/ace16-insert-example-build-2026-10-27.json)
binds compiled revision `4f9ee2da2f38651ea145209e8d568f26bc15a58d` and its frozen
artifact. Receipt revision is `c399fce61fce657a33f8cb86acc6f6a962a9f54c`.
`--verify-git` additionally checks the receipt and all 745 compiled source blobs
in one local `git cat-file --batch` process. Without that option the output
explicitly reports historical Git verification as unrun. Neither mode invokes
native preview, Java, Cargo, generators or public UPDATE/INSERT actions.

The original receipt names a disposable `target/...d` dependency-info location.
The verifier checks the separately retained byte-identical archive against that
receipt and its compiled source closure. It does not require or claim that the
historical target file is still present or unchanged. This archive is an
additional preservation check, not an alteration of the committed receipt,
producer protocol or first-run report.

Successful replay qualifies these four saved engineering candidates and their
read-only observations. It does not establish Microsoft Access application
execution, native save/reopen, transactions, data-macro semantics or broader
INSERT-profile compatibility.
