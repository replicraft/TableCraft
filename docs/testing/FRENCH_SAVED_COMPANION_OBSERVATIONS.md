# French saved-source companion observations

The [compact receipt](results/french-saved-companions-2026-10-26.json) records two
new companion files with one inert FrenchFields SELECT each. All 32 controls
agree: four syntax-only reconstruction controls, 24 successful query executions
and four intentional buffered LIKE refusals. Executions compare 568 rows and 936
cells. This is headless engineering evidence over actual native metadata/fields,
not native Access SQL/NLS, save/reopen or ordinary writer qualification.

## Separate identities and inert metadata creation

[FrenchSavedCompanion.java](FrenchSavedCompanion.java) follows the existing
repository-owned [SavedSelectFixture.java](SavedSelectFixture.java) recipe and
[SavedSelectOracle.java](SavedSelectOracle.java) getter. It copies an unchanged
[field original](FRENCH_COMPARISON_FIELD_OBSERVATIONS.md) to a fresh path, then
uses public MSysObjects/MSysQueries table maps and the pinned QueryImpl.Row /
QueryFormat metadata helper API. These are version-specific reviewed JAR APIs,
not a general query DDL contract. No upstream implementation was copied.

The reviewed Jackcess 5.0.4 artifact remains SHA-256
`19a7494a56f439a57fda85c059b019b665802085cec5ae8186d1e028cc1bdfbc`.
Expressions are disabled, UTC is explicit and linked opens throw. The generator
selects the actual existing Tables/type 3 namespace, collision-checks a fresh
query ID and permits explicit system-catalog ID insertion through the public
AutoNumber-insert setting. User fields have no AutoNumber. It never patches a
header/descriptor/flag or edits an accepted original.

Actual getters report a visible SELECT named FrenchFields, object ID
-2147479648, parent 251658241, type 5/flags 0, no parameters, owner-access option or
query properties. Eight fragments contain only START, TYPE SELECT, flags 0,
local Comparisons/src source, three ordered field projections and END:

```sql
SELECT [src].[ID], [src].[FrenchShort], [src].[FrenchMemo]
FROM Comparisons AS src;
```

There is no action, UDF, remote source or stored executable content. The Java
producer/getter reconstructs metadata but does not execute SQL. The separate
[capture wrapper](capture_french_saved_companions.py) checks this definition
before any CLI reconstruction/execution.

## Original table and payload preservation

Both original files remain byte-identical. All 76 original rows and 304 complete
Text/Memo cells retain their exact descriptors, properties, values, locations,
index metadata and order in the companions, including 20,000-unit Memos.
A separate raw-byte proof preserves 285 complete original pages: the actual user
definition/data/index-root pages plus every original LVAL-signature page.
Protecting extra raw-signature pages does not assert global ownership semantics.

V2016 protects 143 pages and V2019 protects 142. Both companions add one metadata
page and change the same existing page numbers: 1, 2, 4, 7, 8, 11, 12, 17. None is among
the protected user payload pages. Whole-file equality with the source is not
claimed; new metadata necessarily differs. Full page numbers/hashes and complete
field getters remain in ignored hash-pinned captures.

## Reconstruction and explicit execution

Frozen CLI `596d40ea41a4bafefbd5b34af4ef35a8a4e2cde4`, SHA-256
`bcff00ad245c7a70582db222fa48e5eb2f796bd944d5ccbeefe759fbd010a576`,
is identified by its [build receipt](results/french-comparison-cli-build-2026-10-26.json).
All 53 selected compiled objects are checked against that immutable Git revision,
independently of later engine changes.

For each profile, the independent JAR SQL is normalized through the existing
syntax-only parser, then compared with native `query saved` reconstruction.
This separates native fragment assembly from query execution and does not
independently qualify the parser itself. Seven execution cases run in ordinary
and declared arithmetic modes on both companions:

- Direct `--saved FrenchFields` returns full ID/FrenchShort/FrenchMemo values.
- Named Short sort and 20k Memo sort retain the French field policy.
- Derived aliases over each named source retain that policy and order.
- Named Short GROUP/Count retains the expected classes/first payloads.
- French field LIKE refuses with status 2 and empty stdout.

Expected values reuse the [accepted independent Short/Latin models](results/french-comparison-fields-replay-2026-10-26.json),
not CLI-derived answers or truncated long keys. The 20k accent order distinguishes
French field policy from the actual General operation defaults. Separate native
and comparison allowances are explicit: 64 MiB and 10 million steps for each pool.
No defaults are changed. All original and companion hashes match before/after.
There were no failures, selector changes, dropped columns or retries.

[compare_french_saved_companions.py](compare_french_saved_companions.py) is an
import-safe bounded runner. The repository ledger records these new identities;
the original 112 corpus and its absence of saved definitions remain historical
facts, not a claim that named queries are unimplemented. The new proof qualifies
only this existing saved-SELECT/native-source composition under the provisional
comparison model. It adds no normal engine/CLI query-writing API or macro authority.
