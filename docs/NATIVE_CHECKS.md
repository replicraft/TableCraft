# Read-only native table consistency checks

Field declarations and foreign-key records now have a separate explicit
[constraint audit](NATIVE_CONSTRAINTS.md); the structural/index mode below retains
its existing coverage.

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
Every index eligible for the [qualified numeric/GUID profile](NATIVE_INDEXES.md#bounded-indexed-reads)
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

## Qualified index constraints

Completed one-to-ten-component numeric/GUID checks also verify
each known index declaration. Primary indexes require unique keys and a value in
every component. Ordinary unique indexes reject duplicate tuples unless every
component is NULL; repeated entirely NULL tuples are allowed. Partially NULL tuples
must remain unique. The physical required-key flag rejects any NULL component.
Logical aliases sharing a primary backing inherit its uniqueness/non-NULL rules.
At most one logical primary declaration is accepted. Checks use exact integers,
scaled Currency or all canonical GUID bytes
in the fully validated native order, with cancellation/work charged per row.
Constraint failures return typed IndexConstraintViolation errors without names,
keys or partial reports. Ordinary indexed reads retain their structural contract;
this extra semantic validation belongs to explicit check_table operations.

IndexConstraints exposes these decoded declarations separately from raw flags.
Unknown flag/type profiles have no decoded declaration. CLI --indexes adds Primary,
Unique, Required key and Ignore NULLs fields; --check identifies checked index
constraints separately from unsupported coverage. Index Required is not the
column Required property; scalar source nullability remains conservative.

Microsoft's [Primary](https://learn.microsoft.com/en-us/office/client-developer/access/desktop-database-reference/index-primary-property-dao)
and [Unique](https://learn.microsoft.com/en-us/office/client-developer/access/desktop-database-reference/index-unique-property-dao)
documents describe the semantic rules. Physical flags and repeated-NULL behavior
were researched in the pinned [Jackcess IndexData](https://github.com/jahlborn/jackcess/blob/be0a2139b087a227d14cafd4ea231306a5a3a862/src/main/java/com/healthmarketscience/jackcess/impl/IndexData.java)
reference, not copied as implementation. The DAO Required documentation does not
establish field-property decoding; no such support is inferred.

The original [constraint recipe](testing/ConstraintFixture.java) creates unique,
required and primary Int32 tables, verifies rejected inserts using Jackcess, then
reopens read-only. [Independent comparison](testing/compare_constraints.py) matches
all six index declarations, completed checks and all 18 tagged rows across Jet 4
and ACE. Repeated NULLs in a unique index are included. Rust tests additionally
cover changed unique/primary flags, shared backing, multiple-primary rejection and
a 256-case independent set model. This is not Microsoft Access qualification.

```sh
java -cp path/to/jackcess-5.0.4.jar docs/testing/ConstraintFixture.java path/to/new.accdb V2007
java -cp path/to/jackcess-5.0.4.jar docs/testing/ConstraintFixture.java path/to/new.mdb V2000
cargo build --locked -p tablecraft-cli -p tablecraft-ace --examples
python3 docs/testing/compare_constraints.py --cli path/to/tablecraft \
  --example path/to/table_data --jar path/to/jackcess-5.0.4.jar \
  --ace path/to/new.accdb --jet path/to/new.mdb
```

## What remains unchecked

Text/other key types, ignore-NULL indexes, unqualified flags/types and
foreign-reference profiles have no tree/row validation. Their definitions remain
visible, including coverage gaps. Entire tables with unsupported scalar fields
fail; the checker does not pretend to validate selected columns only.

Field Required/default/validation properties and foreign-key membership use the
separate explicit constraint audit. This structural check does not qualify Text/other unqualified
index constraints, table data macros, application objects/VBA, unknown-content preservation,
allocation repair, transactions, recovery and shared-file isolation remain
unqualified. Unmapped/orphan pages are not discovered by these owned-page scans.
Undecodable metadata fails rather than becoming a skipped-profile entry.
The current unencoded Jet 4/ACE-code-2/5/6 and structural limits apply. Int64 keys
require the qualified 2016/2019 scalar profiles.
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
