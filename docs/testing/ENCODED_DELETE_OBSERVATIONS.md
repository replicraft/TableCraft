# Original encoded Text/Binary DELETE observations

The [reference receipt](results/ace16-delete-encoded-original-reference-2026-10-26.json)
and [current candidate receipt](results/ace16-delete-encoded-internal-chains-originals-2026-10-26.json)
record **48 controls with zero disagreements: 36 candidates and twelve profile
refusals** against frozen revision `cd52af09c3f9253ed9aa7e3195a2210bf316efdc`.
Both Modern Text height-2 delete-all cases succeed. The run checks 4,374 removed
rows, 8,676 surviving target rows and 158 retired pages; each Modern delete-all
retires 35 pages across its distinct physical indexes. This is independent
Jackcess evidence, not native Access execution or save/reopen qualification.
Normal writes, UPDATE codecs, journal/apply and data macros remain outside this
experiment.

The [earlier candidate receipt](results/ace16-delete-encoded-original-replay-2026-10-26.json)
records the same 48 selectors at `d5690381ef6eb394a998b43255f025b163329466`:
34 candidates, twelve unsupported-profile refusals and two linked-internal
contraction refusals. Those two capability limits are resolved in the current
run. Both records retain their original scope; no input was regenerated and
repeated selectors are not counted as new distinct controls.

## Original inputs and independent reference

[EncodedDeleteFixture.java](EncodedDeleteFixture.java) is original repository
code under the root license. It creates fresh V2016/V2019 files with public
Jackcess builders and deletes from fresh copies through public `Table.deleteRow`.
No upstream implementation or fixture was copied. The reviewed 5.0.4 JAR has
SHA-256 `19a7494a56f439a57fda85c059b019b665802085cec5ae8186d1e028cc1bdfbc`;
its provenance and all eight file hashes are in [the fixture ledger](FIXTURES.md).
Expressions are disabled, time zone is UTC, and linked database access throws.
Getter reopens are read-only. No stored query or active content executes.

Each original contains 2,184 rows in thirteen tables: six positive tables with
2,175 rows, six negative tables with eight rows, and one preserved Memo/OLE row.
The positive tables have Primary(ID), ascending KeyValue, a real logical Twin
alias of that same physical index, and descending KeyValue. Text contains NULL,
empty, case/accent/expansion differences, trailing spaces, supplementary Unicode,
NUL and a nonunique 255-character overflow key. Binary includes NULL/empty and
1/7/8/9/16/17/255-byte values with high bytes and duplicates. The actual Text
sort descriptors are `0x00000409` General and `0x01000409` General; Binary has no
Text sort descriptor.

Each profile has independent public-writer copies deleting ID 4, deleting NULL
ID 1, and deleting all positive rows. Their total populations are 2,178, 2,178
and nine respectively. Public getters establish exact UTF-16/Binary values,
physical row identities and index encounter order. Read-only implementation
getters expose encoded keys, roots and historical counters against the pinned
JAR; they are not a stable public native-format API. Independent raw framing
checks reconstruct full prefix-compressed keys and match key/reference sequences
to those getters. The generator-value model checks 13,026 surviving value cells
across the eight files.

## Observed topology, including the unsupported case

| Positive family | Rows | Key-index height | Asc/Desc owned pages | Linked nonroot nodes |
| --- | ---: | ---: | --- | ---: |
| TextLegacyLeaf, TextModernLeaf, BinaryLeaf | 24 each | 0 | 1 / 1 each | 0 |
| TextLegacyNode | 701 | 1 | 11 / 9 | 0 |
| BinaryNode | 701 | 1 | 11 / 13 | 0 |
| TextModernNode | 701 | 2 | 18 / 17 | 2 per key tree |

Both profiles have these actual shapes. Names and populations were not used as
proof of tree height. The Modern Text height-2 cases were retained, including
contrary evidence: one-row tie/NULL deletions succeed, while delete-all refuses
with `Unsupported(IndexedColumn)` in this frozen revision because contraction
encounters linked internal nodes. No input flags or original bytes were edited
to make those cases pass. Later implementations must reproduce these same
positive targets rather than silently omit them.

The twelve profile-negative controls cover unique Text overflow, Memo indexes,
Text/Binary composite indexes and Text/Binary ignore-NULL indexes in both
profiles. They produce no candidate and no stdout. Exact diagnostics are in the
candidate receipt; composite refusal is `Inspect(Unsupported)`, distinct from
the other gate classes.

## Candidate and preservation checks

Successful controls delete 2,972 rows in total and check 8,676 surviving target
rows and 88 retired pages. Every candidate is independently reopened. Target
rows and index order match the public reference deletion; every retained encoded
key and row reference matches the original entry, including nonunique overflow
bytes. The checker does not generate new Text weights or substitute semantic
Binary re-encoding. Physical aliases, roots and historical counters are retained.
Historical counters are not live cardinalities and are not decremented.

The checker also compares all unrelated tables and Memo/OLE getter hashes to the
original. Whole-file byte checks permit only selected slot tombstones, the exact
table count, qualified index page regions and aggregated ownership/global bits.
Retired page headers become zero in their first four bytes; all residual bytes
remain unchanged. Data ownership maps, file length and every other byte remain
unchanged. All eight original/reference file hashes are rechecked throughout.
The receipts pin each of the 34 candidate files, getter stdout/stderr and all
48 request/command associations; expanded raw output and databases stay ignored.

## Historical capture and public reproduction

Accepted generation and the first corrected replay used private sources under
`.cache/encoded-delete-originals-next-20261026/`. The Java promotion is byte-identical.
The exact private [capture](encoded_delete_originals/historical/capture.py.txt),
[qualification](encoded_delete_originals/historical/qualify.py.txt),
[replay](encoded_delete_originals/historical/replay.py.txt) and
[wire](encoded_delete_originals/historical/wire.py.txt) sources are archived as
inert text; the receipts preserve their actual paths and hashes. An initial 48/0 run had incorrect command-list pointers
in its report. It remains excluded; a fresh 48/0 run fixed only those pointers
and added source pins. These are repeated observations of the same 48 controls,
not additional controls.

The maintained [capture](encoded_delete_originals/capture.py),
[qualification](encoded_delete_originals/qualify.py),
[replay](encoded_delete_originals/replay.py) and
[raw framing](encoded_delete_originals/wire.py) helpers use explicit paths,
import-safe entry points and independent named layout constants. Their separate
read-only verification uses the same unchanged originals and frozen executable;
the receipt distinguishes it from the historical private-source run. A later
`os.pathsep` portability correction leaves every recorded macOS Java argument
unchanged; exact executed and current helper source pins remain separate. Public
receipts normalize artifact labels to repository-relative paths while retaining
actual command arguments and hash-pinned ignored raw reports unchanged. Output
directories and reports must be fresh. Do not use Python optimization (`-O`),
which disables the independent assertions.

To reproduce with retained originals, set `corpus` to their input directory,
`getter` to a directory containing the compiled getter class, and `jar` to the
reviewed JAR. Compile the public Java source into a **fresh** getter directory
with `javac -cp "$jar" -d "$getter" docs/testing/EncodedDeleteFixture.java`.
All input, helper, example and output artifacts must be inside this checkout;
`common.pin` resolves symlinks and rejects paths outside it. Then run from the
repository root, choosing fresh repository-local `.cache` capture/report paths:

```sh
python3 docs/testing/encoded_delete_originals/capture.py \
  --corpus "$corpus" --helper-dir "$getter" \
  --helper-source docs/testing/EncodedDeleteFixture.java --jar "$jar" \
  --output-dir "$fresh_capture" --report "$fresh_observations"
python3 docs/testing/encoded_delete_originals/qualify.py \
  --capture "$fresh_observations" --report "$fresh_qualification"
python3 docs/testing/encoded_delete_originals/replay.py \
  --corpus "$corpus" --capture-dir "$fresh_capture" \
  --observations "$fresh_observations" --qualification "$fresh_qualification" \
  --helper-dir "$getter" --helper-source docs/testing/EncodedDeleteFixture.java \
  --jar "$jar" --example "$frozen_example" \
  --example-sha256 b5f30b31199d63a09a1aabcbeca8d841f89888f946a0390dd279cca1566afd66 \
  --compiled-revision d5690381ef6eb394a998b43255f025b163329466 \
  --workdir "$fresh_candidates" --report "$fresh_replay" \
  --allow-internal-chain-refusal
```

That explicit flag reproduces the historical 34-success/14-refusal boundary.
Without it, all 36 positive selections must succeed and only the twelve profile
negatives may refuse. A newer binary requires its own reviewed hash, revision
and build receipt; passing a revision argument alone does not prove provenance.
For new inputs, invoke the public Java `generate OUTPUT_DIRECTORY V2016` and
`generate OUTPUT_DIRECTORY V2019` commands in a new directory before capture.
Never regenerate or overwrite accepted originals. Fresh creation bytes may
differ; compare observed values/topology and retain new hashes separately.
