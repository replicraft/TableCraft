# Native read failure and continuation reliability

Native page readers now check cancellation immediately after the host page decoder
returns successfully, before interpreting or retaining its decoded buffer. This
uses the existing cumulative controls without adding a page-work charge. Decoder
errors still propagate directly. A decoder may mutate only the supplied local
buffer; neither decoding nor failure handling changes the original source bytes.
This is separate from raw exact-read completion checks in [I/O](IO.md).

A successful callback can trigger cancellation before returning. Previously a
subsequent page-signature check could report Malformed first. The new completion
check reports Cancelled at that boundary, before traversal consumes decoded data.
This does not interrupt a blocking callback or replace host timeout ownership.

## Cursor guarantees

A source-bound scalar cursor can have delivered earlier successful batches when
its next fetch fails. It publishes none of the failed batch, retains its last
successfully delivered position, and becomes terminal. Subsequent next_page and
into_snapshot requests return CursorFailed without I/O or refunded work. Even if
physical traversal consumed part of the failing suffix, that suffix cannot become
a complete table snapshot. Invalid request ranges retain their distinct existing
nonterminal validation behavior.

Explicit [streaming CLI output](STREAMING_ROWS.md) can retain earlier published
batches. Buffered snapshots and ordinary queries remain unpublished until complete
EOF/count validation. A fresh explicit query source retry must start from the
beginning; it cannot reuse incomplete caches. Existing adapter tests retain that
contract. None of these guarantees establishes coherence against a live,
noncooperating Access writer; stable/offline inputs remain required.

## Verification

The original table_pages/faults tests use generated 403-row, three-data-page
inputs under the Jet 4, ACE 2007 and ACE 2016/2019 read profiles. After a successful
one-row prefix, each remaining read/seek/page-hook callback is failed in turn.
Injected cases cover sanitized I/O errors, premature EOF, impossible read counts,
wrong reported absolute seek positions, decoder errors and decoder-triggered
cancellation after altering its local buffer. Each failure discards the pending
suffix, preserves the delivered position, closes continuation and retains charges.
All callback counts and original bytes are checked; no native fixture is modified.

Separate controls deliver complete 403-row results under 1-, 17- and 4,095-byte
short reads with recurring Interrupted callbacks, using one cumulative allowance
through all 127-row batches. Persistent Interrupted callbacks exhaust explicit
work and close continuation without automatic retry using a fresh budget.

These are adversarial host-I/O and synthetic native-framing checks. They do not
qualify a new encrypted codec, concurrent-file snapshot, allocation repair,
native Access save/reopen or Windows behavior. Broader storage/scale acceptance
remains in [the read/query checklist](READ_QUERY_ACCEPTANCE.md).


## Maximum-width malformed suffixes

The wide_rows integration slice independently builds chained 255-column
metadata and three wide binary rows under all four priority layouts. It checks
complete, ID-only and empty projected cursors. After delivering one good row, a
fetch containing another good row followed by malformed framing fails without
publishing either pending row. The delivered position remains one; subsequent
fetch/materialization returns CursorFailed without I/O or budget changes.

Faults cover high-slot descending offsets, offsets outside the variable payload,
a first offset before fixed storage, variable counts above the schema ceiling
and future logical row counts. Positive controls retain high null-mask bits and
all 254 variable values through one-row batches. These tests validate required
framing; they do not promote unselected payloads to checked values. An initial
negative expectation used a shortened variable tail, which is not universally
invalid framing for a projection; the test was corrected to exceed the schema's
variable ceiling rather than broaden production admission.

[Original independent location reporting](testing/WideRowLocationOracle.java)
identifies the last appended row in the registered maximum-width inputs. The
[corrupted-copy runner](testing/compare_wide_row_failures.py) changes only fresh
temporary copies. Across two tables/four profiles/five faults it passes 280 CLI
controls: buffered rows, wildcard/ID/COUNT(*)/indexed queries fail with empty
stdout; full and ID-only streaming retain exactly the preceding valid rows.
Original and derivative hashes remain unchanged during all reads. Independent
getters locate targets only; they do not qualify SQL or native Access behavior.
