# High-offset native Memo and OLE read reliability

The native cursor now has focused synthetic qualification for external Memo/OLE
payloads at the 2 GiB source boundary, in addition to the ordinary high-offset
Int32 page controls. This adds acceptance evidence; the existing decoder and
native source limits did not require a production change.

The original `table_pages/large_long_values.rs` helper exposes a virtual 2 GiB
read/seek source with only a small catalog/schema prefix and explicitly authored
payload pages. The final physical page contains a single-row Memo. Other rows
follow nonadjacent long-value chains that move between high pages and page 14,
including backward page references. Unmapped holes fail reads; no large image,
whole-file scan or zero-filled fallback is available.

Independent literal bytes encode compressed-Unicode `A中B`, with mode transitions
split across chain boundaries, and OLE bytes `00 ff 01 80 00 42`. All four existing
priority layouts (Jet 4 and ACE 2007/2016/2019) preserve these values through
one-row cursor batches under a 256 KiB cumulative allowance. Complete source
buffers remain unchanged. These are independently authored format controls, not
public-writer fixtures or native Access comparisons.

The five tests cover 44 profile/subcases:

- Four complete sparse payload reads, with exact Memo and OLE goldens.
- Eight ID-only or empty projections whose unavailable external payload pages
  are never visited.
- Sixteen late suffix faults: foreign LVAL signature, prematurely ended chain,
  cycle and absent page. After one delivered row, a batch containing a good row
  followed by the fault publishes neither pending row. Malformed versus host I/O
  errors remain distinct. Continuation and snapshot conversion become terminal
  without additional reads or refunded work.
- Four decoder-completion cancellations at a high payload page. Cancellation
  remains typed even when the host decoder first changes its local buffer.
- Twelve cumulative byte/row/work exhaustions after a good prefix, retaining the
  previously delivered position and terminal failure boundary.

The full `table_pages` target passes 35 unique tests, including these five;
strict engine library/target Clippy passes at that checkpoint. A subsequent
readability-only change names the four fault cases with an enum; the final
five-test rerun passes against that source. It is a subset of the 35-test count. The first exploratory run passed four tests and exposed an
authored EOF assumption: fetching exactly the remaining row count does not promise
EOF in that full batch. The corrected test performs an explicit empty final
fetch, preserving checked EOF rather than changing the cursor.

Exact commands, logs, source hashes and the excluded first run are recorded in
[the scoped verification](../../testing/results/read-query-high-offset-payloads-2026-10-25.json).
Existing seek/short-read, shared-lock and ordinary high-offset page evidence is
separate in [read failure reliability](NATIVE_READ_FAILURES.md). These controls do
not qualify live-file coherence, native Access save/reopen, million-row throughput,
new codecs or broader encryption-management/image-capture limits.
