# Large native encryption copy evidence

The [page-copy implementation](../access/storage/NATIVE_ENCRYPTION.md#page-copy-preservation-and-failure-boundary)
passes a 92-command independent replay over both unchanged original
[large ACE 16-family inputs](LARGE_DEPLOYMENT.md). The V2016 file is 77,041,664
bytes; V2019 is 77,111,296 bytes. Both exceed the image API's retained 64 MiB cap.
Only fresh temporary candidates are rewritten; originals, reference tools and
frozen executables retain their recorded hashes.

This is engineering evidence from Jackcess 5.0.4, Jackcess Encrypt 5.0.2 and
Bouncy Castle 1.86. It does not establish Microsoft Access save/reopen, VBA
application preservation, live-file coherence, large legacy-cipher coverage or
the complete 2 GiB envelope. There is no performance or peak-memory claim.

## What was checked

Each original contains 1,100 authored rows with exact Id/Score/Label values and
32,768-character Memos. The original integer/byte model checks every row and Memo
hash through the [inert getter driver](LargeEncryptedReadOracle.java), separately
from TableCraft SQL. Expression evaluation is disabled in the reference readers.

For each profile, the [runner](compare_large_encryption.py) checks:

| State or operation | Evidence |
| --- | --- |
| Original | Every getter row/Memo hash, decoded SID hashes, data-page digests and CLI browsing/query results |
| Add encryption | Fresh Agile candidate reopens through independent getters, SID and page readers; CLI results match the model |
| Change password | New credential reopens; old credential refuses; all data-page ciphertext bytes and independently decoded page digests match the added candidate |
| Remove from added/changed | Both complete plaintext images match the original SHA-256 |
| Independent removal from added/changed | The independent encryption library recovers the same complete original images |
| Existing output | Refusal preserves the candidate hash |
| Wrong credential | Empty stdout and no published output |
| Earlier executable | Both identical large originals refuse at the former structural limit without output |

CLI browsing checks table enumeration/counts and exact Memo rows 1, 550 and 1,100.
Progressive queries check all projected Id/Score/Label rows and Count/Min/Max/Sum.
The aggregate expectation is 1,100 rows, IDs 1 through 1,100 and score sum
10,284,450. The recovered plaintext candidates match every original byte,
including page zero; these ACE 16-family originals require no header promotion.

The [report](results/large-encryption-2026-10-25.json) records:

- 62 current CLI commands, two earlier-executable commands and 28 Java commands.
- 84 successes and eight expected refusals, including both baseline refusals.
- 8,800 getter row comparisons and 8,800 complete CLI projection row comparisons:
  four opened states across two 1,100-row originals, rather than 8,800 distinct
  authored records.
- All 37,633 data pages per opened state, with complete independently recovered
  images from both credentials and both removal implementations.
- Seventeen immutable pins and fourteen selected committed source snapshots,
  rehashed separately after the replay. Sixteen aggregate/projection stdout
  digests were independently recomputed with literal integer expectations and
  the standard CSV writer.

The first diagnostic attempt supplied query-only `--native-steps` to `tables`.
The runner corrected that argument scope and then completed all 92 commands;
product code, inputs and expected values did not change. Its private report hash
is retained in the final record. The final replay has no omitted command failures.

## Reproduction

The tested executable was built at `bf1df067a4d4cabe09f65544d2c1c6fc8281d1ea`
and frozen with SHA-256
`284414fef496a705f0382d11851df43a58233f6975ccd95b08933d4a6fa91f74`.
The earlier executable was built at `9b256bf6ba3c9dd808a028d44c46efeb5b1a1a8c`
and has SHA-256
`f54042f115c43b35c413798b1d93a01bc6f4beded39822987f38d260e540b5a3`.
Both are ignored local engineering artifacts. Selected live source files must
match the requested implementation revision; a report always needs a fresh path.
The runner does not build, download reference tools or execute active content.

```sh
python3 -B docs/testing/compare_large_encryption.py \
  --directory .cache/tablecraft-fixtures/large-deployment-20261025 \
  --jar .cache/tablecraft-fixtures/jackcess-5.0.4.jar \
  --crypt-jar .cache/tablecraft-fixtures/jackcess-encrypt-5.0.2.jar \
  --bc-jar .cache/tablecraft-fixtures/bcprov-jdk18on-1.86.jar \
  --cli .cache/tablecraft-bin/encryption-pages-cli-20261025 \
  --cli-sha256 284414fef496a705f0382d11851df43a58233f6975ccd95b08933d4a6fa91f74 \
  --baseline .cache/tablecraft-bin/japanese-proper-space-cli-20261025 \
  --baseline-sha256 f54042f115c43b35c413798b1d93a01bc6f4beded39822987f38d260e540b5a3 \
  --revision bf1df067a4d4cabe09f65544d2c1c6fc8281d1ea \
  --work-directory .cache/large-encryption-work-20261025 \
  --report FRESH_REPORT.json
```

The independent Java processes have a 512 MiB heap ceiling and each subprocess
has a timeout. One profile's temporary copies are removed before proceeding to
the next. The source and reference artifacts must already exist and match the
reviewed fingerprints. New crypto randomness makes candidate byte hashes differ
between runs; recovered original hashes and modeled values must remain exact.
