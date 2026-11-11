# Native decoded Memo reads

`LongTextReadLimits` defaults to the maximum UTF-8 expansion implied by the
native encoded-value bound: twice `0x3fffffff` bytes. This is a representation
bound for supported compressed/UTF-16 forms, not an extra Access text quota.
Microsoft describes Long Text storage as up to about 1 GB; form/report display
limits are distinct from stored data. See
[Microsoft's Long Text reference](https://support.microsoft.com/en-us/access/the-memo-data-type-is-now-called-long-text).

Both `rows` and `query run` accept `--native-text-bytes` to request an explicitly
lower native decoding budget. `LongValueReadSource::with_text_limits` applies the
policy outside existing native encryption/container sources. The encoded
[long-value policy](LARGER_ENCODED_VALUES.md) remains independent. UTF-8 byte
length, native encoded bytes, Unicode scalars and Access UTF-16 `Len` units are
not interchangeable.

Decoding validates Unicode, charges work/allocation, checks cancellation and
publishes only complete values. Original owner, cycle, page, terminal-reference
and length checks remain. Individual query Text and CLI output also have no
implicit product-size ceilings; `--text-bytes` and `--output-bytes` remain explicit
caller budgets. Removing invented limits does not establish arbitrary native
query or mutation compatibility.

## Historical verification

The following evidence describes captured builds and deliberately selected
budgets. Current limits are specified above.

### Recorded independent evidence

The original [generator/getter](../../../tests/access/storage/native/LargerMemoFixture.java) supplies fresh
inert Jet 4, ACE 2007, ACE 2016 and ACE 2019 inputs. Each stores 3 MiB ASCII,
600,000 Chinese characters, 300,000 `é🦀` pairs, empty Memo
and NULL. Read-only getters independently confirm UTF-8 lengths/SHA-256 and
UTF-16 unit counts; no reference SQL or application content executes.

The [runner](../../../tests/access/storage/native/compare_larger_memos.py) passes 76 controls, comparing
complete query payloads, Len, count-only pruning, selected-index reads, exact
UTF-8 boundaries and separate encoded/native/query/output refusals. Both original
inputs and fixed CLI hashes remain unchanged. [Recorded results](../../../tests/results/storage/larger-memos-2026-10-24.json)
retain the inventory, JAR/source/binary hashes and count. Engine controls additionally
reach exactly 16 MiB and reject the next byte, cover all five decode routes and
cancel every observed checkpoint after successful prefixes. Five query large-Text
tests and five CLI range/scope/output tests pass; scoped Clippy, minimal checks,
strict rustdoc, formatting and the dependency guard pass. Full workspace and
Windows Access checks were not run for this batch. This is engineering evidence,
not complete native compatibility or a performance claim.
