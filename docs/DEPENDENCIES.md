# Adopted dependency and licensing baseline

Recorded on 17 October 2026. The repository's existing source license
is MIT, copyright 2026 RepliCraft. No source license change is introduced.

| Component | Pinned baseline | Role and review |
| --- | --- | --- |
| Rust / Cargo | Rust 1.99.0; Cargo 1.99.0 (5f94df478) observed locally | Edition 2024, minimum Rust 1.99. Rust toolchain is distributed under MIT/Apache-2.0; build tooling is not bundled with the application. |
| rustfmt / Clippy | Components of Rust 1.99.0 | Formatting and linting; installed by the toolchain file. |
| Rust standard library | Rust 1.99.0 | MIT/Apache-2.0 with component notices supplied by the toolchain. Platform runtime/linker requirements follow the selected Rust target. |
| clap | 4.6.7 | MIT OR Apache-2.0; Rust 1.85 minimum. CLI argument parsing via Parser derive, generated help and version. Default features disabled; enabled: std, help, usage, error-context, suggestions, derive. |
| clap_builder | 4.6.7 | MIT OR Apache-2.0; Rust 1.85 minimum. Transitive command builder. |
| clap_lex | 1.1.1 | MIT OR Apache-2.0; Rust 1.85 minimum. Transitive argument lexer. |
| anstyle | 1.0.14 | MIT OR Apache-2.0; Rust 1.66.0 minimum. Transitive formatting types; clap terminal color support remains disabled. |
| strsim | 0.11.1 | MIT; Rust 1.56 minimum. Transitive argument suggestions. |
| clap_derive | 4.6.7 | MIT OR Apache-2.0; Rust 1.85 minimum. Parser procedural macro. |
| heck | 0.5.0 | MIT OR Apache-2.0; Rust 1.56 minimum. Compile-time case conversion. |
| proc-macro2 | 1.0.107 | MIT OR Apache-2.0; Rust 1.71 minimum. Compile-time token processing; includes a build script. |
| quote | 1.0.47 | MIT OR Apache-2.0; Rust 1.71 minimum. Compile-time code generation; includes a build script. |
| syn | 3.0.9 | MIT OR Apache-2.0; Rust 1.71 minimum. Compile-time Rust syntax parsing. |
| unicode-ident | 1.0.27 | (MIT OR Apache-2.0) AND Unicode-3.0; Rust 1.71 minimum. Compile-time identifier classification. |
| thiserror | 2.0.21 | MIT OR Apache-2.0; Rust 1.77 minimum. Library Error/Display derives; default std feature. |
| serde | 1.0.229 | MIT OR Apache-2.0; Rust 1.56 minimum. Optional core dependency with derive/std, used only for scalar value exchange. |
| proptest | 1.12.0 | MIT OR Apache-2.0; Rust 1.88 minimum. Core development dependency, defaults disabled, std enabled; no fork/timeout subprocess dependencies. |
| serde_json | 1.0.154 | MIT OR Apache-2.0; Rust 1.71 minimum. Core development dependency, std enabled; direct typed serialization tests only. |
| actions/checkout | v4 tag | MIT; CI-only source checkout. Uses a version tag per project preference; credentials are not persisted. |

The core's normal minimal dependency is thiserror; Serde is opt-in. Proptest and
serde_json are development dependencies and do not enter release builds. The CLI
still depends only on clap and does not acquire the core's dependencies.

Cargo.lock captures exact resolved versions and registry checksums. Locked Cargo
metadata was checked for declared licenses, MSRVs, feature sets, build scripts and
native-link declarations. This is a metadata review, not an independent source-
level or advisory audit. No package declares a Cargo native-library link. Build
scripts include proc-macro2, quote, thiserror, serde/serde_core and development
packages serde_json, zmij, num-traits, getrandom and libc.
Proptest's RNG dependencies may use platform facilities and internal unsafe code.
Workspace unsafe-code lints apply to project code, not dependency internals.

The serde feature is limited to Value and its scalar carriers. Validated scalars
use their constructors during deserialization. Runtime state and errors are not
serialized. Formats must support the encoded integer widths; no implicit float
conversion or JSON-specific string policy is introduced. See [CORE_TYPES.md](CORE_TYPES.md).

Adopted source references: [thiserror](https://docs.rs/thiserror/latest/thiserror/),
[Serde attributes](https://serde.rs/container-attrs.html),
[Proptest configuration](https://docs.rs/proptest/latest/proptest/test_runner/struct.Config.html),
[serde_json](https://docs.rs/serde_json/latest/serde_json/).
Native target qualification follows CI; only macOS has been exercised locally.
The lockfile and behavior/round-trip tests govern upgrades; the error derives,
property tests and optional serialization can be replaced at their existing
feature boundaries without changing the Access-specific numeric rules.

## Query parser adoption

`tablecraft-query` adopts sqlparser 0.63.0 with default features disabled and
explicit std/recursive-protection. Serde, bigdecimal, visitor, derive-dialect and
JSON example features are disabled. Its other production dependency is the
existing thiserror; proptest is development-only. The CLI/core do not depend on
the query crate. No Tokio, network, GUI or database runtime is introduced.

Additional locked packages and their declared license/toolchain metadata:

| Package | Version | License | Declared minimum Rust / role |
| --- | --- | --- | --- |
| sqlparser | 0.63.0 | Apache-2.0 | No rust-version declared; tested with workspace Rust 1.99.0. Lexer/expression parser; registry includes LICENSE.TXT, no separate NOTICE. |
| log | 0.4.34 | MIT OR Apache-2.0 | 1.71.0; parser logging facade, no logger enabled. |
| recursive | 0.1.1 | MIT | No rust-version declared; stack protection. |
| recursive-proc-macro-impl | 0.1.1 | MIT | No rust-version declared; compile-time stack-protection macro. |
| syn | 2.0.119 | MIT OR Apache-2.0 | 1.71; macro syntax, alongside existing syn 3.0.9. |
| stacker | 0.1.25 | MIT OR Apache-2.0 | 1.63; stack growth, native build script. |
| psm | 0.1.32 | MIT OR Apache-2.0 | 1.88.0; platform stack manipulation, assembly build script. |
| cc | 1.8.0 | MIT OR Apache-2.0 | 1.65.0; native compilation helper. |
| find-msvc-tools | 0.1.14 | MIT OR Apache-2.0 | 1.65.0; native Windows toolchain discovery. |
| shlex | 2.0.1 | MIT OR Apache-2.0 | 1.46.0; build argument parsing. |
| ar_archive_writer | 0.5.3 | Apache-2.0 WITH LLVM-exception | 1.88.0; psm archive build dependency. |
| object | 0.39.1 | Apache-2.0 OR MIT | 1.65; archive/object support, build script. |
| windows-sys | 0.61.2 | MIT OR Apache-2.0 | 1.71; target-specific Windows stack APIs. |
| windows-link | 0.2.1 | MIT OR Apache-2.0 | 1.71; Windows import support. |

Existing cfg-if 1.0.5 (MIT OR Apache-2.0; Rust 1.32) and libc 0.2.190
(MIT OR Apache-2.0; Rust 1.65) now also occur in the query production closure.
The stacker/psm implementation contains unsafe/FFI and architecture-specific
assembly. Project code remains unsafe-forbidden. Native builds need a supported
C/assembly toolchain; Windows and Unix use different stack implementations.
No package declares Cargo `links`, which does not imply absence of native code.
Browser/Wasm and other architectures have not been qualified. Upstream stack
protection is retained after a bounded debug stress test overflowed a test-thread
stack without it. Parser input/nesting/AST limits remain independently enforced.

The registry manifests, enabled feature graph, bundled license files and build
scripts were inspected for this boundary; this is not a full dependency source or
security audit. Cargo.lock records exact checksums. See [SQL_PARSER.md](SQL_PARSER.md)
for the supported syntax and [upstream features](https://github.com/apache/datafusion-sqlparser-rs/blob/v0.63.0/Cargo.toml).

The CLI targets native Windows, macOS and Linux in CI. The local baseline was
built and run on aarch64-apple-darwin; other targets need their actual CI results.
No browser target, C database library, Microsoft runtime, GUI toolkit, network
runtime, font or icon has been adopted. The user-supplied Access file is recorded
as a candidate fixture in FIXTURES.md. Its bytes match the pinned upstream
Access-projects/Access-examples revision; its original MIT notice is retained in
licenses/Access-examples-MIT.txt. Runtime compatibility remains unverified.

New dependencies require the review described in DEVELOPMENT.md. Requested future
choices are in [LIBRARY_POLICY.md](LIBRARY_POLICY.md); they are not adopted yet.
No vulnerability scan or independent unsafe-code audit was run in this change.
