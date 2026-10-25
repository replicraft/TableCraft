# Original native integral join replay

The [accepted report](results/native-equijoins-2026-10-25.json) records 20 passing
controls over the four immutable Decimal corpus databases: Jet 4, ACE 2007,
ACE 2016 and ACE 2019. It compares 16,012 rows and 32,024 cells with no
disagreements. These are independent getter/model checks, not native Access SQL
qualification or a timing benchmark.

[The runner](compare_native_equijoins.py) obtains actual ID values and their
encounter order through the existing inert sequential Jackcess getter. Python
integer equality models self joins, reversed ON operands, post-join filtering,
and left/right unmatched rows. Outer-row encounter order checks TableCraft's
existing execution contract; SQL without ORDER BY does not promise that order
across engines. Missing IDs use an explicit -1 projection sentinel.

Each original table has 1,000 distinct IDs. Self joins visit one million pairs
under a 10,000-row query allowance, exercising rejection before copied candidate
allocation. All selected source reads retain their existing checks. The runner
uses finite native/query budgets and compares complete TSV headers and rows;
nonzero exits, stderr, and value/order disagreements fail the control.

The executable was frozen from revision
`8a3a15203c78ca616dcb60b8190651e5f785c890`, with SHA-256
`a143f3a3bfb6bf7089759cf07f9382b9d688f483a9e9fa6b570ad9781caeb401`.
All nine immutable inputs/tools and twelve scoped compiled-revision source hashes
were independently rechecked after the run. Compiled source is read from that
revision, not from concurrently changing live files.

The report and runner retain commands, hashes, cardinalities and refusal details.
The runner requires a fresh report path and the four registered original hashes;
replay it using the recorded CLI, reviewed reader and input arguments. Original
bytes and provenance remain in [FIXTURES.md](../FIXTURES.md). No new binary
fixtures or dependencies were adopted.

This corpus covers ordinary non-null Int32 IDs. NULLs, duplicate IDs, other
admitted integral carriers, paired descriptors, eager source failures,
cancellation and resource failures have separate focused
[implementation checks](../QUERY_EQUIJOINS.md). Text joins, compound predicates,
other carriers, general hash joins and native Access comparison remain outside
this evidence.
