# Bounded query editing

`tablecraft_query::plan_update` consumes a backend-neutral `EditableTable` and
owned SQL query, returning an `UpdatePlan` with matched-row count and a concrete
backend candidate. It imports [DB contracts](DB.md), never concrete engines or
adapters. Planning performs no durable writes. The Access adapter delegates native
checks/preservation to [ACE](NATIVE_WRITES.md).

## Supported profile

One local table, optional alias and one SET assignment to an Int32 field are
supported. Qualified/unqualified references use the existing ASCII-insensitive
binder. Pure scalar expressions, BETWEEN, IN lists and three-valued WHERE reuse
retrieval evaluation, including eager operand errors. Only Boolean TRUE selects a
row; final numeric predicate coercion belongs to read-only retrieval. Shared
[exact integer operators](INTEGER_LOGIC.md) can compute assignment values and
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
After full planning, the CLI creates a new output with create_new, writes/syncs
the complete candidate and prints matched/changed counts. Existing outputs,
including the input path, are never overwritten. The source is unchanged. No
SQLite editing, VBA, macros or providers execute. CLI work ceilings are 256 MiB
cumulative bytes, 100,000 rows and 100,000,000 steps; native structural caps remain.

A write/sync failure after creation can leave an incomplete output and returns
a diagnostic with no success stdout. The CLI neither promises atomic output
publication/durable directory entries nor edits a live database in place. Optional
[library journal integration](NATIVE_WRITES.md#optional-journaled-persistence)
remains the separate explicit in-place persistence boundary.

## References and evidence

Microsoft documents [UPDATE syntax and criteria](https://learn.microsoft.com/en-us/office/client-developer/access/desktop-database-reference/update-statement-microsoft-access-sql).
The implemented single-field profile is narrower than Access SQL.

Six query tests cover aliases, binding on empty inputs, checked assignments,
BETWEEN/IN/NULL behavior, explicit collation, late/eager errors and limits/cancellation
with a backend-call spy. Adapter integration drives SQL into a native plan. Three
CLI tests cover exact byte changes, read-only source preservation, rejected
statements/profiles, existing paths and reopening through query run. The independent
[update comparison](testing/compare_updates.py) confirms CLI and engine outputs
match byte for byte, reopen with Jackcess and retain native primary-index checks.
Microsoft Access comparison, general result-set updateability and broader write
profiles remain unqualified.
