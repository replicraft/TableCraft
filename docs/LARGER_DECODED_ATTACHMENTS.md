# Explicit larger decoded attachment files

`AttachmentDecodeLimits` supplies a validated per-file allowance from one byte to
16 MiB. Existing `decode_attachment`, `AttachmentDecoder::new` and
`NativeSource::with_decoded_attachments` retain their 1 MiB default. New consumers
use `decode_attachment_with_limits`, `AttachmentDecoder::new_with_limits`, or
`NativeSource::with_decoded_attachment_limits`. Limits are consumer policy, not
native format maxima or permission to execute/open file content.

The CLI exposes `query run --attachment-bytes BYTES` with the existing explicit
`--database`, `--collections` and `--decoded-attachments` options. It does not
raise native/query allocations, work, rows, Text allowances or output limits.
For the registered original fixture below:

```sh
tablecraft query run \
  'SELECT ID,Files.FileName,Files.FileData FROM LargeFiles ORDER BY ID' \
  --database .cache/tablecraft-fixtures/larger-decoded-attachments-20261024/files-2016.accdb \
  --collections --decoded-attachments --attachment-bytes 3145728 \
  --native-bytes 134217728 --native-steps 100000000 \
  --execution-bytes 134217728 --execution-steps 100000000 \
  --output-bytes 16777216 --format tsv
```

## Separate storage, decoder and output bounds

Encoded FileData still has the qualified 2 MiB stored-value ceiling, with existing
LVAL framing/chain limits. Type headers remain bounded to 4 KiB. The decoder
rejects declared bodies above the selected file allowance plus that header cap,
then validates the actual file length after complete header/codec validation.
Zlib checksums, exact consumed/produced lengths, output sentinels, encoding/type tags,
UTF-16 headers, trailing-input rejection and bounded cancellation checks remain
mandatory. Unknown encodings do not fall back to another codec.

The explicit allowance can admit compressible files above 2 MiB when their
encoded storage fits. It does not admit incompressible/stored wrappers above
the encoded ceiling. A 16 MiB file allowance also does not imply its hexadecimal
CLI display fits the separate 16 MiB output ceiling. Full native FileData SQL
representation remains unqualified: this is the existing explicit TableCraft
decoded-file view. Broader encoded stored values remain separate work.

Sessions retain one lazy reusable codec workspace. A large ceiling allocates
neither that workspace nor a file-sized buffer at construction; each file uses
its actual declared size under the original cumulative budget. Metadata/scalar
projections do not decode unrequested payloads, even with a tiny file allowance.
Complete decoded columns publish only after every child succeeds. No caller pool
is raised, replaced or refunded to admit a file.

## Cache and configuration behavior

Changing a NativeSource file allowance discards its decoded caches. Complete
stored snapshots and metadata remain reusable; a lower allowance cannot reuse a
previously admitted oversized decoded value. Raising it again requires fresh
decoding under the remaining original budget. First enabling decoded views also
rebuilds previously cached metadata-only member inventories to expose FileData.
Callers should prepare queries after selecting the desired source configuration.
This fixes a stale member-inventory path without introducing stored values,
implicit payload reads, writers, providers or active database content.

## Evidence

Six new decoder controls cover stored and compressed files, exact 1.5 MB/3 MiB/
16 MiB boundaries, default/lower refusals, invalid policies, unchanged encoded/
header caps, checksums/lengths/trailing bytes, sampled large-stream cancellation,
cumulative allocation refusal, independent later recovery and lazy workspace
reuse under a small original pool. Three adapter controls cover failed-then-raised
decoding, lower/re-raised policy changes, stored-cache reuse without I/O, no
budget refund and first enabling FileData after metadata inventory. CLI controls
cover option requirements and ranges.

The original [Java recipe](testing/LargeDecodedAttachmentFixture.java) creates
inert ACE 2007/2016/2019 files with a 3 MiB compressed attachment, a 1,500,000-byte
stored attachment and an empty parent. The [runner](testing/compare_larger_decoded_attachments.py)
passes 33 controls against independent Python bytes and read-only Jackcess
lengths/digests/codec tags. Complete CLI bytes, metadata/scalar pruning, counts,
default/lower caps and independent native/query/output refusals agree. Original
hashes remain unchanged; [fixture records](FIXTURES.md#larger-decoded-attachment-inputs)
retain the identities. Native Access, large encoded wrappers, release performance
and broader attachment SQL semantics remain unqualified.

```sh
cargo build -p tablecraft-cli --locked --offline
python3 docs/testing/compare_larger_decoded_attachments.py \
  --jar .cache/tablecraft-fixtures/jackcess-5.0.4.jar --cli target/debug/tablecraft \
  --directory .cache/tablecraft-fixtures/larger-decoded-attachments-20261024
```

`--generate` requires a new directory and refuses existing output files. Fresh
metadata timestamps may change hashes; register a generation before immutable
reuse. Databases and the reviewed external JAR stay in ignored persistent storage.
