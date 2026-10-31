# Saved genuine preserved-Int32 evidence

This package owns the pure source-framing and original/request expectation model
used by the genuine preserved-Int32 qualification. Its closed corpus profile is
`PreservedRows`: ordinary Score, independent stored calculation, Boolean, Memo,
OLE and attachments, plus an untouched Sentinel table. It is not a general table
serializer or an engine permission API.

```sh
python3 -B docs/testing/preserved_int32_genuine/verify.py
```

The command reads saved artifacts only. It never starts a process, builds a
producer, invokes an SDK, updates a database, imports cached Python code or writes
an expected/candidate file. The ignored source/expected/native/SDK artifacts must
already exist. `--root` selects their canonical repository-shaped artifact root;
`--record` selects the explicit result JSON. Missing or changed inputs fail.

`framing.py` independently checks actual raw TD fields, logical-to-physical index
identity, map records, row order, masks and variable footer boundaries against
original SDK observations. Rich sibling payloads remain opaque. `model.py`
reconstructs only the selected four-byte Score span from original requests and
compares every other byte exactly; no-ops are whole-image identical.

The verifier uses those maintained modules, never the frozen private program
copies. It verifies their historical pins as data, rederives all ten expectations
in memory, compares complete saved native files, checks exact matched/changed CLI
reports and compares whole fresh-SDK observations. Only Score may differ; stored
stale caches, attachments, LVAL values, indexes, metadata and Sentinel remain
unchanged. Recorded build receipts and archive/inventory/binary pins preserve the
isolated committed-producer provenance without rerunning that build.

The result keeps the initial SDK-source compilation failure and the first pure
framing failure separate from their corrections. The physical descriptor header
1923 is a marker; logical data-number fields select positional physical owners.
No source, literal request or expected payload policy was retuned from a producer.
Public-SDK comparisons do not qualify Microsoft Access execution, native event
coverage, Windows behavior or normal ACE saves/journaling.
