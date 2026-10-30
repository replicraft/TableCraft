# Fixed UPDATE DATA/INDEX ownership

Fixed UPDATE preparation now refuses ordinary DATA pages also claimed by a
physical INDEX owner. The original defect was reader-valid: table rows, logical
indexes and constraints could check successfully while an INDEX-owned map also
listed the DATA page. Existing Standard fixed planning then changed that page.

The shared read-only guard derives DATA membership from the immutable source and
checks every local table's authoritative physical INDEX map, including map
containers and descriptors without logical aliases. It grants no map rewrite or
allocation authority and does not interpret another table's keys. Source map
allocation, metadata reads, comparisons and cancellation use the caller budget.
The definition owner supplies physical descriptor offsets; sorted membership from
the existing allocation reader supports the paid binary searches.

The guard runs before patches in normal Jet 4 fixed planning, ACE Int32 previews,
fixed single updates and each selected nonallocating batch table. A named group
with no assignments still checks its source. An entirely empty group list has
no target table and retains its exact no-op behavior. Existing preserved Boolean
and allocating paths keep their separate stronger source checks. Required rules,
key-profile limits, normal ACE save admission and non-journalable preview types
remain unchanged. No stored content or external provider executes.

The independent SOURCE gates ran before the finite mutation probe: nine healthy
original/complete expected pairs plus nine reader-valid bad originals (27 views),
then three selected-orphan originals. They cover Jet 4 and both ACE 16 header
profiles, same-table claims, an index belonging to another local table and an orphan
physical descriptor in another table. Healthy expectations change only the known Score payload
byte and its key byte. Original fixtures and expected images stayed frozen.

The pre-fix probe first matched all 42 healthy complete images, then recorded all
bad outcomes before asserting: 42 body-overlap calls were accepted; 14 selected-
orphan calls already refused with `IndexedColumn`. An early static inference
missed the downstream physical/logical coverage check, and the probe initially
guessed `ConstraintCoverage`. Only that diagnostic expectation was corrected;
there was no second maintenance acceptance defect and no new orphan rule.
After the fix, all 42 body calls refuse with `RowLayout`; the 42 healthy images
and 14 established orphan refusals pass. The original acceptance diagnostic also
passes. The [execution record](results/ace16-fixed-index-body-ownership-2026-10-30.json)
retains the source gates, first acceptance failures and corrected results.

Regression checks pass: 541 native mutation functions (one existing ignored
helper), seven larger-native functions in the combined working tree, Access 69,
and CLI 86. Native strict/minimal, formatting, links and boundaries pass. The first
CLI regression invocation named a nonexistent target and ran no tests; only its
corrected run supplies CLI evidence. Existing exact-pool/final-cancellation tests
exercise the shared guard. An additional capture-through-finish normal-Jet-4/ACE-
batch control on the frozen other-table-index source passes exact pools,
one-below quotas and final cancellation. The full causal matrix and strict test
check also pass after that test-helper addition.
This correctness fix establishes ownership exclusion within current bounded
profiles, not native Access save compatibility or complete database integrity.

The isolated staged P0 source tree also compiles and passes both SOURCE gates,
the complete causal matrix and the quota/cancellation test in an initially
absent target directory. All twelve code blobs match that snapshot. This checks
the fix independently of pending larger-image changes; debug information was
disabled for this separate build and incremental compilation kept its default.
