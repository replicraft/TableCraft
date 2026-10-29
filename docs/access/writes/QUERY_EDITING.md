# Bounded query editing

`tablecraft_query::plan_update` consumes a backend-neutral `EditableTable` and
owned SQL query, returning an `UpdatePlan` with matched-row count and a concrete
backend candidate. It imports [DB contracts](../../foundation/DB.md), never concrete engines or
adapters. Planning performs no durable writes. The Access adapter delegates native
checks/preservation to [ACE](NATIVE_WRITES.md).

## Supported profile

The ordinary `ExistingInt32` profile supports one local table, optional alias
and one SET assignment to an Int32 field. Qualified/unqualified references use the existing ASCII-insensitive
binder. Pure scalar expressions, BETWEEN, IN lists and three-valued WHERE reuse
retrieval evaluation, including eager operand errors. Only Boolean TRUE selects a
row; final numeric predicate coercion belongs to read-only retrieval. Shared
[exact integer operators](../../expressions/numeric/INTEGER_LOGIC.md) can compute assignment values and
comparison operands inside the existing edit profile. Native field/write-format
gates are unchanged.
Text comparisons require an explicit host collation; ordinal comparison does not
claim Access locale fidelity. Checked Byte/Int16/Int32/Int64 narrowing supplies
Int32 assignments. NULL, nonintegral carriers and overflow fail rather than
truncate or silently coerce. Native changes additionally require existing non-NULL
fixed Int32 storage and every native rule/preservation gate. Indexed fields also
require the qualified [existing-tree Int32 profile](NATIVE_WRITES.md#existing-tree-int32-index-maintenance).

Parameters, multiple assignments, joins, subqueries, calls/aggregates, saved or
external sources, INSERT/DELETE and DDL remain unsupported for editing. Binding
rejects unavailable forms/names even on empty tables. All matching expressions
finish before one consuming backend call; a late error publishes no candidate.
A zero-match request still passes native gates. Snapshot-issued handles prevent
using positions from a different image. Matched rows include unchanged assignments;
the native plan separately reports changed rows. Neither count is a native Access
RecordsAffected equivalence claim.

Caller budgets accumulate across binding allowance, evaluation and backend checks.
The existing binder retains its hard byte/node/depth ceilings, with its maximum
retained allocation allowance precharged. Temporary parser/binder work is bounded
structurally rather than a strict allocator quota. Retrieval retains its existing
QueryLimits behavior; edit planning does not reset its supplied shared budget.
Cancellation is checked before/after bounded binding and between evaluation units.

## Fresh-copy CLI

```sh
tablecraft query update 'UPDATE Items SET Score=Score+7 WHERE Id=2' --database offline.mdb --output updated.mdb
```

The explicit input opens read-only under a shared OS lock and must be offline.
After full planning and report formatting, the CLI closes the source and publishes
a fresh complete candidate before printing matched/changed counts. Existing
outputs, including input aliases and destination symlinks, are never overwritten.
The source is unchanged. No
SQLite editing, VBA, macros or providers execute. CLI work ceilings are 256 MiB
cumulative bytes, 100,000 rows and 100,000,000 steps; native structural caps remain.

The [common fresh-copy publisher](ENGINEERING_COPIES.md#bounds-and-publication)
charges the complete output-byte allowance before creating an owned temporary in
the output directory. It writes the complete candidate, syncs the file and
publishes with `persist_noclobber`. Write, file-sync and publication failures clean
only that temporary and produce no success stdout. A source-close failure occurs
before publication. A later stdout failure may leave a complete published copy.
File synchronization and no-clobber publication supply no directory-entry or
power-loss durability guarantee. The CLI does not edit a live database in place
or activate a journal. Optional
[library journal integration](NATIVE_WRITES.md#optional-journaled-persistence)
remains the separate explicit in-place persistence boundary.

The separate [experimental engineering-copy commands](ENGINEERING_COPIES.md)
expose typed ACE UPDATE/INSERT/DELETE preparation on fresh outputs. They do not
change this normal Jet 4 query-editing profile, certify data-macro absence or
convert engineering candidates into qualified save/journal plans.

## References and evidence

Microsoft documents [UPDATE syntax and criteria](https://learn.microsoft.com/en-us/office/client-developer/access/desktop-database-reference/update-statement-microsoft-access-sql).
The implemented single-field profile is narrower than Access SQL.

Six query tests cover aliases, binding on empty inputs, checked assignments,
BETWEEN/IN/NULL behavior, explicit collation, late/eager errors and limits/cancellation
with a backend-call spy. Adapter integration drives SQL into a native plan. Three
CLI tests cover exact byte changes, read-only source preservation, rejected
statements/profiles, existing paths and reopening through query run. The independent
[update comparison](../../testing/compare_updates.py) confirms CLI and engine outputs
match byte for byte, reopen with Jackcess and retain native primary-index checks.
Microsoft Access comparison, general result-set updateability and broader write
profiles remain unqualified.

The shared-publisher extraction passes seven publication unit functions and two
normal Jet 4 host functions within the complete 28-function CLI unit run. The host
controls qualify independent source/expected images before planning, verify
source unlock before a single publication call, and inject write, sync and
publication failures without retry or visible output. The four existing CLI
update integration functions also pass in the current regression run (14 tests
across three CLI integration targets).

## Explicit StoredScalar consumer

The separate [SQL engineering-copy command](SQL_ENGINEERING_COPIES.md) uses
`AccessEngineeringEditor` and the opt-in `StoredScalar` profile. Nullable NULL
and exact non-Int32 stored carriers are accepted under shared descriptors; Int32
retains checked integral assignment. Normal `NativeEditor`, `query update`, the
existing Int32 profile and its ordinary ACE refusal are unchanged. The new command
returns only an experimental, non-journalable copy under explicit native modes.
