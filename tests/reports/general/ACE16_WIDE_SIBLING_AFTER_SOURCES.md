# Wider numeric keys with ordinary siblings: expected update files

Sixteen independently constructed update files now pass native read/canonical
Source checks and 240 fresh read-only independent-reader observations. Together
with the two unchanged originals and their thirty previously qualified reports,
this covers eighteen complete files and thirty-six native table contexts.
The [original qualification](ACE16_WIDE_SIBLING_ORIGINALS.md) precedes these models.
No TableCraft mutation producer was called to construct or qualify them.

Each header/table combination covers MIN to zero, MAX to NULL, NULL to MIN and
an atomic MIN/MAX swap. Int64 and Currency retain distinct carriers; Currency
uses exact scaled integers. The model changes only the selected fixed payload
and presence bit, plus the affected numeric leaf free-byte count, bitmap and
active key/ROW entries. An independent raw replay checks the entire complement:
Boolean values, Text, Binary payload/empty/NULL, variable footers, primary indexes,
aliases, historical counts, maps, the other table, inactive tails and EOF remain
exact. NULL assignment preserves the original inactive fixed payload.

The fitting-leaf representation deliberately retains the original prefix zero.
NULL to MIN therefore has 39 logical and physical active bytes. An equivalent
prefix-one representation would use 37 physical bytes; this corpus does not
require compression normalization or qualify that alternative. The policy and
original geometry were frozen before constructing expected files.

All complete inventory and index reports match. Original reports round-trip and
are reused rather than rerun. Copied-receipt controls reject thirteen malformed
records, and 96 raw-role controls reject changes outside the exact request laws.
Saved verification checks 482 concrete artifacts without starting children.
The native gate independently reconstructs whole files from original requests
and checks both tables, all six aliases, counters, constraints and fresh Source.

The first strict check found a range-loop lint in the test replay. Enumerating
the same roots in the same order resolved it without changing expected bytes;
the affected native gate and strict checks then passed. See the
[acceptance record](../../results/general/ace16-wide-sibling-after-sources-2026-11-02.json).

Actual mutation admission, commits, recovery, resource limits and typed/SQL/CLI
saves remain separate work. Independent readers do not establish native Microsoft
Access save/reopen or full format compatibility.

```sh
TABLECRAFT_WIDE_SIBLING_AFTER_SOURCE_DIR=/absolute/path/to/expected-first \
  cargo test -p tablecraft-ace --all-features --lib \
  table::mutation::normal_ordinary_sources::wide_sibling_after::wider_numeric_sibling_after_sources_match_literal_requests_and_complete_canonical_reads \
  -- --ignored --exact
```
