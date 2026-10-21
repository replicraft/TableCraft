# Native saved UNION reads

The Access adapter now reconstructs saved UNION metadata through
`reconstruct_saved_read`, returning `SavedRead` with unchanged native identity and
fragments plus TableCraft-owned query syntax. SELECT dispatch reuses the existing
[SELECT compiler](NATIVE_SAVED_QUERIES.md#owned-select-reconstruction).
`SavedReadError` and `NativeSavedReadError` distinguish adaptation and native-read
failures. Existing SavedSelect/error type names remain compatibility aliases;
`reconstruct_saved_select` and `NativeSource::read_saved_select` still reject UNION.

`NativeSource::read_saved_query` reads SELECT/UNION definitions under the same
cumulative native budget/cancellation controls as later source reads. Its named
query cache exposes these definitions only when explicitly referenced. Binding
retains the existing parameter scopes, cycle detection, planning limits and lazy
user-row reads. No inspection/open operation executes a saved query.

## Stored profile

The query-object family is 0x80 under the existing 0xf0 family mask, and a single
required Type row must contain 9. Two Table rows contain SQL in Expression and
exact Name2 labels `X7YZ_____1` and `X7YZ_____2`. Labels select branch order;
physical row order does not reverse their meaning. Duplicate/missing/unknown
labels, extra Name1 values and nonzero table flags fail rather than being ignored.

The optional Flags row uses bit 0x02 for duplicate-removing UNION; absence, NULL or
zero means UNION ALL. Bit 0x04 preserves owner-access syntax without granting
permission. Other bits or payload fields fail. Optional Start/End rows have no
payload and must be unique. Unknown/NULL tags, unsupported clauses and unused
non-NULL extras/names are rejected. Query fragments are never silently repaired.

Ordered native parameter rows reuse the SELECT declaration/type/length checks.
Ordered OrderBy rows reuse expression guards and accept only absent/A/D direction,
without an ignored second name or nonzero flag. Duplicate/reversed/non-four-byte
ordering keys fail. The original bytes and fragment order remain unchanged.

Each SQL part is parsed in a UNION wrapper with newline boundaries. The wrapper
admits SELECT, TABLE and named-query operands while rejecting embedded declarations,
actions, statement terminators, final sorting/owner clauses and unmatched syntax.
A trailing line comment cannot swallow the inserted UNION or ordering syntax.
The complete composition is then parsed under combined parser byte/token/depth
limits. Mixed UNION/ALL chains retain textual left-to-right association; individual
branch strings may themselves contain UNION chains. Native sort/owner metadata is
assigned separately to owned syntax, followed by explicit parameter declarations.
All temporary text/AST allowances, metadata rows and loops consume shared budgets;
cooperative cancellation is checked before publication.

Read expressions can retain unresolved calls and parser-supported source syntax.
That does not make UDFs, external sources or unsupported expressions executable.
Action, DDL, pass-through, crosstab and unknown native families remain outside this
entry point. Reconstruction is not a writable query or native shared-file snapshot.

## CLI and query semantics

```sh
tablecraft query saved --database copy.accdb 'UnionAll'
tablecraft query run --database copy.accdb --saved 'UnionParameter' --param Floor=long:2
tablecraft query run 'SELECT * FROM UnionAll' --database copy.accdb
```

`query saved` buffers normalized SQL without binding or execution. `query run`
uses the existing bounded read executor and output buffering, including explicit
collation/wildcard policies. Reconstructable owner-access/UDF queries still fail
execution. Named dependencies, including bare UNION references, use the same
metadata cache and cycle checks; there is no automatic startup/provider activation.

Execution keeps existing UNION limits: equal output widths, first-operand names,
no implicit union-wide carrier promotion and explicit Text comparison policies.
Those limits and wider Variant/native behavior remain acceptance gaps, rather
than being supplied by native metadata reconstruction.

## Evidence

Five authored adapter tests cover flags/labels, mixed chains, parameters, ordering,
SELECT-only compatibility, four header layouts, same-source/named execution,
metadata/statement refusals, budgets/parser ceilings and early/late cancellation.
Two CLI tests cover inert printing, explicit/named execution and buffered owner,
UDF, cycle and action refusals. Existing SELECT behavior remains regression-tested.

The original [fixture recipe](testing/SavedUnionFixture.java) creates fourteen
inert definitions on fresh Jet 4 and ACE 2007/2016/2019 files with expressions
disabled. The [read-only oracle](testing/SavedReadOracle.java) uses Jackcess's own
SQL renderer; [the comparison](testing/compare_saved_unions.py) checks normalized
assembly, saved/direct/named execution, getter-derived expectations and malformed
metadata on copies. [FIXTURES.md](FIXTURES.md#saved-union-inputs) records hashes.
The comparison passed 268 controls with original sources unchanged. Jackcess never
executes SQL here; these are independent storage/assembly/model results. Native
Microsoft Access parsing/execution, versions and source files still need Windows
qualification, including wider native fragment layouts and type promotion.

References: [Microsoft UNION syntax](https://learn.microsoft.com/en-us/office/client-developer/access/desktop-database-reference/union-operation-microsoft-access-sql),
[Jackcess UNION metadata](https://github.com/jahlborn/jackcess/blob/master/src/main/java/com/healthmarketscience/jackcess/impl/query/UnionQueryImpl.java)
and [format constants](https://github.com/jahlborn/jackcess/blob/master/src/main/java/com/healthmarketscience/jackcess/impl/query/QueryFormat.java).
The reviewed 5.0.4 source artifact supplied layout facts; no upstream code/data
was copied into this feature.
