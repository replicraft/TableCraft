# UN/LOCODE sample: metadata-only reading qualification

Observed 9 November 2026 on the retained local sample. Although its extension is
`.mdb`, TableCraft reports `Ace2007` and the independent read-only Jackcess probe
reports `V2007 [VERSION_12]`: this is the **ACE 12 / Access 2007 file-format
family**, not a Jet 4 MDB qualification. The format does not identify which
Access application release created or last edited it.

The original is 10,956,800 bytes. Its SHA-256 before and after these observations,
including a fresh check for this report, is unchanged:
`e174caa2d7048a3ad22b18618af679298bb14802e515e77102289602b454c7ac`.
No database rows, field values or private absolute paths are reproduced here.

## Read observations

| Check | Observed result |
| --- | --- |
| CLI metadata/catalog/table/index inspection | Successful; five user tables |
| Independent SDK row traversal | Five local user tables; 106,001 actual rows total; no linked tables |
| SDK largest-table metadata | Declared and actual count 100,969; 12 columns |
| SDK other table metadata | 249 × 2, 10 × 2, 14 × 2 and 4,759 × 3 rows/columns; declared and actual counts agree |
| SDK indexes | Zero on all five user tables |
| Complete CLI streaming read of largest table | Exit 0; 100,969 decoded rows × 12 columns; 12,824,990 output bytes |
| Complete remaining user tables | Release CLI decoded every row/column of all four smaller tables; counts 249, 10, 14 and 4,759 match the SDK; all five tables now total 106,001 completely decoded rows |
| Independent derivation from that stream | 249 distinct country keys |
| MCP opening and first pages | Five user tables; first-page counts 100, 100, 10, 100 and 14; all requests successful |
| MCP late page of largest table | Offset 100,900 returned the final 69 rows and `has_more=false` |
| Separate MCP query | Inspection succeeded; count query returned 249 |

The SDK probe opens read-only, disables expression evaluation and rejects linked
opens. Its cursor counts request no column values: its format/schema/count
agreement is **not** a cell-by-cell comparison of the complete CLI output.
The complete typed CLI reads now cover all five tables. The MCP observations
qualify bounded pages and explicit counts, not a complete MCP scan of all tables.

## Query budgets and retained refusals

- Under default allowances, the largest-table `COUNT(*)` refused with
  `query structural limit exceeded`. With explicit native allowances of 256 MiB,
  200,000 rows and 100,000,000 steps, and execution allowances of 256 MiB,
  1,000,000 rows and 100,000,000 steps, buffered and streaming counts both
  completed and returned 100,969.
- **Initial General grouping attempts** refused: the recorded explicit-budget
  attempt exhausted steps; another attempt exhausted bytes. Those failures remain
  part of the observation record.
- **Current-binary General follow-up completed** with the same explicit allowances:
  exit 0, 20 groups, and every count matching the independent complete-stream
  derivation. The recorded **unoptimized debug** run took about 2.05 seconds; this is one
  observation, not a portable benchmark. The original source hash remained unchanged.
- The grouped/HAVING query using explicit ordinal comparison completed with
  20 groups and matched independently derived counts from the complete stream.
  Ordinal success does not qualify General or native locale-specific equivalence.
- One initial index-inspection invocation used the wrong CLI argument position
  and was rejected by clap. The corrected `--table TABLE --indexes` invocation
  succeeded; the argument error was not a database-format failure.

## Release performance follow-up

The original 2.05-second observation used an unoptimized development executable.
A later Hyperfine 2.0.0 comparison measured the same complete CLI process, source,
database-General `GROUP BY`/`HAVING`/`ORDER BY` query, output and explicit budgets.
Each executable was checked against the independent complete-stream counts first;
both returned the expected 20 groups. Both warmups and measured runs succeeded,
and the original source hash remained unchanged.

The local machine was an Apple M1 Pro with 32 GiB RAM, macOS 14.7.2 ARM64.
Hyperfine used two warmups and ten measured runs per executable, without a shell.
These are warm-cache local timings, not a cold-disk or native Access benchmark.

| Build | Mean wall time ± sample standard deviation | Mean peak resident memory |
| --- | --- | --- |
| Development / unoptimized | 1,324.5 ± 35.0 ms | 55.4 MiB |
| Release / optimized | 245.3 ± 6.8 ms | 46.2 MiB |

The optimized executable was about 5.4 times faster than the contemporaneous
development executable. This comparison changes the **build profile**, not the
algorithm between the two binaries. The earlier algorithm change replaced a
linear scan of retained group representatives with explicit policy-scoped bucket
hints; every matching bucket still uses the full checked comparer. The sample
has no indexes, and no index was created or used to achieve these results.

Native projection already avoids decoding unreferenced columns. Remaining
optimization candidates include repeated full General-key construction and
per-row retained allocations. Those need separate release measurements and
correctness qualification; no key cache or aggregate-storage rewrite is claimed
here.

Release executable SHA-256:
`3879aa271625f3c404df6e7f7101a828ab97e4037dfe4e9545c0a6b79a886b41`.
Development executable SHA-256:
`55f5d203b6e17d041ecae2069e350d6eb495b290884dcbc051e0e196b6b1ba6e`.

## Evidence and scope

Private observations remain in `.cache/unlocode-mdb-before.json`,
`.cache/unlocode-mdb-cli.json`, `.cache/unlocode-complete-reading.json`,
`.cache/unlocode-mcp-reader.json`, `.cache/unlocode-ordinal-group-query.json`,
`.cache/unlocode-general-grouping-after.json`,
`.cache/unlocode-complete-all-tables.json`,
`.cache/unlocode-grouping-hyperfine.json`,
`.cache/unlocode-grouping-profile-summary.json` and
`.cache/mdb-sample-qualification-20261109/`. Raw CLI captures can contain source
values and are deliberately not copied into the public report.

The successful current-binary General run records implementation commit
`3a773ad4c05fe77629dbb10b822cbc4e1fb837e2` and executable SHA-256
`6ec89b6f9a5659e069fc84c95dc6246d3f390d34559ca1c77eac1935f630a016`.

This is bounded read/query evidence for one ACE 2007-format sample. It does not
establish Jet 4 support, full ACE compatibility, native Microsoft Access
execution, write/save/reopen interoperability, or General-collation compatibility
beyond the tested grouping request. No Windows/Access comparison was performed
here.
