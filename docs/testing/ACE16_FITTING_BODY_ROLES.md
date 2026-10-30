# ACE 16 fitting UPDATE body ownership

Fitting mixed UPDATE previews now qualify complete original ownership before
changing records or INDEX bodies. A reader-valid source that claims ordinary
DATA through an INDEX-owned map refuses with `Unsupported(RowLayout)`.
This corrects a reproduced preservation defect in the earlier fitting route.
It does not change normal ACE save permission or required data-macro handling.

## Nonallocating protection

The [metadata owner](../../crates/engines/tablecraft-ace/src/table/mutation/variable_split/metadata.rs)
provides a distinct fitting capture. It requires complete catalog ownership
framing, original ordinary-owned map exclusivity and every affected physical
INDEX owner's exact identity. An affected tree's reachable pages must equal its
complete owned-map inventory. Aliases are checked before physical deduplication.
The existing independent raw tree inventory is shared with the split auditor.

Original logical-root and final forwarded-payload DATA pages must remain owned
and exclusive through the existing record resolver. Metadata protection also
rejects those pages when claimed by INDEX, column-owned LVAL, foreign DATA,
definition or map storage. Affected INDEX pages must be disjoint from every
ordinary DATA claim, column-owned LVAL claim, other INDEX owner and metadata
storage. The source GLOBAL bitmap must mark selected ordinary DATA and affected
INDEX pages allocated; a live-body/free-bit contradiction refuses.

Fitting does not require a candidate hint or grant allocation authority. GLOBAL
and candidate map rewrite roles are not selected. After source exclusivity is
checked, the fitting capture clears its rewrite-role list, so even an exact
original owned-map record cannot be rewritten through that capability. The
existing allocating capture retains its actual map-rewrite behavior.

Both the [producer stage](../../crates/engines/tablecraft-ace/src/table/mutation/fitted/stage.rs)
and the [independent record proof](../../crates/engines/tablecraft-ace/src/table/mutation/fitted/record_proof.rs)
reacquire protection from the immutable original. Producer caches do not supply
proof authority. Existing complete-record, exact original-value, joint-key,
constraint, budget and cancellation checks remain in place. Additional catalog
and tree inspection is paid work; identical historical resource costs are not
claimed.

## Reproduction and fixture qualification

The independently authored causal source has 64 ordinary rows, two physical
INDEX owners and aliases. Its healthy complete candidate matched the frozen
expected image. A variant changes one INDEX-owned bitmap bit to claim DATA page
12; ordinary read, INDEX and constraint checks still pass. Before the fix, the
fitting preview accepted it and exactly matched the aliased expected image.
That first failure is retained. After the fix, the healthy image stays exact
and the alias source refuses. A separate direct control proves that exact map
records writable by an allocating capture are denied by a fitting capture.

The first old fitting regression run passed 20 tests and failed seven with
`Unsupported(RowLayout)`. A read-only diagnostic established that their old
sources pass scalar/INDEX admission but have incomplete zero-filled ownership
list tails: definition pages 2, 7 and 10 at offsets 398, 449 and 663. These are
missing ownership declarations, rather than a candidate-hint requirement or
GLOBAL free-bit contradiction.

Those original constructors and expected-image bodies remain unchanged. The
new [pure fixture derivative](../../crates/engines/tablecraft-ace/src/table/mutation/mixed_fitted_tests/framed.rs)
clones each original and expected image and changes exactly six bytes: an
explicit `FF FF` empty-list terminator at each of the three known tails. It
asserts the original zero suffixes and complete changed-offset list. All 43
pairs, including the four zero-slack cases, passed read, INDEX, constraint and
full fitting-ownership checks before any derived mutation API ran. The old raw
sources retain read coverage and explicit active/empty mutation refusals.

Native positive, forgery and budget controls now visibly consume those framed
derivatives. Access and CLI tests include the same pure fixture owner; their
43-pair SOURCE gates passed before their positive mutation fixtures migrated.
No historical oracle was repaired from a producer result.

## Validation

The corrected native fitting namespace passes 30 tests. This includes the causal
ownership controls, historical read/refusal checks, all framed complete-image
comparisons, exact budget boundaries, cancellation and correlated record/key
forgeries. Access passes 58 library tests. CLI passes 37 unit tests, 25 query
UPDATE integration tests and six typed-write tests. Strict all-target/all-feature
ACE/Access/CLI checks and minimal ACE/Access checks pass.

The broader native mutation regression passes all 529 tests. See the [structured result](results/ace16-fitting-body-roles-2026-10-30.json)
for chronology and exact source pins. This is engineering-preview preservation
evidence; native Access comparison, journalability and normal ACE saves are not
qualified by these tests. The [fitting service contract](ACE16_FITTED_MIXED_UPDATES.md)
and existing growth services retain their distinct allocation scopes.
