# DELETE-generated NULLs and UPDATE cascades

With `ace16-write-preview`, the separate
`NativeImage::preview_ace16_cascading_deletes` service now follows established
`ON UPDATE CASCADE` relationships when a DELETE-generated SET NULL changes a
surviving referenced parent key. It extends the
[atomic DELETE and SET NULL preview](ACE16_SET_NULL_DELETES.md); the public
entry point and non-journalable candidate type are unchanged.

This is engineering candidate preparation. It does not authorize normal saves,
journal application, required data macro execution or native Access transaction
semantics. Microsoft’s [Access CONSTRAINT contract](https://support.microsoft.com/en-us/access/constraint-clause)
describes both DELETE SET NULL and UPDATE CASCADE. Their established native
declarations are used here. This continuation does not invent or admit a stored
`ON UPDATE SET NULL` encoding, nor establish native SQL mixed-action timing.

## Two closures over original associations

The service first closes the existing DELETE Cascade graph and fixes the complete
set of deleted logical roots. DELETE SET NULL then clears foreign-key presence
bits on matched surviving rows. Opaque fixed payload bytes remain unchanged.
A deleted child receives no NULL assignment.

Effective surviving NULL cells activate outgoing UPDATE relationships. Parent
and child matches use complete original tuples and original row identities.
Changing one parent component propagates NULL only to its corresponding child
component; unchanged components retain their values. This differs from DELETE
SET NULL, which clears every component of the matched foreign key.

NULL facts are monotone. Identical demands coalesce, already-NULL original cells
add no effective fact, and propagation stops when no new cell is added. A final
pass compares each activated surviving Cascade pair’s entire final tuple. An
incompatible NULL demand on an otherwise unchanged component returns `Conflict`;
an all-NULL final key does not bypass this action consistency check. Matching
never moves a child to a different parent after an earlier assignment.

Deleted parent and child endpoints suppress UPDATE propagation and its final
association check. Tombstoning a parent dispatches its DELETE actions, rather
than an extra UPDATE action. `deleted_rows()` counts only removed logical roots,
excluding all surviving NULL changes.

## Nullable UNIQUE parents and immutable projections

UPDATE lookup permits a checked, ordered UNIQUE parent key, including nullable
alternate keys. It does not reuse DELETE lookup’s required-primary restriction.
Component types and Decimal precision/scale must match. The existing exact
integral, Currency, GUID and Decimal relationship-key profiles remain in force.

Original parent tuples that are entirely NULL are excluded from lookup; entirely
NULL child tuples remain exempt. Partial-NULL tuples are matched exactly.
Duplicate non-all-NULL original parent tuples refuse as ambiguous associations,
even if nullable UNIQUE metadata otherwise permits that source population.
Repeated all-NULL parents are not ambiguous lookup targets. Complete before-image
relationship and index checks remain mandatory. A genuine ordinary UNIQUE alias
must remain available where raw relationship inspection requires it; qualifying
foreign aliases during an index check does not manufacture that declaration.

The existing DELETE projection and its bound positions remain immutable. A
separate, lazy UPDATE projection reads only relationship-key fields when effective
NULL assignments activate that table. It verifies source identity, row alignment
and overlapping projected values. Native-column lookup then resolves original
expected values through those immutable facts. This avoids eagerly decoding
unrelated Memo/OLE fields or shifting positions in already-bound DELETE links.
Complete before/final validation still checks required payloads and the same
resource limits; the smaller redundant projection grants no new write eligibility.
The path with no effective NULL assignments creates no UPDATE projection or graph.

Each physical index receives one original-image transform for deleted entries
and changed surviving NULL keys. Logical aliases share that transform. Existing
retirement, sibling, map, required-key and final membership checks remain active;
no intermediate candidate or new page allocation is used. Qualified Ignore NULL
omission uses the DELETE consumer. Ordinary UPDATE Ignore NULL admission remains
unchanged.

## Restrict and cycle policies

UPDATE RESTRICT follows the existing fixed-update engineering contract. If an
original parent key changes and an original matching child survives, the operation
returns `ReferentialAction`, even when another Cascade or direct DELETE SET NULL
path independently NULLs that child. Explicitly deleting the child suppresses
that UPDATE association. This policy is independent of relationship iteration
order and is not replaced with a final-membership-only test.

DELETE Restrict retains the separate batch’s complete final-image membership
rule. These two rules must not be described as native SQL action timing.

The DELETE action graph keeps its existing acyclic, cross-table boundary. UPDATE
Cascade-only cycles use the existing bounded original-association fixed-point
policy: surviving NULL demands may converge, while final tuple or constraint
conflicts still refuse. This does not expand self/cyclic DELETE support or claim
native mixed-action cycle qualification.

Effective NULL cells share the fixed assignment ceiling. Deleted roots plus
distinct surviving rows with NULL changes share the delete batch row ceiling.
Table discovery, projections, original matching, sorting, deduplication and
propagation charge the caller’s work and allocation budget and check cancellation.
Failures publish no candidate.

## Independent validation

The repository-authored
[update-closure tests](../../crates/engines/tablecraft-ace/tests/ace16_set_null_delete_contract/update_closure.rs)
and their [literal model](../../crates/engines/tablecraft-ace/tests/ace16_set_null_delete_contract/update_closure/model.rs)
construct complete source and expected images before invoking the preview. Cases
cover both ACE 16 headers, chains, diamonds, convergent UPDATE-only cycles,
deleted endpoints, no matching descendant, downstream Restrict and required keys,
stale requests, budgets and measured terminal cancellation.

The separate
[partial-tuple tests](../../crates/engines/tablecraft-ace/tests/ace16_set_null_delete_contract/partial_tuple.rs)
and [model](../../crates/engines/tablecraft-ace/tests/ace16_set_null_delete_contract/partial_tuple/model.rs)
use two genuine physical indexes on the intermediate table. A single-field DELETE
relationship feeds a distinct reversed two-component UNIQUE UPDATE parent key.
Four complete source/expected images cover a retained non-NULL component and an
originally partial-NULL parent key. Their source gate passes before the API test;
the unchanged component and opaque source payloads remain exact.

The first new cycle source gate rejected an incomplete authored parent population
before any preview API ran. The fixture was corrected to supply genuine DATA,
index and counter entries for every original referenced key; source membership
checks were retained. No production behavior or expected result was adjusted to
accept an orphan.

The final focused integration batch passes all 22 functions. The Restrict-diamond
source/expected gate establishes that independently
NULLing a surviving child would satisfy final membership, while the action test
still requires `ReferentialAction`; deleting that child instead produces the exact
complete expected image. The full engine library passes 624 tests with two existing
generators ignored, and 76 existing integration regression functions pass across
ten targets. Those broader runs preceded the two additional Restrict-diamond tests;
production behavior did not change between runs. Strict all-target/all-feature
Clippy, strict minimal-library Clippy and workspace formatting pass. The
[verification record](results/ace16-delete-null-update-cascades-2026-10-28.json)
retains source hashes, actual test groups and the failed initial source gate.

This continuation has independent literal validation. The original-file comparison
in the preceding SET NULL document qualifies that earlier scoped producer; it does
not establish original-file or Windows Access qualification for these additional
UPDATE propagation paths.
