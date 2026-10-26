# Progressive native scalar row output

`rows --stream` publishes one native table as TSV through the existing source-bound
`NativeBrowser` cursor. It continues until checked EOF without rescanning earlier
roots or materializing a complete table. Ordinary `rows`, queries and inspection
retain buffered output. Streaming is an explicit progressive protocol: a later
read, limit, formatting or output failure can follow already published rows.
Check the exit status before treating an export as complete.

```sh
tablecraft rows example.accdb Customers --stream --limit 256 \
  --native-bytes 67108864 --native-rows 100000 --native-steps 10000000 > customers.tsv

tablecraft rows example.mdb Customers --stream --column ID --column Name
```

With `--stream`, `--limit` is the batch size, default 100 and maximum 16,384. It
is not a total-row limit. Empty tables produce one header after EOF/count checks.
A full final batch can require another fetch to establish checked EOF. A declared
count mismatch remains an error, even if preceding batches already returned all
observable data rows.

## Selection and source boundaries

Streaming defaults to every ordinary scalar column. Repeat `--column NAME` to
select a scalar subset of a mixed native schema. Fields appear in native logical
order, irrespective of selection order. Missing, duplicate or unsupported selected
fields fail during cursor preparation, including on empty tables. Unavailable
fields are not synthesized as NULL; unselected payloads remain unchecked. Native
schema, complete usage metadata and fetched framing retain the existing checks.

Only one stable/offline local native table is selected. MDB/ACCDB and existing
qualified native encryption/package input paths retain their input contracts;
streaming does not add new format or codec qualification. ACCDT's reconstructed
template tables have no streaming cursor in this command. No query, lookup,
calculated expression, VBA or external provider executes. Native shared-file
coordination and portable/persisted bookmarks remain unimplemented. Raw CLI
streams now retain a [shared OS lock](../access/writes/NATIVE_TRANSACTIONS.md#cli-read-coordination)
through cursor use, excluding cooperating exclusive writers without adding a
live Access snapshot.

The mode conflicts with `--all`, `--offset`, `--stored-calculations`, `--multivalues`
and `--collections`. Those commands retain their existing page contracts. `--column`
requires `--stream`. Native read limits also apply to explicit buffered
`--collections`/`--multivalues` pages. `--native-bytes` also applies to ordinary
buffered row pages, including container opening; `--native-rows` and
`--native-steps` still require streaming or collection modes. Rejected combinations and numeric ranges fail before file I/O. See [larger
collection pages](LARGER_NATIVE_COLLECTIONS.md#buffered-larger-cli-pages).

## Output and errors

Streaming always uses TSV, including with `--format auto` on a terminal. Explicit
`--format table` is rejected before file I/O. Cells use the same escaped display
representations and `csv::WriterBuilder` transport as ordinary redirected rows,
including quoted Text, NULL distinctions, controls, exact numbers, Binary and dates.
There is exactly one header. Parse with a TSV/CSV reader, not by splitting raw lines.

Each batch is read, formatted and CSV-encoded completely before its write begins.
A first-batch read/format failure therefore emits no header. Later such failures
leave only earlier completed batches. An output-device failure can interrupt a
write itself; batch preparation does not make writes transactional. Each successful
batch is flushed, and write/flush errors stop immediately without another fetch.
Broken pipe retains the CLI's successful closed-consumer policy; other output
errors exit 1. Read, schema, resource and formatting failures exit 2. Exit 0 means
the selected stream reached checked EOF or its output consumer closed the pipe.

The dispatcher distinguishes command failures from stdout I/O failures. It does
not route native reading or formatting into main. Normal buffered commands still
finish before their first stdout write. Diagnostics contain only operation labels,
counts and byte totals, never rows, selected names, paths or credentials.

## Cumulative controls

| Allowance | Default | Maximum |
| --- | --- | --- |
| `--native-bytes` | 16 MiB | 256 MiB |
| `--native-rows` | 16,384 | 1,000,000 |
| `--native-steps` | 1,000,000 | 100,000,000 |
| Encoded/logical output per batch | 1 MiB | 1 MiB |
| Encoded output over the entire stream | 256 MiB | 256 MiB |

Read budgets include opening/unlocking, schema/catalog metadata and failed reads
or allocations. Native row allowances include metadata rows, so they are not an
exact requested count of displayed records. Counters never refill between batches.
Per-fetch physical read quotas remain separate. Output caps are checked before
publishing each encoded batch. Existing formatter/dependency allocation boundaries
still apply; these controls are not a process-wide heap quota. This is scalar
browsing continuation, not larger or streaming SQL execution.

## Evidence

Focused CLI tests cover Jet 4/ACE 2007/ACE 16 header layouts, byte-identical buffered
and streamed TSV, one header, stored controls/quotes, empty tables, mixed-schema
projections, first/late failures, count mismatches, argument validation and shared
quotas. Publisher tests cover byte ceilings, write/flush errors, stopping at the
current cursor position and cancellation between batches. The existing browsing,
help, query retrieval and native-byte controls also remain regression-tested.

The independent [20,000-row recipe](../testing/StreamRowsFixture.java) creates inert
dense original tables with NULLs, Unicode/supplementary characters, tabs, quotes
and newlines. [Comparison](../testing/compare_stream_rows.py) uses reviewed read-only
Jackcess getters and Python's CSV reader, rather than a second TableCraft scan.
It passes 20 full-stream/projection/quota controls across four generated files,
including batch sizes 1/127/1,000, 20,000 exact rows and outputs exceeding 1 MiB.
Cumulative quota failure prefixes and unchanged original bytes are checked. These are independent reader/
transport checks, not native Access SQL, shared-file or save/reopen qualification.


```sh
python3 docs/testing/compare_stream_rows.py \
  --jar path/to/reviewed/jackcess-5.0.4.jar --cli target/debug/tablecraft \
  --directory path/to/fresh-stream-inputs --generate
```

[Fixture provenance and hashes](../testing/FIXTURES.md#original-progressive-row-inputs) record
the observed files. Native Access, live coordination and larger SQL execution
remain separate qualification/implementation work.
