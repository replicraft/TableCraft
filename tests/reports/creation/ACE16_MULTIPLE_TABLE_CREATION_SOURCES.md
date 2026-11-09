# Multiple-table creation sources

Four independently authored header-5 originals exercise shared catalog and map
storage without invoking a production creator. They retain the qualified
single-table system envelope, definitions and records, with explicit per-table
owner and map references.

| Case | User tables | Rows | Purpose |
| --- | --- | --- | --- |
| two-empty | 2 | 0 | Different fixed and variable schemas |
| populated | 2 | 5 | Shared column names and distinct table-local row identities |
| candidate-bits | 2 | 58 | Full fixed table beside an empty variable table; distinct free-space hints |
| sixteen | 16 | 0 | Maximum proposed table count and complete catalog/map capacity |

Two-table files have eleven pages; sixteen tables have thirty-nine. The first
sixteen map records retain the original system, first-user and catalog LVAL
roles. Each additional table appends its own owned/candidate pair. Definitions
store exact references; no reader infers roles from a table-count formula.
Every user table owns a separate DATA page and has its own declared count.

The native gate passes all four original images, twenty-two table selections and
sixty-three rows, including full source metadata checks and exact unchanged-byte
comparisons outside the explicit relocated fields. The fresh exporter uses
create-new files and verifies complete readback.

A separately frozen read-only SDK observer enumerates every user and system
table and attempts all four metadata APIs. All four first calls pass without
changing original bytes. The independent saved comparator reconstructs all four
complete files and matches the entire ordered inventories, exact values and raw
record references. Source and reader templates remain distinct from producer
outputs; earlier single-table evidence is unchanged.

The [source record](results/ace16-multiple-table-creation-source-2026-10-31.json)
pins complete hashes, source/observer inputs and separate validation phases.
These are repository-authored fixtures under the repository license. This phase
does not implement multiple-table creation, qualify native Access saves or grant
normal imported-file write, macro-absence or recovery permission.
