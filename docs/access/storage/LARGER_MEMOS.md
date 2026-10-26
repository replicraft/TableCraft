# Explicit larger decoded Memo reads

`query run --native-text-bytes BYTES` selects a native decoded Memo UTF-8
allowance from one byte through 16 MiB. The default remains 1 MiB. It requires
`--database`, refuses SQLite and template inputs, and changes no source bytes.
Ordinary browsing and other commands retain their existing defaults.

The engine owns `LongTextReadLimits` and the `NativeReadSource::long_text_limits`
hook. `LongValueReadSource::with_text_limits` applies a validated decoded policy
outside existing encryption/container sources while preserving original raw I/O
and native header/page hooks. An encoded-only outer wrapper preserves its inner
decoded policy; mutable reborrows forward it. Policy construction performs no I/O.
The source policy is fixed before snapshots/caches are created.

Encoded storage remains a separate [long-value policy](LARGER_ENCODED_VALUES.md),
with a 2 MiB default and explicit 32 MiB ceiling. A decoded UTF-8 byte count is not
the encoded UTF-16/compressed byte count, Unicode scalar count or SQL Len result.
For example, 600,000 Chinese characters occupy 1,800,000 UTF-8 bytes while Len is
600,000; supplementary characters consume two UTF-16 units each for Len.

Memo decoding validates Unicode and measures actual UTF-8 size before allocating
the output String. The ASCII path checks length before output allocation; the
other path retains a bounded, charged UTF-16 scratch buffer and checks the decoded
ceiling before reserving exactly the required UTF-8 bytes. Native work reserves
four logical encoded-input passes for classification, unit conversion, sizing
and emission. Cancellation is checked before/after decoding and at bounded checkpoints within
classification, unit construction, sizing and emission. A checkpoint covers at
most 4,096 bytes, UTF-16 units or decoded scalars, so the longest loop interval
covers at most 16,384 encoded bytes. Allocation calls themselves are not
interruptible. Failed decoding never publishes a
partial Memo. Existing length, owner, cycle, terminal-reference and page checks
remain mandatory.

## Query and output controls

Individual query Text values still default to 65,536 bytes. An explicit
`--text-bytes BYTES` / `ExecutionOptions::max_text_bytes` can now request up to
16 MiB. This applies to all query sources, parameters and generated intermediates;
it does not increase native admission, parser/binder limits, cumulative query
allocations/work or buffered output. Source-free eval retains its defaults.

A stable/offline file containing larger Memos can be queried with separate limits:

```sh
tablecraft query run 'SELECT ID,Len([Memo]) FROM LargeMemos' \
  --database example.accdb \
  --native-value-bytes 16777216 --native-text-bytes 16777216 \
  --text-bytes 16777216 --native-bytes 268435456 \
  --native-steps 100000000 --execution-bytes 268435456 \
  --execution-steps 100000000 --format tsv
```

Displaying the actual Memo additionally needs enough `--output-bytes` (maximum
16 MiB) for escaped display cells and final CSV quoting. A maximum-sized Text
value can exceed the output cap after rendering overhead; Len/count/filter work
need not publish the payload. Higher per-value limits never guarantee that a
whole table, intermediate or rendered result fits the independent cumulative
pools. These are consumer ceilings, not native format maxima or an unlimited
query/streaming profile. Native Access qualification remains separate.


## Independent evidence

The original [generator/getter](../../testing/LargerMemoFixture.java) supplies fresh
inert Jet 4, ACE 2007, ACE 2016 and ACE 2019 inputs. Each stores 3 MiB ASCII,
600,000 Chinese characters, 300,000 `é🦀` pairs, empty Memo
and NULL. Read-only getters independently confirm UTF-8 lengths/SHA-256 and
UTF-16 unit counts; no reference SQL or application content executes.

The [runner](../../testing/compare_larger_memos.py) passes 76 controls, comparing
complete query payloads, Len, count-only pruning, selected-index reads, exact
UTF-8 boundaries and separate encoded/native/query/output refusals. Both original
inputs and fixed CLI hashes remain unchanged. [Recorded results](../../testing/results/larger-memos-2026-10-24.json)
retain the inventory, JAR/source/binary hashes and count. Engine controls additionally
reach exactly 16 MiB and reject the next byte, cover all five decode routes and
cancel every observed checkpoint after successful prefixes. Five query large-Text
tests and five CLI range/scope/output tests pass; scoped Clippy, minimal checks,
strict rustdoc, formatting and the dependency guard pass. Full workspace and
Windows Access checks were not run for this batch. This is engineering evidence,
not complete native compatibility or a performance claim.
