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
| Independent derivation from that stream | 249 distinct country keys |
| MCP opening and first pages | Five user tables; first-page counts 100, 100, 10, 100 and 14; all requests successful |
| MCP late page of largest table | Offset 100,900 returned the final 69 rows and `has_more=false` |
| Separate MCP query | Inspection succeeded; count query returned 249 |

The SDK probe opens read-only, disables expression evaluation and rejects linked
opens. Its cursor counts request no column values: its format/schema/count
agreement is **not** a cell-by-cell comparison of the complete CLI output.
The complete typed CLI read concerns the largest table; the MCP observations
qualify bounded pages, not a complete MCP scan of all tables.

## Query budgets and retained refusals

- Under default allowances, the largest-table `COUNT(*)` refused with
  `query structural limit exceeded`. With explicit native allowances of 256 MiB,
  200,000 rows and 100,000,000 steps, and execution allowances of 256 MiB,
  1,000,000 rows and 100,000,000 steps, buffered and streaming counts both
  completed and returned 100,969.
- Grouping with database General text comparison did not complete: the recorded
  explicit-budget attempt exhausted steps; another attempt exhausted bytes.
  These refusals remain qualification limits, not passing group-query evidence.
- The grouped/HAVING query using explicit ordinal comparison completed with
  20 groups and matched independently derived counts from the complete stream.
  Ordinal success does not qualify General or native locale-specific equivalence.
- One initial index-inspection invocation used the wrong CLI argument position
  and was rejected by clap. The corrected `--table TABLE --indexes` invocation
  succeeded; the argument error was not a database-format failure.

## Evidence and scope

Private observations remain in `.cache/unlocode-mdb-before.json`,
`.cache/unlocode-mdb-cli.json`, `.cache/unlocode-complete-reading.json`,
`.cache/unlocode-mcp-reader.json`, `.cache/unlocode-ordinal-group-query.json` and
`.cache/mdb-sample-qualification-20261109/`. Raw CLI captures can contain source
values and are deliberately not copied into the public report.

This is bounded read/query evidence for one ACE 2007-format sample. It does not
establish Jet 4 support, full ACE compatibility, native Microsoft Access
execution, write/save/reopen interoperability, or completed General-collation
large-table grouping. No Windows/Access comparison was performed here.
