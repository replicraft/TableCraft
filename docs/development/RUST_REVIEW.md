# Rust source review — 9 November 2026

Historical baseline: the subsequent [265-rule review and fix batch](RUST_SKILLS_REVIEW.md)
implements the uncertain-save protocol described below and records its accepted
verification. The original scope, inventories and failed/unverified evidence here
remain unchanged.

This review covers reusable libraries and executable hosts under edition 2024,
MSRV 1.99, using the Beautiful Rust best-practices and review skills. It combines
repository-wide inventories, callsite and owner inspection, compiler/lint checks,
and focused behavioral tests. It is not a line-by-line proof of the whole codebase,
every allocation, every possible panic, or dependency/platform internals.

The most important remaining finding is the native save actor's handling of an
unexpected execution panic. Module consistency, several unnecessary allocations,
read-worker failure reporting, thread startup and shutdown races were corrected.

## Must fix

### Native save actor needs an uncertain-outcome exit protocol

Location: [`editing/native/save.rs`](../../crates/application/tablecraft-runtime/src/editing/native/save.rs),
the task dispatch and subsequent report publication; see also
[`SaveReport::stopped_before_target_writes`](../../crates/application/tablecraft-runtime/src/editing/save.rs).

**Rule:** Beautiful Rust best-practices §5 and §7; Rust review §3, terminal error
handling and ownership across worker failure.

**Problem:** An unexpected panic during native apply/recover/release can terminate
the actor before it clears the active mailbox and publishes a report. Filesystem
visibility may already have changed, and unwinding can release native resources.
A constructor startup refusal or a failed read is a different event.

**Required fix:** Add an ownership-aware, exact-operation terminal health contract.
Refuse further operations on the failed actor and surface uncertain visibility
without inventing a rollback, retained lease, successful recovery or safe retry.
Preserve enough recovery information for the existing explicit reopening flow.
A panic must never be mapped to a generic failure with `state: None`: the existing
before-write classifier uses that state as evidence that setup did not begin.
The eventual regression must include this requirement:

```rust,ignore
assert!(!uncertain_report.stopped_before_target_writes());
```

This protocol is **not implemented by this review**. Native execution was not
wrapped in a catch-and-continue handler or classified as a no-write failure.

## Correctness findings fixed

### Worker startup and shutdown

Locations: runtime [`thread_start.rs`](../../crates/application/tablecraft-runtime/src/thread_start.rs),
[`task_signal.rs`](../../crates/application/tablecraft-runtime/src/task_signal.rs),
[`worker.rs`](../../crates/application/tablecraft-runtime/src/worker.rs),
[`creation/worker.rs`](../../crates/application/tablecraft-runtime/src/creation/worker.rs)
and the native save actor.

**Rule:** Beautiful Rust §5/§7: expected OS failures are fallible; destructor and
notification behavior must preserve resource ownership.

Raw infallible thread creation became fallible startup. Creation/save return the
original request or capability before accepting a task or creating a journal.
A read worker whose thread cannot start produces an explicit terminal error.

Atomic stop plus an unretained condition-variable notification could lose a wake
between the actor's predicate check and wait. A capacity-one channel now retains
and coalesces wakes. Creation/save drain accepted work before exiting; reads may
revoke pending work. Shutdown neither joins nor acquires the actor mailbox from
the UI. There is no periodic actor polling timer.

The read actor now exposes terminal health for the latest outstanding request,
destroys its captured document before reporting disconnection, and never resumes
that document after a panic. Request generations still reject stale publications.
Potentially large command/reply drops occur outside mailbox guards. Poison is
not recovered through `into_inner`.

### Retained and transient allocation accounting

Locations: query [`binding/relation/slots.rs`](../../crates/query/tablecraft-query/src/binding/relation/slots.rs),
[`binding/retrieval/body.rs`](../../crates/query/tablecraft-query/src/binding/retrieval/body.rs),
and UI [`field_drop/model.rs`](../../crates/presentation/tablecraft-ui/src/workspace/field_drop/model.rs).

**Rule:** Beautiful Rust §3/§5: deliberate ownership, fallible allocation at bounded
library boundaries, and accurate resource contracts.

JOIN/comma-product namespaces copied names, qualifiers and named collection members
without charging every new owned copy. The binder now uses its existing cumulative
budget, reserves fallibly, checks work/cancellation and publishes no partial result.
UNION output names move from already-owned sorting slots into a paid final vector.
Existing conservative downstream charges remain; limits were not increased.

The outgoing field-deletion request remains owned by a host through review, save
and reopening. The UI now reserves that actual snapshot, including capacities,
until the matching release/transfer. Abandoning an unbound save token does not
pretend the host dropped its request. This accounting grants no native authority.

### Event-thread font discovery

Location: desktop [`font_inventory.rs`](../../apps/tablecraft-desktop/src/font_inventory.rs).

**Rule:** Beautiful Rust §7: keep filesystem traversal and expensive provider work
off event/async executor threads.

Desktop and synthetic hosts initially retain bundled fonts. One fallible background
operation populates the process catalog; hosts observe it without waiting on
`OnceLock` initialization. Failure retains bundled choices with payload-free
warnings. This changes neither database services nor font asset ownership.

## Ownership and organization fixes

**Rule:** Beautiful Rust §3/§6/§9 and the repository's explicit module/ownership
policy. Borrow or move when one owner suffices; preserve genuine public APIs.

- All 273 `mod.rs` files now use `feature.rs` beside `feature/`. The runtime's
  platform implementations use natural module discovery and a selected internal
  platform boundary. All 18 `lib.rs` files contain composition and API exports;
  command/response/error/table definitions, comparison logic and inline root tests
  moved into their owning files. The CI layout guard enforces this convention.
- Date-format/output-trait/LIKE carriers, native request/encoding/map helpers and
  CLI collation consumers import their existing owner directly. Genuine public
  crate APIs, platform contracts and exposed dependency-version types remain.
  No private proof type was made public merely to remove a re-export.
- Query Design borrows its source list instead of cloning the complete draft each
  frame. SQL construction clones an expression only when projection and ORDER BY
  both retain it; hidden expressions still undergo their original validation.
- Navigation selection comparisons borrow identities; F6 filtering uses its fixed
  stack array. Template field matching checks zero/one/multiple matches without
  collecting an intermediate vector.
- Fluent message arguments accept borrowed carriers through `AsRef<str>`.
  Datasheets borrow names and displayed text instead of copying their full payload
  solely to format a tooltip/accessibility message. The final localized output
  remains owned. All eight locale contracts and literal input are preserved.
- Currency multiplication binds its checked integer operand once instead of
  checking and then unwrapping a repeated conversion.

Serial side-effect-only `for_each` calls use ordinary loops. The encryption
transfer's Rayon `ParallelIterator::for_each` remains parallel; replacing it with
a serial loop would change execution policy.

These are structural allocation improvements, not benchmarked speedup claims.

## Patterns intentionally retained

`clone`, `unwrap` and `collect` are not blanket-banned.

- Arc/Bytes sharing, immutable snapshots, visitor-error preservation and distinct
  asynchronous owners are deliberate. Tests need independent mutation/oracle
  images; the three `clone_from` oracle sites were retained.
- Guarded decoder conversions and private state-machine invariants were inspected
  rather than mechanically converted to fallbacks. Test/fixture unwraps remain
  failure assertions. New/changed worker poison expectations explain the invariant.
- Owned results and random-access collections remain where consumers need them.
  Existing XML, schema, parameter and publication precharges must be considered
  before diagnosing an allocation solely from its syntax.
- Synchronous engine/I/O APIs and CLI operations may block their caller. Application
  hosts offload native reads, proofs, saves, creation and font traversal. Starting
  a new thread for each low-level file call would weaken those ownership boundaries.

## Coverage and remaining follow-through

The generated inventories are ignored local work products, not shipped artifacts
or native compatibility evidence. They retain source hashes and per-site review
status. Their snapshots differ as modules and callsites changed during the audit:

| Inventory | Recorded scope | Limits |
| --- | --- | --- |
| Ownership/materialization | 12,168 sites in 5,749 files; 969 production/other-cfg sites triaged, including 157 deeper function/carrier reviews | 440 caller/resource contracts were explicitly open in that snapshot; later closures/fixes do not silently rewrite its counts. Test/fixture sites are primarily classified by ownership, not individually proved. |
| Explicit panic/assertion | 489 production/ambiguous candidates: 463 inspected, 26 delegated to the worker review | Tests/examples remain separate. Indexing, arithmetic, allocator and dependency panics are not exhausted by a method/macro inventory. |
| Blocking/threading | 5,542 lexical candidate lines in 5,741 app/crate files, with owner/caller classification | Full actor/host/encryption owners and representative platform paths were read; not every filesystem branch or indirect dependency call was individually audited. |
| Re-exports | 1,078 declarations inventoried; bounded parent/foreign/alias candidates inspected | API facades were not automatically declared unnecessary. Compiler checks validate changed visibility and imports. |

Further work includes exact retained-layout/resource follow-through, comparison-only
Find/cell DTO copies, and the query designer's per-frame Add Tables name list.
These are follow-ups, not 440 demonstrated bugs or a compatibility percentage.
Read generation exhaustion still needs a typed admission contract instead of
counter reuse. Native picker dependency fallback/startup behavior and native
platform execution remain separate verification work. No new malformed-input panic
was demonstrated in the inspected guarded decoder or unsafe-wrapper sites.

## Validation

The coordinated accepted set contains **4,670 distinct passing tests**, with
**265 existing ignored tests excluded**: 3,882 across 19 unit executables, 670 UI
checks, and 118 public query integration checks across eight targets. Repeated
focused, builder, desktop and failed UI runs are not added again. Targeted
GUID/integral/ordinary-row and builder reruns also pass after the final serial-loop
cleanup; these repeated checks are excluded from the total.

The first 670-test UI run had six failures: three unhandled test texture deltas,
one expectation omitting a retained quota diagnostic, a globally ambiguous Save
selector, and a harness that failed to bind an inspection between key-down/key-up
frames. Corrections retain the production quota guards, exact modal/action/native
identity checks and no-mutation assertions. The complete corrected UI run passed.

Workspace all-feature/all-target strict Clippy and warnings-as-errors rustdoc pass.
Formatting covers 885 target roots; dependency and documentation guards pass.
Final Windows/Linux I/O/runtime cross-checks and strict minimal ACE read/journal
Clippy checks pass after the worker changes. The diagnostic WASM check
stopped at the existing `getrandom` browser-feature requirement; it does not
qualify a browser host. Native windows were not launched. Windows/Access runtime
comparison and physical durability qualification remain unrun.
