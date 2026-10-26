# Original delete storage observations

`DeleteStorageFixture.java` creates inert original tables with public Jackcess
builders, disables expression evaluation, then performs public `Table.deleteRow`
operations on separate copies. The same helper reopens each file read-only and
records rows, live counts, index membership and pinned implementation metadata
getters. Python independently reads the original slot words, table counts, index
counter bytes and leaf-page entry masks/references. No reference SQL, imported
active content or native Microsoft Access execution is used.

The bounded corpus covers Jet 4 and ACE 2007/2016/2019, with four states per
profile: original, delete ID2, delete nullable-key ID3, and delete all rows.
An indexed six-row table has primary, nonunique ascending/descending, unique,
compound and ignore-NULL indexes. `GroupAsc` and `GroupTwin` actually share one
physical declaration. A three-row unindexed table and an untouched sentinel table
provide separate ordinary/delete-preservation controls. Each original indexed row
also has Unicode Memo data and an 8 KiB OLE payload. The directory uses `.accdb`
filenames for inventory; the actual header/profile, including Jet 4, controls reads.

Observed facts across all four profiles:

- Deletion changes each selected slot word by OR `0xc000`, retaining its original
  payload offset. Both high flags are set. Ordinary row payload bytes, including
  long-value descriptors, remain identical even for deleted rows.
- Stored table live counts and reopened getter populations decrease correctly.
  Deleting all rows gives zero live rows and zero current index entries.
- The getter named `getUniqueEntryCount()` and its corresponding stored bytes
  remain unchanged in these states. For example the primary index stays 6 while
  its current membership becomes 5 or 0; nonunique GroupAsc stays 4. These observations
  do not establish universal semantics for that counter or authorize decrementing
  it as if it were current live cardinality.
- All six distinct physical indexes own one leaf page before and after deletion.
  After deleting all rows, each leaf has zero entry boundaries and zero prefix
  width. The logical alias retains its shared physical identity and counter offset.
- Original/copy file lengths match. Reference delete ID2, delete ID3 and delete-all
  change 55, 119 and 168 bytes respectively in each profile. No changed byte falls
  outside the selected slot words, table live-count fields and owned index pages.
  Unrelated pages and Memo/OLE storage remain byte-identical. These are reference
  change-locality observations, not a new TableCraft mutation proof or write API.

The reviewed Jackcess 5.0.4 JAR hash is
`19a7494a56f439a57fda85c059b019b665802085cec5ae8186d1e028cc1bdfbc`.
The producer/helper and Python runner are original repository code under the
repository license; no third-party source or fixture bytes were copied. The JAR's
existing provenance/notices remain in [the fixture ledger](FIXTURES.md).
Generated databases and raw captures stay under ignored `.cache`; the report
records their hashes and exact commands, without machine-specific paths.

## Replay boundary

The initial frozen-reader run retained 192 controls and 28 disagreements. Sixteen
were a runner diagnostic-wording error for the existing unsupported selected
ignore-NULL index profile. Twelve full checks exhausted the CLI's default work
allowance on Memo/OLE data, including the untouched originals; they were not
counter corruption or delete decoding failures. That run, its script and its
raw outputs remain preserved. The missing inspect work override was a separate
CLI host-control gap; native reader limits and defaults were not relaxed.

This corpus does not implement TableCraft deletion, node-tree contraction,
allocation, INSERT, macro absence/execution or native Access save/reopen. Ordinary
ACE write gates remain unchanged, and unknown required behavior cannot be bypassed
by copying the observed tombstone bits.

## Accepted frozen-reader replay

The accepted replay uses the same 16 immutable inputs with the committed inspect
work override and explicitly authorizes 10,000,000 steps for full checks only.
All 192 controls agree: 176 successful inspections/queries and 16 expected selected
ignore-NULL index refusals. All 16 full-check commands succeed; each reports the
known unsupported ignore-NULL check separately rather than pretending complete
index coverage. Supported required-index member order matches the reopened
reference cursors, including nonunique ties, compound keys, aliases and empty
roots. Scalar queries/counts match independently observed live rows. Inputs,
helper, runner, JAR and frozen CLI hashes remain unchanged during reads.

The [accepted report](results/delete-storage-final-2026-10-25.json) records 16
profile/states, 37 immutable pins, raw slot/index observations and every control.
The frozen CLI hash is
`1093091eb45bd7e78fe1f0536da4994439e7012682b1f19f358ca13330cb52a0`,
built from committed revision `32b5d157fea3512cd9f624bdff5a3f3a4b3d2e2b`.
The initial 192/default-budget run remains excluded and hash-pinned; it was not
rewritten into passing default controls. No engine read or write gate changed.

Create fresh original inputs with these commands, using the reviewed JAR:

```sh
for profile in V2000 V2007 V2016 V2019; do
  java -Dstdout.encoding=UTF-8 -cp .cache/tablecraft-fixtures/jackcess-5.0.4.jar \
    docs/testing/DeleteStorageFixture.java generate FRESH_DIRECTORY "$profile"
done
```

The directory must exist and contain none of the selected output names. Producer
files are not claimed byte-reproducible: record the actual generated hashes.
Run the independent getter/byte/native comparison with fresh work/report paths:

```sh
python3 docs/testing/compare_delete_storage.py \
  --jar .cache/tablecraft-fixtures/jackcess-5.0.4.jar \
  --cli .cache/tablecraft-bin/inspect-native-work-cli-20261025 \
  --cli-sha256 1093091eb45bd7e78fe1f0536da4994439e7012682b1f19f358ca13330cb52a0 \
  --compiled-revision 32b5d157fea3512cd9f624bdff5a3f3a4b3d2e2b \
  --inputs ORIGINAL_DIRECTORY --workdir FRESH_REPLAY_DIRECTORY \
  --report FRESH_REPORT.json
```

Omit `--inputs` to generate the four profile sets in the fresh work directory.
The accepted evidence reused its registered original inputs, preserving the
initial producer/diagnostic captures rather than generating replacement data.
