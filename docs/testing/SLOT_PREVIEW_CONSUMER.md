# Text/Binary engineering consumer

`preview_ace16_slot` is an opt-in example for
[qualified existing-slot previews](ACE16_WRITE_PREVIEWS.md#existing-width-short-text-and-binary-slots).
It returns a disposable candidate through the engine's separate
`NativeImage::preview_ace16_slot_updates` API. It does not enable normal CLI or
backend writes, journals, arbitrary page allocation or native Access save/reopen
compatibility.

Build and invoke it explicitly:

```sh
cargo build --locked --offline -j2 -p tablecraft-ace --features ace16-write-preview --example preview_ace16_slot
target/debug/examples/preview_ace16_slot original.accdb candidate.accdb Items Score requests.tsv
```

The five positional arguments are `SOURCE OUTPUT TABLE COLUMN TSV`. Every request
addresses the same exact table/column and a zero-based live-row ordinal in the
original captured image. The output must be a fresh path. Engine field, index,
constraint, expected-value, width and profile gates remain unchanged.

An explicit `--page-updates` flag selects the separate
`NativeImage::preview_ace16_page_updates` API. It can reuse an existing record's
capacity or append a hidden forwarding target on the original logical root's
page, subject to its space, slot-count, ownership and free-space-hint checks. It never
allocates another page or moves neighboring records. Without this flag, the
original exact-slot-span contract remains in effect.

```sh
target/debug/examples/preview_ace16_slot --page-updates original.accdb candidate.accdb Items Score requests.tsv
```

## Request format

The input is headerless UTF-8 TSV parsed with the existing `csv` dependency.
Each record has exactly five fields:

| Field | Meaning |
| --- | --- |
| `ROW` | Nonnegative original live-row ordinal |
| `EXPECTED_KIND` | `null`, `text` or `binary` |
| `EXPECTED_DATA` | Exact original value in the selected kind |
| `KIND` | `null`, `text` or `binary` |
| `DATA` | Replacement value in the selected kind |

Kinds are lowercase. `null` requires an empty data field. `text` contains literal
UTF-8, including embedded NUL; an empty Text value remains distinct from NULL.
`binary` contains an even number of ASCII hexadecimal digits, with either letter
case; an empty Binary value remains distinct from NULL. These are typed carriers,
not SQL expressions, escape sequences or inferred numeric values.

Tabs, newlines and quotes inside Text use CSV-style quoting: surround the field
with double quotes and double any embedded quote. The parser's established quoted
TSV behavior applies; this example does not add a separate strict CSV grammar.

For example, this record replaces literal Text `a` with `é` at original ordinal 2
(the separators below are tabs):

```text
2	text	a	text	é
```

The complete file is captured and parsed before the database or output is opened.
A malformed later request therefore produces no candidate and cannot cause an
earlier request to be published. Empty files are refused. The file ceiling is
16 MiB, the count ceiling is 4096 assignments, and each data field is at most
1024 encoded bytes. Engine storage/type limits can be narrower.

## Shared bounds and publication

Input capture, parser scratch, typed values, image capture and engine validation
share 256 MiB of charged bytes, 100,000 rows and 100,000,000 work steps. These are
cumulative operation allowances. File/count ceilings do not promise that every
input below them fits the remaining allowance.

The reader captures an exact bounded file length through `tablecraft-io` and
rechecks its length. Parsing reuses one byte record with explicit input-sized
byte and field capacities. Conservative prepayment covers that record, the CSV
reader's first-record byte/string caches, its input buffer and the bounded request
container. A hostile record with many separators cannot obtain uncharged field
capacity merely because it will eventually fail the five-field check.

After a complete engine result, publication uses `create_new`, writes all bytes
and synchronizes the output file. Existing output and source paths are never
overwritten. A write or synchronization failure removes the newly owned partial
output; a cleanup failure is reported alongside the original failure. The
example does not implement a normal save or journal durability protocol.

Successful execution prints `candidate_changed_rows=N`; it also prints an
engineering qualification notice to stderr. Failures are reported by the
executable boundary without printing request values.

## Focused checks

The example tests cover exact quoted carriers and NULL/empty distinctions,
malformed later input before a missing database, file/count/value ceilings,
byte/row/work exhaustion, hostile field counts, positional arguments, fresh
publication, injected partial-write cleanup and a real engine-backed filesystem
candidate. The latter reopens the result through the ordinary reader/checker and
verifies that existing output and source bytes survive overwrite attempts.
An additional engine-backed control verifies that growth requires the explicit
page mode, the source stays unchanged and an existing candidate cannot be
overwritten.

On 26 October 2026, all ten example tests pass with `ace16-write-preview`;
strict Clippy for this example and scoped formatting also pass. These checks
cover the consumer boundary. The engine's independent preservation controls are
recorded in the [page-update contract](ACE16_PAGE_UPDATES.md).

These authored tests do not establish native Access behavior. Fresh original
fixture comparisons and any frozen consumer receipts remain separate evidence.

The [page-mode build receipt](results/ace16-page-example-build-2026-10-26.json)
records the executable built from committed revision `fd2a23cd`, its SHA-256,
94 selected committed source pins and the consumer test/lint logs. The original
exact-slot comparison executable remains separate and unchanged. Building this
tool does not establish original-file or native Access outcomes.
