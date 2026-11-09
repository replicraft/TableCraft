# Rust skills review

Reviewed 9 November 2026 against main at `4760ac6f`, before implementation changes.
The local user-supplied `rust-skills` version 1.5.1 contains 265 detailed rules
across 26 categories. Its entry-point SHA-256 is
`5a74070e740c8aacec1264adfc537daeab14c0f629bb604943295c08fd9252e6`.
The private skill is not redistributed here. This report records TableCraft's
own review results. The [complete rule inventory](RUST_SKILLS_RULES.md) gives
every rule an individual disposition, rationale and source evidence.

## Scope and limits

Three parallel source reviews covered ownership/API/memory, safety/concurrency/
numeric/serialization, and structure/documentation/performance/testing. Consolidation
verified all 265 unique rule IDs and rule hashes, and every cited baseline source
hash and line range. No implementation patches, Cargo commands or tests preceded
this report. The unrelated user `.gitignore` change remains unstaged.

Coverage is complete **by rule**, not a proof of every function or every execution
path. The inventory records 135 observed, 85 conditional, 17 not applicable,
16 not verified and 12 finding dispositions. These are not compatibility or
safety percentages. Several finding dispositions refer to the same underlying
defect. The previous [Rust review](RUST_REVIEW.md) retains its separate inventories
and verification evidence; those test runs are not counted as this review's tests.

Keep the existing strengths: semantic carriers validate construction and serde
entry points; query publication uses explicit budgets and stale-source guards;
native I/O stays outside UI execution; and tracing deliberately omits payloads.
Independent fixture oracles and consuming review capabilities are useful boundaries
that an idiom cleanup must preserve.

In particular, lexical counts of `clone`, `unwrap` and `collect` do not establish
unnecessary ownership, reachable panic or wasteful allocation. Reviewed ownership
boundaries retain copies needed for asynchronous publication, independent snapshots,
validated SQL buffers and consuming write capabilities. Fallible bounded allocation
must not be replaced by infallible convenience collection. Existing invariant
assertions require caller-specific proof rather than mechanical substitution.

## Confirmed findings and fix batch

| ID | Priority | Finding and baseline evidence | Required correction |
| --- | --- | --- | --- |
| F01 | P1 | Native save execution can unwind before clearing active state or publishing a terminal report: `crates/application/tablecraft-runtime/src/editing/native/save.rs:269–399`. | Observe actor lifetime, preserve exact accepted operation, report uncertainty until acknowledgement, invalidate the actual source authority, refuse further execution and remove lost recovery grants. Never fabricate rollback, no-write evidence or a receipt. Cover operation-specific UI acknowledgement and INSERT retry evidence. Rules: `async-joinset-structured`, `err-result-over-panic`, `async-cancel-safety`. |
| F02 | P2 | Read request/cancel generation uses `fetch_add + 1`, which wraps or debug-panics and can reuse reserved zero: `crates/application/tablecraft-runtime/src/worker.rs:743,755,805,820`. | Checked monotonic issuance with typed permanent exhaustion, cancellation that still revokes the final request, and caller handling. Exercise near-maximum values. Rules: `num-overflow-explicit`, `err-result-over-panic`. |
| F03 | P3 | Comparison-only deep copies in `apps/tablecraft-desktop/src/workspace/find.rs:71` and `apps/tablecraft-desktop/src/workspace/editing.rs:241`. | Borrow exact cross-layer comparison fields while preserving stale-result and edit guards. Rule: `own-borrow-over-clone`. |
| F04 | P3 | Add Tables clones and collects every eligible name each frame: `crates/presentation/tablecraft-ui/src/workspace/query_design/dialog.rs:25–33`. | Borrow disjoint database metadata; own only a selected payload escaping the frame. Rules: `own-borrow-over-clone`, `closure-disjoint-capture`. |
| F05 | P2 | Copy builder `CreateColumn::with_required` silently discards configuration if its return is ignored: `crates/engines/tablecraft-ace/src/creation/column.rs:150`. | Add a descriptive must-use annotation without changing constructor validation or native bytes. Rules: `api-builder-must-use`, `api-must-use`. |
| F06 | P3 | Public start-error extraction suggests every rejection returns a review, although recovery/release return none: `crates/application/tablecraft-runtime/src/editing/save.rs:186`. Worker and creation error/poison contracts also lack explicit documentation. | Document actual returned ownership, startup failure, exhaustion and internal-poison boundaries at the public methods; do not pretend poison recovery is safe. Rules: `doc-all-public`, `doc-errors-section`, `doc-panics-section`, `lint-missing-docs`, `err-doc-errors`. |
| F07 | P3 | CLI after-help says GUI hosts are not implemented: `apps/tablecraft-cli/src/main.rs:27`. | Describe the implemented desktop host and current CLI boundary accurately. Rule: `doc-crate-readme` (documentation freshness). |
| F08 | P3 | Benchmark `Failure` displays a retained typed cause but omits `Error::source`: `crates/engines/tablecraft-ace/benches/encryption_pages.rs:65–80`. | Expose the actual cause to standard error traversal. Rule: `err-source-chain`. |

F07 and F08 are supplemental concrete findings within otherwise observed scoped
rule evidence; a rule can have both compliant and noncompliant sites. Implementation
will follow this finished report in focused commits, with tests collected into a
single subsequent regression phase. Corrections and measured results will be
recorded below without erasing this baseline.

## Deliberate exceptions

- Keep modern `feature.rs` plus `feature/` consistently. The skill's `mod.rs`
  example does not override the user's choice. Module entry points compose; they
  do not accumulate business logic. Retain genuine API facades, not compatibility
  relays or gratuitous preludes/glob imports.
- Do not set distributable binaries to `target-cpu=native`, or introduce fat LTO,
  PGO, arenas, ThinVec or another test framework without measured benefit and a
  working consumer. Existing proptest, benchmark harnesses and local fakes remain
  valid. No new library is needed for the confirmed fixes.
- Exact floating-point comparisons remain valid for Access semantics, raw carriers,
  zeros and explicit invariants. Generic epsilon rewrites would change behavior.
- Preserve optional validated serde boundaries. Unknown-field/default/skip advice
  must not weaken constructor validation or silently accept incompatible formats.
- Keep payload-free tracing. Automatic argument/error Debug instrumentation can
  disclose SQL, credentials, paths or row contents.
- Generic native/CLI I/O is intentionally synchronous. Desktop actors and async
  hosts own offloading. Adding Tokio to the engine or aborting durable writes does
  not improve structured ownership. `panic=abort` would remove the actor's unwind
  failure protocol; retain the existing process behavior.
- Miri, Loom, Windows runtime comparisons, complete unsafe-path proof, atomic
  interleaving proof and workload-wide performance improvement are not established
  by this static report. The 16 not-verified entries remain explicit work, not
  invented passes. Existing Access qualification restrictions still apply.

## Regression phase

After the implementation commits: format modified Rust, run source/dependency and
documentation guards, strict workspace Clippy, the affected runtime/desktop/UI and
creation/query suites in large batches, and available platform compilation checks.
Inspect all failures; add focused correction commits and rerun only affected checks
when justified. Keep headless-only verification, distinguish pre-existing platform
blocks, and report native Access behavior as unverified without native evidence.

## Implemented follow-up

The report was committed first as `a0a3c58b`. The subsequent fix batch resolves
the eight confirmed findings within their stated scopes:

| Findings | Implementation commits | Result |
| --- | --- | --- |
| F01 | `077f2662`, `eac6a1f7` | Exact uncertain actor reports, real source invalidation, lost-grant refusal, sticky INSERT retry blocker and operation-specific UI acknowledgement. |
| F02 | `a6c96891`, `d9b51a0c`, `eac6a1f7`, `b8071b9c` | Checked request issuance, cancellation-only sentinel and rejection cleanup for existing reads, reserved inspections and composed witness reopens. |
| F03–F04 | `544c94e6` | Borrowed exact request/edit guards and frame-local table metadata. |
| F05 | `c719ed76`, `cafcdb32` | Must-use creation and collection declarations plus reviewed configured evaluator/comparison/source owners; paired runnable and compile-fail examples. |
| F06 | `c719ed76`, `077f2662`, `a6c96891` | Explicit returned ownership, error, poisoning and uncertain-outcome contracts. |
| F07–F08 | `94a8d29e` | Current CLI help and standard benchmark error-source traversal. |

The consolidated checks exposed missing test glue, a stale help assertion,
intentional descriptor/probe discards and two collapsible guards. Focused follow-up
commits corrected these; the failed attempts remain part of the verification
history. There are no new dependencies or performance-speedup claims.

## Accepted verification

- Workspace all-feature library/binary batch: **4,568 passed**, **265 existing
  ignored**, no failures across 20 test executables. Compilation had no warnings.
- Eight public query-contract targets: **173 passed**. These are separate from
  the unit batch. CLI process smoke: **5 passed**. ACE rustdoc: **8 passed**,
  including both new positive examples and both new compile-fail examples.
  The collation-resolver integration target also passed all **8 cases** after
  making its constructor-only cancellation probe explicit.
- After the final behavior-preserving lint corrections, the three actual actor
  health tests and desktop uncertainty mapping test passed again. These repeated
  cases are not added to the totals above.
- Strict workspace/all-target/all-feature Clippy and strict workspace rustdoc
  passed. Minimal ACE reader/journal Clippy and minimal runtime Clippy passed.
- Windows MSVC and Linux runtime/I/O cross-compilation passed after the runtime
  cleanup. These are compile checks, not platform execution or native Access proof.
- Formatting, modern module/composition guards, dependency boundaries and local
  documentation links passed; final documentation checks are retained separately
  from Rust execution.

Commands use `--offline --locked`, the existing target cache and normal incremental
compilation. Source was frozen during each execution batch. The full unit batch
preceded the last lint-only cleanup; affected checks were rerun instead of repeating
every suite. No native application window was launched.

The actor tests distinguish real resources from protocol setup: one consumes a
genuine review and holds a real exclusive session before injected panic, without
performing a native write. Recover/Release identity tests and the retained-history
case exercise private actor/mailbox protocol state; they do not qualify a new
native recovery mutation or an Access save. Existing native recovery tests remain
separate evidence. The revision-stale UI test preserves a proposed field value
through lost retained controls, acknowledgement and dismissal.

The 16 baseline **not verified** rule dispositions remain explicit limitations,
including complete panic/cast callsite proof, atomic-model checking and workload
profiling. They are not confirmed defects silently marked fixed. This review does
not certify every callsite, dependency, platform or Access compatibility profile.

## Clone follow-up — 9 November 2026

The initial audit did not establish that every remaining clone was necessary.
The selected desktop `workspace/objects/tests.rs` has now received a complete
review of its 38 explicit calls: eight comparison-only copies were replaced by
borrowed comparisons, and one last-use target now moves. The remaining 29 are
26 owned command inputs retained for later commands or independent expectations,
two owned returns and one shared egui Context. The
[per-site ledger](../testing/results/objects-test-clones-2026-11-09.json)
records every decision and the exact before/after source hashes.

Five shared UI implementation files now borrow readonly descriptions, types and
tab titles, defer an inspection target copy until the action occurs, and avoid
copying the query-grid original before constructing its necessary edit buffer.
Buffered and progressive CLI query consumers now move parsed parameter values
directly into bindings instead of cloning a second value vector. Parameter names
borrow the raw arguments. The cached Jiff time-zone clone remains appropriate:
the conversion requires an owned zone, whose representation supports shared data.

This is scoped ownership work, not repository-wide clearance. The UI inventory
contains 330 production-source candidates, of which 58 were individually read;
272 remain open. The runtime/CLI follow-up considered 116 production clone sites
by carrier and ownership groups, without claiming complete independent workflow
proof for all 116. Lexical counts alone do not establish unnecessary allocation.

The follow-up passed 201 desktop tests (one existing ignored), 112 CLI unit tests
and 65 parameter/progressive integration cases. Its UI batch passed 681 cases
and exposed one newly added retry test observing before the next-frame action
collector. A test-only timing correction then passed all 32 DROP UI cases;
repeated cases are not added to the totals. Strict workspace/all-target/all-feature
Clippy passed. The [GUI record](../testing/results/saved-field-drop-gui-2026-11-09.json)
retains the failed attempt and subsequent focused result separately. Native
startup/File/Open review resumed with permission; native database save interaction
was not established by that graphical attempt.
