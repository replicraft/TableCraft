# Read-only native table consistency checks

`tablecraft_ace::check_table` checks one explicit scalar table on a caller-owned
stable/offline Read + Seek source. It returns a completed TableCheck with native
schema, sequential live-root count and coverage for every inventoried logical
index. It performs no writes, repairs, provider activation or active-content work.
The standalone library retains no query/adapter/application dependency.

```sh
cargo run --locked -p tablecraft-cli -- inspect \
  docs/examples/Access_Example_VBA.accdb --table ROULETTE_TB --check
```

The registered candidate reports 38 rows, two checked Int32 indexes and one
unchecked Text index. Each index has a separate Checked or Not checked status.
`--check` requires --table and conflicts with --indexes; ordinary inspection
remains metadata-only. Exit zero means the report completed, including any stated
gaps; it is not a database integrity certificate. A supported-path failure exits
2 with no partial stdout. Existing escaped/buffered output and sanitized errors
remain; original bytes are unchanged.

## What the check establishes

The complete [sequential reader](NATIVE_TABLES.md#bounded-scalar-snapshots) decodes
all live roots in owned data pages and matches their count to declared metadata.
Every index eligible for the [single-Int32 profile](NATIVE_INDEXES.md#bounded-indexed-reads)
then receives a full tree/row read: no range pruning. It checks signatures,
ownership, separators, order, sibling links, root liveness, counts and decoded key
agreement. Shared column schema, table identity/root and counts must agree across
these reads.

Strict key/row ordering and decoded key agreement prevent duplicate indexed root
references; each referenced live root belongs to the same owned data set. Matching
that unique subset to the complete sequential count establishes root coverage on
the caller's stable source. This is a physical coverage argument, without SQL
equality/coercion or a multiset comparison of semantic Values.

Metadata/profile eligibility has one owner shared with indexed reads. Unsupported
profiles are explicitly reported; errors during a selected traversal propagate
instead of becoming skips. Returned reports omit payload Debug/Serde. Trace spans
record operation/counts only.

## What remains unchecked

Composite/Text/other key types, ignore-NULL indexes, unqualified flags/types and
foreign-reference profiles have no tree/row validation. Their definitions remain
visible, including coverage gaps. Entire tables with unsupported scalar fields
fail; the checker does not pretend to validate selected columns only.

Required/default/validation properties, unique/primary/foreign-key constraint
semantics, table data macros, application objects/VBA, unknown-content preservation,
allocation repair, transactions, recovery and shared-file isolation remain
unqualified. Unmapped/orphan pages are not discovered by these owned-page scans.
Undecodable metadata fails rather than becoming a skipped-profile entry.
The current unencoded Jet 4/ACE-code-2 and structural limits apply.
A successful table report does not qualify the database as a whole or authorize
writes. Native Microsoft Access comparison remains unrun.

## Budgets, evidence and measurement

One caller ResourceBudget accumulates every catalog/schema/sequential/index read,
report allocation and completion step. Failed work is not refunded, and no retry
or fallback is automatic. Cancellation/limit checks apply before publication.
The CLI retains 16 MiB, 16,384 cumulative rows and 1,000,000 steps. Large tables or
many indexes can exceed those allowances even when each separate read succeeds.
Hosts own timeouts and stable-source lifetime; repeated reads imply no transaction.

Original tests add an extra live data root while leaving a count-correct index
intact: the index alone succeeds and the combined check rejects the mismatch.
Other tests cover supported corruption, explicitly unverified corrupt Text pages,
exact cumulative row allowances, cancellation and every I/O failure callback.
CLI tests verify coverage output, argument requirements, unchanged bytes, no
partial output and trace redaction.

Generated Jet 4/ACE Indexed fixtures report 1,000 rows, two checked indexes and
three unqualified profiles. Seven-page Wide/EmptyWide fixtures report seven/zero
rows and both indexes checked. These are independent-writer inputs and local Rust
execution, not native Access integrity results. [Check benchmarks](testing/NATIVE_BENCHMARKS.md#table-consistency-checks)
measure the additional complete traversal work without comparing it to a weaker
validation operation as a claimed speedup.
