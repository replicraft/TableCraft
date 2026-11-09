# Clone ownership audit — 9 November 2026

Every explicit non-test `.clone()` in the tracked workspace was reviewed against
the original source at `732b46b3`. The census covers ordinary code, conditional
platform/feature modules, examples and benchmarks. It follows actual test module
boundaries rather than excluding files merely by name. Testkit fixtures/support
and actual test bodies are test code and are outside this request.

The [per-call record](../../tests/results/general/clone-ownership-audit-2026-11-09.json)
contains **707 production/example/benchmark calls**, one rustdoc example and
16 additional associated/iterator clone forms in the hosts. All baseline calls
reconcile with the parser/module inventory and macro-body cross-check. Every
remaining explicit call has an individual ownership decision and a `NO_CLONE`
explanation in source. Shared alternatives are considered before retaining a copy.

## Changes

- Borrow comparison-only UI reply targets; callers no longer copy identities
  solely to enter a setter. Filter lookup compares borrowed revision/table/ID
  fields instead of allocating a temporary presentation target.
- Move owned SQL token strings into the parser model. Resolve a borrowed field
  reference directly without constructing a temporary copied expression.
- Use Jiff's borrowed `TimeZone::to_datetime` conversion. This corrects the earlier
  conclusion that the CLI needed an owned zone to obtain civil time. CLI casing
  controls also borrow their policy. No explicit production CLI clones remain.
- Use `Cow<str>` for text-edit candidates, including new-database paths, draft
  names/descriptions and QBE field/alias cells. Egui borrows unchanged text and
  detaches on actual input; rejected changes retain the original owner and limits.
- Share the immutable submitted Query Design draft with `Arc` across the action,
  UI pending state and runtime command. The mutable editor remains independent.
  Full revision/content checks remain, and logical retention still charges the
  complete draft plus out-of-line/control storage. Tests check shared pointer
  identity, separate editable storage, retention and cancellation.
- Make payload-free `QueryError` a `Copy` type. Sixteen guard clones become normal
  value copies. This is a representation correction, not a heap-memory saving.
- Remove final-use copies, repeated readonly labels and unnecessary shared-handle
  increments. Native range copies are identified as bounds rather than byte copies.

The current source contains **596 explicit non-test calls**. Of the 111 fewer
method calls, 109 were removed, one deep copy became an `Arc` handoff, and one
necessary owned name now uses `to_owned()` after its redundant earlier copy was
removed. That last spelling change is not counted as a saved copy. The count is
not an allocation measurement: fixed carriers, iterator state and shared handles
have different costs from owned text/buffers. Other edit buffers already using
`to_owned()` also gained conditional ownership without affecting this census.

## Retained ownership and comments

Borrowing/moving is preferred when an owner need not survive. `Arc` fits immutable
submissions crossing the worker boundary; `Rc` cannot satisfy that boundary.
`Cow` fits synchronous edits with a retained original. It cannot keep a reusable
prepared plan independent of the caller's borrowed AST without acquiring ownership.
Mutable rollback candidates and independently published metadata retain their
actual separate values. Reference counting around allocation-free ranges, cursors
or scalar cells would introduce overhead without sharing a backing payload.

Native images already use shared `Bytes` storage. The deliberate first clone/drop
materializes lazy control storage inside its charged allowance; it copies no
database payload. Subsequent cursor clones retain that shared owner. Locale,
egui context, galley and worker coordination handles also already share storage.

Each justification begins `// NO_CLONE:` above the complete statement, field or
match arm. Comments do not split declarations, method chains or argument lists.
The [repository guidance](../../AGENTS.md#working-practices) requires this format
and forbids disguising equivalent copies with another spelling to reduce a count.
The source census and per-file retained ledger reconcile exactly; marker presence
alone is not the semantic justification.

## Verification

On macOS 14.7.2 ARM64, the consolidated workspace all-feature library/binary batch
passed **4,586 tests** across 20 executables, with **265 existing ignored** and no
failures. Eight query integration targets passed **121 cases**; nine CLI targets
passed **84**. The affected collection-members target subsequently passed **37**;
the final QBE-focused continuation passed **25** repeated UI cases. The SQLite
rustdoc example passed its one test. Repeated cases are not added to the unit total.

Strict workspace/all-target/all-feature Clippy, strict workspace rustdoc, the
offscreen example build, formatting, module layout and dependency boundaries pass.
Earlier compiler/lint attempts and their corrections remain in the evidence
record rather than being reported as green runs. Two light/dark offscreen frames
exercise initial Design descriptions, preserve authored Unicode text and were
directly inspected at 1280 × 820 logical points/1×.

Headless-only verification is reinstated. No native app windows were opened in
this follow-up. These results do not qualify Windows/Linux execution, Microsoft
Access parity or measured allocator/RSS/performance improvements. The audit covers
explicit cloning; it does not certify every implicit allocation or manually copied
buffer elsewhere in the program. No production dependencies or write profiles changed.
