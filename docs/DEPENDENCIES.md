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
| thiserror | 2.0.21 | MIT OR Apache-2.0; Rust 1.77 minimum. Core Error/Display derives; default std feature. |
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
