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

The registered candidate reports 38 rows and three checked indexes: two Int32
and one General legacy Short Text index. Each index has a separate Checked or Not checked status.
`--check` requires --table and conflicts with --indexes; ordinary inspection
remains metadata-only. Exit zero means the report completed, including any stated
gaps; it is not a database integrity certificate. A supported-path failure exits
2 with no partial stdout. Existing escaped/buffered output and sanitized errors
remain; original bytes are unchanged.

The CLI's explicit `--native-steps STEPS` can increase cumulative inspection work
from the 1,000,000 default up to 100,000,000. It does not increase byte/row limits
or turn unchecked index profiles into checked ones. Long payloads can be decoded
again for each eligible index, so row count alone does not predict the required
work allowance. Exhaustion publishes no partial report.

## What the check establishes

Historical [known fixed-prefix reads](NATIVE_TABLES.md#deleted-column-histories)
validate surviving values, prefix/mask bounds and row counts without qualifying an
ambiguous deleted-variable tail. Required visible variable tails still validate.
Index agreement concerns the surviving decoded fields; it does not turn ignored
deleted payloads into a complete historical-record integrity certificate. Metadata
patching and native write gates retain their stricter geometry requirements.


The complete [sequential reader](NATIVE_TABLES.md#bounded-scalar-snapshots) decodes
all live roots in owned data pages and matches their count to declared metadata.
Every index eligible for the [qualified profile](NATIVE_INDEXES.md#bounded-indexed-reads)
then receives a full tree/row read: no range pruning. It checks signatures,
ownership, separators, order, sibling links, root liveness, counts and decoded key
agreement. Shared column schema, table identity/root and counts must agree across
these reads.
Indexed reads reuse one current data-page buffer only for consecutive root
references. Every root still receives live-slot, owner and row/key validation;
relocations still read their physical targets. Physical read counts fall without
skipping records or changing this complete coverage argument.

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

Foreign logical indexes can now receive the same physical checks after complete
[reciprocal/catalog validation](NATIVE_INDEXES.md#foreign-physical-traversal-and-checks).
Malformed metadata fails without a report; unsupported profiles remain explicitly
unverified. Checked foreign trees do not establish row membership, execute
cascades or enable mutation. Raw `--indexes` inspection still does not qualify
their declarations; `--check` reports the checked operation's coverage.

Completed one-to-ten-component Boolean/numeric/legacy DateTime/GUID checks also verify
each known index declaration. Primary indexes require unique keys and a value in
every component. Ordinary unique indexes reject duplicate tuples unless every
component is NULL; repeated entirely NULL tuples are allowed. Partially NULL tuples
must remain unique. The physical required-key flag rejects any NULL component.
Logical aliases sharing a primary backing inherit its uniqueness/non-NULL rules.
At most one logical primary declaration is accepted. Checks use exact integers,
scaled Currency, Boolean atoms, exact IEEE/DATE serial bit keys or all canonical GUID bytes
in the fully validated native order, with cancellation/work charged per row.
Constraint failures return typed IndexConstraintViolation errors without names,
keys or partial reports. Ordinary indexed reads retain their structural contract;
this extra semantic validation belongs to explicit check_table operations.
Floating keys retain physical signed-zero/NaN distinctions; these checks do not
provide SQL floating equality or native Access non-finite write qualification.
Legacy DATE constraints likewise retain physical serial distinctions, including
calendar aliases. Every DATE key must validate its native calendar envelope,
including internal node separators without a directly fetched row. Physical
uniqueness does not replace SQL temporal equality or establish Access conformance.
ACE 16-family single and composite Decimal checks preserve exact i128 coefficients
and each field's scale/precision, with private key models. Every magnitude,
including internal separators, must fit that field's precision. Older numeric
index variants remain unverified; this adds no Decimal foreign-key/coercion policy.

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

Text compound keys, Memo unique constraints, full-width Short Text unique
constraints, other unqualified
key types, ignore-NULL profiles other than qualified single Int32, unqualified flags/types and
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


Text-bearing compound indexes now participate in full traversal and row-key
checks, including mixed numeric/GUID/Decimal/Short Binary components. Constraint
NULL facts come from complete rows, not possibly truncated separator bytes.
Unique checks stay unqualified for any Memo component or a full-width/digested
key. Unknown collation and other ignore-NULL profiles remain explicit gaps. See
[compound text contracts and evidence](NATIVE_TEXT.md#compound-text-index-reads).


[Compound Short Binary indexes](testing/BINARY_INDEX_TUPLES.md) now also participate
in complete traversal/row-key checks. Exact short-key unique checks and row-derived
NULL facts retain the shared tuple policy; full-width/digested unique keys remain
unqualified. Primary/foreign Binary fixture qualification and OLE keys stay pending.


## Complete checks with projected index payloads

`check_table` first completes its full sequential scalar read, including Memo and
OLE payload decoding. Each qualified logical index then uses the existing full
index projection reader with every physical key field selected. This retains
page traversal, key/row agreement, complete counts and live-root coverage while
avoiding repeated decoding of unrelated long payloads. Original logical column
positions map explicitly to the dense key-only rows before the unchanged
unique, required and NULL constraint checks run. Full scalar schema, object and
definition identity checks remain in place.

Four new tests and 43 focused regressions pass. A counted original `USysRibbons`
source records exactly the same LVAL page-visit multiset as its standalone full
sequential read, with fewer LVAL visits and total read bytes than full decoding
through each index. Corrupting an observed non-key payload page still fails.
Gapped, reversed composite keys retain duplicate-key and required-NULL failures.
Strict engine linting passes; see the [verification record](testing/results/native-check-projection-2026-10-25.json).
An initial new-test size assumption failed and was corrected separately from
production behavior; its log remains excluded in that record.

This changes repeated decoding work, not check coverage, index eligibility or
default allowances. Sources must still remain stable/offline throughout the
operation. Fewer rereads do not establish concurrent-file isolation, and this
evidence does not qualify native Microsoft Access integrity results or writes.

## Single Int32 ignore-NULL membership

A completed check can validate known single-column Int32 indexes that omit NULL
keys. The checker first validates the complete sequential table, including its
declared live count, and counts actual non-NULL Int32 roots for each qualified
index. It then drops that snapshot and traverses the complete physical index
under the same cumulative allowance. Every entry must reference a distinct live
row with the exact non-NULL key, and the entry population must match the eligible
sequential count. Thus an omitted NULL cannot replace a missing non-NULL entry.
Empty and entirely NULL tables have an empty physical population.

Existing unique-key checks apply to the retained non-NULL entries; required-key
checks also examine omitted sequential NULL roots. Logical aliases retain
individual coverage entries. `TableCheck::rows()` and its schema still describe
the full table population. `Checked` means complete qualified physical coverage,
which does not make this index a complete-table retrieval path. Public indexed
`All`/`Null` requests remain refused.

All work, eligible-count allocation, row decoding, traversal and constraints
share the caller's budget/cancellation contract. A failure yields no partial
report. Unknown declarations and other ignore-NULL carrier profiles remain
visible as unsupported. Existing native mutation gates explicitly retain the
ignore-NULL refusal despite the new read-check coverage; this is no new write
support or native Microsoft Access integrity qualification.
