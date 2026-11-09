# Deletion comparison request files

The feature-gated `preview_ace16_delete` engineering example accepts either
repeated `--delete ROW:HEX` arguments or `--delete-file PATH`. Both prepare the
same [unqualified deletion candidate](ACE16_DELETE_PREVIEWS.md); neither enables
ordinary CLI/backend saves. A request file lets exact-record batches exceed OS
argument-size limits without changing engine eligibility or resource limits.

```sh
cargo run --locked -p tablecraft-ace --features ace16-write-preview \
  --example preview_ace16_delete -- original.accdb candidate.accdb Items \
  --delete-file requests.tsv
```

The file is headerless two-column TSV, parsed by the csv crate. The first column
is the zero-based sequential live-row ordinal in the captured source; the second
is the nonempty exact original physical record payload in ASCII hexadecimal.
Standard TSV quoting is accepted. Hex letter case does not change bytes. Include
the stored column-count header and final NULL/Boolean mask, excluding page headers
and slot-directory words. These requests are not SQL values or persistent row IDs.

Input capture and complete parsing happen before database capture or fresh output
creation. A malformed later record never returns an earlier partial batch. File
length is checked before and after capture; this does not claim coherent capture
of a concurrently modified input. Use stable local request and database files.

Input ceilings are 32 MiB per request file, 4096 requests and 4096 decoded bytes
per expected record, all subject to the example's cumulative operation budget.
The reader prepays CSV buffers, record payloads and field-end arrays, including
the crate's two first-record caches even with headers disabled. Delimiter-heavy
input can exhaust the operation budget below the file ceiling. The engine still
checks duplicate/stale ordinals, exact records and its own narrower profile.

Four focused example tests cover clap source exclusivity, quoted records and exact
bytes, malformed batches, payload/row limits and independent byte/row/work budget
failures. Original-file comparison evidence remains separate from parser tests.

## Explicit multi-table comparison example

The separate `preview_ace16_batch_delete` example accepts repeated exact
`--group TABLE DELETE_TSV_PATH` pairs. By default it calls the
[explicit final-image batch API](ACE16_DELETE_PREVIEWS.md#explicit-multi-table-deletion-batches),
which retains RESTRICT-only admission. The explicit `--cascade` flag instead calls
the [separate acyclic cascade API](ACE16_DELETE_PREVIEWS.md#separate-acyclic-cascading-deletions)
after the same complete file capture. Neither mode enables ordinary saves.

```sh
cargo run --locked -p tablecraft-ace --features ace16-write-preview \
  --example preview_ace16_batch_delete -- original.accdb candidate.accdb \
  --group Children children.tsv --group Parents parents.tsv
```

To expand qualified original parent/child associations from seed requests:

```sh
cargo run --locked -p tablecraft-ace --features ace16-write-preview \
  --example preview_ace16_batch_delete -- original.accdb candidate.accdb \
  --cascade --group Parents parent-seeds.tsv
```

This mode keeps the closed outgoing primary-key, action, acyclic-graph and
preservation gates of the separate API. Expanded roots share the engine's table
and row ceilings and the same operation budget as explicit seeds. It does not
claim native SQL or mixed per-row action timing; self and cyclic cascades remain
refused.

Every file uses the same quoted, headerless ROW/HEX format and the unchanged
32 MiB/4096-request/4096-byte-per-record ceilings. Empty files refuse. Table names
must be UTF-8; native filesystem paths remain `PathBuf` values without lossy
conversion. Repeated table selectors refuse case-insensitively. The engine's
cumulative table/request ceilings also apply; the example does not reset its
256 MiB byte, 100,000 row and 100 million step operation budget between files,
database capture or candidate planning.

All request files must be captured and parsed before the database is opened.
An invalid later file cannot publish a valid earlier group. After the complete
engine candidate passes, the output is created with exclusive fresh-file
semantics. Existing outputs remain untouched. A write/sync failure attempts to
remove the newly created incomplete copy; a cleanup failure is reported explicitly
rather than hiding the remaining file. The source is always opened read-only.

The single-table example's positional arguments and mutually exclusive
`--delete`/`--delete-file` switches are unchanged; they are not batch switches.
The [consumer verification record](results/ace16-batch-delete-example-2026-10-26.json)
separates six new tests from four shared request-parser tests that execute in both
example targets. These checks are example/input evidence, not additional engine
compatibility tests or native Access action/save qualification.

The explicit cascade extension has a separate
[consumer verification record](results/ace16-cascading-delete-example-2026-10-26.json):
17 executions cover 13 distinct test definitions, including three new example
functions. The four shared request-parser definitions execute once in each example
target and are counted once as definitions. Tests pin default RESTRICT refusal,
explicit parent-seed expansion, retained NULL children, fresh-output protection,
clap pair admission and malformed later-file failure before database opening.
Scoped strict lint and formatting pass. These are consumer checks, not additional
engine tests or original-file/native Access qualification.
