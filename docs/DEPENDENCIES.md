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
| actions/checkout | v4 tag | MIT; CI-only source checkout. Uses a version tag per project preference; credentials are not persisted. |

Clap is the only direct third-party Cargo dependency, with ten transitive
packages. Exact versions and registry checksums are committed in Cargo.lock.
Package licenses and minimum Rust versions above were checked against downloaded
Cargo metadata. The derive feature adds compile-time procedural macros and build
scripts in proc-macro2 and quote. The closure declares no native-library links
and requires no extra system libraries. Workspace lint
policy forbids unsafe Rust in project packages; it does not audit or forbid unsafe
code in dependencies or the platform runtime. No new application FFI is introduced.
Clap's Parser derive owns argument parsing, with no direct std::env argument handling. Native target
qualification follows the CI matrix; only macOS has been exercised locally.

Clap can be upgraded through the lockfile and CLI behavior tests or replaced at
the CLI argument boundary. Upstream API reference:
[clap Parser](https://docs.rs/clap/4.6.7/clap/trait.Parser.html).
An advisory scan and independent source-level unsafe-code audit have not been run.

The CLI targets native Windows, macOS and Linux in CI. The local baseline was
built and run on aarch64-apple-darwin; other targets need their actual CI results.
No browser target, C database library, Microsoft runtime, GUI toolkit, network
runtime, font or icon has been adopted. The user-supplied Access file is recorded
as a candidate fixture in FIXTURES.md. Its bytes match the pinned upstream
Access-projects/Access-examples revision; its original MIT notice is retained in
licenses/Access-examples-MIT.txt. Runtime compatibility remains unverified.

New dependencies require the review described in DEVELOPMENT.md;
no vulnerability audit of unadopted candidates is claimed.
