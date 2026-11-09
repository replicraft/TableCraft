# Original native exact-key query replay

The [accepted report](results/native-exact-keys-2026-10-25.json) passes 76 controls
on the four immutable Decimal corpus databases: Jet 4, ACE 2007, ACE 2016 and
ACE 2019. Sixty-four successful queries compare 48,304 rows and 72,584 cells;
twelve intended resource/output refusals preserve empty stdout. There are no
disagreements. This is independent engineering evidence, not native Access SQL
qualification or a timing benchmark.

[The runner](compare_native_exact_keys.py) obtains original Int32 IDs through the
existing inert sequential Jackcess getter. Python integer/NULL and ordered-count
models supply complete expected results. Each format runs both materialized and
explicit declared-arithmetic profiles. Eight successful shapes per profile cover
direct GROUP/DISTINCT, repeated derived keys, computed DISTINCT with carrier
inspection, DISTINCT/ALL written-prefix boundaries, grouped UNION ALL populations
and nullable keys. Carrier codes describe TableCraft's existing explicit result
profiles; they are not asserted native Access result types.

Successful queries retain finite allowances: 5,000 native rows, 50,000 query rows,
128 MiB query allocation and 500,000 query steps. Separate per-format controls
lower work, native rows and output allowances to force buffered refusal. The
runner checks exact TSV headers and rows, process status and stderr. It retains
raw failure output if a control disagrees.

The CLI was frozen from clean revision
`8a47ef4d3d9d510d0f228d86963e5a619a9dcb15`, SHA-256
`58d9284689479c0a48d576e1540e3e7e8455b267c192f4269992769ed77460c9`.
All ten immutable tool/original hashes and fourteen scoped compiled-source hashes
were independently rechecked. Scoped source bytes come from that revision;
concurrent live edits are not part of the pinned executable. No original changed.

The runner reuses the separately pinned native-join helper for registered hashes,
getter access and process invocation. Its own integer grouping/set models are
independent of TableCraft query output. The report retains exact commands and
pins; replay requires the reviewed reader, four registered originals, frozen CLI
and a fresh report path. Provenance remains in [FIXTURES.md](FIXTURES.md);
no new binary fixtures or dependencies were adopted.

These originals each contain 1,000 IDs. Generated 12,000-row scale controls,
Boolean/other integral carriers, collisions, unexpected carrier fallback,
paired descriptors, eager errors, cancellation and quota/growth behavior have
separate [focused tests](../query/keys/EXACT_QUERY_KEYS.md). Text, mixed/unproved keys,
other SQL families and general optimizer/native comparisons retain their own
contracts. This replay does not establish complete read/query acceptance.
