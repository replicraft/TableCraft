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
| F07 | P3 | CLI after-help says GUI hosts are not implemented: `apps/tablecraft-cli/src/main.rs:27`. | Describe the implemented desktop host and current CLI boundary accurately. Rule: `doc-module-level` (freshness). |
| F08 | P3 | Benchmark `Failure` displays a retained typed cause but omits `Error::source`: `crates/engines/tablecraft-ace/benches/encryption_pages.rs:65–80`. | Expose the actual cause to standard error traversal. Rule: `err-chain-source`. |

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
