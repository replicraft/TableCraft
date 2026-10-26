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
