# General Short Text and Binary split sources

Two fresh ACE 16 independent-writer originals now provide measured same-width
index-growth inputs. They contain 2,701 rows each, with explicit General Legacy,
General 2010 and Short Binary fields, true index aliases and retained Memo/OLE
sentinels. This is source qualification, not new engine mutation support or
Microsoft Access qualification. The twelve later public UPDATE/getter calls and
the old-slot native baseline controls have not run.

## Actual source profile

`TextLegacy` and `TextModern` each contain 900 rows with raw, uncompressed
`KeyValue = "aaa"`. Their declared Short Text length is 510 bytes and their
stored sort descriptors are respectively `1033:0:0` and `1033:0:1`.
`BinaryOne` contains 900 rows with one-byte `00` values in a declared 255-byte
Binary field. Every target also has manual LONG `ID = 1..900`, LONG
`Marker = ID * 17`, an ID primary, nonunique Ascending/Twin definitions sharing
one physical index, and a distinct Descending index. All selected fields have
actual flags `2`, extended flags `0`, variable index `0`; selected index flags
are the unmodified public-writer value `0x80`.

Each source has four user tables, 26 captured map roles, 13 logical indexes and
10 physical indexes. The six affected physical roots are leaves. The three ID
primary roots have height one; the one-row sentinel primary remains a leaf.
`LongSentinel` retains raw Memo U+0100 repeated 33 times and OLE `00ff41`.

| Profile | File bytes | Pages | SHA-256 |
| --- | ---: | ---: | --- |
| V2016 | 557,056 | 136 | `362a2f603036f836489f87cd24d58c7c4383329dddd065349b48452bd5f480c8` |
| V2019 | 630,784 | 154 | `bbe12b91fa29b78032eb1485b801350d309e31d58680630686046716a442bd1b` |

The raw variable footer proves six-byte Text and one-byte Binary slots in every
target row. ID and Marker occupy fixed spans starting at record offsets 2 and 6;
KeyValue starts at offset 10. The proof obtains these ranges from actual column
metadata and footers rather than reusing the earlier fixed-Int32 offset 6.

## Actual keys and capacity

The saved getter supplies both persisted key streams and independently encoded
old/requested wires. The requested replacements are `aaa` to `zzz`, or Binary
`00` to `ff`, on ID 1 only. Neither replacement has been written yet.

| Profile/direction | Original key | Requested key | Changed root bytes | Mixed 450-entry half bytes |
| --- | --- | --- | ---: | ---: |
| Legacy ASC | `7f4a4a4a0100` | `7f7878780100` | 8,101 | 4,051 |
| Legacy DESC | `80b5b5b5feff00` | `80878787feff00` | 9,001 | 4,501 |
| Modern ASC | `7f0e020e020e020100` | `7f0ea90ea90ea90100` | 9,902 | 4,952 |
| Modern DESC | `80f1fdf1fdf1fdfeff00` | `80f156f156f156feff00` | 10,802 | 5,402 |
| Binary ASC | `7f000000000000000001` | `7fff0000000000000001` | 11,701 | 5,851 |
| Binary DESC | `80fffffffffffffffffe` | `8000fffffffffffffffe` | 11,701 | 5,851 |

The page payload capacity is 3,616 bytes. Original active payloads are
3,606–3,610 bytes. Actual source framing agrees with public key-only prefix
compression, with a physical ROW reference retained in every entry. For each
measured family/direction one mixed half still exceeds capacity, while the
other half fits. This establishes repeated-overflow demand for these concrete
requests; it does not predict the later public tree partition or establish the
engineering planner's full-key/ROW packing policy.

## Execution history and saved proof

The first javac failed because the helper called `getIndexData()` on the public
`Index` interface. It created no database. Its exact helper, freeze, stderr and
partial bounded-channel class remain preserved. A separate helper revision added
only an `IndexImpl` import and cast at that lookup. Its fresh five-command source
phase passed: one compile, then create and inert inspect for each profile. Thus
the observed history is **one failed compile plus five successful source
commands**, not an all-success original five-command phase.

Each child had a 30-second deadline and 512 MiB heap cap. A bounded channel
enforced the 8 MiB database ceiling; live capture retained at most 16 MiB per
stream with bounded child/pipe cleanup. Both profiles and all frozen inputs
remained unchanged through getters and the separate saved-file postcheck.
Expression evaluation was disabled, foreign-key checks enabled, free-page reuse
disabled, time zone UTC, and linked opens refused. No SQL or active database
content was executed.

The [exact corrected helper](encoded_index_split_sources/EncodedSplitProbe.java)
matches the executed source byte for byte. The maintained
[pure model](encoded_index_split_sources/model.py) adjusts its repository-root
composition and terminal whitespace; all six function ASTs match the executed model. The
[saved verifier](encoded_index_split_sources/verify.py) rechecks 48 unique saved
pins, 5,402 typed rows, 5,400 variable slots and 21,602 logical index entries
including aliases, without starting a compiler, JVM or native process. Its first
read-only attempt exposed JSON integer-key normalization in the verifier;
normalizing the recomputed JSON shape corrected only that comparison. The
original source facts and fixture runs were not repeated or changed.

The [compact receipt](results/encoded-index-split-source-observations-2026-10-27.json)
pins the first failure, corrected phase, postcheck, exact sources and saved
verification. Root independently repeated the zero-process saved verification.
The Jar/source provenance follows the already reviewed Apache-2.0 Jackcess 5.0.4
artifacts; no upstream implementation or weight table was copied.

```sh
python3 -B docs/testing/encoded_index_split_sources/verify.py
```

This does not qualify NULL/empty inputs, unique overflow, compound splits,
French/unknown sorts, macro execution, normal native saves or a new Text/Binary
split API. Old-slot noops and changed requests require their separately pinned
native consumer plan; parser rejection by a numeric-only consumer is not storage
admission evidence.
