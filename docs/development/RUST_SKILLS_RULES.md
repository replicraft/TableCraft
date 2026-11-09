# Rust skills: complete rule inventory

Review date: 9 November 2026. Baseline: `4760ac6f`. See the [review report](RUST_SKILLS_REVIEW.md) for scope, findings, exceptions and validation.

Every one of the 265 detailed rules has a disposition below. Evidence is frozen at the baseline; line numbers may move during fixes. **Observed** means the cited boundary follows the rule, not that every site is proven correct. **Conditional** records a context-dependent choice; **not applicable** records an absent mechanism in the inspected scope; **not verified** identifies outstanding evidence. Findings can overlap multiple rules.

## `anti-clone-excessive` — conditional

Actual copies preserve owned SQL buffers/preferences or source leases; unnecessary deep copies require caller-specific proof. The assigned ownership agent owns the broader clone inventory, not this representative review.

Evidence: [`crates/presentation/tablecraft-ui/src/workspace/objects/memory.rs:17`](../../crates/presentation/tablecraft-ui/src/workspace/objects/memory.rs); [`crates/presentation/tablecraft-ui/src/workspace/saved_query/sql/rebind.rs:29`](../../crates/presentation/tablecraft-ui/src/workspace/saved_query/sql/rebind.rs).

## `anti-collect-intermediate` — conditional

Some retained UI graphs and complete query caches deliberately materialize owned collections for repeated traversal/atomic publication. The separate ownership agent reviewed concrete clone/collect waste; no automatic lazy rewrite may drop eager errors or accounting.

Evidence: [`crates/presentation/tablecraft-ui/src/workspace/objects/memory.rs:17`](../../crates/presentation/tablecraft-ui/src/workspace/objects/memory.rs).

## `anti-empty-catch` — conditional

Signal Full/Disconnected handling is intentional coalescing/teardown, not a filesystem error fallback; actual sink/backend error wrappers retain typed sources. No unconditional retry or hidden persistence success is introduced.

Evidence: [`crates/application/tablecraft-runtime/src/task_signal.rs:18`](../../crates/application/tablecraft-runtime/src/task_signal.rs); [`crates/query/tablecraft-query/src/execution/progressive.rs:54`](../../crates/query/tablecraft-query/src/execution/progressive.rs); [`crates/backends/tablecraft-access/src/editing/offline/error.rs:342`](../../crates/backends/tablecraft-access/src/editing/offline/error.rs).

## `anti-expect-lazy` — conditional

OS startup environmental failure now uses typed refusal and the explicit-site panic audit has concrete invariants. Save execution unwind remains a known unresolved boundary; this record neither rubber-stamps every indexing/allocation panic nor invents no-write after unwind.

Evidence: [`crates/application/tablecraft-runtime/src/editing/native/save/startup_tests.rs:9`](../../crates/application/tablecraft-runtime/src/editing/native/save/startup_tests.rs); [`crates/application/tablecraft-runtime/src/worker.rs:586`](../../crates/application/tablecraft-runtime/src/worker.rs).

## `anti-format-hot-path` — not verified

No allocation profile of formatting hot paths was run. UI diagnostics and one-shot executable messages may legitimately allocate; the ownership lane owns concrete paid-allocation findings.

Evidence: [`crates/engines/tablecraft-ace/benches/image_capture.rs:1`](../../crates/engines/tablecraft-ace/benches/image_capture.rs).

## `anti-index-over-iter` — conditional

Checked slices, chunk traversal and iterator loops are used at inspected byte boundaries. Physical format offsets and complete ownership proofs legitimately retain indexed addressing; no measured bounds-check hotspot was established.

Never replace checked parsing with unchecked access solely to satisfy a tuning rule; no unsafe optimization is proposed.

Evidence: [`crates/platform/tablecraft-io/src/read.rs:13`](../../crates/platform/tablecraft-io/src/read.rs); [`crates/engines/tablecraft-ace/src/table/indexes/mutation/encoding.rs:26`](../../crates/engines/tablecraft-ace/src/table/indexes/mutation/encoding.rs).

## `anti-lock-across-await` — conditional

Native worker mutex sections are synchronous and callbacks occur after guard release. Optional paused SQLite async code is not expanded; its broader async safety belongs to the concurrency lane.

Evidence: [`crates/application/tablecraft-runtime/src/worker.rs:58`](../../crates/application/tablecraft-runtime/src/worker.rs); [`crates/backends/tablecraft-sqlite/Cargo.toml:14`](../../crates/backends/tablecraft-sqlite/Cargo.toml).

## `anti-over-abstraction` — observed

The new generic spawner has three real worker consumers and deterministic refusal tests; the signal owner owns durable notification only, not a generic save policy framework.

Evidence: [`crates/application/tablecraft-runtime/src/thread_start.rs:4`](../../crates/application/tablecraft-runtime/src/thread_start.rs).

## `anti-panic-expected` — conditional

OS startup environmental failure now uses typed refusal and the explicit-site panic audit has concrete invariants. Save execution unwind remains a known unresolved boundary; this record neither rubber-stamps every indexing/allocation panic nor invents no-write after unwind.

Evidence: [`crates/application/tablecraft-runtime/src/editing/native/save/startup_tests.rs:9`](../../crates/application/tablecraft-runtime/src/editing/native/save/startup_tests.rs); [`crates/application/tablecraft-runtime/src/worker.rs:586`](../../crates/application/tablecraft-runtime/src/worker.rs).

## `anti-premature-optimize` — not verified

Maintained benchmark modes demonstrate measurement machinery, but this pass ran no profiler or benchmark and does not validate all historic speed claims. New tuning must remain evidence-led.

Evidence: [`crates/engines/tablecraft-ace/benches/image_capture.rs:1`](../../crates/engines/tablecraft-ace/benches/image_capture.rs); [`crates/query/tablecraft-query/benches/averages.rs:11`](../../crates/query/tablecraft-query/benches/averages.rs).

## `anti-string-for-str` — conditional

The explicit &String rebind copier reserves the original spare capacity, which &str does not expose. This is a real capacity-preservation contract, not an unnecessary restrictive string argument.

Evidence: [`crates/presentation/tablecraft-ui/src/workspace/saved_query/sql/rebind.rs:29`](../../crates/presentation/tablecraft-ui/src/workspace/saved_query/sql/rebind.rs).

## `anti-stringly-typed` — observed

Actual operation directions, native declarations and journal families are typed enums/checked codecs. Native stored names and SQL text must remain raw user data rather than invented exhaustive enums.

Evidence: [`crates/platform/tablecraft-crypto/src/aes.rs:12`](../../crates/platform/tablecraft-crypto/src/aes.rs); [`crates/engines/tablecraft-ace/src/creation/column.rs:36`](../../crates/engines/tablecraft-ace/src/creation/column.rs).

## `anti-type-erasure` — conditional

Desktop file-dialog futures must retain an erased asynchronous operation across frames; worker spawner uses static dispatch where one concrete callable suffices. Blanket dyn removal would change real host ownership.

Evidence: [`apps/tablecraft-desktop/src/file_dialog.rs:12`](../../apps/tablecraft-desktop/src/file_dialog.rs); [`crates/application/tablecraft-runtime/src/thread_start.rs:4`](../../crates/application/tablecraft-runtime/src/thread_start.rs).

## `anti-unwrap-abuse` — conditional

OS startup environmental failure now uses typed refusal and the explicit-site panic audit has concrete invariants. Save execution unwind remains a known unresolved boundary; this record neither rubber-stamps every indexing/allocation panic nor invents no-write after unwind.

Evidence: [`crates/application/tablecraft-runtime/src/editing/native/save/startup_tests.rs:9`](../../crates/application/tablecraft-runtime/src/editing/native/save/startup_tests.rs); [`crates/application/tablecraft-runtime/src/worker.rs:586`](../../crates/application/tablecraft-runtime/src/worker.rs).

## `anti-vec-for-slice` — conditional

Actual retained-memory accounting reads Vec.capacity and cannot be expressed equivalently from a slice. Existing ptr_arg exceptions document this capacity contract.

Evidence: [`crates/presentation/tablecraft-ui/src/workspace/objects/memory.rs:17`](../../crates/presentation/tablecraft-ui/src/workspace/objects/memory.rs).

## `api-builder-must-use` — finding

CreateColumn is Copy and with_required returns modified Self without #[must_use] at method or type. Ignoring it compiles without consuming the original and silently loses Required policy.

Fallible setters return Result and already inherit must-use diagnostics.

A targeted method annotation is enough; preserve constructor/default bytes.

Evidence: [`crates/engines/tablecraft-ace/src/creation/column.rs:15`](../../crates/engines/tablecraft-ace/src/creation/column.rs); [`crates/engines/tablecraft-ace/src/creation/column.rs:150`](../../crates/engines/tablecraft-ace/src/creation/column.rs).

## `api-builder-pattern` — observed

CreateColumn exposes named native declaration factories and policy setters; required typed parameters remain explicit. Multiple configuration dimensions are not an unnamed tuple constructor.

No generic framework or builder crate needed for existing real API.

Evidence: [`crates/engines/tablecraft-ace/src/creation/column.rs:15`](../../crates/engines/tablecraft-ace/src/creation/column.rs); [`crates/engines/tablecraft-ace/src/creation/column.rs:150`](../../crates/engines/tablecraft-ace/src/creation/column.rs); [`crates/engines/tablecraft-ace/src/creation/column.rs:78`](../../crates/engines/tablecraft-ace/src/creation/column.rs).

## `api-common-traits` — observed

Copy identities derive common equality/hash traits; sensitive expression state supplies redacted Debug; SchemaName intentionally omits Debug/Serde; prepared journals omit Clone/Debug.

The skill minimum Debug+Clone recommendation is not valid for unique resource owners or sensitive native payloads.

Repository forbids automatic payload diagnostics and duplicated capability authority.

Evidence: [`crates/foundation/tablecraft-core/src/ids/identity.rs:7`](../../crates/foundation/tablecraft-core/src/ids/identity.rs); [`crates/foundation/tablecraft-schema/src/name.rs:11`](../../crates/foundation/tablecraft-schema/src/name.rs); [`crates/foundation/tablecraft-core/src/values/expression.rs:47`](../../crates/foundation/tablecraft-core/src/values/expression.rs); [`crates/platform/tablecraft-io/src/journal/transaction.rs:13`](../../crates/platform/tablecraft-io/src/journal/transaction.rs).

## `api-default-impl` — observed

Parser limits have explicit bounded defaults; ResourceBudget intentionally has no unlimited/default budget and SchemaName has no valid empty default.

Default is appropriate only with a semantically meaningful domain default.

Preserve Access creation defaults/settings; never invent a source generation or mutation capability.

Evidence: [`crates/query/tablecraft-sql/src/parser/limits.rs:26`](../../crates/query/tablecraft-sql/src/parser/limits.rs); [`crates/foundation/tablecraft-core/src/limits.rs:31`](../../crates/foundation/tablecraft-core/src/limits.rs); [`crates/foundation/tablecraft-schema/src/name.rs:11`](../../crates/foundation/tablecraft-schema/src/name.rs).

## `api-extension-trait` — conditional

Owned source semantics live in Source; foreign io errors convert through From into a local type. No concrete repeated foreign-type helper requiring extension trait was established.

Adding a trait merely to wrap a single free function would not satisfy repository ownership goals.

Evidence: [`crates/platform/tablecraft-io/src/error.rs:30`](../../crates/platform/tablecraft-io/src/error.rs); [`crates/query/tablecraft-query/src/source.rs:25`](../../crates/query/tablecraft-query/src/source.rs).

## `api-from-not-into` — observed

IoError implements From<io::Error>; fallible ExpressionValue storage uses TryFrom. Reviewed conversions do not implement Into directly.

Contextual conversions requiring budgets/scope remain named fallible operations; infallible traits cannot carry that context.

Evidence: [`crates/platform/tablecraft-io/src/error.rs:30`](../../crates/platform/tablecraft-io/src/error.rs); [`crates/foundation/tablecraft-core/src/values/expression.rs:25`](../../crates/foundation/tablecraft-core/src/values/expression.rs).

## `api-impl-asref` — observed

Fluent argument formatting uses AsRef<str>, supporting owned and borrowed arguments without copied payloads; font inventory accepts borrowed generic names.

Plain &str is simpler for one concrete borrowed type; no blanket genericization.

Evidence: [`crates/presentation/tablecraft-ui/src/localization.rs:257`](../../crates/presentation/tablecraft-ui/src/localization.rs); [`crates/presentation/tablecraft-ui/src/fonts/inventory.rs:34`](../../crates/presentation/tablecraft-ui/src/fonts/inventory.rs).

## `api-impl-fromiterator` — conditional

ObjectCatalog is a validated scoped inventory, not a generic unvalidated Bag; its constructor can reject scope, count and duplicate identities.

FromIterator cannot return typed validation failure or receive caller scope/limits. Adding it would require hiding errors or weaker invariants.

Do not bypass budget/profile admission just to permit collect().

Evidence: [`crates/foundation/tablecraft-model/src/catalog.rs:24`](../../crates/foundation/tablecraft-model/src/catalog.rs); [`crates/foundation/tablecraft-schema/src/name.rs:11`](../../crates/foundation/tablecraft-schema/src/name.rs); [`crates/foundation/tablecraft-model/src/catalog.rs:62`](../../crates/foundation/tablecraft-model/src/catalog.rs).

## `api-impl-into` — observed

Error conversion implements From and callers can use Into. Native read API deliberately borrows a fixed mutable slice rather than taking ownership.

impl Into<String> can allocate before budget admission; keep owned/borrowed contracts explicit. The rule hot-path dispatch concern is not a measured fact for monomorphized Into.

Evidence: [`crates/platform/tablecraft-io/src/error.rs:30`](../../crates/platform/tablecraft-io/src/error.rs); [`crates/platform/tablecraft-io/src/read.rs:15`](../../crates/platform/tablecraft-io/src/read.rs).

## `api-must-use` — observed

Prepared native journal carries a descriptive must-use annotation; Result-producing preparation inherits standard diagnostics. One pure builder return remains API-01.

The skill Bad example incorrectly suggests an ignored Result is unmarked; standard Result already has must_use.

Evidence: [`crates/platform/tablecraft-io/src/journal/transaction.rs:13`](../../crates/platform/tablecraft-io/src/journal/transaction.rs); [`crates/engines/tablecraft-ace/src/creation/column.rs:150`](../../crates/engines/tablecraft-ace/src/creation/column.rs).

## `api-newtype-safety` — observed

Separate ObjectId/TableId/OperationId/CursorId domains prevent accidental interchange; schema names are validated newtypes.

Native numeric identity remains separate from scoped IDs; no generic untyped identity wrapper proposed.

Evidence: [`crates/foundation/tablecraft-core/src/ids/identity.rs:7`](../../crates/foundation/tablecraft-core/src/ids/identity.rs); [`crates/foundation/tablecraft-schema/src/name.rs:11`](../../crates/foundation/tablecraft-schema/src/name.rs); [`crates/foundation/tablecraft-core/src/ids/identity.rs:42`](../../crates/foundation/tablecraft-core/src/ids/identity.rs).

## `api-non-exhaustive` — conditional

Public error enums can evolve, while NameComparison and expression-state enums define explicit current closed semantics and are exhaustively consumed across crates.

No semver evolution promise or downstream compatibility test was established for blanket non_exhaustive.

Do not hide new authority/resource variants behind wildcard fallback or change frozen intent decoding.

Evidence: [`crates/platform/tablecraft-io/src/error.rs:7`](../../crates/platform/tablecraft-io/src/error.rs); [`crates/foundation/tablecraft-schema/src/name.rs:25`](../../crates/foundation/tablecraft-schema/src/name.rs); [`crates/foundation/tablecraft-core/src/values/expression.rs:12`](../../crates/foundation/tablecraft-core/src/values/expression.rs).

## `api-operator-overload` — observed

Currency exposes checked_add/checked_sub with typed overflow and exact scale; no surprising infallible arithmetic coercion/operator is introduced.

Add returning Self would lose failure behavior; an operator should not imply unsupported Variant promotion.

Evidence: [`crates/foundation/tablecraft-core/src/values/currency.rs:26`](../../crates/foundation/tablecraft-core/src/values/currency.rs); [`crates/foundation/tablecraft-core/src/values/currency.rs:42`](../../crates/foundation/tablecraft-core/src/values/currency.rs).

## `api-parse-dont-validate` — observed

Private-field constructors enforce schema names/Decimal; expression-to-stored conversion rejects language-only states instead of trusting upstream validation.

Native profile/revision/resource admission remains per operation; a valid name/value does not grant storage permission.

Evidence: [`crates/foundation/tablecraft-schema/src/name.rs:11`](../../crates/foundation/tablecraft-schema/src/name.rs); [`crates/foundation/tablecraft-core/src/values/decimal.rs:31`](../../crates/foundation/tablecraft-core/src/values/decimal.rs); [`crates/foundation/tablecraft-core/src/values/expression.rs:25`](../../crates/foundation/tablecraft-core/src/values/expression.rs).

## `api-sealed-trait` — conditional

Source and DurableStorage are intentional host implementation contracts with documented obligations. Sealing them would block useful independent providers/fixtures.

Trust boundaries additionally validate provider outputs and retain native proof; a public trait is not by itself write authority.

User policy explicitly requires host-owned sources/storage and standalone engine boundaries.

Evidence: [`crates/query/tablecraft-query/src/source.rs:25`](../../crates/query/tablecraft-query/src/source.rs); [`crates/platform/tablecraft-io/src/journal/storage.rs:13`](../../crates/platform/tablecraft-io/src/journal/storage.rs).

## `api-serde-optional` — observed

Core serde is optional with empty default features; Decimal deserialization routes through its validated constructor.

A format/protocol crate genuinely about serialization may require serde; absence of optional in such a crate is not automatically a violation.

Evidence: [`crates/foundation/tablecraft-core/Cargo.toml:15`](../../crates/foundation/tablecraft-core/Cargo.toml); [`crates/foundation/tablecraft-core/src/values/decimal.rs:86`](../../crates/foundation/tablecraft-core/src/values/decimal.rs).

## `api-typestate` — observed

PreparedImage and CheckedV1 are distinct consuming states with private fields and retained resource borrows. Integrity session explicitly does not imply native recovery permission.

Runtime UI phases intentionally need enums because heterogeneous callback states coexist; no typestate boilerplate migration.

Evidence: [`crates/platform/tablecraft-io/src/journal/transaction.rs:13`](../../crates/platform/tablecraft-io/src/journal/transaction.rs); [`crates/platform/tablecraft-io/src/journal/checked_v1.rs:36`](../../crates/platform/tablecraft-io/src/journal/checked_v1.rs).

## `async-async-fn-bounds` — not applicable

Reviewed higher-order call boundaries accept synchronous actor closures and concrete picker Future; no F: Fn()->Fut higher-order async API was found in the async-owner search. This is scoped to authored apps/crates, not dependency internals.

Evidence: [`crates/backends/tablecraft-sqlite/src/worker/reader.rs:54`](../../crates/backends/tablecraft-sqlite/src/worker/reader.rs); [`apps/tablecraft-desktop/src/file_dialog.rs:12`](../../apps/tablecraft-desktop/src/file_dialog.rs).

## `async-bounded-channel` — observed

Capacity-one durable wake token plus one replaceable read command/reply bound actor mailbox cardinality; SQLite try_acquire_owned rejects a second query before queueing. Report/capacity payload byte budgets are separate, not proved by channel size.

Repository requires synchronous generic I/O/native semantics independent of hosts; desktop uses dedicated long-lived actors. Do not add Tokio or one thread per read merely to satisfy an async recipe. Existing SQLite expansion is paused.

Evidence: [`crates/application/tablecraft-runtime/src/task_signal.rs:13`](../../crates/application/tablecraft-runtime/src/task_signal.rs); [`crates/application/tablecraft-runtime/src/worker.rs:73`](../../crates/application/tablecraft-runtime/src/worker.rs); [`crates/backends/tablecraft-sqlite/src/worker/reader.rs:81`](../../crates/backends/tablecraft-sqlite/src/worker/reader.rs).

## `async-broadcast-pubsub` — not applicable

Actual task wake/request consumers are single actors, not independent event subscribers. Adding broadcast would change semantics; no authored broadcast pub/sub requirement identified.

Evidence: [`crates/application/tablecraft-runtime/src/task_signal.rs:12`](../../crates/application/tablecraft-runtime/src/task_signal.rs); [`crates/application/tablecraft-runtime/src/worker.rs:73`](../../crates/application/tablecraft-runtime/src/worker.rs).

## `async-cancel-safety` — conditional

SQLite cancellation owns worker permit through cleanup; read-generation cancellation revokes publication without claiming I/O stopped. No authored tokio::select! branch found by search; cancellation of external picker future internals remains dependency-specific.

Repository requires synchronous generic I/O/native semantics independent of hosts; desktop uses dedicated long-lived actors. Do not add Tokio or one thread per read merely to satisfy an async recipe. Existing SQLite expansion is paused.

Evidence: [`crates/backends/tablecraft-sqlite/src/worker/reader.rs:68`](../../crates/backends/tablecraft-sqlite/src/worker/reader.rs); [`crates/backends/tablecraft-sqlite/src/worker/reader.rs:90`](../../crates/backends/tablecraft-sqlite/src/worker/reader.rs); [`crates/application/tablecraft-runtime/src/worker.rs:772`](../../crates/application/tablecraft-runtime/src/worker.rs); [`apps/tablecraft-desktop/src/file_dialog.rs:107`](../../apps/tablecraft-desktop/src/file_dialog.rs).

## `async-cancellation-token` — conditional

Synchronous actors deliberately use atomics and durable wake messages, not Tokio CancellationToken. Read cancellation revokes publication; save Drop never interrupts accepted publication. Cancellation cannot stop an in-progress OS syscall.

Repository requires synchronous generic I/O/native semantics independent of hosts; desktop uses dedicated long-lived actors. Do not add Tokio or one thread per read merely to satisfy an async recipe. Existing SQLite expansion is paused.

Evidence: [`crates/application/tablecraft-runtime/src/task_signal.rs:17`](../../crates/application/tablecraft-runtime/src/task_signal.rs); [`crates/application/tablecraft-runtime/src/worker.rs:803`](../../crates/application/tablecraft-runtime/src/worker.rs); [`crates/application/tablecraft-runtime/src/editing/native/save.rs:720`](../../crates/application/tablecraft-runtime/src/editing/native/save.rs).

## `async-clone-before-await` — observed

SQLite clones gate/healthy/cancellation ownership before awaited worker call; no borrowed mutex guard crosses that await in the reviewed reader. Not a global borrow/future Send certification.

Evidence: [`crates/backends/tablecraft-sqlite/src/worker/reader.rs:85`](../../crates/backends/tablecraft-sqlite/src/worker/reader.rs); [`crates/backends/tablecraft-sqlite/src/worker/reader.rs:90`](../../crates/backends/tablecraft-sqlite/src/worker/reader.rs); [`crates/backends/tablecraft-sqlite/src/worker/reader.rs:174`](../../crates/backends/tablecraft-sqlite/src/worker/reader.rs).

## `async-fn-in-trait` — not applicable

Reviewed authored async entrypoints are inherent methods/concrete boxed GUI future. No async_trait or async-trait definition found in authored Rust search; dynamic picker ownership is a real non-Send GUI requirement.

Evidence: [`crates/backends/tablecraft-sqlite/src/worker/reader.rs:54`](../../crates/backends/tablecraft-sqlite/src/worker/reader.rs); [`apps/tablecraft-desktop/src/file_dialog.rs:12`](../../apps/tablecraft-desktop/src/file_dialog.rs).

## `async-join-parallel` — conditional

Native read/save operations depend on serial Document/one retained save owner. Their sequence is not independent concurrent futures; no beneficial independent authored async join candidate established.

Repository requires synchronous generic I/O/native semantics independent of hosts; desktop uses dedicated long-lived actors. Do not add Tokio or one thread per read merely to satisfy an async recipe. Existing SQLite expansion is paused.

Evidence: [`crates/backends/tablecraft-sqlite/src/worker/reader.rs:68`](../../crates/backends/tablecraft-sqlite/src/worker/reader.rs); [`crates/application/tablecraft-runtime/src/worker.rs:73`](../../crates/application/tablecraft-runtime/src/worker.rs); [`crates/application/tablecraft-runtime/src/editing/native/save.rs:73`](../../crates/application/tablecraft-runtime/src/editing/native/save.rs).

## `async-joinset-structured` — finding

Accepted native save execution can unwind out of detached actor before active reset/report publication. Dropped JoinHandle alone does not expose actor death; current UI may retain an indefinitely active save. It is unsafe to synthesize a no-write result after potential publication.

Repository requires synchronous generic I/O/native semantics independent of hosts; desktop uses dedicated long-lived actors. Do not add Tokio or one thread per read merely to satisfy an async recipe. Existing SQLite expansion is paused.

Evidence: [`crates/application/tablecraft-runtime/src/editing/native/save.rs:205`](../../crates/application/tablecraft-runtime/src/editing/native/save.rs); [`crates/application/tablecraft-runtime/src/editing/native/save.rs:269`](../../crates/application/tablecraft-runtime/src/editing/native/save.rs); [`crates/application/tablecraft-runtime/src/editing/native/save.rs:399`](../../crates/application/tablecraft-runtime/src/editing/native/save.rs).

## `async-mpsc-queue` — conditional

Mailbox+capacity-one wake implements the actual bounded synchronous actor queue; SaveWorker rejects replacement rather than queueing review capabilities. Tokio mpsc is not required for these non-async actors.

Repository requires synchronous generic I/O/native semantics independent of hosts; desktop uses dedicated long-lived actors. Do not add Tokio or one thread per read merely to satisfy an async recipe. Existing SQLite expansion is paused.

Evidence: [`crates/application/tablecraft-runtime/src/task_signal.rs:12`](../../crates/application/tablecraft-runtime/src/task_signal.rs); [`crates/application/tablecraft-runtime/src/worker.rs:751`](../../crates/application/tablecraft-runtime/src/worker.rs); [`crates/application/tablecraft-runtime/src/editing/native/save.rs:607`](../../crates/application/tablecraft-runtime/src/editing/native/save.rs).

## `async-no-lock-await` — observed

Reviewed SQLite budget guards end in synchronous methods inside call_raw; awaits occur in reader outside guards. Picker poll retains no Rust mutex guard. All runtime mailbox loops are synchronous. No global dependency-future proof claimed.

Evidence: [`crates/backends/tablecraft-sqlite/src/worker/reader.rs:94`](../../crates/backends/tablecraft-sqlite/src/worker/reader.rs); [`crates/backends/tablecraft-sqlite/src/work.rs:31`](../../crates/backends/tablecraft-sqlite/src/work.rs); [`apps/tablecraft-desktop/src/file_dialog.rs:107`](../../apps/tablecraft-desktop/src/file_dialog.rs).

## `async-oneshot-response` — conditional

Read reply remains one replaceable exact-generation slot, while a zero-data Infallible channel observes actor lifetime only. Replacing the public polling protocol with oneshot is not a missing correctness requirement.

Repository requires synchronous generic I/O/native semantics independent of hosts; desktop uses dedicated long-lived actors. Do not add Tokio or one thread per read merely to satisfy an async recipe. Existing SQLite expansion is paused.

Evidence: [`crates/application/tablecraft-runtime/src/worker.rs:28`](../../crates/application/tablecraft-runtime/src/worker.rs); [`crates/application/tablecraft-runtime/src/worker.rs:776`](../../crates/application/tablecraft-runtime/src/worker.rs).

## `async-select-racing` — not applicable

No authored select!/race async coordination found in apps/crates search. Actual caller cancels via exact publication ownership rather than racing native writes. Dependency picker selection is outside this claim.

Evidence: [`crates/backends/tablecraft-sqlite/src/worker/reader.rs:68`](../../crates/backends/tablecraft-sqlite/src/worker/reader.rs); [`apps/tablecraft-desktop/src/file_dialog.rs:107`](../../apps/tablecraft-desktop/src/file_dialog.rs).

## `async-spawn-blocking` — observed

Current source has font discovery on fallible background task; read proof/I/O in dedicated actor; encryption CPU pages in borrowed Rayon pool. Review-only stage found no new confirmed expensive work directly inside authored async reader polling.

Repository requires synchronous generic I/O/native semantics independent of hosts; desktop uses dedicated long-lived actors. Do not add Tokio or one thread per read merely to satisfy an async recipe. Existing SQLite expansion is paused.

Evidence: [`apps/tablecraft-desktop/src/font_inventory.rs:119`](../../apps/tablecraft-desktop/src/font_inventory.rs); [`crates/application/tablecraft-runtime/src/worker.rs:582`](../../crates/application/tablecraft-runtime/src/worker.rs); [`crates/engines/tablecraft-ace/src/encryption/parallel/transfer.rs:137`](../../crates/engines/tablecraft-ace/src/encryption/parallel/transfer.rs).

## `async-tokio-fs` — conditional

Native filesystem operations are synchronous caller-bound APIs invoked by background actors/CLI; SQLite open delegates dedicated worker. No direct std::fs call in inspected async functions. Cannot replace native sync/journal primitives with tokio::fs without changing contracts.

Repository requires synchronous generic I/O/native semantics independent of hosts; desktop uses dedicated long-lived actors. Do not add Tokio or one thread per read merely to satisfy an async recipe. Existing SQLite expansion is paused.

Evidence: [`crates/backends/tablecraft-sqlite/src/worker/reader.rs:54`](../../crates/backends/tablecraft-sqlite/src/worker/reader.rs); [`crates/platform/tablecraft-io/src/lock.rs:26`](../../crates/platform/tablecraft-io/src/lock.rs); [`apps/tablecraft-desktop/src/file_dialog.rs:107`](../../apps/tablecraft-desktop/src/file_dialog.rs).

## `async-tokio-runtime` — conditional

Tokio is feature-scoped sync support for paused SQLite worker; desktop is an egui polling host and does not own a Tokio executor. No runtime creation/tuning obligation for synchronous core.

Repository requires synchronous generic I/O/native semantics independent of hosts; desktop uses dedicated long-lived actors. Do not add Tokio or one thread per read merely to satisfy an async recipe. Existing SQLite expansion is paused.

Evidence: [`Cargo.toml:158`](../../Cargo.toml); [`crates/backends/tablecraft-sqlite/src/worker/reader.rs:54`](../../crates/backends/tablecraft-sqlite/src/worker/reader.rs); [`apps/tablecraft-desktop/src/file_dialog.rs:12`](../../apps/tablecraft-desktop/src/file_dialog.rs).

## `async-try-join` — not applicable

No independent concurrent fallible futures identified; early dropping a native save/recovery authority is not acceptable fail-fast parallelism. No authored try_join found.

Evidence: [`crates/backends/tablecraft-sqlite/src/worker/reader.rs:68`](../../crates/backends/tablecraft-sqlite/src/worker/reader.rs); [`crates/application/tablecraft-runtime/src/editing/native/save.rs:269`](../../crates/application/tablecraft-runtime/src/editing/native/save.rs).

## `async-watch-latest` — conditional

Latest read reply and process font catalog are existing single/latest observation mechanisms. No event-history requirement or Tokio watch consumer exists; font immutable cache is polled without initialization waiting.

Repository requires synchronous generic I/O/native semantics independent of hosts; desktop uses dedicated long-lived actors. Do not add Tokio or one thread per read merely to satisfy an async recipe. Existing SQLite expansion is paused.

Evidence: [`crates/application/tablecraft-runtime/src/worker.rs:776`](../../crates/application/tablecraft-runtime/src/worker.rs); [`apps/tablecraft-desktop/src/font_inventory.rs:89`](../../apps/tablecraft-desktop/src/font_inventory.rs).

## `closure-disjoint-capture` — observed

QBE ownership now clones only when two AST owners are necessary; Add Tables takes session separately but still snapshots all database names, a bounded borrow-splitting opportunity.

Edition-2021/2024 minimal capture applies to move closures too; skill statements that move always captures a whole named struct are too broad. Explicit local binding remains useful for readability.

Maintain session restoration/cancellation even when borrowing the disjoint database field.

Evidence: [`crates/presentation/tablecraft-builders/src/query/build.rs:447`](../../crates/presentation/tablecraft-builders/src/query/build.rs); [`crates/presentation/tablecraft-ui/src/workspace/query_design/dialog.rs:25`](../../crates/presentation/tablecraft-ui/src/workspace/query_design/dialog.rs); [`crates/application/tablecraft-runtime/src/worker.rs:582`](../../crates/application/tablecraft-runtime/src/worker.rs).

## `closure-fn-trait-bounds` — observed

Thread starter consumes task exactly once with FnOnce; repeated row callbacks use FnMut; cancellation remains repeatable read-only Fn.

Fn may read Cell/atomic state; it does not imply pure computation or no external effect.

Evidence: [`crates/application/tablecraft-runtime/src/thread_start.rs:5`](../../crates/application/tablecraft-runtime/src/thread_start.rs); [`crates/query/tablecraft-query/src/execution/multivalue/parent/joined.rs:56`](../../crates/query/tablecraft-query/src/execution/multivalue/parent/joined.rs); [`crates/query/tablecraft-query/src/source/progressive.rs:11`](../../crates/query/tablecraft-query/src/source/progressive.rs).

## `closure-impl-fn-return` — conditional

Reviewed actor closure is passed generically to Spawner and then OS thread; no one-concrete-closure Box return was found here.

Stored repaint Arc<dyn Fn+Send+Sync> genuinely outlives setup and is heterogeneous; it cannot simply return borrowed impl Fn.

Evidence: [`crates/application/tablecraft-runtime/src/worker.rs:582`](../../crates/application/tablecraft-runtime/src/worker.rs); [`crates/application/tablecraft-runtime/src/thread_start.rs:5`](../../crates/application/tablecraft-runtime/src/thread_start.rs).

## `closure-move-capture` — observed

Worker clones the shared Arc then moves it into the actor; owned executor/document stays in the actor and mutable mailboxes remain synchronized.

Do not replace shared owner with a borrowed stack reference or deep-clone the native document.

Native sources/caches are intentionally retained on one worker.

Evidence: [`crates/application/tablecraft-runtime/src/worker.rs:582`](../../crates/application/tablecraft-runtime/src/worker.rs); [`crates/application/tablecraft-runtime/src/worker.rs:75`](../../crates/application/tablecraft-runtime/src/worker.rs).

## `closure-static-vs-dyn` — observed

Generic FnMut evaluation hooks can inline with runtime; borrowed cancellation dyn Fn avoids boxing and explosion across IO/native call chains; repaint is stored.

No benchmark result asserted for either dispatch style.

Evidence: [`crates/platform/tablecraft-io/src/read.rs:13`](../../crates/platform/tablecraft-io/src/read.rs); [`crates/query/tablecraft-query/src/execution/multivalue/parent/joined.rs:56`](../../crates/query/tablecraft-query/src/execution/multivalue/parent/joined.rs); [`crates/application/tablecraft-runtime/src/worker.rs:31`](../../crates/application/tablecraft-runtime/src/worker.rs).

## `coll-binaryheap` — conditional

Inspected query owner stores plans and performs bounded catalog lookup, not repeated dynamic priority extraction. A heap would alter semantics and add budgeted storage without a demonstrated workload.

No whole-repository absence proof or profiling claim; this is applicability of reviewed real owners only.

Evidence: [`crates/query/tablecraft-query/src/plan.rs:120`](../../crates/query/tablecraft-query/src/plan.rs); [`crates/foundation/tablecraft-model/src/catalog.rs:62`](../../crates/foundation/tablecraft-model/src/catalog.rs).

## `coll-map-choice` — observed

Font inventory BTreeMap provides deterministic lowercase-key ordering and first canonical spelling; template BTreeMap provides ordered package-part inventory.

Native identifier collation is not font lowercase; do not reuse a generic hash/name normalization policy.

HashMap substitution would lose deterministic iteration unless explicitly restored.

Evidence: [`crates/presentation/tablecraft-ui/src/fonts/inventory.rs:39`](../../crates/presentation/tablecraft-ui/src/fonts/inventory.rs); [`crates/presentation/tablecraft-ui/src/fonts/inventory.rs:79`](../../crates/presentation/tablecraft-ui/src/fonts/inventory.rs); [`crates/formats/tablecraft-formats/src/template.rs:95`](../../crates/formats/tablecraft-formats/src/template.rs).

## `coll-seq-choice` — observed

Plans use contiguous Vec for append/index/ordered traversal; joined parent scratch is reused. Neither is a FIFO pop-front queue.

No observed LinkedList or repeatedly shifting queue in these owners; no broad no-queue claim.

Preserve written expression order and native snapshot order.

Evidence: [`crates/query/tablecraft-query/src/plan.rs:120`](../../crates/query/tablecraft-query/src/plan.rs); [`crates/query/tablecraft-query/src/execution/multivalue/parent/joined.rs:78`](../../crates/query/tablecraft-query/src/execution/multivalue/parent/joined.rs).

## `coll-set-membership` — observed

ObjectCatalog uses a fallibly reserved HashSet for full ID membership; tiny fixed seven-pane focus uses Vec.contains and a fixed array ordering.

Hashing is not inherently better for <=7 items and requires additional allocation.

Name comparisons with explicit native policy cannot be changed to derived Hash/Eq without proof.

Evidence: [`crates/foundation/tablecraft-model/src/catalog.rs:24`](../../crates/foundation/tablecraft-model/src/catalog.rs); [`crates/presentation/tablecraft-ui/src/workspace/focus.rs:17`](../../crates/presentation/tablecraft-ui/src/workspace/focus.rs).

## `conc-atomic-ordering` — not verified

Observed Acquire/Release/AcqRel boundaries and mutex-owned linked state, rather than indiscriminate SeqCst. Full concurrent interleaving proof/loom validation of multiple generation/active/liveness atomics has not run. Do not mark all atomic orderings safe from syntax alone.

Evidence: [`crates/application/tablecraft-runtime/src/worker.rs:743`](../../crates/application/tablecraft-runtime/src/worker.rs); [`crates/application/tablecraft-runtime/src/worker.rs:790`](../../crates/application/tablecraft-runtime/src/worker.rs); [`crates/application/tablecraft-runtime/src/editing/native/save.rs:209`](../../crates/application/tablecraft-runtime/src/editing/native/save.rs); [`apps/tablecraft-desktop/src/font_inventory.rs:44`](../../apps/tablecraft-desktop/src/font_inventory.rs).

## `conc-rayon-par-iter` — observed

Encryption uses caller-borrowed Rayon pool and disjoint par_chunks plus original-order outcome slots; I/O stays coordinator-side and all page phases join before buffers/resource reuse. No new speedup claim made.

Evidence: [`crates/engines/tablecraft-ace/src/encryption/parallel/transfer.rs:137`](../../crates/engines/tablecraft-ace/src/encryption/parallel/transfer.rs); [`crates/engines/tablecraft-ace/src/encryption/parallel/transfer.rs:138`](../../crates/engines/tablecraft-ace/src/encryption/parallel/transfer.rs); [`crates/engines/tablecraft-ace/src/encryption/parallel.rs:41`](../../crates/engines/tablecraft-ace/src/encryption/parallel.rs).

## `conc-scoped-threads` — conditional

Long-lived source/save actors genuinely outlive call stack and use Arc; scoped threads would join/block host and violate lifecycle. Short page parallelism already borrows through Rayon scope. No raw short-lived borrowing thread gap identified.

Evidence: [`crates/application/tablecraft-runtime/src/worker.rs:73`](../../crates/application/tablecraft-runtime/src/worker.rs); [`crates/application/tablecraft-runtime/src/editing/native/save.rs:205`](../../crates/application/tablecraft-runtime/src/editing/native/save.rs); [`crates/engines/tablecraft-ace/src/encryption/parallel/transfer.rs:137`](../../crates/engines/tablecraft-ace/src/encryption/parallel/transfer.rs).

## `conc-thread-local` — not applicable

Authored apps/crates search found no static mut or thread_local scratch state. Immutable global constants/OnceLock and owned actors are real alternatives, not missing thread-local wrappers.

Evidence: [`Cargo.toml:197`](../../Cargo.toml); [`crates/platform/tablecraft-io/src/lib.rs:32`](../../crates/platform/tablecraft-io/src/lib.rs).

## `const-block` — conditional

Compile-time widths use named owning constants and size_of; a new anonymous const block is useful only for a concrete invariant. No missing compile-time assertion bug was demonstrated.

Evidence: [`crates/engines/tablecraft-ace/src/table/indexes/mutation/encoding.rs:26`](../../crates/engines/tablecraft-ace/src/table/indexes/mutation/encoding.rs).

## `const-fn` — observed

Pure identity/declaration constructors and accessors are const. This is representative source evidence, not a claim that every const-eligible function is annotated.

A published const-callable API must not later lose const support casually, despite the skill note suggesting removal is always harmless.

Evidence: [`crates/foundation/tablecraft-core/src/ids/identity.rs:11`](../../crates/foundation/tablecraft-core/src/ids/identity.rs); [`crates/engines/tablecraft-ace/src/creation/column.rs:36`](../../crates/engines/tablecraft-ace/src/creation/column.rs).

## `const-generics` — observed

Actual native key encoding uses const N arrays rather than duplicated width-specific implementations or trait-object buffers.

Evidence: [`crates/engines/tablecraft-ace/src/table/indexes/mutation/encoding.rs:26`](../../crates/engines/tablecraft-ace/src/table/indexes/mutation/encoding.rs).

## `const-vs-static` — observed

Inspected small identity/format values are owning constants; mutable worker state uses atomics/owned mailboxes. No static-mut policy expansion is proposed.

Evidence: [`crates/foundation/tablecraft-core/src/ids/identity.rs:42`](../../crates/foundation/tablecraft-core/src/ids/identity.rs); [`crates/engines/tablecraft-ace/src/format/text/ascii.rs:2`](../../crates/engines/tablecraft-ace/src/format/text/ascii.rs).

## `conv-asmut-mutable` — observed

read_exact accepts &mut[u8], which already admits arrays/Vec coercions while retaining clear in-place semantics; caller owns buffers and byte charge.

AsMut is optional when several concrete borrowed forms need generic ownership flexibility; adding generics here offers no demonstrated benefit.

Evidence: [`crates/platform/tablecraft-io/src/read.rs:15`](../../crates/platform/tablecraft-io/src/read.rs).

## `conv-fromstr-parsing` — conditional

Validated schema names use a consuming String constructor; native Text declarations require additional width/operation policy. A context-free FromStr for these would not replace those checks.

No repeated standalone domain parser needing .parse ergonomics was established in these source owners.

Explicit query/locale/native parsing limits must not become implicit global defaults via FromStr.

Evidence: [`crates/foundation/tablecraft-schema/src/name.rs:11`](../../crates/foundation/tablecraft-schema/src/name.rs); [`crates/engines/tablecraft-ace/src/creation/column.rs:78`](../../crates/engines/tablecraft-ace/src/creation/column.rs).

## `conv-tryfrom-fallible` — observed

Fallible expression storage and Decimal wire representation use TryFrom with concrete typed errors.

Native qualification requiring source/budget/cancel cannot be expressed as context-free TryFrom safely.

Evidence: [`crates/foundation/tablecraft-core/src/values/expression.rs:25`](../../crates/foundation/tablecraft-core/src/values/expression.rs); [`crates/foundation/tablecraft-core/src/values/decimal.rs:86`](../../crates/foundation/tablecraft-core/src/values/decimal.rs).

## `doc-all-public` — finding

Critical runtime start-error variants and ownership extraction are externally visible but incompletely documented. In particular into_reviewed returns None for recovery/release rejection, while the type-level sentence promises a returned capability. The current CLI after_help also contains a directly contradicted GUI-host status sentence (STRUCT-03).

Bounded correction should document actual existing variants/ownership, not publish private proof types or enable writes.

Evidence: [`crates/application/tablecraft-runtime/src/editing/save.rs:186`](../../crates/application/tablecraft-runtime/src/editing/save.rs); [`apps/tablecraft-cli/src/main.rs:27`](../../apps/tablecraft-cli/src/main.rs); [`apps/tablecraft-desktop/src/main.rs:43`](../../apps/tablecraft-desktop/src/main.rs); [`Cargo.toml:24`](../../Cargo.toml).

## `doc-cargo-metadata` — conditional

All 20 parsed packages are publish=false and inherit version/license/repository/MSRV. Crates.io keyword/category/docs.rs tuning is not a release requirement for these unpublished workspace members.

Evidence: [`Cargo.toml:1`](../../Cargo.toml); [`crates/query/tablecraft-query/Cargo.toml:31`](../../crates/query/tablecraft-query/Cargo.toml).

## `doc-crate-readme` — finding

Current executable CLI help says GUI hosts are not implemented, despite the actual desktop default member and entry point. Correct that current operational sentence in its owner; do not automatically include the repository README into every crate.

There is a separate concrete stale CLI help sentence; it should be corrected in its actual executable owner rather than homogenizing all README/rustdoc surfaces.

Evidence: [`crates/foundation/tablecraft-core/src/lib.rs:8`](../../crates/foundation/tablecraft-core/src/lib.rs); [`crates/query/tablecraft-query/src/lib.rs:4`](../../crates/query/tablecraft-query/src/lib.rs); [`apps/tablecraft-cli/src/main.rs:27`](../../apps/tablecraft-cli/src/main.rs); [`apps/tablecraft-desktop/src/main.rs:43`](../../apps/tablecraft-desktop/src/main.rs); [`Cargo.toml:24`](../../Cargo.toml).

## `doc-errors-section` — finding

Creation public factories use only short prose and do not enumerate typed rejection conditions, and runtime extraction omits ownership-return exceptions. Document a bounded set of key constructors/start methods with actual error classes and returned-owner behavior; do not claim a generic Errors heading alone proves coverage. The concrete benchmark Failure additionally owns InspectError but omits Error::source; this source-chain finding is primarily err-source-chain and is cross-linked here without treating missing headings as lost error ownership.

Evidence: [`crates/engines/tablecraft-ace/src/creation/column.rs:78`](../../crates/engines/tablecraft-ace/src/creation/column.rs); [`crates/application/tablecraft-runtime/src/editing/save.rs:186`](../../crates/application/tablecraft-runtime/src/editing/save.rs); [`crates/application/tablecraft-runtime/src/editing/native/save.rs:671`](../../crates/application/tablecraft-runtime/src/editing/native/save.rs); [`crates/engines/tablecraft-ace/benches/encryption_pages.rs:66`](../../crates/engines/tablecraft-ace/benches/encryption_pages.rs); [`crates/engines/tablecraft-ace/benches/encryption_pages.rs:80`](../../crates/engines/tablecraft-ace/benches/encryption_pages.rs).

## `doc-examples-section` — observed

The query crate provides a real public API executable example and CI runs workspace doctests plus warning-denied rustdoc. This pass did not execute doctests or claim all public functions have examples.

Evidence: [`crates/query/tablecraft-query/src/lib.rs:4`](../../crates/query/tablecraft-query/src/lib.rs); [`.github/workflows/ci.yml:176`](../../.github/workflows/ci.yml).

## `doc-hidden-setup` — observed

The actual query crate example propagates failure with ? and hides its final Result boilerplate. Native compile_fail examples intentionally demonstrate rejected ownership and need not imitate successful error propagation.

Evidence: [`crates/query/tablecraft-query/src/lib.rs:17`](../../crates/query/tablecraft-query/src/lib.rs).

## `doc-intra-links` — observed

read_exact_at links its actual helper and CI denies rustdoc warnings. Link correctness is represented by source/CI configuration here, not a newly executed rustdoc gate.

Evidence: [`crates/platform/tablecraft-io/src/read.rs:13`](../../crates/platform/tablecraft-io/src/read.rs); [`.github/workflows/ci.yml:176`](../../.github/workflows/ci.yml).

## `doc-link-types` — observed

read_exact_at links its actual helper and CI denies rustdoc warnings. Link correctness is represented by source/CI configuration here, not a newly executed rustdoc gate.

Evidence: [`crates/platform/tablecraft-io/src/read.rs:13`](../../crates/platform/tablecraft-io/src/read.rs); [`.github/workflows/ci.yml:176`](../../.github/workflows/ci.yml).

## `doc-module-inner` — observed

Inspected crate/feature owners use inner module documentation rather than attaching overview text accidentally to the next item. Not all 5,000+ modules were checked individually for prose quality.

Evidence: [`crates/foundation/tablecraft-core/src/lib.rs:8`](../../crates/foundation/tablecraft-core/src/lib.rs); [`crates/platform/tablecraft-io/src/read.rs:13`](../../crates/platform/tablecraft-io/src/read.rs).

## `doc-panics-section` — finding

Public runtime Worker request/poll and creation/save methods now intentionally expect unpoisoned internal mailboxes, but their API comments do not state the fatal poisoning/invariant boundary. Document that bounded contract; do not fabricate recovery or change native save uncertainty.

Evidence: [`crates/application/tablecraft-runtime/src/worker.rs:58`](../../crates/application/tablecraft-runtime/src/worker.rs); [`crates/application/tablecraft-runtime/src/worker.rs:586`](../../crates/application/tablecraft-runtime/src/worker.rs).

## `doc-question-mark` — observed

The actual query crate example propagates failure with ? and hides its final Result boilerplate. Native compile_fail examples intentionally demonstrate rejected ownership and need not imitate successful error propagation.

Evidence: [`crates/query/tablecraft-query/src/lib.rs:17`](../../crates/query/tablecraft-query/src/lib.rs).

## `doc-safety-section` — conditional

The assigned practical safety inspection found no public unsafe API needing a new caller contract; private platform ABI internals remain with their actual safety owner. This record does not replace the other safety agent review.

Evidence: [`crates/platform/tablecraft-io/src/read.rs:13`](../../crates/platform/tablecraft-io/src/read.rs).

## `err-anyhow-app` — conditional

CLI contextual executable failure reporting and runtime matchable thiserror are deliberate distinct boundaries. Desktop must retain typed save/resource variants; replacing them with anyhow would lose control semantics.

Repository explicitly requires typed library errors and contextual reporting at executable boundaries.

Evidence: [`apps/tablecraft-cli/src/main.rs:40`](../../apps/tablecraft-cli/src/main.rs); [`crates/application/tablecraft-runtime/src/error.rs:16`](../../crates/application/tablecraft-runtime/src/error.rs).

## `err-context-chain` — observed

Runtime wraps domain errors transparently with From; executable formats failures at handling boundary. Payload privacy restricts attaching literal paths/names to tracing contexts. This is sampled owner coverage, not every error site.

Evidence: [`crates/application/tablecraft-runtime/src/error.rs:22`](../../crates/application/tablecraft-runtime/src/error.rs); [`crates/application/tablecraft-runtime/src/error.rs:133`](../../crates/application/tablecraft-runtime/src/error.rs); [`apps/tablecraft-cli/src/main.rs:93`](../../apps/tablecraft-cli/src/main.rs).

## `err-custom-type` — observed

Domain read/save failures expose typed enums usable for source/cancel/resource/recovery classification; no public String error is introduced by actors. Generic boxed diagnostic subscriber error is executable-private.

Evidence: [`crates/application/tablecraft-runtime/src/error.rs:16`](../../crates/application/tablecraft-runtime/src/error.rs); [`crates/application/tablecraft-runtime/src/editing/save.rs:1`](../../crates/application/tablecraft-runtime/src/editing/save.rs).

## `err-doc-errors` — finding

Public Worker methods lack explicit # Errors/# Panics contract sections; request returns u64 while hidden generation exhaustion can panic or reuse zero. Constructor startup behavior is documented only indirectly in request prose.

Evidence: [`crates/application/tablecraft-runtime/src/worker.rs:79`](../../crates/application/tablecraft-runtime/src/worker.rs); [`crates/application/tablecraft-runtime/src/worker.rs:737`](../../crates/application/tablecraft-runtime/src/worker.rs); [`crates/application/tablecraft-runtime/src/worker.rs:803`](../../crates/application/tablecraft-runtime/src/worker.rs).

## `err-expect-bugs-only` — observed

Changed mailbox locks use expect with poison reasons; poisoned linked state is not recovered via into_inner. Native calls return typed errors for expected I/O/source failures. A blanket ban on invariant expect is not warranted.

Earlier accepted Beautiful Rust policy treats poisoned locks as fatal, not recoverable.

Evidence: [`crates/application/tablecraft-runtime/src/editing/native/save.rs:224`](../../crates/application/tablecraft-runtime/src/editing/native/save.rs); [`crates/application/tablecraft-runtime/src/worker.rs:754`](../../crates/application/tablecraft-runtime/src/worker.rs); [`crates/application/tablecraft-runtime/src/creation/worker.rs:96`](../../crates/application/tablecraft-runtime/src/creation/worker.rs).

## `err-from-impl` — observed

thiserror From conversions retain actual typed domain causes; validated serde TryFrom reuses constructor failure. Explicit map_err remains needed where discriminating Source-only refusal from cancellation/resource failure.

Evidence: [`crates/application/tablecraft-runtime/src/error.rs:22`](../../crates/application/tablecraft-runtime/src/error.rs); [`crates/application/tablecraft-runtime/src/error.rs:137`](../../crates/application/tablecraft-runtime/src/error.rs); [`crates/foundation/tablecraft-core/src/values/decimal.rs:87`](../../crates/foundation/tablecraft-core/src/values/decimal.rs).

## `err-lowercase-msg` — observed

Inspected runtime Display messages begin lowercase without trailing punctuation; operation acronyms inside messages are not sentence capitalization. Localized UI prose is a different contract. Whole repository text audit not claimed.

Evidence: [`crates/application/tablecraft-runtime/src/error.rs:19`](../../crates/application/tablecraft-runtime/src/error.rs); [`crates/application/tablecraft-runtime/src/error.rs:30`](../../crates/application/tablecraft-runtime/src/error.rs); [`crates/application/tablecraft-runtime/src/error.rs:80`](../../crates/application/tablecraft-runtime/src/error.rs).

## `err-no-unwrap-prod` — not verified

Reviewed actors replaced bare lock unwrap with invariant expect; tests legitimately unwrap fixture results. Root/other lane owns complete per-site panic ledger; this record does not certify every production unwrap. Paused SQLite .lock().ok() loses poison reporting rather than recovering state.

Evidence: [`crates/application/tablecraft-runtime/src/worker.rs:754`](../../crates/application/tablecraft-runtime/src/worker.rs); [`crates/application/tablecraft-runtime/src/editing/native/save.rs:224`](../../crates/application/tablecraft-runtime/src/editing/native/save.rs); [`crates/backends/tablecraft-sqlite/src/work.rs:74`](../../crates/backends/tablecraft-sqlite/src/work.rs).

## `err-question-mark` — observed

Reviewed constructors, operations and adapters propagate fallible sources through ?/From. Pattern matches remain required for retained-resource outcome control and should not be compressed into obscuring combinators.

Evidence: [`crates/application/tablecraft-runtime/src/error.rs:137`](../../crates/application/tablecraft-runtime/src/error.rs); [`crates/foundation/tablecraft-core/src/values/decimal.rs:52`](../../crates/foundation/tablecraft-core/src/values/decimal.rs); [`crates/backends/tablecraft-sqlite/src/worker/reader.rs:59`](../../crates/backends/tablecraft-sqlite/src/worker/reader.rs).

## `err-result-over-panic` — finding

Save execution actor death is not terminally observable; read generation fetch_add+1 can overflow despite recoverable issuance-exhaustion semantics. Actual save publication is uncertain after panic, so a fabricated Failed/no-write Result is unacceptable.

Evidence: [`crates/application/tablecraft-runtime/src/editing/native/save.rs:269`](../../crates/application/tablecraft-runtime/src/editing/native/save.rs); [`crates/application/tablecraft-runtime/src/editing/native/save.rs:399`](../../crates/application/tablecraft-runtime/src/editing/native/save.rs); [`crates/application/tablecraft-runtime/src/worker.rs:743`](../../crates/application/tablecraft-runtime/src/worker.rs).

## `err-source-chain` — observed

Transparent thiserror wrappers preserve domain source chains in reviewed runtime boundary. Tracing intentionally does not print full potentially sensitive source payloads; use sanitized typed classification there.

Repository tracing policy forbids names/paths/SQL/row values even when errors carry them.

Evidence: [`crates/application/tablecraft-runtime/src/error.rs:22`](../../crates/application/tablecraft-runtime/src/error.rs); [`crates/application/tablecraft-runtime/src/error.rs:133`](../../crates/application/tablecraft-runtime/src/error.rs); [`crates/application/tablecraft-runtime/src/error.rs:144`](../../crates/application/tablecraft-runtime/src/error.rs).

## `err-thiserror-lib` — observed

Runtime uses thiserror and matchable typed wrappers; core constructor failures remain typed ValueError. No recommendation to replace library contracts with anyhow.

Evidence: [`crates/application/tablecraft-runtime/src/error.rs:16`](../../crates/application/tablecraft-runtime/src/error.rs); [`crates/foundation/tablecraft-core/src/values/decimal.rs:87`](../../crates/foundation/tablecraft-core/src/values/decimal.rs).

## `lint-cargo-metadata` — conditional

All 20 parsed packages are publish=false and inherit version/license/repository/MSRV. Crates.io keyword/category/docs.rs tuning is not a release requirement for these unpublished workspace members.

Evidence: [`Cargo.toml:1`](../../Cargo.toml); [`crates/query/tablecraft-query/Cargo.toml:31`](../../crates/query/tablecraft-query/Cargo.toml).

## `lint-cfg-check` — observed

The pinned Cargo/rustc supports built-in unexpected_cfgs and CI checks declared feature combinations with -D warnings. No custom cfg requiring registration was confirmed; no nonexistent feature scaffold is added.

Evidence: [`rust-toolchain.toml:2`](../../rust-toolchain.toml); [`.github/workflows/ci.yml:94`](../../.github/workflows/ci.yml).

## `lint-clippy-nursery-selected` — conditional

Only the all baseline is configured; selective higher-noise lints require concrete value. The missing must_use on consuming policy builders is separately confirmed as a bounded API-hardening finding, not a request to enable whole pedantic/nursery groups.

Evidence: [`Cargo.toml:196`](../../Cargo.toml); [`crates/engines/tablecraft-ace/src/creation/column.rs:150`](../../crates/engines/tablecraft-ace/src/creation/column.rs).

## `lint-deny-correctness` — observed

Workspace clippy::all is warn and CI runs clippy with -D warnings, making correctness diagnostics errors at the gate. An additional redundant per-crate deny is not needed to claim the configured gate exists.

Evidence: [`Cargo.toml:196`](../../Cargo.toml); [`.github/workflows/ci.yml:112`](../../.github/workflows/ci.yml).

## `lint-missing-docs` — finding

Most public libraries deny missing_docs, while runtime exposes undocumented ownership/error methods. Add bounded public contract docs first; do not enable a new global denial before understanding unrelated host/UI documentation volume.

Evidence: [`crates/foundation/tablecraft-core/src/lib.rs:8`](../../crates/foundation/tablecraft-core/src/lib.rs); [`crates/application/tablecraft-runtime/src/editing/save.rs:186`](../../crates/application/tablecraft-runtime/src/editing/save.rs).

## `lint-pedantic-selective` — finding

A concrete consuming-policy API omission is confirmed: Copy CreateColumn::with_required returns Self without type/method must_use, allowing the requested policy to be silently discarded. Annotate that actual API; selectively enabling a lint is optional and enabling the whole pedantic group is not proposed. This is primarily api-builder-must-use/api-must-use, cross-linked here as the assigned enforcement rule.

Evidence: [`Cargo.toml:196`](../../Cargo.toml); [`crates/engines/tablecraft-ace/src/creation/column.rs:150`](../../crates/engines/tablecraft-ace/src/creation/column.rs); [`crates/engines/tablecraft-ace/src/creation/column.rs:14`](../../crates/engines/tablecraft-ace/src/creation/column.rs); [`crates/engines/tablecraft-ace/src/creation/column.rs:150`](../../crates/engines/tablecraft-ace/src/creation/column.rs); [`crates/engines/tablecraft-ace/src/table/complex/page/request.rs:24`](../../crates/engines/tablecraft-ace/src/table/complex/page/request.rs).

## `lint-rustfmt-check` — observed

CI invokes the actual bounded formatter script over every Cargo target, followed by the repository layout guard. Formatter success is not used as a substitute for manual logical spacing.

Evidence: [`.github/workflows/ci.yml:62`](../../.github/workflows/ci.yml).

## `lint-unsafe-doc` — observed

Workspace unsafe_code=forbid is stricter than a generic undocumented-unsafe lint for ordinary crates; actual Windows exceptions are narrow owning modules with SAFETY comments reviewed separately. No broad allowance is proposed.

Evidence: [`Cargo.toml:196`](../../Cargo.toml).

## `lint-warn-complexity` — observed

Workspace clippy::all=warn covers these standard groups and all 20 manifests inherit workspace lints. Gate execution remains root-owned; configuration alone is not a pass result.

Evidence: [`Cargo.toml:196`](../../Cargo.toml); [`.github/workflows/ci.yml:112`](../../.github/workflows/ci.yml).

## `lint-warn-perf` — observed

Workspace clippy::all=warn covers these standard groups and all 20 manifests inherit workspace lints. Gate execution remains root-owned; configuration alone is not a pass result.

Evidence: [`Cargo.toml:196`](../../Cargo.toml); [`.github/workflows/ci.yml:112`](../../.github/workflows/ci.yml).

## `lint-warn-style` — observed

Workspace clippy::all=warn covers these standard groups and all 20 manifests inherit workspace lints. Gate execution remains root-owned; configuration alone is not a pass result.

Evidence: [`Cargo.toml:196`](../../Cargo.toml); [`.github/workflows/ci.yml:112`](../../.github/workflows/ci.yml).

## `lint-warn-suspicious` — observed

Workspace clippy::all=warn covers these standard groups and all 20 manifests inherit workspace lints. Gate execution remains root-owned; configuration alone is not a pass result.

Evidence: [`Cargo.toml:196`](../../Cargo.toml); [`.github/workflows/ci.yml:112`](../../.github/workflows/ci.yml).

## `lint-workspace-lints` — observed

Workspace clippy::all=warn covers these standard groups and all 20 manifests inherit workspace lints. Gate execution remains root-owned; configuration alone is not a pass result.

Evidence: [`Cargo.toml:196`](../../Cargo.toml); [`.github/workflows/ci.yml:112`](../../.github/workflows/ci.yml).

## `macro-export-crate-path` — not applicable

The repository-wide scan found no #[macro_export] definitions. Actual local macro owners do not require a public __private facade or exported macro relay.

Do not add public implementation scaffolding when no exported macro consumer exists.

Evidence: [`crates/foundation/tablecraft-core/src/ids/identity.rs:3`](../../crates/foundation/tablecraft-core/src/ids/identity.rs).

## `macro-fragment-specifiers` — observed

Identity generation captures ident/literal, crypto transform captures ty, and section embedding captures literal; inspected local macros use precise fragments for their real grammar.

Evidence: [`crates/foundation/tablecraft-core/src/ids/identity.rs:3`](../../crates/foundation/tablecraft-core/src/ids/identity.rs); [`crates/platform/tablecraft-crypto/src/aes.rs:24`](../../crates/platform/tablecraft-crypto/src/aes.rs); [`crates/presentation/tablecraft-ui/src/localization/catalogs.rs:3`](../../crates/presentation/tablecraft-ui/src/localization/catalogs.rs).

## `macro-prefer-functions` — observed

Inspected macros generate repeated distinct type/impl items or cipher type instantiations; ordinary runtime work remains in functions. No fixed-arity computation macro requiring replacement was confirmed.

Evidence: [`crates/foundation/tablecraft-core/src/ids/identity.rs:3`](../../crates/foundation/tablecraft-core/src/ids/identity.rs); [`crates/platform/tablecraft-crypto/src/aes.rs:24`](../../crates/platform/tablecraft-crypto/src/aes.rs).

## `macro-private-helpers` — not applicable

The repository-wide scan found no #[macro_export] definitions. Actual local macro owners do not require a public __private facade or exported macro relay.

Do not add public implementation scaffolding when no exported macro consumer exists.

Evidence: [`crates/foundation/tablecraft-core/src/ids/identity.rs:3`](../../crates/foundation/tablecraft-core/src/ids/identity.rs).

## `macro-proc-error-spans` — not applicable

All 20 package manifests were parsed and none declares lib.proc-macro=true. This project consumes upstream derives but does not own a proc-macro implementation to audit.

Evidence: [`Cargo.toml:1`](../../Cargo.toml).

## `macro-proc-syn-quote` — not applicable

All 20 package manifests were parsed and none declares lib.proc-macro=true. This project consumes upstream derives but does not own a proc-macro implementation to audit.

Evidence: [`Cargo.toml:1`](../../Cargo.toml).

## `macro-proc-two-crate` — not applicable

All 20 package manifests were parsed and none declares lib.proc-macro=true. This project consumes upstream derives but does not own a proc-macro implementation to audit.

Evidence: [`Cargo.toml:1`](../../Cargo.toml).

## `macro-rules-hygiene` — conditional

Actual macros are local to the defining module, with intentionally captured item names and literal paths. Cross-crate $crate hygiene is not needed without exports; future exported macros would require a new review.

Evidence: [`crates/foundation/tablecraft-core/src/ids/identity.rs:3`](../../crates/foundation/tablecraft-core/src/ids/identity.rs); [`crates/presentation/tablecraft-ui/src/localization/catalogs.rs:3`](../../crates/presentation/tablecraft-ui/src/localization/catalogs.rs).

## `mem-arena-allocator` — conditional

Query plans persist beyond parser frames and journal/review owners outlive requests. An arena lifetime migration would be an architecture change, not a local optimization.

No arena performance experiment was run. The skill scratch example resets while a RefCell shared borrow remains live and must not be copied literally.

No new allocator dependency without measured consumer and drop/charge proof.

Evidence: [`crates/query/tablecraft-query/src/plan.rs:192`](../../crates/query/tablecraft-query/src/plan.rs); [`crates/query/tablecraft-query/src/plan.rs:120`](../../crates/query/tablecraft-query/src/plan.rs); [`crates/platform/tablecraft-io/src/journal/transaction.rs:13`](../../crates/platform/tablecraft-io/src/journal/transaction.rs).

## `mem-arrayvec` — conditional

Actual hard-bound focus order and native read buffers already use arrays/slices. No variable-length no-heap container was shown to need ArrayVec.

ArrayVec::push/collect examples can panic at capacity; typed refusal remains required.

Do not add a dependency just to replace a seven-element fixed array.

Evidence: [`crates/presentation/tablecraft-ui/src/workspace/focus.rs:17`](../../crates/presentation/tablecraft-ui/src/workspace/focus.rs); [`crates/platform/tablecraft-io/src/read.rs:15`](../../crates/platform/tablecraft-io/src/read.rs).

## `mem-assert-type-size` — not verified

Slot allocations use size_of, so budget charges track compiled representation. There is no measured size ceiling/ABI contract for these logical types in this review.

A platform-independent exact size assertion cannot be inferred from 64-bit example values; no build/type-size tool was run.

Add assertions only for actual format/FFI or measured hot representation contracts.

Evidence: [`crates/query/tablecraft-query/src/binding/relation/slots.rs:43`](../../crates/query/tablecraft-query/src/binding/relation/slots.rs); [`crates/engines/tablecraft-ace/src/creation/column.rs:15`](../../crates/engines/tablecraft-ace/src/creation/column.rs).

## `mem-avoid-format` — observed

Static preview marker and fallback message identifiers are borrowed literals; final UI copy is owned. No confirmed constant-only format allocation in reviewed production paths.

Error/startup formatting and retained localized output can legitimately allocate; static snippets alone do not measure hot paths.

Evidence: [`crates/presentation/tablecraft-ui/src/localization.rs:187`](../../crates/presentation/tablecraft-ui/src/localization.rs); [`crates/presentation/tablecraft-ui/src/datasheet/preview.rs:18`](../../crates/presentation/tablecraft-ui/src/datasheet/preview.rs).

## `mem-box-large-variant` — observed

Recursive plan branches use Box for finite layout and large nested plans. This is direct structural evidence, not measured optimum for every enum.

No new size measurements were run; blanket >256-byte boxing would add uncharged allocations.

Evidence: [`crates/query/tablecraft-query/src/plan.rs:192`](../../crates/query/tablecraft-query/src/plan.rs); [`crates/query/tablecraft-query/src/plan.rs:120`](../../crates/query/tablecraft-query/src/plan.rs).

## `mem-boxed-slice` — observed

FontFamilies retains immutable Arc<[String]>; native immutable bytes retain whole-capacity witness. Both encode immutability and ownership purpose.

Converting native Vec to boxed slice could shrink/reallocate and conceal its original retained capacity.

Preserve capacity provenance, not just active length.

Evidence: [`crates/presentation/tablecraft-ui/src/fonts/inventory.rs:20`](../../crates/presentation/tablecraft-ui/src/fonts/inventory.rs); [`crates/engines/tablecraft-ace/src/preservation/image/source.rs:17`](../../crates/engines/tablecraft-ace/src/preservation/image/source.rs).

## `mem-clone-from` — conditional

Current reviewed clone examples either create an independent owner or compact active bytes; neither has a preexisting mutable destination to reuse.

Generic clone_from may allocate infallibly and retain excess capacity; it cannot replace paid fallible copies without an owning contract.

No blanket replacement of shared snapshots or exact-capacity tests.

Evidence: [`crates/foundation/tablecraft-core/src/values/expression_string.rs:19`](../../crates/foundation/tablecraft-core/src/values/expression_string.rs); [`crates/presentation/tablecraft-builders/src/query/build.rs:447`](../../crates/presentation/tablecraft-builders/src/query/build.rs).

## `mem-compact-string` — conditional

Schema names and slots own Strings. No workload distribution/retained-memory measurement establishes replacing them with compact string crates.

Public &str/String boundaries remain portable; compact representations change capacity accounting.

Do not force an unreviewed dependency or normalize native names.

Evidence: [`crates/foundation/tablecraft-schema/src/name.rs:11`](../../crates/foundation/tablecraft-schema/src/name.rs); [`crates/query/tablecraft-query/src/binding/relation/slots.rs:10`](../../crates/query/tablecraft-query/src/binding/relation/slots.rs).

## `mem-drop-order` — observed

Worker explicitly releases mailbox guard before native execution; actor catches panics while owned executor is inside the closure. PreparedImage dropping intentionally leaves journal intact, documented at type.

No automated proof of all repository destructors; these are concrete reviewed drop contracts.

Never add best-effort silent rollback to satisfy a generic RAII example.

Evidence: [`crates/application/tablecraft-runtime/src/worker.rs:582`](../../crates/application/tablecraft-runtime/src/worker.rs); [`crates/application/tablecraft-runtime/src/worker.rs:840`](../../crates/application/tablecraft-runtime/src/worker.rs); [`crates/platform/tablecraft-io/src/journal/transaction.rs:13`](../../crates/platform/tablecraft-io/src/journal/transaction.rs).

## `mem-reuse-collections` — observed

Joined parent evaluation charges and reserves views/absent vectors outside the row loop, then clears/reuses them; row values are moved into output.

Fresh row output vectors are intentionally independent retained results, not scratch to clear.

Evidence: [`crates/query/tablecraft-query/src/execution/multivalue/parent/joined.rs:78`](../../crates/query/tablecraft-query/src/execution/multivalue/parent/joined.rs).

## `mem-smaller-integers` — observed

Decimal scale is u8; identity is u128 and budgets are u64 for explicit full-range domains. Narrowing the latter for size would violate tested representation limits.

No repr(Rust) field-layout size claim is made; compiler may reorder ordinary fields.

Preserve native high-water/identity widths and checked conversions.

Evidence: [`crates/foundation/tablecraft-core/src/values/decimal.rs:31`](../../crates/foundation/tablecraft-core/src/values/decimal.rs); [`crates/foundation/tablecraft-core/src/ids/identity.rs:7`](../../crates/foundation/tablecraft-core/src/ids/identity.rs); [`crates/foundation/tablecraft-core/src/limits.rs:31`](../../crates/foundation/tablecraft-core/src/limits.rs).

## `mem-smallvec` — conditional

Function operands use Vec; fixed F6 order uses an array. Distribution and end-to-end allocator benefit for SmallVec have not been measured.

SmallVec may grow enum/stack sizes and silently spill; its common-case inline capacity needs evidence.

No dependency or broad container conversion is justified without consumer and budget design.

Evidence: [`crates/query/tablecraft-query/src/plan.rs:192`](../../crates/query/tablecraft-query/src/plan.rs); [`crates/presentation/tablecraft-ui/src/workspace/focus.rs:17`](../../crates/presentation/tablecraft-ui/src/workspace/focus.rs).

## `mem-take-replace` — observed

Worker takes pending/reply ownership out of Option mailboxes and drops values after releasing guards. It does not deep-clone requests to empty them.

UI/native retained terminal owner must not be taken/dropped before recovery/release; default sentinel alone is not sufficient authority.

Evidence: [`crates/application/tablecraft-runtime/src/worker.rs:31`](../../crates/application/tablecraft-runtime/src/worker.rs); [`crates/application/tablecraft-runtime/src/worker.rs:840`](../../crates/application/tablecraft-runtime/src/worker.rs).

## `mem-thinvec` — conditional

ReadPlan owns many vectors, including optional metadata. A ThinVec benefit requires population/distribution and access-cost measurements.

None versus empty can be semantic (undiscovered schema versus discovered empty); never collapse it for memory savings.

No new dependency or null-state reinterpretation.

Evidence: [`crates/query/tablecraft-query/src/plan.rs:120`](../../crates/query/tablecraft-query/src/plan.rs).

## `mem-with-capacity` — observed

Known native-image and slot widths are charged then fallibly reserved exactly before filling, avoiding repeated growth with typed allocation failure.

Bounded UI preview uses a small with_capacity; standard Vec::new does not itself allocate.

Literal skill with_capacity examples must not replace paid try_reserve paths or introduce panic-only allocation.

Evidence: [`crates/platform/tablecraft-io/src/journal/storage.rs:118`](../../crates/platform/tablecraft-io/src/journal/storage.rs); [`crates/query/tablecraft-query/src/binding/relation/slots.rs:43`](../../crates/query/tablecraft-query/src/binding/relation/slots.rs).

## `mem-write-over-format` — conditional

Currency Display streams into Formatter; localized UI output must remain owned. No demonstrated per-row format-then-append bug in this bounded review.

fmt::Write into String can allocate/grow; write! is not automatically allocation-free or a budget substitute.

Do not introduce unwrap merely because skill examples do so.

Evidence: [`crates/foundation/tablecraft-core/src/values/currency.rs:42`](../../crates/foundation/tablecraft-core/src/values/currency.rs); [`crates/presentation/tablecraft-ui/src/localization.rs:204`](../../crates/presentation/tablecraft-ui/src/localization.rs).

## `mem-zero-copy` — observed

Native snapshots transfer Vec to shared Bytes once and expose borrowed bytes; checked journals return slices of their retained paid images.

Writable AFTER construction necessarily owns a separate image; source authority cannot be invented from arbitrary sliced Bytes.

Evidence: [`crates/engines/tablecraft-ace/src/preservation/image/source.rs:17`](../../crates/engines/tablecraft-ace/src/preservation/image/source.rs); [`crates/engines/tablecraft-ace/src/preservation/image/source.rs:43`](../../crates/engines/tablecraft-ace/src/preservation/image/source.rs); [`crates/platform/tablecraft-io/src/journal/checked_v1.rs:58`](../../crates/platform/tablecraft-io/src/journal/checked_v1.rs).

## `name-acronym-word` — observed

Inspected public SourceGeneration/Direction types and Encrypt/Decrypt variants use Rust case conventions. Native format spelling in strings and Windows ABI names is not a Rust naming defect.

Evidence: [`crates/foundation/tablecraft-core/src/ids/identity.rs:42`](../../crates/foundation/tablecraft-core/src/ids/identity.rs); [`crates/platform/tablecraft-crypto/src/aes.rs:12`](../../crates/platform/tablecraft-crypto/src/aes.rs).

## `name-as-free` — observed

ExpressionString::as_bytes borrows the validated active range without allocating. The identity as_u128 scalar accessor is an intentional cheap newtype projection.

Evidence: [`crates/foundation/tablecraft-core/src/values/expression_string.rs:42`](../../crates/foundation/tablecraft-core/src/values/expression_string.rs).

## `name-consts-screaming` — observed

Repetition ceilings and SIMD lane/mask constants use SCREAMING_SNAKE_CASE at owning boundaries.

Evidence: [`crates/query/tablecraft-query/benches/averages.rs:11`](../../crates/query/tablecraft-query/benches/averages.rs); [`crates/engines/tablecraft-ace/src/format/text/ascii.rs:2`](../../crates/engines/tablecraft-ace/src/format/text/ascii.rs).

## `name-crate-no-rs` — observed

All 20 workspace package names are TableCraft domain names rather than -rs/-rust suffixes. The deferred external vba-rs project name is not a repository package rename request.

Evidence: [`Cargo.toml:1`](../../Cargo.toml).

## `name-funcs-snake` — observed

The inspected public read_exact/fields and related feature module names use snake_case; FFI and stored Access names retain their external spelling.

Evidence: [`crates/platform/tablecraft-io/src/read.rs:13`](../../crates/platform/tablecraft-io/src/read.rs); [`crates/engines/tablecraft-ace/src/creation/request.rs:94`](../../crates/engines/tablecraft-ace/src/creation/request.rs).

## `name-into-ownership` — observed

SaveStartError::into_reviewed consumes the rejection and returns its retained owned payload. Its undocumented None case is separately recorded as a documentation finding.

Evidence: [`crates/application/tablecraft-runtime/src/editing/save.rs:186`](../../crates/application/tablecraft-runtime/src/editing/save.rs).

## `name-is-has-bool` — observed

Existing public source eligibility/capability predicates distinguish metadata eligibility from actual save authority; no renaming of semantic action methods is necessary merely because they return bool.

Evidence: [`crates/application/tablecraft-runtime/src/task_signal.rs:18`](../../crates/application/tablecraft-runtime/src/task_signal.rs).

## `name-iter-convention` — conditional

CreateTable::fields returns an explicit Fields iterator for named ordered entries. The rules permit semantic iterator names; no requirement to add unused generic iter/iter_mut/into_iter APIs follows.

Evidence: [`crates/engines/tablecraft-ace/src/creation/request.rs:94`](../../crates/engines/tablecraft-ace/src/creation/request.rs).

## `name-iter-method` — conditional

CreateTable::fields returns an explicit Fields iterator for named ordered entries. The rules permit semantic iterator names; no requirement to add unused generic iter/iter_mut/into_iter APIs follows.

Evidence: [`crates/engines/tablecraft-ace/src/creation/request.rs:94`](../../crates/engines/tablecraft-ace/src/creation/request.rs).

## `name-iter-type-match` — conditional

CreateTable::fields returns an explicit Fields iterator for named ordered entries. The rules permit semantic iterator names; no requirement to add unused generic iter/iter_mut/into_iter APIs follows.

Evidence: [`crates/engines/tablecraft-ace/src/creation/request.rs:94`](../../crates/engines/tablecraft-ace/src/creation/request.rs).

## `name-lifetime-short` — observed

Inspected native generic code uses P/N and I/O uses R; domain lifetime labels remain short. More descriptive parameter names are acceptable when they disambiguate proof owners.

Evidence: [`crates/engines/tablecraft-ace/src/table/indexes/mutation/encoding.rs:26`](../../crates/engines/tablecraft-ace/src/table/indexes/mutation/encoding.rs); [`crates/platform/tablecraft-io/src/read.rs:13`](../../crates/platform/tablecraft-io/src/read.rs).

## `name-no-get-prefix` — conditional

Most inspected carriers use field-name getters. SourceGeneration::get is a small numeric-wrapper projection established by the API; renaming it without an actual ambiguity would create churn.

Evidence: [`crates/foundation/tablecraft-core/src/ids/identity.rs:42`](../../crates/foundation/tablecraft-core/src/ids/identity.rs).

## `name-to-expensive` — conditional

Floating carrier to_bits/to_float and GUID to_bytes follow established standard scalar conversion names despite being cheap. The general cost heuristic is not an absolute ban on these conventional APIs.

Evidence: [`crates/foundation/tablecraft-core/src/values/floating_point.rs:23`](../../crates/foundation/tablecraft-core/src/values/floating_point.rs).

## `name-type-param-single` — observed

Inspected native generic code uses P/N and I/O uses R; domain lifetime labels remain short. More descriptive parameter names are acceptable when they disambiguate proof owners.

Evidence: [`crates/engines/tablecraft-ace/src/table/indexes/mutation/encoding.rs:26`](../../crates/engines/tablecraft-ace/src/table/indexes/mutation/encoding.rs); [`crates/platform/tablecraft-io/src/read.rs:13`](../../crates/platform/tablecraft-io/src/read.rs).

## `name-types-camel` — observed

Inspected public SourceGeneration/Direction types and Encrypt/Decrypt variants use Rust case conventions. Native format spelling in strings and Windows ABI names is not a Rust naming defect.

Evidence: [`crates/foundation/tablecraft-core/src/ids/identity.rs:42`](../../crates/foundation/tablecraft-core/src/ids/identity.rs); [`crates/platform/tablecraft-crypto/src/aes.rs:12`](../../crates/platform/tablecraft-crypto/src/aes.rs).

## `name-variants-camel` — observed

Inspected public SourceGeneration/Direction types and Encrypt/Decrypt variants use Rust case conventions. Native format spelling in strings and Windows ABI names is not a Rust naming defect.

Evidence: [`crates/foundation/tablecraft-core/src/ids/identity.rs:42`](../../crates/foundation/tablecraft-core/src/ids/identity.rs); [`crates/platform/tablecraft-crypto/src/aes.rs:12`](../../crates/platform/tablecraft-crypto/src/aes.rs).

## `num-cast-try-from` — not verified

Concrete casts were inspected: fixed bounded Windows buffer lengths fit u32; ExtendedDateTime casts happen before Jiff validates components and rely on invalid negative components refusing. Encryption u64 offset->usize requires target/image-size reasoning. No overflow exploit proved here, but all authored numeric casts were not audited.

Evidence: [`crates/engines/tablecraft-ace/src/encryption/parallel/transfer.rs:66`](../../crates/engines/tablecraft-ace/src/encryption/parallel/transfer.rs); [`crates/engines/tablecraft-ace/src/encryption/parallel/transfer.rs:97`](../../crates/engines/tablecraft-ace/src/encryption/parallel/transfer.rs); [`crates/foundation/tablecraft-core/src/values/datetime/extended.rs:71`](../../crates/foundation/tablecraft-core/src/values/datetime/extended.rs); [`crates/platform/tablecraft-io/src/windows_fs/security.rs:106`](../../crates/platform/tablecraft-io/src/windows_fs/security.rs).

## `num-float-compare` — conditional

Single/Double equality is intentionally representational integer-bit equality, including signed zero/NaN payloads. Approximate epsilon would violate native storage contract. Temporal/query comparisons are separate owners; not every numeric evaluator path reviewed.

AGENTS: Value equality is representational, never SQL equality; strict relational keys own set/order semantics.

Evidence: [`crates/foundation/tablecraft-core/src/values/floating_point.rs:4`](../../crates/foundation/tablecraft-core/src/values/floating_point.rs); [`crates/foundation/tablecraft-core/src/values/datetime/legacy.rs:96`](../../crates/foundation/tablecraft-core/src/values/datetime/legacy.rs).

## `num-nonzero` — observed

Shared Text width uses NonZeroU8. Worker uses reserved zero sentinel plus u64 IDs; changing to NonZero alone would not solve atomic generation exhaustion and must follow checked issuance design.

Evidence: [`crates/presentation/tablecraft-builders/src/table/field_addition.rs:23`](../../crates/presentation/tablecraft-builders/src/table/field_addition.rs); [`crates/application/tablecraft-runtime/src/worker.rs:24`](../../crates/application/tablecraft-runtime/src/worker.rs).

## `num-overflow-explicit` — finding

Atomic fetch_add wraps and subsequent +1 debug-panics/release-wraps to reserved zero at u64::MAX; publication_is_current rejects zero and ordering assumptions use monotonic IDs. Existing document_revision checked_add demonstrates typed exhausted behavior elsewhere.

Evidence: [`crates/application/tablecraft-runtime/src/worker.rs:743`](../../crates/application/tablecraft-runtime/src/worker.rs); [`crates/application/tablecraft-runtime/src/worker.rs:755`](../../crates/application/tablecraft-runtime/src/worker.rs); [`crates/application/tablecraft-runtime/src/worker.rs:805`](../../crates/application/tablecraft-runtime/src/worker.rs); [`crates/application/tablecraft-runtime/src/worker.rs:820`](../../crates/application/tablecraft-runtime/src/worker.rs).

## `num-saturating-clamp` — observed

UI clamping/saturating size estimates are explicit and suitable for bounded presentation. They must not replace checked semantic request IDs or native offset/format arithmetic. Full arithmetic coverage not claimed.

Evidence: [`crates/presentation/tablecraft-ui/src/theme.rs:126`](../../crates/presentation/tablecraft-ui/src/theme.rs); [`crates/presentation/tablecraft-ui/src/text_input.rs:106`](../../crates/presentation/tablecraft-ui/src/text_input.rs); [`crates/application/tablecraft-runtime/src/worker.rs:743`](../../crates/application/tablecraft-runtime/src/worker.rs).

## `obs-error-chain` — conditional

Host handlers retain typed source errors; worker tracing emits sanitized payload-free outcomes once. Rule advice to Debug whole errors is explicitly constrained by repository privacy policy. Full chains may contain database paths/names/SQL and must not be blindly logged.

AGENTS/TRACING prohibit automatic Debug arguments, SQL, values, names and paths in tracing.

Evidence: [`docs/development/TRACING.md:10`](../../docs/development/TRACING.md); [`crates/application/tablecraft-runtime/src/editing/native/save.rs:408`](../../crates/application/tablecraft-runtime/src/editing/native/save.rs); [`crates/application/tablecraft-runtime/src/worker.rs:729`](../../crates/application/tablecraft-runtime/src/worker.rs).

## `obs-instrument-spans` — observed

Reviewed synchronous task spans include safe operation metadata. SQLite entered guard is inside synchronous worker closure, not across outer await. Explicit spans avoid automatic sensitive argument capture.

Use explicit payload-free spans/events rather than automatic instrument argument capture.

Evidence: [`crates/application/tablecraft-runtime/src/worker.rs:703`](../../crates/application/tablecraft-runtime/src/worker.rs); [`crates/application/tablecraft-runtime/src/editing/native/save.rs:267`](../../crates/application/tablecraft-runtime/src/editing/native/save.rs); [`crates/backends/tablecraft-sqlite/src/worker/reader.rs:103`](../../crates/backends/tablecraft-sqlite/src/worker/reader.rs).

## `obs-levels-filter` — conditional

Subscriber explicitly maps CLI verbosity to DEBUG/TRACE and is disabled normally; with_max_level is deliberate host-owned filtering, not mandatory EnvFilter. Library error/warn levels match absorbed callback/startup events.

TRACING contract enables diagnostics only for explicit verbose invocation.

Evidence: [`apps/tablecraft-desktop/src/diagnostics.rs:5`](../../apps/tablecraft-desktop/src/diagnostics.rs); [`apps/tablecraft-cli/src/diagnostics.rs:5`](../../apps/tablecraft-cli/src/diagnostics.rs); [`docs/development/TRACING.md:4`](../../docs/development/TRACING.md).

## `obs-library-facade` — observed

Search finds subscriber try_init in executable diagnostic owners; libraries emit through tracing facade. Test/example subscribers are outside normal library initialization. No library installs global subscriber observed.

Evidence: [`apps/tablecraft-cli/src/diagnostics.rs:11`](../../apps/tablecraft-cli/src/diagnostics.rs); [`apps/tablecraft-desktop/src/diagnostics.rs:11`](../../apps/tablecraft-desktop/src/diagnostics.rs); [`Cargo.toml:56`](../../Cargo.toml).

## `obs-no-sensitive-data` — observed

Inspected worker messages are stable/payload-free; Value Debug redacts data. No automatic request Debug or SQL/path fields in reviewed worker spans. This is owner-specific review, not formal whole-dependency privacy proof.

Evidence: [`crates/application/tablecraft-runtime/src/editing/native/save.rs:395`](../../crates/application/tablecraft-runtime/src/editing/native/save.rs); [`crates/application/tablecraft-runtime/src/worker.rs:729`](../../crates/application/tablecraft-runtime/src/worker.rs); [`crates/foundation/tablecraft-core/src/values/stored.rs:11`](../../crates/foundation/tablecraft-core/src/values/stored.rs); [`docs/development/TRACING.md:10`](../../docs/development/TRACING.md).

## `obs-structured-fields` — observed

Safe retained_resources boolean/operation classification use typed structured fields; messages remain static. No path/name interpolation is required to satisfy this rule.

Evidence: [`crates/application/tablecraft-runtime/src/editing/native/save.rs:395`](../../crates/application/tablecraft-runtime/src/editing/native/save.rs); [`crates/application/tablecraft-runtime/src/worker.rs:703`](../../crates/application/tablecraft-runtime/src/worker.rs).

## `obs-tracing-over-log` — observed

Operational diagnostics use tracing. CLI stdout/stderr is the requested product output/error interface, not accidental logging; engineering println output is not normal runtime diagnostics.

CLI buffered/streamed output contracts deliberately write stdout; do not route user data through tracing.

Evidence: [`crates/application/tablecraft-runtime/src/worker.rs:729`](../../crates/application/tablecraft-runtime/src/worker.rs); [`crates/application/tablecraft-runtime/src/editing/native/save.rs:408`](../../crates/application/tablecraft-runtime/src/editing/native/save.rs); [`apps/tablecraft-cli/src/main.rs:65`](../../apps/tablecraft-cli/src/main.rs).

## `opt-bounds-check` — conditional

Checked slices, chunk traversal and iterator loops are used at inspected byte boundaries. Physical format offsets and complete ownership proofs legitimately retain indexed addressing; no measured bounds-check hotspot was established.

Never replace checked parsing with unchecked access solely to satisfy a tuning rule; no unsafe optimization is proposed.

Evidence: [`crates/platform/tablecraft-io/src/read.rs:13`](../../crates/platform/tablecraft-io/src/read.rs); [`crates/engines/tablecraft-ace/src/table/indexes/mutation/encoding.rs:26`](../../crates/engines/tablecraft-ace/src/table/indexes/mutation/encoding.rs).

## `opt-cache-friendly` — not verified

Fixed arrays and chunked SIMD provide concrete contiguous layouts, but no hardware cache profile or AoS/SoA comparison was run. A cache speed claim is unverified.

Evidence: [`crates/engines/tablecraft-ace/src/table/indexes/mutation/encoding.rs:26`](../../crates/engines/tablecraft-ace/src/table/indexes/mutation/encoding.rs); [`crates/engines/tablecraft-ace/src/format/text/ascii.rs:2`](../../crates/engines/tablecraft-ace/src/format/text/ascii.rs).

## `opt-codegen-units` — conditional

The workspace does not override release codegen units or LTO. Those settings are deployment tradeoffs rather than correctness defects; benchmark evidence is required before changing them.

The skill recommended panic=abort would disable the actual worker unwind/uncertain-visibility boundary. Portable distribution targets also preclude blindly copying host tuning.

Evidence: [`Cargo.toml:1`](../../Cargo.toml); [`rust-toolchain.toml:2`](../../rust-toolchain.toml); [`crates/application/tablecraft-runtime/src/worker.rs:586`](../../crates/application/tablecraft-runtime/src/worker.rs).

## `opt-cold-unlikely` — not verified

Some recursive query boundaries deliberately use inline(never); the inspected source supplies no branch-frequency or cross-crate inlining measurements. Adding cold/likely/inline annotations is not justified by this read-only pass.

Evidence: [`crates/query/tablecraft-query/src/binding/retrieval/body.rs:21`](../../crates/query/tablecraft-query/src/binding/retrieval/body.rs); [`crates/query/tablecraft-query/benches/averages.rs:11`](../../crates/query/tablecraft-query/benches/averages.rs).

## `opt-inline-always-rare` — observed

The repository-wide textual scan recorded no inline(always) sites. This observes absence of forced inlining, not optimized assembly or performance.

Evidence: [`crates/engines/tablecraft-ace/src/format/text/ascii.rs:2`](../../crates/engines/tablecraft-ace/src/format/text/ascii.rs).

## `opt-inline-never-cold` — not verified

Some recursive query boundaries deliberately use inline(never); the inspected source supplies no branch-frequency or cross-crate inlining measurements. Adding cold/likely/inline annotations is not justified by this read-only pass.

Evidence: [`crates/query/tablecraft-query/src/binding/retrieval/body.rs:21`](../../crates/query/tablecraft-query/src/binding/retrieval/body.rs); [`crates/query/tablecraft-query/benches/averages.rs:11`](../../crates/query/tablecraft-query/benches/averages.rs).

## `opt-inline-small` — not verified

Some recursive query boundaries deliberately use inline(never); the inspected source supplies no branch-frequency or cross-crate inlining measurements. Adding cold/likely/inline annotations is not justified by this read-only pass.

Evidence: [`crates/query/tablecraft-query/src/binding/retrieval/body.rs:21`](../../crates/query/tablecraft-query/src/binding/retrieval/body.rs); [`crates/query/tablecraft-query/benches/averages.rs:11`](../../crates/query/tablecraft-query/benches/averages.rs).

## `opt-likely-hint` — not verified

Some recursive query boundaries deliberately use inline(never); the inspected source supplies no branch-frequency or cross-crate inlining measurements. Adding cold/likely/inline annotations is not justified by this read-only pass.

Evidence: [`crates/query/tablecraft-query/src/binding/retrieval/body.rs:21`](../../crates/query/tablecraft-query/src/binding/retrieval/body.rs); [`crates/query/tablecraft-query/benches/averages.rs:11`](../../crates/query/tablecraft-query/benches/averages.rs).

## `opt-lto-release` — conditional

The workspace does not override release codegen units or LTO. Those settings are deployment tradeoffs rather than correctness defects; benchmark evidence is required before changing them.

The skill recommended panic=abort would disable the actual worker unwind/uncertain-visibility boundary. Portable distribution targets also preclude blindly copying host tuning.

Evidence: [`Cargo.toml:1`](../../Cargo.toml); [`rust-toolchain.toml:2`](../../rust-toolchain.toml); [`crates/application/tablecraft-runtime/src/worker.rs:586`](../../crates/application/tablecraft-runtime/src/worker.rs).

## `opt-pgo-profile` — not verified

CI and maintained benchmarks were inspected; no PGO training corpus or merged profile was established. PGO benefits remain unmeasured and are not an implementation requirement.

Evidence: [`.github/workflows/ci.yml:62`](../../.github/workflows/ci.yml); [`crates/engines/tablecraft-ace/benches/image_capture.rs:1`](../../crates/engines/tablecraft-ace/benches/image_capture.rs).

## `opt-simd-portable` — observed

The native ASCII classifier uses the reviewed stable wide crate over fixed lanes and bytemuck, rather than unconditional architecture intrinsics. Existing tests/benchmarks own functional/performance qualification; none were run here.

Evidence: [`crates/engines/tablecraft-ace/src/format/text/ascii.rs:2`](../../crates/engines/tablecraft-ace/src/format/text/ascii.rs); [`Cargo.toml:34`](../../Cargo.toml).

## `opt-target-cpu` — conditional

CI explicitly spans x86_64 and aarch64 across Windows/macOS/Linux; no global target-cpu=native flag exists. Native-only tuning would be inappropriate for portable distributed artifacts.

Repository platform/feature scope overrides the generic native-CPU recommendation.

Evidence: [`.github/workflows/ci.yml:62`](../../.github/workflows/ci.yml); [`Cargo.toml:1`](../../Cargo.toml).

## `own-arc-shared` — observed

Read worker shares mailbox/liveness across OS thread; Localizer shares immutable concurrent bundles. Clones retain owners rather than copying payloads.

Arc is not blanket approval for unbounded owner creation; native image owner charges control explicitly.

Evidence: [`crates/application/tablecraft-runtime/src/worker.rs:75`](../../crates/application/tablecraft-runtime/src/worker.rs); [`crates/application/tablecraft-runtime/src/worker.rs:582`](../../crates/application/tablecraft-runtime/src/worker.rs); [`crates/presentation/tablecraft-ui/src/localization.rs:137`](../../crates/presentation/tablecraft-ui/src/localization.rs).

## `own-borrow-over-clone` — finding

Three current production paths duplicate owned strings only for immediate comparison or frame-local reading. Find conversion copies pattern/table/scope/options; edit conversion copies table/column; Add Tables eagerly copies every name every frame.

Other find clones transfer requests to asynchronous pending/UI owners and are not automatically removable.

Preserve exact stale-response and mutation-admission predicates, every request field and budget ownership.

Evidence: [`apps/tablecraft-desktop/src/workspace/find.rs:71`](../../apps/tablecraft-desktop/src/workspace/find.rs); [`apps/tablecraft-desktop/src/workspace/find.rs:203`](../../apps/tablecraft-desktop/src/workspace/find.rs); [`apps/tablecraft-desktop/src/workspace/editing.rs:241`](../../apps/tablecraft-desktop/src/workspace/editing.rs); [`crates/presentation/tablecraft-ui/src/workspace/editing.rs:113`](../../crates/presentation/tablecraft-ui/src/workspace/editing.rs); [`crates/presentation/tablecraft-ui/src/workspace/query_design/dialog.rs:25`](../../crates/presentation/tablecraft-ui/src/workspace/query_design/dialog.rs).

## `own-clone-explicit` — observed

Lossless expression String defines deep clone explicitly; NativeImageSource clones share Bytes; QBE expression cloning now occurs only for dual projection/order ownership.

The deliberate Bytes clone eagerly creates shared control while its allowance is paid; removing it changes accounting.

Do not add Clone to exclusive source/review/journal capabilities.

Evidence: [`crates/foundation/tablecraft-core/src/values/expression_string.rs:19`](../../crates/foundation/tablecraft-core/src/values/expression_string.rs); [`crates/engines/tablecraft-ace/src/preservation/image/source.rs:37`](../../crates/engines/tablecraft-ace/src/preservation/image/source.rs); [`crates/presentation/tablecraft-builders/src/query/build.rs:447`](../../crates/presentation/tablecraft-builders/src/query/build.rs).

## `own-copy-small` — observed

Semantic IDs/generations and name policy are small plain values with explicit Copy derives.

CreateColumn is a larger borrowed declaration; Copy is existing intentional API, not a measured memcpy issue.

Evidence: [`crates/foundation/tablecraft-core/src/ids/identity.rs:7`](../../crates/foundation/tablecraft-core/src/ids/identity.rs); [`crates/foundation/tablecraft-core/src/ids/identity.rs:42`](../../crates/foundation/tablecraft-core/src/ids/identity.rs); [`crates/foundation/tablecraft-schema/src/name.rs:25`](../../crates/foundation/tablecraft-schema/src/name.rs).

## `own-cow-conditional` — observed

Datasource cell preview borrows unchanged text and owns only its explicitly bounded shortened prefix; no normalization changes stored text.

An async retained string or final localized/accessibility result still requires ownership.

Evidence: [`crates/presentation/tablecraft-ui/src/datasheet/preview.rs:18`](../../crates/presentation/tablecraft-ui/src/datasheet/preview.rs); [`crates/presentation/tablecraft-ui/src/datasheet/preview.rs:50`](../../crates/presentation/tablecraft-ui/src/datasheet/preview.rs).

## `own-lifetime-elision` — observed

Simple preview returns inferred lifetime; checked journal uses explicit lifetime because it must select journal borrow separately from budget/cancellation references.

Removing inspect_v1 lifetime would obscure which input outlives the retained borrow.

Evidence: [`crates/presentation/tablecraft-ui/src/datasheet/preview.rs:18`](../../crates/presentation/tablecraft-ui/src/datasheet/preview.rs); [`crates/platform/tablecraft-io/src/journal/checked_v1.rs:46`](../../crates/platform/tablecraft-io/src/journal/checked_v1.rs).

## `own-move-large` — conditional

Prepared journal is consumed; recursive expression children are boxed. No large inline repeatedly moved payload was demonstrated as a bottleneck.

Moves may be optimized/elided; size thresholds alone do not establish measurable copies.

Boxing adds allocation and must preserve fallible admission/Drop behavior.

Evidence: [`crates/platform/tablecraft-io/src/journal/transaction.rs:13`](../../crates/platform/tablecraft-io/src/journal/transaction.rs); [`crates/query/tablecraft-query/src/plan.rs:192`](../../crates/query/tablecraft-query/src/plan.rs).

## `own-mutex-interior` — observed

Pending and response mailboxes are synchronized; actor takes pending under lock and explicitly drops lock before executing callbacks.

Worker poisoned-state handling is a deliberate terminal policy, not an instruction to recover poisoned data.

Do not blindly replace std Mutex with parking_lot and alter poisoning semantics.

Evidence: [`crates/application/tablecraft-runtime/src/worker.rs:31`](../../crates/application/tablecraft-runtime/src/worker.rs); [`crates/application/tablecraft-runtime/src/worker.rs:582`](../../crates/application/tablecraft-runtime/src/worker.rs).

## `own-rc-single-thread` — conditional

Worker really crosses a thread boundary, so Arc cannot be replaced with Rc. Single-thread native/query adapters borrow RefCell pools rather than introducing shared Rc ownership.

No measured production Arc-only-single-thread bottleneck established; library threading models must remain supported.

Do not change Send/Sync contracts merely to remove atomics.

Evidence: [`crates/application/tablecraft-runtime/src/worker.rs:75`](../../crates/application/tablecraft-runtime/src/worker.rs); [`apps/tablecraft-cli/src/commands/query/external.rs:134`](../../apps/tablecraft-cli/src/commands/query/external.rs).

## `own-refcell-interior` — observed

Owned/shared native pools use fallible RefCell borrowing; CLI translates borrow failure rather than panicking or resetting the common allowance.

These synchronous pools are not cross-thread state.

Evidence: [`crates/backends/tablecraft-access/src/source/budget.rs:19`](../../crates/backends/tablecraft-access/src/source/budget.rs); [`apps/tablecraft-cli/src/commands/query/external.rs:134`](../../apps/tablecraft-cli/src/commands/query/external.rs).

## `own-rwlock-readers` — conditional

Mailbox reads take/mutate one replaceable slot, so even publication checks are not a read-heavy immutable cache justifying RwLock.

No contention/read-write ratio measurements were run; rule percentage thresholds are guidance only.

Changing lock semantics requires measured real consumers and retention of terminal poison policy.

Evidence: [`crates/application/tablecraft-runtime/src/worker.rs:31`](../../crates/application/tablecraft-runtime/src/worker.rs).

## `own-slice-over-vec` — observed

Native reads and checked journal access accept slices. The reviewed &Vec parameter intentionally inspects capacity in retained-memory accounting.

A slice conversion for retained_inputs would discard capacity information and undercount.

Do not mechanically replace capacity-aware &Vec APIs.

Evidence: [`crates/platform/tablecraft-io/src/read.rs:15`](../../crates/platform/tablecraft-io/src/read.rs); [`crates/platform/tablecraft-io/src/journal/checked_v1.rs:58`](../../crates/platform/tablecraft-io/src/journal/checked_v1.rs); [`crates/presentation/tablecraft-ui/src/workspace/query/parameters.rs:61`](../../crates/presentation/tablecraft-ui/src/workspace/query/parameters.rs).

## `pat-at-bindings` — conditional

At-bindings are a readability choice when the whole matched value is reused; ordinary declaration matches do not need them. No redundant destructure/reconstruct bug was found.

Evidence: [`crates/engines/tablecraft-ace/src/creation/column.rs:36`](../../crates/engines/tablecraft-ace/src/creation/column.rs).

## `pat-exhaustive-enum` — observed

Crypto Direction and native Declaration matches enumerate owned variants explicitly. Separate profile admission deliberately uses explicit wildcard refusal for broader unsupported input; do not turn a refusal into a fallback.

Evidence: [`crates/platform/tablecraft-crypto/src/aes.rs:12`](../../crates/platform/tablecraft-crypto/src/aes.rs); [`crates/engines/tablecraft-ace/src/creation/column.rs:36`](../../crates/engines/tablecraft-ace/src/creation/column.rs).

## `pat-if-let-chains` — observed

The actual error-source wrapper uses left-to-right if-let/Boolean chains under the pinned edition-2024 toolchain. Validation order remains visible.

Evidence: [`crates/backends/tablecraft-access/src/editing/offline/error.rs:342`](../../crates/backends/tablecraft-access/src/editing/offline/error.rs).

## `pat-let-else` — observed

Mailbox actor extraction uses let-else with an explicit stop/continue path before work; the happy path remains flat without inventing task success.

Evidence: [`crates/application/tablecraft-runtime/src/creation/worker.rs:97`](../../crates/application/tablecraft-runtime/src/creation/worker.rs).

## `pat-matches-macro` — observed

The real retained failure cause discriminator uses matches! for a Boolean variant test; error ownership remains in the subsequent typed branch.

Evidence: [`crates/backends/tablecraft-access/src/editing/offline/error.rs:342`](../../crates/backends/tablecraft-access/src/editing/offline/error.rs).

## `perf-ahash` — conditional

No profiled hasher bottleneck or trusted-only map was established. Imported native/query names are untrusted; replacing hashing globally with FxHash would need a separate threat/cost review.

Evidence: [`Cargo.toml:1`](../../Cargo.toml).

## `perf-black-box-bench` — observed

Inspected query benches and capture comparison consume results through std::hint::black_box and verify warm results. No timing claims or fresh benchmark execution are made.

Evidence: [`crates/query/tablecraft-query/benches/averages.rs:73`](../../crates/query/tablecraft-query/benches/averages.rs); [`crates/engines/tablecraft-ace/benches/image_capture.rs:1`](../../crates/engines/tablecraft-ace/benches/image_capture.rs).

## `perf-chain-avoid` — conditional

Avoiding chain is only a measured hot-path choice. Replacing a lazy chain with an allocated combined collection can violate resource/short-circuit semantics; no such speculative rewrite is proposed.

Evidence: [`crates/query/tablecraft-query/benches/averages.rs:11`](../../crates/query/tablecraft-query/benches/averages.rs).

## `perf-collect-into` — conditional

The rule itself marks collect_into nightly-only; stable extend/clear are the applicable alternatives. The pinned stable toolchain is not a reason to introduce a nightly feature.

Evidence: [`rust-toolchain.toml:2`](../../rust-toolchain.toml).

## `perf-collect-once` — conditional

Some retained UI graphs and complete query caches deliberately materialize owned collections for repeated traversal/atomic publication. The separate ownership agent reviewed concrete clone/collect waste; no automatic lazy rewrite may drop eager errors or accounting.

Evidence: [`crates/presentation/tablecraft-ui/src/workspace/objects/memory.rs:17`](../../crates/presentation/tablecraft-ui/src/workspace/objects/memory.rs).

## `perf-drain-reuse` — conditional

Reusing capacity can help scratch buffers, but atomic owner transfers deliberately move old allocations and retained SQL capacities. No changed ownership strategy is justified without a concrete caller proof.

Evidence: [`crates/presentation/tablecraft-ui/src/workspace/objects/memory.rs:17`](../../crates/presentation/tablecraft-ui/src/workspace/objects/memory.rs).

## `perf-entry-api` — not verified

The sampled query implementation does not establish a map insert/update hotspot. Repository-wide contains_key matches require semantic review before classifying duplicate lookups as a performance defect.

Evidence: [`Cargo.toml:1`](../../Cargo.toml).

## `perf-extend-batch` — conditional

Native loops often interleave cancellation/resource checks and fallible work with each appended item; a generic extend rewrite could skip those boundaries. No unconditional batch insertion finding is made.

Evidence: [`crates/platform/tablecraft-io/src/read.rs:13`](../../crates/platform/tablecraft-io/src/read.rs).

## `perf-io-buffering` — observed

CLI progressive output explicitly flushes and propagates errors; native I/O is already page/buffer oriented. The generic claim that every Read call equals one syscall does not apply to Cursor/captured sources.

Evidence: [`apps/tablecraft-cli/src/commands/query/stream/publisher.rs:89`](../../apps/tablecraft-cli/src/commands/query/stream/publisher.rs); [`crates/platform/tablecraft-io/src/read.rs:13`](../../crates/platform/tablecraft-io/src/read.rs).

## `perf-iter-lazy` — conditional

Some retained UI graphs and complete query caches deliberately materialize owned collections for repeated traversal/atomic publication. The separate ownership agent reviewed concrete clone/collect waste; no automatic lazy rewrite may drop eager errors or accounting.

Evidence: [`crates/presentation/tablecraft-ui/src/workspace/objects/memory.rs:17`](../../crates/presentation/tablecraft-ui/src/workspace/objects/memory.rs).

## `perf-iter-over-index` — conditional

Checked slices, chunk traversal and iterator loops are used at inspected byte boundaries. Physical format offsets and complete ownership proofs legitimately retain indexed addressing; no measured bounds-check hotspot was established.

Never replace checked parsing with unchecked access solely to satisfy a tuning rule; no unsafe optimization is proposed.

Evidence: [`crates/platform/tablecraft-io/src/read.rs:13`](../../crates/platform/tablecraft-io/src/read.rs); [`crates/engines/tablecraft-ace/src/table/indexes/mutation/encoding.rs:26`](../../crates/engines/tablecraft-ace/src/table/indexes/mutation/encoding.rs).

## `perf-profile-first` — not verified

Maintained benchmark modes demonstrate measurement machinery, but this pass ran no profiler or benchmark and does not validate all historic speed claims. New tuning must remain evidence-led.

Evidence: [`crates/engines/tablecraft-ace/benches/image_capture.rs:1`](../../crates/engines/tablecraft-ace/benches/image_capture.rs); [`crates/query/tablecraft-query/benches/averages.rs:11`](../../crates/query/tablecraft-query/benches/averages.rs).

## `perf-release-profile` — conditional

The workspace does not override release codegen units or LTO. Those settings are deployment tradeoffs rather than correctness defects; benchmark evidence is required before changing them.

The skill recommended panic=abort would disable the actual worker unwind/uncertain-visibility boundary. Portable distribution targets also preclude blindly copying host tuning.

Evidence: [`Cargo.toml:1`](../../Cargo.toml); [`rust-toolchain.toml:2`](../../rust-toolchain.toml); [`crates/application/tablecraft-runtime/src/worker.rs:586`](../../crates/application/tablecraft-runtime/src/worker.rs).

## `proj-bin-dir` — conditional

CLI and desktop are separate application packages with one actual main each, not multiple unrelated binaries in one package. Moving them into a common src/bin would break established host/dependency ownership.

Evidence: [`apps/tablecraft-cli/src/main.rs:44`](../../apps/tablecraft-cli/src/main.rs); [`apps/tablecraft-desktop/src/main.rs:43`](../../apps/tablecraft-desktop/src/main.rs).

## `proj-build-rs-minimal` — not applicable

All 20 manifests were parsed; none has a build override or crate-root build.rs. Files named build.rs inside feature folders are ordinary Rust modules, not Cargo build scripts.

Evidence: [`Cargo.toml:1`](../../Cargo.toml).

## `proj-feature-additive` — observed

Runtime editing/creation and standalone read configurations are independently enumerated in CI, and optional native proof/recovery items remain additive. This is scoped feature-composition evidence, not all-target execution in this pass.

Evidence: [`.github/workflows/ci.yml:94`](../../.github/workflows/ci.yml).

## `proj-flat-small` — conditional

This is a multi-domain 20-package application, not a small flat tutorial crate. Feature-local directories are justified by actual engine/adapter/runtime/UI consumers; no empty crate scaffolding was identified here.

Evidence: [`Cargo.toml:1`](../../Cargo.toml); [`docs/architecture/ARCHITECTURE.md:1`](../../docs/architecture/ARCHITECTURE.md).

## `proj-lib-main-split` — observed

Application mains own CLI/window/subscriber composition while engine/runtime behavior lives under crates. A separate lib.rs for an already-thin CLI is not automatically necessary; main help currently has a concrete stale sentence.

Evidence: [`apps/tablecraft-cli/src/main.rs:44`](../../apps/tablecraft-cli/src/main.rs); [`apps/tablecraft-desktop/src/main.rs:43`](../../apps/tablecraft-desktop/src/main.rs); [`docs/architecture/ARCHITECTURE.md:1`](../../docs/architecture/ARCHITECTURE.md).

## `proj-mod-by-feature` — observed

Current policy and real engine/runtime/UI paths group implementations and focused tests by feature. Root module moves retain feature ownership rather than regrouping all structs/errors/services by type.

Evidence: [`docs/development/DEVELOPMENT.md:21`](../../docs/development/DEVELOPMENT.md); [`docs/architecture/ARCHITECTURE.md:1`](../../docs/architecture/ARCHITECTURE.md).

## `proj-mod-rs-dir` — conditional

Repository policy explicitly requires feature.rs plus feature/ and checks it in CI. This overrides the skill preference for mod.rs in large modules.

Do not reverse the completed 273-module migration to mechanically satisfy generic skill examples.

Evidence: [`docs/development/DEVELOPMENT.md:21`](../../docs/development/DEVELOPMENT.md); [`.github/workflows/ci.yml:62`](../../.github/workflows/ci.yml).

## `proj-msrv-declare` — observed

Workspace rust-version=1.99 and rust-toolchain=1.99.0 agree; all 20 member manifests inherit MSRV and CI installs the pinned native host. No independent older-toolchain compatibility is claimed.

Evidence: [`rust-toolchain.toml:2`](../../rust-toolchain.toml); [`.github/workflows/ci.yml:62`](../../.github/workflows/ci.yml); [`Cargo.toml:1`](../../Cargo.toml).

## `proj-prelude-module` — conditional

The repository requires direct owning imports and rejects convenience-only facade relays. No actual extension-trait consumer justifies adding a prelude.

Repository explicit-owner guidance overrides the generic prelude recommendation.

Evidence: [`docs/development/DEVELOPMENT.md:21`](../../docs/development/DEVELOPMENT.md); [`docs/architecture/ARCHITECTURE.md:1`](../../docs/architecture/ARCHITECTURE.md).

## `proj-pub-crate-internal` — observed

The actual shared spawner is crate-local for three worker consumers; actor internals remain private/parent-scoped. No private source/proof owner was made public merely to shorten an import.

Evidence: [`crates/application/tablecraft-runtime/src/thread_start.rs:4`](../../crates/application/tablecraft-runtime/src/thread_start.rs); [`crates/application/tablecraft-runtime/src/creation/worker.rs:97`](../../crates/application/tablecraft-runtime/src/creation/worker.rs).

## `proj-pub-super-parent` — observed

The actual shared spawner is crate-local for three worker consumers; actor internals remain private/parent-scoped. No private source/proof owner was made public merely to shorten an import.

Evidence: [`crates/application/tablecraft-runtime/src/thread_start.rs:4`](../../crates/application/tablecraft-runtime/src/thread_start.rs); [`crates/application/tablecraft-runtime/src/creation/worker.rs:97`](../../crates/application/tablecraft-runtime/src/creation/worker.rs).

## `proj-pub-use-reexport` — conditional

Genuine crate API, Fluent type/version and platform selections remain exports; convenience parent/foreign relays were separately removed by direct-owner changes. A blanket flattened facade conflicts with repository ownership policy.

Evidence: [`crates/foundation/tablecraft-core/src/lib.rs:8`](../../crates/foundation/tablecraft-core/src/lib.rs); [`docs/architecture/ARCHITECTURE.md:1`](../../docs/architecture/ARCHITECTURE.md).

## `proj-workspace-deps` — observed

The 20-package domain workspace centralizes dependency versions/defaults; actual query member uses workspace inheritance and local feature selection.

Evidence: [`Cargo.toml:1`](../../Cargo.toml); [`Cargo.toml:34`](../../Cargo.toml); [`crates/query/tablecraft-query/Cargo.toml:31`](../../crates/query/tablecraft-query/Cargo.toml).

## `proj-workspace-large` — observed

The 20-package domain workspace centralizes dependency versions/defaults; actual query member uses workspace inheritance and local feature selection.

Evidence: [`Cargo.toml:1`](../../Cargo.toml); [`Cargo.toml:34`](../../Cargo.toml); [`crates/query/tablecraft-query/Cargo.toml:31`](../../crates/query/tablecraft-query/Cargo.toml).

## `serde-custom-with` — conditional

Transparent integer/byte carriers deliberately match exact stored representation; constructors with validation use try_from. No external duration/base64/timestamp format mismatch identified requiring with helper. Do not invent another wire encoding.

Evidence: [`crates/foundation/tablecraft-core/src/values/floating_point.rs:8`](../../crates/foundation/tablecraft-core/src/values/floating_point.rs); [`crates/foundation/tablecraft-core/src/values/currency.rs:9`](../../crates/foundation/tablecraft-core/src/values/currency.rs); [`crates/foundation/tablecraft-core/src/values/guid.rs:9`](../../crates/foundation/tablecraft-core/src/values/guid.rs).

## `serde-default-compat` — conditional

Core scalars have required exact tags/fields; missing coefficient/scale must refuse rather than default. No compatible newly optional config field was identified. Native original-intent protocols are custom closed binary codecs, not serde defaults.

Preserve exact old protocol languages and constructor validation; never infer omitted default as zero/NULL.

Evidence: [`crates/foundation/tablecraft-core/src/values/stored.rs:17`](../../crates/foundation/tablecraft-core/src/values/stored.rs); [`crates/foundation/tablecraft-core/src/values/decimal.rs:79`](../../crates/foundation/tablecraft-core/src/values/decimal.rs).

## `serde-deny-unknown-fields` — observed

Strict Value and raw validated representations deny unknown fields. Existing tests cover invalid extra Decimal keys. Whole arbitrary host JSON config audit not claimed.

Evidence: [`crates/foundation/tablecraft-core/src/values/stored.rs:21`](../../crates/foundation/tablecraft-core/src/values/stored.rs); [`crates/foundation/tablecraft-core/src/values/decimal.rs:79`](../../crates/foundation/tablecraft-core/src/values/decimal.rs); [`crates/foundation/tablecraft-core/src/values/datetime/extended.rs:26`](../../crates/foundation/tablecraft-core/src/values/datetime/extended.rs).

## `serde-enum-representation` — observed

Value uses deliberate adjacent type/value tagging and snake_case variant names; Null differs from empty text and numeric carriers remain distinct. No untagged decoder retry/fallback in inspected serde owner.

Evidence: [`crates/foundation/tablecraft-core/src/values/stored.rs:17`](../../crates/foundation/tablecraft-core/src/values/stored.rs).

## `serde-flatten` — not applicable

Authored serde attribute search found no flatten. Strict closed value records deliberately reject unknown fields; flatten would conflict with deny_unknown_fields and is not needed.

Evidence: [`crates/foundation/tablecraft-core/src/values/stored.rs:17`](../../crates/foundation/tablecraft-core/src/values/stored.rs); [`crates/foundation/tablecraft-core/src/values/decimal.rs:79`](../../crates/foundation/tablecraft-core/src/values/decimal.rs).

## `serde-rename-all` — observed

Shared Value external tag convention is explicitly snake_case; raw scalar field names already snake_case. No reason to change established representation to camelCase.

Evidence: [`crates/foundation/tablecraft-core/src/values/stored.rs:20`](../../crates/foundation/tablecraft-core/src/values/stored.rs).

## `serde-skip-empty` — conditional

Empty string/binary, SQL NULL, and absent payload have distinct semantics; broad skip-empty would erase data. Value Null serialization already intentionally emits its tag without a fabricated value field.

AGENTS requires omitted vs NULL vs supplied value separation.

Evidence: [`crates/foundation/tablecraft-core/src/values/stored.rs:25`](../../crates/foundation/tablecraft-core/src/values/stored.rs); [`crates/foundation/tablecraft-core/src/values/stored.rs:48`](../../crates/foundation/tablecraft-core/src/values/stored.rs).

## `serde-try-from-validate` — observed

Decimal/LegacyDateTime/ExtendedDateTime Deserialize use Raw TryFrom calling exact constructors; malformed ranges, NaN dates and extra fields cannot bypass validation. Single/Double/Guid/Currency accept every represented bit pattern by contract; transparent derivation is appropriate. Tests inspected, not executed in this review.

Evidence: [`crates/foundation/tablecraft-core/src/values/decimal.rs:14`](../../crates/foundation/tablecraft-core/src/values/decimal.rs); [`crates/foundation/tablecraft-core/src/values/decimal.rs:87`](../../crates/foundation/tablecraft-core/src/values/decimal.rs); [`crates/foundation/tablecraft-core/src/values/datetime/extended.rs:13`](../../crates/foundation/tablecraft-core/src/values/datetime/extended.rs); [`crates/foundation/tablecraft-core/src/values/datetime/extended.rs:36`](../../crates/foundation/tablecraft-core/src/values/datetime/extended.rs); [`crates/foundation/tablecraft-core/src/values/datetime/legacy.rs:18`](../../crates/foundation/tablecraft-core/src/values/datetime/legacy.rs); [`crates/foundation/tablecraft-core/src/values/datetime/legacy.rs:30`](../../crates/foundation/tablecraft-core/src/values/datetime/legacy.rs); [`crates/foundation/tablecraft-core/tests/serde_roundtrips.rs:90`](../../crates/foundation/tablecraft-core/tests/serde_roundtrips.rs).

## `test-arrange-act-assert` — observed

The focused startup case arranges an independent real native source/review, triggers OS startup refusal, checks unchanged ownership/files, then consumes the returned review on a working owner. Its name states the observable contract.

Evidence: [`crates/application/tablecraft-runtime/src/editing/native/save/startup_tests.rs:9`](../../crates/application/tablecraft-runtime/src/editing/native/save/startup_tests.rs).

## `test-cfg-test-module` — observed

Focused tests live beside their real worker owner under cfg(test), with selective/public imports and parent internals as needed. Separate test.rs files follow repository layout rather than mandatory inline bodies.

Evidence: [`crates/application/tablecraft-runtime/src/editing/native/save/startup_tests.rs:9`](../../crates/application/tablecraft-runtime/src/editing/native/save/startup_tests.rs).

## `test-criterion-bench` — conditional

The project has harness=false measured engineering executables with black_box and explicit repetitions, including Hyperfine-compatible modes. Absence of Criterion is not by itself a bug; statistical results must retain methodology limits.

Evidence: [`crates/query/tablecraft-query/benches/averages.rs:11`](../../crates/query/tablecraft-query/benches/averages.rs); [`crates/engines/tablecraft-ace/benches/image_capture.rs:1`](../../crates/engines/tablecraft-ace/benches/image_capture.rs); [`crates/query/tablecraft-query/Cargo.toml:31`](../../crates/query/tablecraft-query/Cargo.toml).

## `test-descriptive-names` — observed

The focused startup case arranges an independent real native source/review, triggers OS startup refusal, checks unchanged ownership/files, then consumes the returned review on a working owner. Its name states the observable contract.

Evidence: [`crates/application/tablecraft-runtime/src/editing/native/save/startup_tests.rs:9`](../../crates/application/tablecraft-runtime/src/editing/native/save/startup_tests.rs).

## `test-doctest-examples` — observed

The query crate provides a real public API executable example and CI runs workspace doctests plus warning-denied rustdoc. This pass did not execute doctests or claim all public functions have examples.

Evidence: [`crates/query/tablecraft-query/src/lib.rs:4`](../../crates/query/tablecraft-query/src/lib.rs); [`.github/workflows/ci.yml:176`](../../.github/workflows/ci.yml).

## `test-fixture-raii` — observed

The actual startup fixture uses TempDir ownership, and native paths/journals remain scoped to it; no manually leaked shared test directory is introduced.

Evidence: [`crates/application/tablecraft-runtime/src/editing/native/save/startup_tests.rs:9`](../../crates/application/tablecraft-runtime/src/editing/native/save/startup_tests.rs).

## `test-integration-dir` — observed

The engine has separate public recovery integration targets and layer-local unit/scenario modules. Private source/proof glue remains unit-local rather than made public just to satisfy integration placement.

Evidence: [`crates/query/tablecraft-query/Cargo.toml:31`](../../crates/query/tablecraft-query/Cargo.toml); [`crates/foundation/tablecraft-core/src/ids.rs:7`](../../crates/foundation/tablecraft-core/src/ids.rs).

## `test-loom-concurrency` — not verified

Current actor tests use controlled channels, paused startup and exact terminal ownership. No Loom model or exhaustive schedule proof was run; deterministic regressions are not described as exhaustive concurrency verification.

Evidence: [`crates/application/tablecraft-runtime/src/worker/liveness_tests.rs:61`](../../crates/application/tablecraft-runtime/src/worker/liveness_tests.rs).

## `test-mock-traits` — observed

The real three-consumer Spawner trait permits injected OS startup failure without globally exhausting threads. It is a working test/production dependency boundary rather than a dormant generic abstraction.

Evidence: [`crates/application/tablecraft-runtime/src/thread_start.rs:4`](../../crates/application/tablecraft-runtime/src/thread_start.rs); [`crates/application/tablecraft-runtime/src/editing/native/save/startup_tests.rs:9`](../../crates/application/tablecraft-runtime/src/editing/native/save/startup_tests.rs).

## `test-mockall-mocking` — conditional

A tiny hand-written failing spawner is sufficient for this concrete boundary. No Mockall dependency is justified merely to replace a few explicit test lines.

Evidence: [`crates/application/tablecraft-runtime/src/thread_start.rs:4`](../../crates/application/tablecraft-runtime/src/thread_start.rs).

## `test-proptest-properties` — observed

Core ResourceBudget properties use real proptest strategies and generated checks; workspace dependency policy keeps it development-only.

Evidence: [`crates/foundation/tablecraft-core/src/limits.rs:135`](../../crates/foundation/tablecraft-core/src/limits.rs); [`Cargo.toml:34`](../../Cargo.toml).

## `test-should-panic` — conditional

Intentional actor panics are checked through their actual terminal contract, not should_panic on the calling thread. Compile-time ownership rejection uses compile_fail. No missing defensive-panic test was established.

Evidence: [`crates/foundation/tablecraft-core/src/ids.rs:7`](../../crates/foundation/tablecraft-core/src/ids.rs); [`crates/application/tablecraft-runtime/src/worker/liveness_tests.rs:61`](../../crates/application/tablecraft-runtime/src/worker/liveness_tests.rs).

## `test-snapshot-testing` — conditional

Independent full-image/hash/raw-model goldens and reviewed offscreen artifacts already serve domain-specific acceptance. A new insta dependency is not required for native exact-byte qualification.

Evidence: [`crates/engines/tablecraft-ace/benches/image_capture.rs:1`](../../crates/engines/tablecraft-ace/benches/image_capture.rs).

## `test-tokio-async` — conditional

Optional existing SQLite async behavior is paused by the user; synchronous native workers do not require Tokio tests or a network runtime. No async feature expansion is authorized.

Evidence: [`crates/backends/tablecraft-sqlite/Cargo.toml:14`](../../crates/backends/tablecraft-sqlite/Cargo.toml); [`Cargo.toml:34`](../../Cargo.toml).

## `test-use-super` — observed

Focused tests live beside their real worker owner under cfg(test), with selective/public imports and parent internals as needed. Separate test.rs files follow repository layout rather than mandatory inline bodies.

Evidence: [`crates/application/tablecraft-runtime/src/editing/native/save/startup_tests.rs:9`](../../crates/application/tablecraft-runtime/src/editing/native/save/startup_tests.rs).

## `trait-associated-type-vs-generic` — observed

Publication Backend has one associated Stage per backend and generic publisher chooses backend implementation; stage type is not a per-call parameter.

Different input conversions correctly use generic From<T>, not an associated input type.

Evidence: [`crates/platform/tablecraft-io/src/publication/protocol.rs:22`](../../crates/platform/tablecraft-io/src/publication/protocol.rs).

## `trait-blanket-impl` — conditional

Reviewed conversion uses standard blanket Into supplied by From; backend implementations have explicit platform-specific guarantees and errors.

No repetitive implementation family needing a new public blanket impl was established; adding one can conflict with downstream specialization.

Do not erase platform-specific durability capabilities through a generic facade.

Evidence: [`crates/platform/tablecraft-io/src/error.rs:30`](../../crates/platform/tablecraft-io/src/error.rs); [`crates/platform/tablecraft-io/src/publication/protocol.rs:22`](../../crates/platform/tablecraft-io/src/publication/protocol.rs).

## `trait-coherence-newtype` — observed

Local Currency implements foreign Display; local IoError implements foreign From for foreign io::Error. Both satisfy the actual owner/orphan boundary.

repr(transparent) is not needed solely to implement Display; no pointer cast is justified by this rule alone.

Evidence: [`crates/foundation/tablecraft-core/src/values/currency.rs:42`](../../crates/foundation/tablecraft-core/src/values/currency.rs); [`crates/platform/tablecraft-io/src/error.rs:30`](../../crates/platform/tablecraft-io/src/error.rs).

## `trait-default-methods` — observed

Source defaults explicitly decline optional capabilities; publication sync_namespace defaults to its documented sync_parent operation and can be overridden by actual platform backend.

Defaults must preserve refusal and cannot silently sequential-fallback after a required native index failure.

Evidence: [`crates/query/tablecraft-query/src/source.rs:43`](../../crates/query/tablecraft-query/src/source.rs); [`crates/platform/tablecraft-io/src/publication/protocol.rs:65`](../../crates/platform/tablecraft-io/src/publication/protocol.rs).

## `trait-dyn-vs-generic` — observed

IO and publication use generic storage backends; row visitors/cancellation use borrowed dyn callbacks, retaining one implementation across many callers without allocation.

No profiling of dispatch/code-size tradeoff was run; generic is not an automatic speedup.

Preserve independent cumulative budgets and callback contracts.

Evidence: [`crates/platform/tablecraft-io/src/read.rs:13`](../../crates/platform/tablecraft-io/src/read.rs); [`crates/query/tablecraft-query/src/source/progressive.rs:11`](../../crates/query/tablecraft-query/src/source/progressive.rs); [`crates/platform/tablecraft-io/src/publication/protocol.rs:22`](../../crates/platform/tablecraft-io/src/publication/protocol.rs).

## `trait-object-safety` — observed

Source callbacks are dyn-compatible borrowed visitors. Spawner intentionally has generic FnOnce method and is used via impl Spawner, not dyn Spawner.

A trait not intended for dyn storage need not be object-safe; do not distort its consuming FnOnce contract.

Evidence: [`crates/query/tablecraft-query/src/source.rs:25`](../../crates/query/tablecraft-query/src/source.rs); [`crates/query/tablecraft-query/src/source/progressive.rs:11`](../../crates/query/tablecraft-query/src/source/progressive.rs); [`crates/application/tablecraft-runtime/src/thread_start.rs:5`](../../crates/application/tablecraft-runtime/src/thread_start.rs).

## `type-deref-coercion` — conditional

ParseContext dereferences to its contained ParseLimits, forwarding limit methods while also retaining recursion state. MemberSlot exposes exactly str spelling.

ParseContext is an internal convenience delegation rather than a true smart pointer; consider explicit limits access on next owner refactor, but no authority bug or public API inheritance established.

Evidence: [`crates/query/tablecraft-sql/src/parser/limits.rs:137`](../../crates/query/tablecraft-sql/src/parser/limits.rs); [`crates/query/tablecraft-query/src/plan.rs:72`](../../crates/query/tablecraft-query/src/plan.rs).

## `type-display-vs-debug` — observed

Currency Display emits user value spelling; expressions redact opaque errors in Debug; typed IO errors avoid host payloads.

Rule saying never derive Display conflicts with repository-approved thiserror; derive(Error) intentionally generates reviewed error Display.

Sensitive values/names/SQL/path data must not be emitted through automatic Debug.

Evidence: [`crates/foundation/tablecraft-core/src/values/currency.rs:42`](../../crates/foundation/tablecraft-core/src/values/currency.rs); [`crates/foundation/tablecraft-core/src/values/expression.rs:47`](../../crates/foundation/tablecraft-core/src/values/expression.rs); [`crates/platform/tablecraft-io/src/error.rs:7`](../../crates/platform/tablecraft-io/src/error.rs).

## `type-enum-states` — observed

ExpressionValue explicitly distinguishes Value/StringBytes/Empty/Missing/Error; mailbox optional state and Infallible liveness distinguish no reply from disconnection.

No Boolean-to-enum rewrite justified absent a concrete impossible-state path.

Evidence: [`crates/foundation/tablecraft-core/src/values/expression.rs:12`](../../crates/foundation/tablecraft-core/src/values/expression.rs); [`crates/application/tablecraft-runtime/src/worker.rs:46`](../../crates/application/tablecraft-runtime/src/worker.rs).

## `type-generic-bounds` — observed

Font inventory names its iterator/item bounds in where; joined evaluation places FnMut at function; read allows ?Sized for borrowed readers.

No universal bound reduction proved; some trait implementations intentionally enforce storage semantics at type level.

Evidence: [`crates/presentation/tablecraft-ui/src/fonts/inventory.rs:34`](../../crates/presentation/tablecraft-ui/src/fonts/inventory.rs); [`crates/query/tablecraft-query/src/execution/multivalue/parent/joined.rs:56`](../../crates/query/tablecraft-query/src/execution/multivalue/parent/joined.rs); [`crates/platform/tablecraft-io/src/read.rs:13`](../../crates/platform/tablecraft-io/src/read.rs).

## `type-never-diverge` — observed

Worker uses standard Infallible for an impossible channel payload and exhaustively matches it; spawned actor legitimately returns when disconnected.

No actual never-returning production API requiring ->! was found in this inspected owner. Do not mark terminating actors diverging.

Evidence: [`crates/application/tablecraft-runtime/src/worker.rs:46`](../../crates/application/tablecraft-runtime/src/worker.rs); [`crates/application/tablecraft-runtime/src/thread_start.rs:5`](../../crates/application/tablecraft-runtime/src/thread_start.rs).

## `type-newtype-ids` — observed

Distinct semantic identity and source-generation wrappers preserve namespaces and full token range.

Host request IDs remain raw u64 in local counters; no confirmed swapped-ID bug established by this review.

Evidence: [`crates/foundation/tablecraft-core/src/ids/identity.rs:7`](../../crates/foundation/tablecraft-core/src/ids/identity.rs); [`crates/foundation/tablecraft-core/src/ids/identity.rs:42`](../../crates/foundation/tablecraft-core/src/ids/identity.rs).

## `type-newtype-validated` — observed

SchemaName and Decimal private fields force constructor validation; serde conversion repeats validation.

Names intentionally leave consumer length and native collation policies at owning boundaries.

Evidence: [`crates/foundation/tablecraft-schema/src/name.rs:11`](../../crates/foundation/tablecraft-schema/src/name.rs); [`crates/foundation/tablecraft-core/src/values/decimal.rs:31`](../../crates/foundation/tablecraft-core/src/values/decimal.rs); [`crates/foundation/tablecraft-core/src/values/decimal.rs:86`](../../crates/foundation/tablecraft-core/src/values/decimal.rs).

## `type-no-stringly` — conditional

Native name comparison is an enum; UI parameter kind remains a checked String boundary with enumerated allowed values and explicit source-spec matching.

A future typed parameter-kind DTO could reduce repeated strings, but requires coordinated runtime/CLI/UI wire semantics; no confirmed typo bug here.

Do not mix display labels, stored types, expression states or opaque native tokens.

Evidence: [`crates/foundation/tablecraft-schema/src/name.rs:25`](../../crates/foundation/tablecraft-schema/src/name.rs); [`crates/presentation/tablecraft-ui/src/workspace/query/parameters.rs:18`](../../crates/presentation/tablecraft-ui/src/workspace/query/parameters.rs).

## `type-numeric-fmt` — conditional

Semantic IDs expose explicit token access; Currency Display represents scaled decimal units. No native mask/address wrapper needing hex-format interoperability was identified in these owners.

Hex on a currency would suggest raw scaled representation; optional hex formatting on identity tokens is ergonomics only, not a correctness fix.

Evidence: [`crates/foundation/tablecraft-core/src/ids/identity.rs:7`](../../crates/foundation/tablecraft-core/src/ids/identity.rs); [`crates/foundation/tablecraft-core/src/values/currency.rs:42`](../../crates/foundation/tablecraft-core/src/values/currency.rs).

## `type-option-nullable` — observed

Catalog lookup returns Option for absence and Result for ambiguity. Preview shortness is separate from absent text; SQL NULL remains a value variant.

Do not conflate absent relation, empty child collection, Empty expression and stored NULL.

Evidence: [`crates/foundation/tablecraft-model/src/catalog.rs:62`](../../crates/foundation/tablecraft-model/src/catalog.rs); [`crates/presentation/tablecraft-ui/src/datasheet/preview.rs:18`](../../crates/presentation/tablecraft-ui/src/datasheet/preview.rs); [`crates/foundation/tablecraft-core/src/values/expression.rs:12`](../../crates/foundation/tablecraft-core/src/values/expression.rs).

## `type-phantom-marker` — not verified

Reviewed retained owners carry actual mutable resource references/lifetimes, so compiler tracks their borrows without PhantomData. No unrepresented type/variance/FFI relationship needing a marker was identified.

No repository-wide variance proof or new generic-handle requirement asserted.

Evidence: [`crates/platform/tablecraft-io/src/journal/checked_v1.rs:36`](../../crates/platform/tablecraft-io/src/journal/checked_v1.rs); [`crates/platform/tablecraft-io/src/journal/transaction.rs:13`](../../crates/platform/tablecraft-io/src/journal/transaction.rs).

## `type-repr-transparent` — conditional

Reviewed semantic newtypes have no FFI ABI or pointer-cast consumer; repr(transparent) is optional for their Rust-only use.

serde(transparent) describes serialization, not C ABI. No layout/FFI qualification inferred.

No blanket representation change to logical identities.

Evidence: [`crates/foundation/tablecraft-core/src/ids/identity.rs:7`](../../crates/foundation/tablecraft-core/src/ids/identity.rs); [`crates/foundation/tablecraft-schema/src/name.rs:11`](../../crates/foundation/tablecraft-schema/src/name.rs).

## `type-result-fallible` — observed

Read, storage conversion and exact numeric constructors expose typed failure and propagate it.

UI Option admission is intentional refusal without exception; no blanket Result conversion needed.

Evidence: [`crates/platform/tablecraft-io/src/read.rs:13`](../../crates/platform/tablecraft-io/src/read.rs); [`crates/foundation/tablecraft-core/src/values/expression.rs:25`](../../crates/foundation/tablecraft-core/src/values/expression.rs); [`crates/foundation/tablecraft-core/src/values/decimal.rs:31`](../../crates/foundation/tablecraft-core/src/values/decimal.rs).

## `unsafe-extern-block` — not applicable

Authored Rust search found no manual extern declaration blocks; Windows ABI declarations come from windows-sys dependency. Edition2024 is explicit; generated dependency declarations are outside authored-code review.

Evidence: [`Cargo.toml:29`](../../Cargo.toml); [`crates/platform/tablecraft-io/src/windows_fs.rs:1`](../../crates/platform/tablecraft-io/src/windows_fs.rs).

## `unsafe-maybeuninit` — not applicable

Authored search found no mem::uninitialized, mem::zeroed, MaybeUninit or assume_init. Windows output buffers are initialized arrays before FFI writes. A helper named zeroed in ACE allocates a zero-filled Vec, not unsafe mem::zeroed.

Evidence: [`crates/platform/tablecraft-io/src/windows_fs/security.rs:99`](../../crates/platform/tablecraft-io/src/windows_fs/security.rs); [`crates/platform/tablecraft-io/src/windows_fs/security.rs:167`](../../crates/platform/tablecraft-io/src/windows_fs/security.rs).

## `unsafe-minimize-scope` — observed

Reviewed FFI/pointer operations use narrow local unsafe blocks with safe bounds checks outside; grouped SD calls share same retained aligned buffer proof. Not a complete proof of every Windows ABI declaration.

Evidence: [`crates/platform/tablecraft-io/src/windows_fs/security.rs:68`](../../crates/platform/tablecraft-io/src/windows_fs/security.rs); [`crates/platform/tablecraft-io/src/windows_fs/security.rs:117`](../../crates/platform/tablecraft-io/src/windows_fs/security.rs); [`crates/platform/tablecraft-io/src/windows_fs/security.rs:255`](../../crates/platform/tablecraft-io/src/windows_fs/security.rs); [`apps/tablecraft-desktop/src/regional/windows.rs:17`](../../apps/tablecraft-desktop/src/regional/windows.rs).

## `unsafe-miri-ci` — conditional

No Miri job found in CI. Actual unsafe code is Windows OS FFI plus Windows locale, which is not directly executable under normal Miri isolation. Review feasible pure-pointer helpers/test subsets and ABI platform tests instead of claiming full dynamic UB coverage.

Evidence: [`.github/workflows/ci.yml:11`](../../.github/workflows/ci.yml); [`crates/platform/tablecraft-io/src/lib.rs:33`](../../crates/platform/tablecraft-io/src/lib.rs).

## `unsafe-no-mangle-unsafe` — not applicable

Authored Rust search found no no_mangle/export_name/link_section attributes; no export migration required. Dependency/generated code was not audited.

Evidence: [`Cargo.toml:29`](../../Cargo.toml).

## `unsafe-safety-comment` — observed

Reviewed actual unsafe blocks explain bounded handle/buffer/SID/ACL invariants; workspace forbids unsafe except explicit Windows boundary and locale leaf. No authored unsafe fn requiring caller #Safety found. Comments are evidence for review, not automatic soundness proof.

Evidence: [`crates/platform/tablecraft-io/src/windows_fs/security.rs:67`](../../crates/platform/tablecraft-io/src/windows_fs/security.rs); [`crates/platform/tablecraft-io/src/windows_fs/security.rs:116`](../../crates/platform/tablecraft-io/src/windows_fs/security.rs); [`crates/platform/tablecraft-io/src/windows_fs/security.rs:254`](../../crates/platform/tablecraft-io/src/windows_fs/security.rs); [`apps/tablecraft-desktop/src/regional/windows.rs:13`](../../apps/tablecraft-desktop/src/regional/windows.rs); [`crates/platform/tablecraft-io/src/lib.rs:32`](../../crates/platform/tablecraft-io/src/lib.rs).

## `unsafe-send-sync-manual` — not applicable

Authored apps/crates search found no unsafe impl Send/Sync. Actors use auto-traits from Arc/Mutex/owned payloads; GUI picker future remains intentionally non-Send on GUI thread. No blanket unsafe trait implementation added.

Evidence: [`Cargo.toml:197`](../../Cargo.toml); [`crates/application/tablecraft-runtime/src/worker.rs:74`](../../crates/application/tablecraft-runtime/src/worker.rs).
