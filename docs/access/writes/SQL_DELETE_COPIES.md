# SQL-selected DELETE engineering copies

`tablecraft query preview-delete` selects original rows with a bounded SQL DELETE
and prepares one fresh experimental ACE 16 copy. It consumes the separate neutral
`DeletableTable` contract and the existing engine deletion services. It does not
enable normal ACE saves, certify data-macro absence, execute active content or
convert a candidate to a journalable `NativeEditPlan`.

```sh
tablecraft query preview-delete 'DELETE FROM Items WHERE Id BETWEEN 1 AND 3' \
  --database offline.accdb --output remaining.accdb

tablecraft query preview-delete 'DELETE p.* FROM Parents AS p WHERE p.Id=1' \
  --database offline.accdb --output cascaded.accdb --cascade-deletes
```

These commands require a source admitted by the existing scalar reader and the
selected native deletion profile. An arbitrary readable Access table is not
necessarily eligible. In particular, this consumer captures a complete scalar
snapshot; a table with unsupported attachment or calculated-field storage remains
refused even when the predicate references only an ordinary field.

## Selection from the original snapshot

The syntax permits one local table, an optional alias and a pure WHERE predicate.
`DELETE FROM`, `DELETE * FROM` and one exact exposed table wildcard are accepted.
When the source has an alias, that alias is its exposed qualifier. Field names
use the existing ASCII-insensitive binding policy. Predicates reuse field
references, literals, unary/binary operators, IS NULL, BETWEEN and IN lists.
Only Boolean TRUE selects a row; FALSE and NULL skip it. Text predicates require
an explicit `--collation ordinal`; no ambient locale is chosen.

The closed [deterministic mutation scalar functions](../../query/MUTATION_SCALAR_FUNCTIONS.md)
are also available, with existing eager argument semantics and explicit
stored-result/comparison policies. Parameters, joins, derived or external
sources, multiple targets, unlisted calls, aggregates and subqueries remain
unsupported. Local-target syntax is checked
before opening a file. Actual field binding still occurs against the source,
including when it has no rows. Eager expression errors retain their existing
behavior. Every selected handle is collected before one consuming backend call;
a late predicate error produces no deletion candidate.

The neutral planner supplies opaque handles issued by this exact immutable
snapshot. `AccessEngineeringDeleter` validates their identity and resolves their
original live-row ordinals. The existing `prepare_ace16_row_deletes` service then
captures complete original record expectations from the same native image.
Users supply neither record hex nor persistent row identifiers. Reconstructed
snapshots cannot reuse old handles, even when their bytes and table names match.
Repeated or unavailable selected rows refuse the complete operation.

## Explicit native action policy

The default selects the existing explicit-delete service with final relationship
membership checks and its current source-admission rules. It does not silently
expand declared actions. `--cascade-deletes` explicitly selects the existing
qualified CASCADE DELETE and SET NULL closure, including its supported downstream
update cascades. Unsupported actions, source profiles and cycles retain their
existing refusals. There is no retry under another mode and no per-row candidate
chain. See the [DELETE SET NULL contract](../../testing/ACE16_SET_NULL_DELETES.md)
and [NULL-generated update cascades](../../testing/ACE16_DELETE_NULL_UPDATE_CASCADES.md)
for the underlying engine boundaries.

Even zero matches invoke native source qualification. The report distinguishes
original WHERE-matched rows from deleted rows in the complete candidate. Cascade
deletions can increase the latter; surviving rows changed by SET NULL do not
become deleted rows. These counts do not claim native Access RecordsAffected
equivalence.

## Bounds and publication

Defaults are 256 MiB cumulative bytes, 100,000 rows and 100,000,000 steps, adjustable
with `--work-bytes`, `--work-rows` and `--work-steps`. Parser, snapshot, engine and
complete-image limits remain independent. Snapshot evaluation, handle storage,
native record capture, report formatting and output all consume the same
operation allowance. Library callers supply cancellation; the CLI does not
install an OS signal handler.

The host uses the original raw read guard and refuses encrypted sources and
deployment packages before image capture. A password does not authorize a
plaintext replacement. It fully prepares the result and report before the
[shared fresh-copy publisher](ENGINEERING_COPIES.md#bounds-and-publication).
Complete output Bytes are charged before an owned temporary is created; the
complete file is synced before no-clobber publication. The source and existing
destinations are never replaced. A later stdout failure can leave the completed
copy. Directory-entry durability, power-loss recovery, in-place editing and
journal activation are not supplied by this command.

## Verification state

The two adapter source-only functions pass before any DELETE API claims. They
qualify independent original and complete expected images for both supported
ACE 16 header profiles: eight ordinary empty/single/multiple/all selections and
four relationship-bearing cascade outcomes. Expectations use authored original
slot flags, live counts, original eligible INDEX entries and untouched byte
ranges, without deriving bytes from a candidate.

Both CLI source-only gates and all four neutral DELETE planner functions pass.
All five adapter DELETE functions, three CLI handler functions and four executable
integration functions also pass their first actual API runs. The adapter functions
pass within its complete 36-function library run; the handler functions pass within
the complete 31-function CLI unit run. A first CLI compile stopped before tests because a test's
`unwrap` required Debug on a production error type; the test now reports its
existing Display text without adding a payload-debug surface. Source and expected
images were unchanged. Focused controls cover original-handle identity,
duplicate selections, late/eager predicate failures, budgets/cancellation,
Restrict refusal and explicit cascades. No Microsoft Access execution or native
save comparison is claimed.

The executable integration target
[`query_preview_deletes.rs`](../../../apps/tablecraft-cli/tests/query_preview_deletes.rs)
adds one passing source-only gate and three passing executable workflow functions. Its six
complete expected images cover selected, no-op and all-row deletion in both header
profiles. The executable controls also cover alias binding, existing/source output
refusal, source preservation, late predicate errors and encrypted-source refusal
with or without the correct password. They reuse the actual authored native and
encryption fixture owners; the source gate validates the encrypted control by
decoding it before any command invocation.

Broader current regression, strict lint and minimal-feature checks remain pending
the final combined round. A later unrelated stale UPDATE integration expectation
stopped that combined CLI round after the DELETE and multi-assignment targets had
passed; it does not replace or invalidate the focused DELETE outcomes above.
