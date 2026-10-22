# Dependency and reference review

Reviewed 18 October 2026 against all three supplied library, Wine and standalone
RustVBA proposals, with the later VBA deferral taking precedence. Every named
library/tool has an explicit disposition below; repetitions share one entry.
Previously selected utilities are included to keep the policy consistent.

This is a fit and upstream metadata/API review, not adoption, benchmarking,
vulnerability scanning, an independent unsafe-code audit or native compatibility
evidence. Candidate versions below are observed upstream metadata, not new Cargo
requirements. Existing locked versions/features remain governed by
[DEPENDENCIES.md](DEPENDENCIES.md). The initial review changed no dependencies;
subsequent working Jiff/zerocopy/wide/bytemuck adoption is reflected below.
The later optional image-journal SHA-256 adoption and its exact transitive closure
are recorded separately in [DEPENDENCIES.md](DEPENDENCIES.md#optional-image-journal-integrity).

Crate links identify the reviewed release and its published license metadata;
[crates.io metadata](https://doc.rust-lang.org/cargo/reference/manifest.html#the-license-and-license-file-fields)
is a declaration, not a review of every bundled file/transitive dependency. Before
adoption, record the exact chosen version, full feature/target/MSRV graph, native
build/unsafe boundaries, notices and working consumer checks. An undeclared MSRV
must be treated as unknown, not as compatibility. Check current advisories and
maintenance at that point. A newer release existing is not a reason to upgrade.

Decisions distinguish **Keep** (already working), **Use/Prefer** (selected for a
real future consumer), **Prototype/Evaluate** (unselected), **Study** (reference,
not linked), **Defer** (outside current work), and **Reject** (unsuitable for the
stated purpose). Directory/package owners below include prospective boundaries;
they do not imply implemented crates. Avoid empty scaffolding or unused manifest
entries. [LIBRARY_POLICY.md](LIBRARY_POLICY.md) remains the concise selection policy.

## Existing foundation and diagnostics

The later [General text-key implementation](NATIVE_TEXT.md#general-text-index-payload-encoding)
adopts reviewed Jackcess 5.0.4 weight data and an Apache-2.0 Rust encoder
adaptation. Exact artifacts, licenses/notices and deterministic generators are
recorded in [the bundled-data record](../crates/engines/tablecraft-ace/data/general/README.md).
It uses existing zerocopy and static lookups, not an OS/generic Unicode collation
substitute. There is no new registry dependency or Java runtime. Independent
encoder equivalence does not establish native Access SQL or text-index traversal.

| Library/tool and reviewed release | Declared license | Decision / owner | Assessment and adoption condition |
| --- | --- | --- | --- |
| [clap 4.6.7](https://docs.rs/crate/clap/4.6.7) | MIT OR Apache-2.0 | Keep; CLI | Parser/Subcommand derives are already integrated; keep CLI-only features and avoid environment-driven parsing. |
| [thiserror 2.0.21](https://docs.rs/crate/thiserror/2.0.21) | MIT OR Apache-2.0 | Keep; Libraries | Typed errors already work; preserve structured failure kinds and sanitized executable diagnostics. |
| [serde 1.0.229](https://docs.rs/crate/serde/1.0.229) | MIT OR Apache-2.0 | Keep optional; core, later concrete protocols | Validated scalar exchange only today; add derives only for real consumers and preserve constructors. |
| [serde_json 1.0.154](https://docs.rs/crate/serde_json/1.0.154) | MIT OR Apache-2.0 | Keep development-only; core tests | Typed round trips only; production JSON needs an explicit wide-number/float policy. |
| [sqlparser 0.63.0](https://docs.rs/crate/sqlparser/0.63.0) | Apache-2.0 | Keep; sql adapter | Lexer/primitives already integrated; keep upstream AST private and Access grammar/semantics owned locally. |
| [time 0.3.55](https://docs.rs/crate/time/0.3.55) | MIT OR Apache-2.0 | Replaced; historical calendar baseline | Removed from manifests/lockfile after Jiff migration. Retain this entry as the evaluated alternative and benchmark baseline. |
| [uuid 1.28.0](https://docs.rs/crate/uuid/1.28.0) | Apache-2.0 OR MIT | Keep; ACE, expressions, CLI | Already handles mixed-endian storage and canonical text; leave RNG/generation disabled until needed. |
| [tracing 0.1.44](https://docs.rs/crate/tracing/0.1.44) | MIT | Keep; Working operation boundaries | Already integrated without subscribers in libraries; record explicit counts/outcomes and exclude payloads. |
| [tracing-subscriber 0.3.23](https://docs.rs/crate/tracing-subscriber/0.3.23) | MIT | Keep host-only; CLI | Already opt-in stderr diagnostics; current lock stays at 0.3.20 despite newer upstream metadata below. |
| [proptest 1.12.0](https://docs.rs/crate/proptest/1.12.0) | MIT OR Apache-2.0 | Keep development-only; Feature-local tests | Existing fixed-seed bounded mutation/invariant tests; preserve shrinking and regression inputs. |

## Native binary formats and buffers

| Library/tool and reviewed release | Declared license | Decision / owner | Assessment and adoption condition |
| --- | --- | --- | --- |
| [binrw 0.15.2](https://docs.rs/crate/binrw/0.15.2) | MIT | Prototype before adoption; ACE codecs | Strong fixed-structure reader/writer candidate. Compare a bounded codec against current checked parsing; derived counts/seeks must retain budgets, cancellation and unknown bytes. |
| [winnow 1.0.4](https://docs.rs/crate/winnow/1.0.4) | MIT | Prototype before adoption; ACE property/object streams | Strong irregular byte-parser candidate; enforce nesting/work/allocation bounds and retain original spans/bytes independently of semantic output. |
| [zerocopy 0.8.63](https://docs.rs/crate/zerocopy/0.8.63) | BSD-2-Clause OR Apache-2.0 OR MIT | Keep; ACE fixed layouts | Adopted for borrowed index headers and long-value descriptors with explicit byte order/alignment and independent goldens. Semantic format checks remain TableCraft-owned. |
| [bilge 0.5.0](https://docs.rs/crate/bilge/0.5.0) | MIT OR Apache-2.0 | Evaluate selectively; ACE dense bitfields | Useful for true packed fields; small masks may be clearer as named constants/bitflags. Preserve reserved bits and qualify derive/MSRV behavior. |
| [deku 0.20.3](https://docs.rs/crate/deku/0.20.3) | MIT OR Apache-2.0 | Alternative, not a second default; ACE bit-level codecs | Can read/write bit structures; compare with binrw/bilge on a concrete format. Avoid overlapping codec frameworks without a separate need. |
| [bytes 1.12.1](https://docs.rs/crate/bytes/1.12.1) | MIT | Use when sharing is needed; I/O and application protocols | Good shared buffers; current fixed pages need no refcounted buffer. Charge retained backing storage and avoid tiny slices pinning large allocations. |
| [bitflags 2.13.2](https://docs.rs/crate/bitflags/2.13.2) | MIT OR Apache-2.0 | Prefer for a working flag abstraction; Native flags and capabilities | Named masks with retained unknown bits can improve APIs; no dependency just to replace a few clear constants, and do not silently truncate reserved bits. |
| [memchr 2.8.3](https://docs.rs/crate/memchr/2.8.3) | Unlicense OR MIT | Benchmark first; Byte scanning | Useful optimized scanning; no correctness change or unmeasured rewrite. Bound input and compare to current slice scans. |
| [cfb 0.15.0](https://docs.rs/crate/cfb/0.15.0) | MIT | Evaluate for identified containers; ACE object preservation/interchange | Reads/writes CFB streams; MDB/ACCDB is not a CFB database. Preserve opaque streams before any project interpretation; VBA execution remains deferred. |
| [bitvec 1.1.1](https://docs.rs/crate/bitvec/1.1.1) | MIT | Evaluate with bitmap consumer; ACE allocation/derived bitmaps | Expresses bit order explicitly; current bounded native usage maps already work. Keep on-disk bit order and allocation accounting independent of container defaults. |
| [byteview 0.10.2](https://docs.rs/crate/byteview/0.10.2) | MIT OR Apache-2.0 | Evaluate with ownership evidence; Immutable page/result buffers | Thin shared immutable slices may suit a future cache; not a parser or substitute for slice::array_windows. Measure retained allocation/lifetime and conversion costs. |

## SQL, numeric and text semantics

| Library/tool and reviewed release | Declared license | Decision / owner | Assessment and adoption condition |
| --- | --- | --- | --- |
| [rust_decimal 1.43.0](https://docs.rs/crate/rust_decimal/1.43.0) | MIT | Adopted; expressions conversions/arithmetic | Checked numeric conversion/arithmetic and explicit bounded rounding behind domain carriers; std only, Serde disabled. Full promotion and native rounding qualification remain pending. Its 96-bit magnitude is not the field envelope; preserve scale, errors and explicit rounding. |
| [bigdecimal 0.4.11](https://docs.rs/crate/bigdecimal/0.4.11) | MIT/Apache-2.0 | Alternative/oracle; Bounded intermediates or tests | Useful exact large coefficients; cap precision, scale, time and allocation. Nonterminating division is not exact and it is not a native storage codec. |
| [decimal-rs 0.2.0](https://docs.rs/crate/decimal-rs/0.2.0) | Apache-2.0 | Alternative; Future broader SQL decimal | 38-digit model could serve broader DECIMAL needs; no current requirement supersedes the field carrier. Compare rounding, scale retention and overflow first. |
| [icu_collator 2.3.1](https://docs.rs/crate/icu_collator/2.3.1) | Unicode-3.0 | Compatibility prototype; expressions and ACE string indexes | The [isolated ordering probe](testing/ICU_COLLATION_PROBE.md) observes differences from both General models even after trailing-space handling. No substring API/default search data; UTF-16 comparison replaces unpaired surrogates. Remains a candidate, with no runtime adoption or native-key substitution. |
| [encoding_rs 0.8.42](https://docs.rs/crate/encoding_rs/0.8.42) | (Apache-2.0 OR MIT) AND BSD-3-Clause | Adopted; explicit scalar character-code pages. Native legacy text/import remain future consumers | [Current single-byte scope](CHARACTER_CODE_PAGES.md) uses non-replacing stack decoding with defaults disabled. WHATWG coverage is not every Windows/OEM code page or native Access equivalence. Preserve original bytes and reject unsupported/unmapped inputs. |
| [jiff 0.2.38](https://docs.rs/crate/jiff/0.2.38) | Unlicense OR MIT | Keep; core and expressions | Replaces time for civil validation/conversion, strict date input and shared Gregorian date extraction. Keep exact Access carriers; std/perf-inline only, without automatic timezone discovery. Benchmarks include its conversion cost. |
| [regex 1.13.1](https://docs.rs/crate/regex/1.13.1) | MIT OR Apache-2.0 | Use with a concrete application feature; Search/import tooling | Bound patterns, compiled size and input. Do not implement Access LIKE by assuming regex syntax/semantics match. |
| [regex-automata 0.4.18](https://docs.rs/crate/regex-automata/0.4.18) | MIT OR Apache-2.0 | Defer until specialized need; Measured search engines | Lower-level engine controls add complexity; prefer regex unless an actual automaton/streaming requirement justifies them. |
| [unicode-normalization 0.1.26](https://docs.rs/crate/unicode-normalization/0.1.26) | (MIT OR Apache-2.0) AND Unicode-3.0 | Use for explicit derived views; Search/display helpers | Previously selected; retain original text and specify normalization form. Normalization alone is not Access collation or identifier equality. |

## Platform I/O, synchronization and caching

| Library/tool and reviewed release | Declared license | Decision / owner | Assessment and adoption condition |
| --- | --- | --- | --- |
| [parking_lot 0.12.5](https://docs.rs/crate/parking_lot/0.12.5) | MIT OR Apache-2.0 | Use when shared state exists; In-process I/O/services | Evaluate against std locks for a real synchronized owner. No cross-process coordination; document non-poisoning and avoid holding guards across awaits. |
| [crossbeam-channel 0.5.17](https://docs.rs/crate/crossbeam-channel/0.5.17) | MIT OR Apache-2.0 | Use for bounded sync workers; Database/application workers | Choose bounded channels and shutdown/backpressure behavior; Tokio channels may fit async owners better. Neither supplies transactions. |
| [fs4 1.1.0](https://docs.rs/crate/fs4/1.1.0) | MIT OR Apache-2.0 | Limited candidate; Generic file locks/allocation | Useful general file operations, not proof of Jet/ACE range locking. Compare the actual APIs and contention semantics before platform adoption. |
| [rustix 1.1.5](https://docs.rs/crate/rustix/1.1.5) | Apache-2.0 WITH LLVM-exception OR Apache-2.0 OR MIT | Preferred Unix API candidate; io/platform | Safe syscall wrappers where available; fcntl_lock convenience API locks a whole file. A qualified range-lock implementation needs appropriate APIs and Unix/SMB tests. |
| [windows-sys 0.61.2](https://docs.rs/crate/windows-sys/0.61.2) | MIT OR Apache-2.0 | Preferred low-level Windows candidate; io/platform | Target-specific file/range-lock APIs remain in scope despite VBA deferral. Narrow features and encapsulate any required unsafe calls under reviewed policy. |
| [windows 0.62.2](https://docs.rs/crate/windows/0.62.2) | MIT OR Apache-2.0 | Defer COM; evaluate other host uses; platform | Higher-level Windows bindings can serve actual host integration; no COM/VBA bridge now. Avoid duplicate overlapping bindings without a consumer. |
| [moka 0.12.16](https://docs.rs/crate/moka/0.12.16) | (MIT OR Apache-2.0) AND Apache-2.0 | Use for recomputable caches; Metadata/plans/services | Previously selected; version/source invalidation and capacity policy required. Best-effort eviction is not a hard quota, page pinning, dirty ownership or mixed-client coherency. |
| [arc-swap 1.9.3](https://docs.rs/crate/arc-swap/1.9.3) | MIT OR Apache-2.0 | Evaluate after simple locks; Read-heavy immutable snapshots | Useful atomic Arc replacement; benchmark update/read patterns and account retained generations. Does not solve mutable page/transaction ownership. |
| [memmap2 0.9.11](https://docs.rs/crate/memmap2/0.9.11) | MIT OR Apache-2.0 | Restrict to qualified stable data; io/cache research | Mapping mutable external files introduces safety/coherency hazards. No writable mixed-client Access mapping; Windows range locks do not exclude mapped access. |

## Execution, collections and host utilities

| Library/tool and reviewed release | Declared license | Decision / owner | Assessment and adoption condition |
| --- | --- | --- | --- |
| [rayon 1.12.0](https://docs.rs/crate/rayon/1.12.0) | MIT OR Apache-2.0 | Use for bounded CPU parallelism; query/engineering/import services | Previously selected; parallelize independent immutable work after measurement. Keep transaction ordering and cooperative cancellation/budget accounting explicit. |
| [tokio 1.53.3](https://docs.rs/crate/tokio/1.53.3) | MIT | Use with async services; Networking/application hosts | Previously selected, not needed by synchronous ACE/CLI. Minimize features; CPU work uses a suitable pool and spawn_blocking needs underlying cancellation checks. |
| [futures 0.3.34](https://docs.rs/crate/futures/0.3.34) | MIT OR Apache-2.0 | Use only needed adapters; Async services | Prefer std Future and existing Tokio facilities when sufficient; add combinators/streams with an actual consumer and clear cancellation semantics. |
| [smallvec 1.16.3](https://docs.rs/crate/smallvec/1.16.3) | MIT OR Apache-2.0 | Benchmark first; Short internal collections | Potential allocation savings for keys/AST children; spilling still allocates and must be budgeted. Avoid exposing it as a permanent public contract prematurely. |
| [arrayvec 0.7.8](https://docs.rs/crate/arrayvec/0.7.8) | MIT OR Apache-2.0 | Use for a real fixed capacity; Bounded internal keys/fields | Can enforce inline capacities; use fallible insertion and explicit overflow. Current arrays may already suffice without another dependency. |
| [dashmap 6.2.1](https://docs.rs/crate/dashmap/6.2.1) | MIT | Evaluate after simple synchronization; Concurrent derived state | Only if measured contention justifies it; guard lifetimes/reentrancy can deadlock. Not a transaction manager or cache-invalidation policy. |
| [slotmap 1.1.1](https://docs.rs/crate/slotmap/1.1.1) | Zlib | Evaluate for ephemeral handles; UI/runtime objects | Generational handles suit local lifetimes, not durable ObjectIds or distributed identity. Validate stale handles and persistence boundaries. |
| [indexmap 2.14.2](https://docs.rs/crate/indexmap/2.14.2) | Apache-2.0 OR MIT | Prefer where insertion order matters; Designer/model properties | Stable iteration can help user-authored definitions; preserve original/native ordering separately. No use solely to replace existing bounded vectors. |
| [anyhow 1.0.104](https://docs.rs/crate/anyhow/1.0.104) | MIT OR Apache-2.0 | Optional executable boundary; CLI/app hosts | Current contextual reporting already works. Adopt if it simplifies a real host; keep library errors typed and payloads redacted. |
| [blake3 1.8.7](https://docs.rs/crate/blake3/1.8.7) | CC0-1.0 OR Apache-2.0 OR Apache-2.0 WITH LLVM-exception | Prefer for new content fingerprints; backup/P2P/artifacts | Define versioned hash domain/canonical bytes first. Keep registered fixture SHA-256 unchanged; hashes alone prove neither authenticity nor consistency of an online backup. |
| [tempfile 3.27.0](https://docs.rs/crate/tempfile/3.27.0) | MIT OR Apache-2.0 | Prefer for real temporary-file consumers; io/tests/exports | Use isolated files with explicit persist/cleanup behavior. Drop cleanup is not crash recovery or atomic durable replacement. |

## Desktop, reporting and interchange

| Library/tool and reviewed release | Declared license | Decision / owner | Assessment and adoption condition |
| --- | --- | --- | --- |
| [egui 0.37.0](https://docs.rs/crate/egui/0.37.0) | MIT OR Apache-2.0 | Selected direction, add with UI; ui/builders | Custom Office-style controls need focus, keyboard and accessibility behavior from the first slice; no GUI dependency in the CLI/engine. |
| [eframe 0.37.0](https://docs.rs/crate/eframe/0.37.0) | MIT OR Apache-2.0 | Selected direction, add with host; Desktop application | Host integration for egui; choose native/browser features with runnable targets rather than pulling defaults into libraries. |
| [wgpu 30.0.1](https://docs.rs/crate/wgpu/30.0.1) | MIT OR Apache-2.0 | Use through qualified renderer; Desktop/render host | Select backends and limits per platform; avoid a separate dependency when eframe already owns it. Core report measurement should remain headless. |
| [accesskit 0.25.1](https://docs.rs/crate/accesskit/0.25.1) | MIT OR Apache-2.0 | High priority with custom controls; ui and desktop host | Wire roles, focus, accessible names/actions and platform adapters with the first controls; visual rendering alone is not accessibility evidence. |
| [parley 0.12.0](https://docs.rs/crate/parley/0.12.0) | Apache-2.0 OR MIT | Prototype shared text measurement; render/reports | Strong shaping/layout/fallback candidate; verify fonts, bidi, line breaks and printer metrics. Preview and PDF/print must share measurements; font licenses need separate review. |
| [krilla 0.8.2](https://docs.rs/crate/krilla/0.8.2) | MIT OR Apache-2.0 | Prototype PDF backend; reports/render | Useful high-level PDF generation; does not implement Access report pagination. Test font embedding, text positioning, output/accessibility needs and native comparisons. |
| [calamine 0.36.1](https://docs.rs/crate/calamine/0.36.1) | MIT | Prefer for spreadsheet reading; connectors/import | Excel/OpenDocument reader; qualify formulas, dates, encodings and supported formats without executing workbook code. Apply row/byte/decompression limits. |
| [rust_xlsxwriter 0.100.0](https://docs.rs/crate/rust_xlsxwriter/0.100.0) | MIT OR Apache-2.0 | Prefer for new XLSX exports; interchange/export | Writes new workbooks, not in-place editing of arbitrary existing Excel files. Test generated output with actual Excel/compatible readers and document loss. |
| [csv 1.4.0](https://docs.rs/crate/csv/1.4.0) | Unlicense/MIT | Adopted for CLI TSV transport; future import/export | Tab delimiter, standard quoting, bounded escaped display cells and explicit NULL/text policy; see [adoption](DEPENDENCIES.md) and [CLI contract](CLI_BROWSING.md). |
| [quick-xml 0.42.0](https://docs.rs/crate/quick-xml/0.42.0) | MIT | Adopted for bounded native Agile descriptors; future Ribbon/import/object codecs | Native descriptors enforce size/depth/namespace/field checks and reject DTDs/extensions; no external resources or callbacks. Other consumers need their own preservation policy. |

## SQLite, P2P, secrets and external interfaces

| Library/tool and reviewed release | Declared license | Decision / owner | Assessment and adoption condition |
| --- | --- | --- | --- |
| [rusqlite 0.40.2](https://docs.rs/crate/rusqlite/0.40.2) | MIT | Adopted synchronous SQLite base; sqlite adapter | Bundled native read-only snapshots, busy/transaction/interrupt checks; see [adoption record](DEPENDENCIES.md#sqlite-reader). Browser storage needs separate implementation/evidence. |
| [tokio-rusqlite 0.8.0](https://docs.rs/crate/tokio-rusqlite/0.8.0) | MIT | Adopted for optional async SQLite reads; sqlite adapter | Worker-owned connection, one admitted request, cancellation and per-request snapshots; see [adoption record](DEPENDENCIES.md#optional-async-sqlite-worker). Dropping a future does not prove SQL stopped. |
| [iroh 1.3.0](https://docs.rs/crate/iroh/1.3.0) | MIT OR Apache-2.0 | Preferred transport direction; p2p | Authenticated encrypted QUIC connections/NAT traversal; qualify discovery/relay/offline identity policy and target footprint. No custom duplicate transport crypto. |
| [loro 1.16.2](https://docs.rs/crate/loro/1.16.2) | MIT | Compare alternative; crdt | Prototype concrete form/report/ordered edits, deterministic replay, persistence and conflicts. Choose versus Automerge; does not enforce relational constraints. |
| [automerge 0.12.0](https://docs.rs/crate/automerge/0.12.0) | MIT | Compare alternative; crdt | Prototype the same workloads, changes/history/size and resolution UX. Do not require both CRDT engines or assume document merge is a valid database transaction. |
| [iroh-blobs 0.103.1](https://docs.rs/crate/iroh-blobs/0.103.1) | MIT OR Apache-2.0 | Later with attachment transfer; p2p/sync | Content-addressed transfer candidate; align iroh versions, limits, verification and permissions. Blob availability is not relational synchronization. |
| [ed25519-dalek 3.0.0](https://docs.rs/crate/ed25519-dalek/3.0.0) | BSD-3-Clause | Evaluate only for separate signature need; security | Device/artifact signatures if transport identity is insufficient; define key lifecycle and canonical signed content. Avoid reimplementing Iroh authentication. |
| [secrecy 0.10.3](https://docs.rs/crate/secrecy/0.10.3) | Apache-2.0 OR MIT | Prefer with credential consumer; security/connectors | Reduce accidental Debug/clone exposure using explicit secret access; does not prevent all copies or protect process memory from an attacker. |
| [zeroize 1.9.1](https://docs.rs/crate/zeroize/1.9.1) | Apache-2.0 OR MIT | Adopted for native Agile key/scratch buffers | Defaults disabled; fixed Zeroizing buffers and RustCrypto zeroization features. Cannot erase arbitrary external copies, swap, registers or backups. |
| [keyring 4.2.0](https://docs.rs/crate/keyring/4.2.0) | MIT OR Apache-2.0 | Prefer qualified OS stores; Desktop/platform | Select platform backend explicitly and handle denied/unavailable stores; no plaintext fallback. Browser and headless environments differ. |
| [wasmtime 49.0.3](https://docs.rs/crate/wasmtime/49.0.3) | Apache-2.0 WITH LLVM-exception | Later plugin prototype; plugins | Capability-scoped host imports, memory/table limits, fuel/epoch interruption and blocking-host-call policy required. Large JIT/native surface; not part of ACE/CLI by default. |
| [cbindgen 0.29.4](https://docs.rs/crate/cbindgen/0.29.4) | MPL-2.0 | Use when C ABI exists; ACE FFI build tooling | Generate headers after a real stable ABI; generator does not make Rust layouts/unwinding/lifetimes C-safe. MPL-2.0 is a tooling license, not an automatic relicense of generated headers. |
| [pyo3 0.30.1](https://docs.rs/crate/pyo3/0.30.1) | MIT OR Apache-2.0 | Selected future Python-host direction; application/tablecraft-python and optional bindings | CPython embedding preferably in a helper process; exact Python/ABI/packaging/isolation and lifetime/error review required with a working consumer. Not adopted and not a prerequisite for standalone engine work. |
| [sqlx 0.9.0](https://docs.rs/crate/sqlx/0.9.0) | MIT OR Apache-2.0 | Not a universal native API; Possible future server connectors | Useful for its supported backends only. Keep native Access source/cursor contracts TableCraft-owned; no mandatory SQLx dependency now. |

## Engineering and verification tooling

| Library/tool and reviewed release | Declared license | Decision / owner | Assessment and adoption condition |
| --- | --- | --- | --- |
| [cargo-fuzz 0.13.2](https://docs.rs/crate/cargo-fuzz/0.13.2) | MIT OR Apache-2.0 | Prioritize a runnable harness; Engineering only | Fuzz current native decoders with budgets and approved corpora; requires an installed compatible toolchain/sanitizer environment, not a production dependency. |
| [libfuzzer-sys 0.4.13](https://docs.rs/crate/libfuzzer-sys/0.4.13) | (MIT OR Apache-2.0) AND NCSA | Harness-only with cargo-fuzz; Fuzz targets | Native libFuzzer build/runtime boundary; keep out of normal product graph and record sanitizer/target/tool versions. |
| [arbitrary 1.5.0](https://docs.rs/crate/arbitrary/1.5.0) | MIT OR Apache-2.0 | Use for structured fuzz targets; Fuzz harnesses | Generate bounded inputs and invalid structures intentionally; derive is not a validity proof or replacement for raw-byte fuzzing. |
| [cargo-nextest 0.9.148](https://docs.rs/crate/cargo-nextest/0.9.148) | Apache-2.0 OR MIT | Optional engineering runner; CI/development | Evaluate speed/isolation on this workspace; retain Cargo/doctest commands and no requirement until installed/runnable evidence exists. |
| [insta 1.49.0](https://docs.rs/crate/insta/1.49.0) | Apache-2.0 | Selective snapshots; Metadata/diagnostic tests | Useful for large stable reports; omit private payloads and review updates. Independent explicit byte/semantic assertions remain necessary. |
| [shuttle 0.9.7](https://docs.rs/crate/shuttle/0.9.7) | Apache-2.0 | Use with first concurrent algorithm; Cache/worker test harnesses | Reproducible randomized schedules require instrumented primitives. No current concurrent engine owner to test; not an SMB or power-loss simulator. |
| [loom 0.7.2](https://docs.rs/crate/loom/0.7.2) | MIT | Use for small concurrency models; Synchronization tests | Explore bounded interleavings using Loom primitives/configuration; not exhaustive for arbitrary production code or real filesystem behavior. |
| [fail 0.5.1](https://docs.rs/crate/fail/0.5.1) | Apache-2.0 | First failpoint candidate; Future write/recovery tests | Established fail-rs-style instrumentation, enabled only in engineering profiles. Preserve errors and stage identities; compare against existing explicit injected Read/Seek failures. |
| [failpoints 0.2.0](https://docs.rs/crate/failpoints/0.2.0) | Apache-2.0 OR MIT | Separate alternative to fail; Future write/recovery tests | Different crate, not an alias. Compare scoped/async behavior and disabled overhead before choosing; no need to install both instrumentation frameworks. |
| [criterion 0.8.2](https://docs.rs/crate/criterion/0.8.2) | Apache-2.0 OR MIT | Add with benchmark workload; bench harnesses | Repeatable read/query/allocation cases and baselines before optimization. Benchmark results do not establish correctness or durable recovery. |
| [cargo-llvm-cov 0.9.1](https://docs.rs/crate/cargo-llvm-cov/0.9.1) | Apache-2.0 OR MIT | Evaluate runnable coverage job; Engineering/CI | Coverage guides missing cases; LLVM tools/target prerequisites and feature profiles must be recorded. No coverage result is claimed now. |
| [cargo-mutants 27.1.0](https://docs.rs/crate/cargo-mutants/27.1.0) | MIT | Targeted future mutation runs; Validation/index/query tests | Check whether assertions reject changed bounds/ordering/errors; bound runtime and interpret surviving/equivalent mutants. Not run in this review. |
| [cargo-deny 0.20.2](https://docs.rs/crate/cargo-deny/0.20.2) | MIT OR Apache-2.0 | Prioritize policy setup; Engineering/CI | License/source/duplicate/advisory policy over real target/feature graphs; review exceptions rather than silently allowing all. Tool not installed/configured by this documentation change. |
| [cargo-audit 0.22.2](https://docs.rs/crate/cargo-audit/0.22.2) | Apache-2.0 OR MIT | Prioritize advisory checks; Engineering/CI | Known RustSec lockfile advisories complement cargo-deny; record database freshness and findings. No scan or clean security claim in this review. |

## Other storage and execution references

| Library/tool and reviewed release | Declared license | Decision / owner | Assessment and adoption condition |
| --- | --- | --- | --- |
| [jetdb 0.3.3](https://docs.rs/crate/jetdb/0.3.3) | MIT OR Apache-2.0 | Study and evaluate isolated reuse; ACE/engineering | Real Rust read-only MDB/ACCDB project, including source extraction research. Upstream lists no index lookup/writer; audit module provenance, bounds and fixtures before reuse. No engine replacement adopted. |
| [redb 4.4.0](https://docs.rs/crate/redb/4.4.0) | MIT OR Apache-2.0 | Study, not native storage; ACE design/engineering | Reject redb as the Access engine. Study transactional copy-on-write B-tree ownership/snapshots; its format/commit protocol cannot replace Access pages or recovery. |
| [fjall 3.1.12](https://docs.rs/crate/fjall/3.1.12) | MIT OR Apache-2.0 | Study, not native storage; ACE/cache/engineering | LSM versioning, testing and caches are useful; no use as native Access pager/index format. A separate auxiliary-store need would require its own decision. |
| [heed 0.22.1](https://docs.rs/crate/heed/0.22.1) | MIT | Study cursor/lifetime APIs; ACE design | LMDB wrapper with native engine/mmap assumptions; heed license does not cover every bundled LMDB component. No production dependency or native Access format reuse. |
| [db-internals 0.1.0](https://docs.rs/crate/db-internals/0.1.0) | MIT OR Apache-2.0 | Educational study only; Engineering research | Illustrative B-tree/WAL/recovery/optimizer algorithms, not evidence of production durability. Crate advertises dual license while repository reports MIT; verify exact files before any reuse. |
| [rocksdb 0.25.0](https://docs.rs/crate/rocksdb/0.25.0) | Apache-2.0 | Reject for native Access storage; Storage research only | Rust C++ wrapper and RocksDB native format/build surface add no MDB/ACCDB compatibility. Not a production backend without a separate justified need. |
| [rhai 1.26.1](https://docs.rs/crate/rhai/1.26.1) | MIT OR Apache-2.0 | Reject as VBA replacement; Deferred language research | A different scripting language cannot supply VBA semantics. No translation/runtime adoption; VBA execution is deferred to independent RustVBA. |
| [tree-sitter 0.27.1](https://docs.rs/crate/tree-sitter/0.27.1) | MIT | Defer VBA editor use; Future external RustVBA/tooling | Incremental syntax infrastructure includes native generated parser/runtime build requirements; syntax trees do not implement language execution. No editor consumer now. |

## Previously selected targeted utilities

| Library/tool and reviewed release | Declared license | Decision / owner | Assessment and adoption condition |
| --- | --- | --- | --- |
| [wide 1.7.1](https://docs.rs/crate/wide/1.7.1) | Zlib OR Apache-2.0 OR MIT | Keep; ACE raw text classification | Adopted ASCII classifiers reduce intermediate allocation. Native bitmap measurements favor word scanning; keep SIMD as a benchmark contender. Numeric/filter batching remains future work. |
| [bytemuck 1.25.2](https://docs.rs/crate/bytemuck/1.25.2) | Zlib OR Apache-2.0 OR MIT | Keep; ACE raw byte vectors | By-value casts of fixed byte arrays into wide byte vectors, with all patterns valid and no alignment/endian assumption. Semantic constructors and stored carriers remain separate. |
| [fst 0.4.7](https://docs.rs/crate/fst/0.4.7) | Unlicense/MIT | Adopted; ACE derived General LIKE element lookup | Immutable compiled single-unit weight sets, not native Jet/ACE index storage. Original regeneration tooling, fixed hashes, exhaustive membership checks and FST/BTree/sorted-vector benchmarks accompany adoption. Broader application search remains future work. |
| [reqwest 0.13.5](https://docs.rs/crate/reqwest/0.13.5) | MIT OR Apache-2.0 | Use with HTTP feature; connectors/services | Explicit TLS/backend/target selection, timeouts and bounded response/decompression behavior; keep network runtime out of synchronous CLI/engine. |
| [sys-locale 0.3.2](https://docs.rs/crate/sys-locale/0.3.2) | MIT OR Apache-2.0 | Use with host localization; platform | Language preferences only, honoring explicit settings. No implicit SQL collation/date parsing change or guaranteed translation availability. |
| [fluent-bundle 0.16.0](https://docs.rs/crate/fluent-bundle/0.16.0) | Apache-2.0 OR MIT | Adopted optionally for numbered query error messages | Flat bounded caller-selected catalogs; [contract](ERROR_MESSAGES.md) and [locked closure](DEPENDENCIES.md#localized-error-messages). UI localization remains future work. |
| [pollster 1.0.1](https://docs.rs/crate/pollster/1.0.1) | Apache-2.0/MIT | Use for small compatible bootstrap; Synchronous host initialization | Only futures that can progress under this executor; not a substitute for Tokio I/O runtime or a blocking call in the UI/async hot path. |

## Reference projects, specifications and non-Cargo tools

These have separate dispositions from the crates above. Upstream claims guide
research; none establishes TableCraft compatibility. Source reuse and redistributed
fixtures require file-level provenance/notices review. Keep the existing pinned
research and Jackcess comparison baselines in their owning native-format docs;
reviewing a fork here does not silently replace them.

| Reference and primary source | License/terms boundary | Decision, owner and limits |
| --- | --- | --- |
| [MDB Tools format documentation (HACKING.md)](https://github.com/mdbtools/mdbtools/blob/dev/HACKING.md) and source | Library LGPL-2.0-or-later; tools GPL-2.0-or-later; review individual files | Essential existing ACE format research for pages/rows/allocation/indexes. Study, do not vendor implementation; reverse-engineered documentation needs golden bytes and independent/native comparison. |
| [Jackcess maintained fork](https://github.com/spannm/jackcess) and [existing jahlborn reference](https://github.com/jahlborn/jackcess) | Apache-2.0 | Essential independent reader/writer reference in engineering. Compare complex columns, index maintenance and version profiles; preserve the exact JAR/source hashes already recorded. Not a runtime dependency or Microsoft oracle. |
| [SQLite architecture](https://www.sqlite.org/arch.html) | Core SQLite public domain; third-party extensions have separate terms | Essential ACE/I/O design study for pager/cache/B-tree boundaries; selected future SQLite backend through rusqlite. Native Access pages remain authoritative. |
| [SQLite locking/recovery](https://www.sqlite.org/lockingv3.html) | SQLite documentation/source terms | Study transaction states and crash invariants; never substitute SQLite locks, WAL or recovery layout for Jet/ACE shared-file behavior. |
| [UCanAccess](https://github.com/spannm/ucanaccess) | Apache-2.0 for the supplied fork; review exact artifacts and transitive notices | High-value SQL compatibility reference: translation, functions, changes and native storage bridging. Engineering comparison candidate only; its results are not Access conformance. |
| [HSQLDB](https://hsqldb.org/web/hsqlLicense.html) | Upstream BSD-style licenses/notices | Study the execution side of UCanAccess. Do not import its general SQL semantics into TableCraft or add a JVM runtime. |
| [LMDB](https://www.openldap.org/software/repo.html) behind heed | OpenLDAP Public License for LMDB; heed's MIT declaration is separate | Study cursor/transaction lifetimes and mapping ownership; no reuse of its file format, locking or mmap assumptions as native Access behavior. |
| [Apache DataFusion](https://github.com/apache/datafusion) | Apache-2.0 and distributed notices | Study optimizer/pushdown/operator separation. Full engine and Arrow remain outside the required executor; an optional analytical backend needs a separate semantic/footprint prototype. |
| [tree-sitter-vba](https://github.com/harumiWeb/tree-sitter-vba) | Repository declares MIT; grammar provenance still needs file-level review | Defer with VBA editor/tooling; possible external RustVBA syntax reference. A grammar is not an interpreter or proof of `.bas`/`.cls`/`.frm` completeness. No grammar copied. |
| [MS-OVBA](https://learn.microsoft.com/en-us/openspecs/office_file_formats/ms-ovba/575462ba-bf67-4190-9fac-c275523c75fc) | Microsoft Open Specifications terms, not a Rust crate license | Retain now for ACE preservation research. Project streams alone do not specify every Access container, compiled cache or safe native write. |
| [MS-VBAL](https://learn.microsoft.com/en-us/openspecs/microsoft_general_purpose_programming_languages/ms-vbal/d5418146-0bd2-45eb-9c7a-fd9502722c74) | Microsoft Open Specifications terms | Language authority for deferred independent RustVBA; not a TableCraft parser/interpreter implementation task. |
| [MS-OAUT](https://learn.microsoft.com/en-us/openspecs/windows_protocols/ms-oaut/2e27b533-5de4-46e9-9b3c-a6b5bcb161de) | Microsoft Open Specifications terms | Retained COM/Automation marshalling reference; dispatch/COM/VBA interfaces are deferred. |
| [Wine](https://github.com/wine-mirror/wine) and its [license](https://github.com/wine-mirror/wine/blob/master/LICENSE) | LGPL-2.1-or-later for Wine; review bundled exceptions/components and separately licensed vendor DLLs | Reference-only Automation date and Boolean/numeric conversion research informs explicit rounding/conversion tests; provider integration remains deferred for external RustVBA. No Wine code/fixtures/runtime adopted. No TableCraft Wine crate/helper; not a sandbox, VBA runtime, Access model or automatic ActiveX renderer. Qualify CPU/OS/bitness and IPC if reconsidered. |
| [LibreOffice VBA support](https://help.libreoffice.org/latest/en-US/text/sbasic/shared/vbasupport.html) | Linked help declares MPL-2.0; product/bundled files need separate review before reuse | Study host/language compatibility limitations only. No LibreOffice runtime or code reuse selected and no inference that Basic compatibility supplies Access objects. |
| [LibreOffice Base guide](https://books.libreoffice.org/en/BG73/BG73.html) and [source](https://github.com/LibreOffice/core) | LibreOffice uses MPL-2.0 plus component-specific terms; inspect exact documentation/source before reuse | Product/frontend reference for visual queries, forms, controls, property editors, reports and backend/ODBC/JDBC separation. Study dbaccess/connectivity/forms/reportdesign; no runtime/code adoption or Jet/ACE storage authority. See [reference scope](REFERENCES.md#database-frontend-and-visual-builder-study). |
| [Kexi](https://kexi-project.org/) | Study only; inspect exact component/artifact licenses before reuse | Complementary standalone visual database application workflow reference. No dependency, code reuse or native Access conformance inferred. |
| [Miri](https://github.com/rust-lang/miri) | Rust project's MIT/Apache-2.0 licensing; toolchain-specific component | Evaluate an eligible test subset with the compatible nightly/component. FFI/OS operations and target support limit coverage; not a substitute for crash/SMB tests. Not installed or run here. |
| Microsoft Access/ACE comparison runtime | Proprietary Microsoft software; provision separately under applicable licenses | Required engineering oracle for native claims, outside production/CLI dependencies. Still unavailable/unrun locally; Jackcess/SQLite/Wine do not replace it. |

## Independent RustVBA proposal: all components deferred

RustVBA is the supplied independent-project direction, not an adopted dependency
or a claim that a released project/API exists. No repository URL or supported
release was supplied. The names below reserve responsibilities for that external
project, not new TableCraft workspace members. Licenses, versions, maintenance,
MSRVs and provider compatibility remain unknown until actual artifacts exist.

| Proposed component | Disposition and responsibility if that independent project proceeds |
| --- | --- |
| `rustvba-cli` | Deferred external CLI; no reason to add it to TableCraft's product closure. |
| `rustvba-core` | Deferred external Variant/coercion/error/object-reference model; TableCraft stored Value is not a replacement VBA runtime. |
| `rustvba-syntax` | Deferred external language parser/model/diagnostics; no TableCraft language implementation now. |
| `rustvba-vm` | Deferred external interpreter/VM, procedures/classes/control flow/error handling. |
| `rustvba-host` | Deferred external embedding/dispatch API; evaluate a real release before reserving dependency versions. |
| `rustvba-project` | Deferred external project reading/writing. TableCraft must still preserve opaque Access VBA without waiting for this crate. |
| `rustvba-wincompat` | Deferred external scripting/path/Windows API emulation; normal TableCraft OS I/O remains independent. |
| `rustvba-com` | Deferred optional external Windows COM provider; no unconditional runtime/native dependency. |
| `rustvba-wine` | Deferred optional external helper/provider; no direct TableCraft-to-Wine integration. |
| `rustvba-testkit` | Deferred external language conformance/fuzzing; TableCraft's future Access host tests would remain separate. |

`tablecraft-vba-host` is the sole reserved future TableCraft integration name,
documentation only. Remove `tablecraft-vba`, `tablecraft-wincompat` and
`tablecraft-wine` from active implementation. Keep `tablecraft-macros` and
`tablecraft-automation` for Access-native macros, non-VBA actions and events;
required table data macros must work or affected writes must fail. Existing VBA
preservation and inert dependency detection remain required. See
[VBA scope](VBA_COMPATIBILITY.md) and [roadmap](ROADMAP.md).

## What to evaluate first

1. Use UCanAccess alongside existing MDB Tools/Jackcess references for SQL/storage
   boundaries. Keep actual Microsoft Access comparisons as the native authority.
2. Prototype binrw/winnow only with a real upcoming codec; compare clarity,
   bounds/cancellation, unknown-byte preservation and dependency cost with current
   parsing. No bulk decoder rewrite is justified by this review.
3. Extend the adopted rust_decimal conversion boundary when general Decimal arithmetic
   contracts are established; evaluate icu_collator against native string-index
   evidence before accepting sort keys.
4. Add runnable fuzz/advisory/license checks as engineering slices; concurrency
   schedule testing and write failpoints follow actual concurrent/write code.
   Future durability work also needs subprocess kill/reopen and real filesystem
   tests. No test library alone proves commit ordering or mixed-client recovery.
5. Introduce AccessKit with the first custom UI controls; prototype Parley/krilla
   together against shared preview/print measurement requirements.

Do not substitute another engine's format, a custom WAL/LSM compactor, generic file
locks, a general cache, a different scripting language or a CRDT merge for required
native Access semantics. Preserve useful off-the-shelf infrastructure at narrow
boundaries, and validate the compatibility-specific behavior independently.

## Native-project and scripting proposal dispositions

These decisions reconcile all seven later proposals. The Python/native-format
proposal supersedes JS/TypeScript as the default; it does not install a runtime.
The links below are primary project documentation/repositories reviewed for role,
not pinned release/license or cross-platform adoption audits. Audit exact artifacts,
features, licenses, maintenance and target closure when introducing real consumers.
The existing PyO3 row above now records its native-host role as well as bindings.

| Candidate/reference | Decision and boundary |
| --- | --- |
| [CPython](https://www.python.org/) | Selected native Python runtime direction; bundle a supported distribution later, with exact version/ABI, licenses, updates and per-target qualification. No interpreter is bundled now. |
| [uv](https://docs.astral.sh/uv/concepts/projects/layout/) | Preferred project environment/lock tool; explicit approved installs, package provenance and platform constraints. Environment/cache/secrets stay outside documents. Not an application dependency yet. |
| [Pyodide](https://pyodide.org/en/stable/usage/wasm-constraints.html) | Browser prototype candidate with reduced package/OS capabilities; no desktop parity or current web implementation. |
| [RustPython](https://github.com/RustPython/RustPython) | Alternative research, not selected over CPython; qualify real packages/extension behavior before any substitution. No second interpreter installed. |
| [Boa](https://github.com/boa-dev/boa) | Earlier JS-host candidate, superseded for initial scripting. Pure-Rust engine research only; no need to prototype/install alongside Python now. |
| [rquickjs](https://github.com/DelSkayn/rquickjs) | Earlier QuickJS-binding alternative, superseded for initial scripting; native build/runtime tradeoffs would need separate justification. |
| [Oxc](https://github.com/oxc-project/oxc) | Later JS/TS parsing/transformation candidate for actual tooling/codegen; not a Python host or a standalone type-checking/execution guarantee. |
| [mlua](https://github.com/mlua-rs/mlua) | Lua/Luau host alternative deferred; no initial second general-purpose language or VBA replacement. |
| [pandas](https://pandas.pydata.org/docs/) | Optional user-project data-analysis package; no automatic installation/bundling or direct bypass of application commands/transactions. |
| [openpyxl](https://openpyxl.readthedocs.io/en/stable/) | Optional user-project spreadsheet package; separate from selecting Rust interchange libraries for TableCraft's own import/export features. |
| [SQLAlchemy](https://www.sqlalchemy.org/) | Optional project-side database toolkit; raw independent connections cannot join host-owned transactions or bypass authorization/validation implicitly. |
| [AccessClone](https://github.com/kentyler/accessclone) | Study intent extraction/source mappings and incremental modernization; upstream-described capabilities are not validated conversion completeness. No code/fixture reuse without exact license review. |
| React / TypeScript web output | Later generated target, not native project scripting. Select/qualify a concrete supported stack with maintainable generated sources and workflow evidence. |
| PHP / Laravel / Symfony output | Later code-generation options, not embedded runtimes. No framework selected and no simultaneous promise of multiple maintained generators. |

SQLite remains the adopted ordinary read-only adapter; its selection as a native
`.tcraft` container is a format decision, not implemented writing/migrations.
No crate/registry package/lockfile change results from this proposal review.

## Agent-interface proposal dispositions

These are selection directions with primary documentation, not installed tools
or exact-release adoption reviews. No manifest/lockfile change follows.

| Candidate | Decision / boundary |
| --- | --- |
| [rmcp / official Rust MCP SDK](https://github.com/modelcontextprotocol/rust-sdk) | Selected for a future thin adapter; qualify exact release/features, client negotiation and optional Tasks/subscriptions. SDK handles do not implement project persistence, authorization, cancellation rollback or native writes. |
| [Ruff](https://docs.astral.sh/ruff/) | Preferred project Python lint/format tool with actual managed developer workflows; do not add it to production Rust dependencies or auto-install on open. |
| Python type checker | Required future developer capability; no specific checker/version selected by the proposal. Review plugin/import behavior and target APIs with real project tooling. |
| [pytest](https://docs.pytest.org/en/stable/) | Preferred ordinary Python test workflow, separately authorized as code execution including discovery/conftest/plugins. Not a safe static inspector or a currently installed application runner. |

The [agent-service contracts](features/AGENT_NATIVE.md) prioritize shared editing,
headless previews and deterministic scenario services over tool count. Existing
rendering/security/query candidates stay with their owners; no new LLM/provider,
chat runtime or separate database engine is required.

## Access template reader candidate

The separate ACCDC deployment reader adopts reviewed `cab` 0.6.0 for bounded
CAB decompression. See the [adoption record](DEPENDENCIES.md#access-deployment-packages-20-october-2026)
for the exact closure/licenses and the
[implemented profile](features/ACCESS_DISTRIBUTION.md#working-accdc-read-profile).
This does not implement the ACCDT OPC/XML template format below.

Reviewed 20 October 2026: [`accdt` 0.1.0](https://github.com/nukleas/accdt),
MIT OR Apache-2.0, edition 2024; no declared rust-version. Published archive
SHA-256 `569c51de568a6833d65b743a4aba29c186bb233fbae3a7cc0371159b5a7e382f`,
VCS metadata `73ede0fbdaa29e17272889d85b61e02d6e004e8f`. Manifest and supplied
MIT license reviewed; no code or fixtures copied. It uses zip 8 with deflate,
roxmltree 0.21 and thiserror 2; default features are empty, with optional
serde/JSON/CLI. It covers template object indexing, schema/data and inert object
parsing, making it a relevant reference/candidate for the required reader.

Not adopted: `Package::from_reader` allocates from entry-reported sizes and reads
all parts to completion without caller resource/cancellation controls; duplicate
names replace earlier entries in a BTreeMap. Directory opening recursively reads
files. Those APIs do not meet TableCraft's bounded, unambiguous package boundary.
Evaluate an upstream bounded API or a separately bounded ZIP/OPC adapter before
adoption; do not route untrusted templates through the unrestricted entrypoint.
No Cargo/lockfile changes result from this review.

## External connectivity direction

Reviewed 19 October 2026 against the upstream documentation linked below. These
are future selections or alternatives, not adopted dependencies, pinned release
reviews, tested integrations or additions to Cargo.lock. Existing calamine,
rust_xlsxwriter, csv and reqwest dispositions above remain applicable.

| Candidate | Disposition and adoption checks |
| --- | --- |
| [Tiberius](https://docs.rs/tiberius/latest/tiberius/) | Preferred native SQL Server/TDS client for `tablecraft-mssql`. Test real SQL Server before Irongate; review TLS/authentication features, exact types and target dependencies. |
| [odbc-api](https://docs.rs/odbc-api/latest/odbc_api/) | Preferred generic ODBC wrapper. Requires installed manager/vendor drivers; review FFI, host-owned environment lifetime, thread affinity, bitness, workers and cancellation. |
| [rs-odbc](https://github.com/mversic/rs-odbc) | Evaluated alternative, deferred in favor of odbc-api; still requires a Driver Manager. No pure-Rust universal protocol substitute. |
| [tokio-postgres](https://docs.rs/tokio-postgres/latest/tokio_postgres/) | Later native PostgreSQL adapter; review TLS, runtime, exact carriers, paging and transaction behavior. |
| [mysql_async](https://docs.rs/mysql_async/latest/mysql_async/) | Later MySQL/MariaDB adapter; qualify servers independently and review TLS/compression/native feature closure. |
| Microsoft Graph / targeted SharePoint REST | Service APIs, not Cargo dependencies. Reuse reqwest with a real consumer; qualify OAuth scopes, field capabilities, paging, conditional writes and throttling. |

At implementation, record exact version, license, MSRV, feature/target/native
requirements in [DEPENDENCIES.md](DEPENDENCIES.md), with locked builds and real
provider tests. A Rust protocol client does not establish a wholly native-free
TLS/authentication dependency closure. No DOCX/mail-merge crate is selected yet;
use system file associations only for explicit opening, without an Office or
ArtCraft dependency. Details and acceptance are in [External Data](features/EXTERNAL_DATA.md).

## Working Unicode casing consumer (20 October 2026)

Adopt icu_casemap 2.3.0 with compiled_data, icu_locale_core 2.3.0 with alloc and
writeable 0.6.4 for the working [LCase/UCase slice](TEXT_CASING.md). All declare
Unicode-3.0; primary ICU packages require Rust 1.88, writeable 1.82, within our 1.99
toolchain. This is distinct from the unadopted ICU collation prototype. The exact
23-package added closure, licenses and feature limits are recorded in DEPENDENCIES.
Upstream security review found [GHSA-7fx9-626j-vqph](https://github.com/unicode-org/icu4x/security/advisories/GHSA-7fx9-626j-vqph);
locked zerovec 0.11.8 and zerovec-derive 0.11.6 include the documented fixes.
No native Access or performance qualification is inferred from this adoption.


## ACCDT package-layer adoption (22 October 2026)

Adopted `zip` 9.0.4 (MIT, Rust 1.88) for the working bounded OPC inventory and
CLI inspection, with defaults disabled and `deflate-flate2` only. It shares the
existing pure Rust flate2 codec. Existing quick-xml is inherited by Access for
control parts; flate2 moves from its dev dependencies to normal dependencies to
select the shared backend. The `accdt` candidate above remains unadopted because
its allocation/cancellation/ambiguity contracts differ from the host boundary.
No third-party code or fixture is copied. [The dependency record](DEPENDENCIES.md#access-template-package-inspection-22-october-2026)
lists the locked closure and platform caveat; [the package contract](ACCESS_TEMPLATES.md)
retains unsupported OPC/ZIP and native qualification limits.


Stored List Definition agreement now uses the existing workspace uuid parser for
field identity normalization; it adds one Access member edge and no registry or
feature change. The interpreter uses existing bounded quick-xml trees, not a SOAP
client. Defaults/validation/application content remains inert. See the
[working agreement profile](ACCESS_TEMPLATE_LISTS.md) and
[dependency record](DEPENDENCIES.md#stored-list-definition-identities-22-october-2026).


## Working OPC URI consumer (22 October 2026)

**Keep:** iri-string 0.7.14 now supplies RFC URI/IRI grammar and string-only target
resolution for ACCDT part reads. It replaces adapter-local reference parsing;
OPC-specific percent mapping and identity checks remain adapter responsibilities.
Only alloc is enabled, with no active transitive dependencies. The exact
license/features/build boundary and local checks are recorded in
[the adoption record](DEPENDENCIES.md#opc-uriiri-consumer-22-october-2026).
No URL fetcher, network client or async runtime is supplied by this consumer.
