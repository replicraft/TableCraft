# Desktop and engine library review

Reviewed 4 November 2026 against the supplied cross-project proposal, current
workspace declarations, source consumers and locked dependencies. Repeated
recommendations share one row below. This is a fit review; it does not claim
new benchmarks, fuzz runs, formal proofs or native Access comparisons.
Candidate links describe APIs, not newly adopted versions. Exact working choices
remain in [DEPENDENCIES.md](DEPENDENCIES.md).

The UI follows the [actual Access visual references](../ui/VISUAL_REFERENCES.md).
Renderer, geometry and icon choices support that layout; they do not establish
Access behavior. Keep Selawik under `assets/fonts/`, icons under `assets/icons/`, and Fluent catalogs under `locales/`.

## Native formats and collections

| Recommendation | Decision and current evidence |
| --- | --- |
| binrw | Prototype on the next new understood codec. Existing checked readers and borrowed layouts work; derives would still need budgets, exact spans and unknown-byte preservation. No blanket conversion. |
| winnow | Prototype on a new irregular property stream. Present transitively, without a direct consumer. Keep the working Access SQL adapter and charge attempted/backtracking work. |
| zerocopy | Already used for qualified borrowed native index/LVAL layouts. Retain endian/alignment validation and independent golden bytes; no new adoption needed. |
| bitvec | Defer to a real mutable bitmap consumer or measured replacement. Existing usage-map word scans are bounded and measured; a new abstraction must preserve native bit order and work admission. |
| bitflags | Useful for the next genuinely typed flags domain. Current named masks remain clear; preserve unknown raw bits and separate supported-mask checks. No mechanical constant migration. |
| bytes | Adopted directly by the runtime for the bounded captured image shared by its reader and opaque reviewed-edit precondition. Moving the captured Vec into Bytes avoids another whole-image copy; cloned handles retain that complete allocation. Native borrowed pages and core Binary ownership remain unchanged. |
| smallvec | Already a direct development benchmark and transitive library dependency. The native index comparison selected ArrayVec; no production SmallVec migration planned for that collection. |
| arrayvec | Already adopted for private bounded native index columns, after measurement. Keep fallible insertion and the existing ten-column ceiling. |
| memchr | Already a development benchmark. The proposed optimized InStrB search changed early-match work admission, so production retains its checked search. |
| encoding_rs | Already used for explicit legacy character-code conversion and strict Japanese 932 whole-string conversion in expressions. Character operations retain their separate single-byte contract. It does not replace native text collation or decoding policy. |
| indexmap | Defer direct use to an ordered keyed property/registry consumer; currently transitive. Bounded vectors retain native order. Rust hashing/equality cannot replace SQL key semantics. |

The [earlier targeted review](DEPENDENCY_REVIEW.md#targeted-backend-review-24-october-2026)
and [collection measurements](../../tests/reports/schema/INDEX_COLUMN_COLLECTIONS.md) retain
the original experiments and limits.

## Presentation, designers and reports

| Recommendation | Decision and current evidence |
| --- | --- |
| egui / eframe | Already adopted at 0.36.2. Shared presentation uses egui; native startup/window integration stays desktop-owned. No version upgrade from a candidate list. |
| egui_extras | Already used by the virtualized datasheet. Keep it initially; row virtualization does not provide editing, rectangular selection or native write conflicts. Keep an eventual editing model independent of this widget. No empty datasheet crate. |
| AccessKit | Already integrated through egui and eframe; the locked dependency is 0.24.1. Existing tests cover cell context and focus, not full grid roles or platform screen readers. Retain accessibility as a control acceptance requirement. |
| arboard | Already supplied by the native egui-winit clipboard integration. Shared UI emits `Context::copy_text`; adding another direct clipboard owner would duplicate platform handling and hinder web reuse. Multi-cell transport remains future work. |
| rfd | Already adopted, desktop-only. Native asynchronous file selection feeds the same runtime Open command; no independent file-reading path. |
| fontdb | Evaluate with real font discovery/report fallback. Current UI bundles Selawik and egui fallback fonts, so no system-font scan is needed. Its database supplies enumeration/matching, not font fallback, shaping or rendering. |
| kurbo | Strong first form/report designer candidate for screen-space geometry; currently transitive. Keep authoritative Access dimensions in exact native units, with explicit conversion. No designer consumer yet. |
| rstar | Benchmark against linear selection when a real control canvas exists. Typical small forms may not justify an R-tree. No current spatial-query consumer. |
| slotmap | Evaluate for ephemeral editor handles; currently transitive. Existing document generations work. Never persist SlotMap keys as native control identity. |
| petgraph | Prefer for the first working relationship/dependency/refactoring graph. Keep stable domain identity and relationship/query/application edge kinds separate, and bound traversal. Graph algorithms supply neither diagram layout, SQL semantics nor safe schema mutation. Preserve cycles where the domain permits them; do not assume every graph is a DAG. Existing bounded query/cycle checks do not justify an unrelated rewrite. |
| parley | Prioritize for the first shared report/text-layout prototype. Qualify bidi, fallback, line breaking and printer metrics; use one authoritative layout for screen and output. No report renderer exists yet. |
| krilla | Strong PDF output candidate for that report-layout consumer. Does not implement native Access pagination; no PDF dependency until output is implemented. |
| resvg | Evaluate when actual SVG assets are introduced. No need for a second renderer for today's text/buttons; imported application images remain separate preserved content. |
| taffy | Defer for native Access layouts: imported positions/layout rules remain authoritative. Revisit only for an actual layout problem that preserves those rules. |
| Fluent UI System Icons | Approved candidate for selected command assets, with upstream MIT license and provenance at adoption. No icons copied in this review. Match the Access references; preserve embedded application images independently. |

Primary API references: [kurbo](https://docs.rs/kurbo/latest/kurbo/),
[rstar](https://docs.rs/rstar/latest/rstar/),
[petgraph](https://docs.rs/petgraph/latest/petgraph/),
[fontdb](https://docs.rs/fontdb/latest/fontdb/),
[arboard](https://docs.rs/arboard/latest/arboard/), and the
[Fluent icon license](https://github.com/microsoft/fluentui-system-icons/blob/main/LICENSE).
Report candidates have distinct jobs: [Parley](https://docs.rs/parley/latest/parley/)
provides text layout; [Krilla](https://docs.rs/krilla/latest/krilla/) provides PDF
output primitives. Neither supplies the Access report model or pagination policy.

## Application services and concurrency

| Recommendation | Decision and current evidence |
| --- | --- |
| tracing | Already used in backend operations, and now extended to the working runtime task boundary and opt-in desktop stderr diagnostics. Hosts own subscribers; record only fixed operation names and success/supersession, never SQL, credentials, paths or error Debug output. |
| parking_lot | Evaluate specific measured contention or lock ergonomics. Current runtime uses std Mutex/Condvar; changing poison semantics needs an explicit decision. Keep OS Access locks independent. Transitive presence is not direct adoption. |
| crossbeam-channel | Defer a direct dependency until a real queue consumer needs it. The read Worker's mailbox replaces pending work and suppresses stale responses. The separate SaveWorker never replaces an accepted write and retains its terminal report/resources through acknowledgement. An ordinary bounded FIFO does not supply either policy. Present transitively. |
| tokio | Retain existing optional SQLite worker use; that backend's expansion remains paused. No executor required for the synchronous native worker or the currently polled native file-dialog future. |
| tokio-util | Prefer CancellationToken for a real asynchronous task tree. Current synchronous read-generation cancellation closures already integrate with native/query budgets. Save cancellation separately retains the actual terminal outcome and recovery resources. Adding a token would not establish rollback, release those resources or replace generation checks. |
| rayon | Policy-selected, not currently a direct working consumer. Adopt with measured independent immutable CPU batches and explicit accounting; never use it to choose transaction order. |
| moka | Selected for future recomputable shared caches. No current shared cache consumer; define revision/policy keys and hard retained-byte/work admission before adoption. Best-effort eviction is not a resource limit. |
| dashmap | Only with measured concurrent lookup demand. Native source-local RefCell state is not a concurrent connection; map guards introduce callback/reentry hazards. No cache or lock-manager replacement. |
| arc-swap | Evaluate when immutable metadata/configuration actually needs read-mostly publication. Current worker owns its document and transfers bounded responses, so there is no such consumer. |
| memmap2 | Reject writable/live Access mapping in the current design. Stable immutable file-region research is separate; mapping must not bypass native byte-range locking. Transitive window-system use does not authorize native storage mapping. |

[CancellationToken](https://docs.rs/tokio-util/latest/tokio_util/sync/struct.CancellationToken.html)
and [ArcSwap](https://docs.rs/arc-swap/latest/arc_swap/) supply mechanisms, not
native transaction outcomes or revision policy. Runtime tracing uses explicit
[span fields](https://docs.rs/tracing/latest/tracing/span/index.html).

## Engineering tools

| Recommendation | Decision and next meaningful consumer |
| --- | --- |
| proptest | Already used by feature-local invariant tests. Retain shared testkit plumbing and independent byte oracles. |
| cargo-fuzz | Prioritize a runnable bounded native-decoder harness with approved seeds and a compatible sanitizer toolchain. No harness or fuzz execution claimed by this review. Tooling, not a production dependency. |
| rstest | Selective candidate for genuinely clearer new parameterized tests. Existing table-driven loops and testkit helpers do not need wholesale migration. |
| insta | Useful for stable semantic UI/model snapshots once those states settle. Review payload/privacy and snapshot diffs. Explicit behavior tests and controlled visual comparisons remain necessary. |
| shuttle | High-value next concurrency prototype for the actual runtime mailbox, which recently needed a race fix. Instrument the same algorithm's synchronization; a copied simplified model would provide weaker evidence. No integrated schedule harness or execution is claimed here. |
| loom | Alternative for narrow exhaustive atomic/lock models. Do not install alongside Shuttle without a separate concrete purpose; real filesystem/native interoperability stays outside both. |
| kani-verifier | Selective bounded offset/size/invariant proof candidate. Requires working harnesses/toolchain and a connection to real Rust; no formal proof supplied here. |
| criterion | Adopt with a new statistical benchmark consumer; existing native benchmarks already have working harnesses and measurements. No speed claim or bulk rewrite of those benches. |
| cargo-nextest | Optional developer/CI runner after a measured workspace trial. Preserve doctests and ordinary Cargo commands; no tool installation or new CI requirement in this review. |
| cargo-mutants | Target sensitive bounds/qualification checks in a scoped engineering run. Surviving/equivalent mutants need interpretation; not a passing-coverage claim. |
| mockall | Defer until a complex real trait requires it. Existing injected worker operations and testkit storage are sufficient for current scenarios. |
| wiremock | Defer until a working HTTP adapter needs protocol-level tests. No current SharePoint/HTTP integration consumer. |
| Lean / hax / Aeneas | Retain research for selective proofs tied to actual implementation. No installed verifier or compatibility claim. |

[Shuttle](https://docs.rs/shuttle/latest/shuttle/) explores randomized schedules;
a passing run would not prove the worker correct. The
[Kani guide](https://model-checking.github.io/kani/) describes bounded verification,
which also needs explicit harness assumptions.

ODBC, SQL Server, SharePoint and CRDT providers remain optional future consumers.
COM/DAO and VBA runtime work remain deferred. Native-project Python and SQLite
expansion remain paused. None of these proposals resumes those subsystems.

ACE and the CLI retain GUI/PDF/provider-free dependency boundaries; the desktop
composes shared UI and runtime services.

The initial follow-up reused existing tracing dependencies and corrected the UI
against actual Access captures. The connected-editing follow-up gives `bytes` a
working runtime consumer; the [adoption record](DEPENDENCIES.md#shared-captured-database-images)
records its version, features and ownership limits. No unused candidate is added.
Source, test, desktop and platform qualification remain distinct.

## Adoption order after this review

1. Retain the current GUI stack and compare each implemented surface with the
   retained Access captures. Keep the shared datasheet selection/edit state
   independent of `egui_extras`; extract a crate only when it has real consumers.
2. Deepen lifecycle/recovery checks around the working connected editing consumer.
   Preserve its separate nonreplaceable SaveWorker, terminal reports, acknowledgement
   and recovery resources. A latest-wins read mailbox cannot own that write;
   neither a FIFO channel nor a cancellation token supplies its retention policy.
3. Prototype schedule exploration on the actual worker synchronization, and a
   bounded native fuzz harness. These are engineering tasks, not additional
   production dependencies or claims that those tools already run in CI.
4. Introduce `kurbo`/`petgraph` with the first real designer geometry/graph service;
   evaluate `rstar`, `slotmap` and `indexmap` against that service's actual needs.
5. Prototype one shared report-layout service with Parley and evaluate Krilla
   as its PDF output. Keep native measurements, printer metrics and Access
   pagination qualification explicit. These are not prerequisites for browsing.

The proposal's immediate-adoption labels do not override existing measurements:
ArrayVec won the index collection comparison, the memchr replacement changed
work-admission behavior, and no bitmap/parser/lock rewrite is justified solely
by a candidate list. Likewise, criterion, rstest and insta should improve a real
benchmark or test rather than duplicate existing harnesses and assertions.

At captured-image adoption, the runtime's standalone read-only feature profile
passed twenty-five unit tests and one diagnostics test, alongside its optional edit
profile. Those are historical adoption counts, not the current suite inventory.
The connected UI/runtime acceptance and native-observation limits are
recorded in the [desktop guide](../ui/DESKTOP.md#connected-offline-editing-preview).
No candidate performance result is inferred from those functional checks.
