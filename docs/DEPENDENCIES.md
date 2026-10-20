# Adopted dependency and licensing baseline

## Access deployment packages (20 October 2026)

The Access backend adopts `cab` 0.6.0, MIT, edition 2021, no declared rust-version,
defaults disabled. It reads CAB metadata and compressed blocks after independent
bounded geometry checks; no archive files are installed or executed. Reviewed
published archive SHA-256
`171228650e6721d5acc0868a462cd864f49ac5f64e4a42cde270406e64e404d2`.
API/source: [rust-cab](https://github.com/mdsteele/rust-cab),
[cab 0.6.0](https://docs.rs/cab/0.6.0/cab/).

New locked closure: byteorder 1.5.0 (Unlicense OR MIT, Rust 1.60), lzxd 0.2.7
(MIT OR Apache-2.0, no declared MSRV), time 0.3.55 and time-core 0.1.9
(MIT OR Apache-2.0, Rust 1.88), num-conv 0.2.2 (same dual license, Rust 1.57),
powerfmt 0.2.1 (same, Rust 1.79) and deranged 0.5.9 (same, Rust 1.85).
Manifests and supplied license notices reviewed; none has a build script.
Time is the CAB library's metadata dependency, not a replacement for selected
Jiff civil/calendar semantics. Existing pure Rust flate2 is reused, including
original test-only MSZIP fixture construction. No existing package is upgraded.
No TableCraft unsafe/FFI, GUI, network or async runtime is added. Third-party
implementation details remain upstream; bounded CAB decoding does not establish
package signature verification or native Access qualification.

## Native encryption rewrite copies (20 October 2026)

The engine promotes existing `getrandom` 0.4.3 to a direct normal dependency
(default features off, MIT OR Apache-2.0, Rust 1.85) for OS-generated fresh
password salts/verifiers. It also enables zeroize's alloc feature for owned
rewrite images. No new crypto primitive is implemented. getrandom's existing
build script only selects memory-sanitizer configuration; supported system
backends use upstream platform/unsafe interfaces.

The CLI adopts the selected `tempfile` 3.27.0 (default secure-name initialization,
MIT OR Apache-2.0, Rust 1.63) for complete staged outputs and `persist_noclobber`.
It adds rustix 1.1.5 (Rust 1.65), linux-raw-sys 0.12.1 (Rust 1.63), errno 0.3.14
(Rust 1.56) and fastrand 2.5.0 (Rust 1.63), reusing existing getrandom/libc/
Windows dependencies. rustix/linux-raw-sys offer Apache-2.0 WITH LLVM-exception OR
Apache-2.0 OR MIT; errno/fastrand offer MIT OR Apache-2.0. Source manifests,
notices and platform APIs were reviewed. rustix's build script selects target/
compiler/platform configuration; tempfile and the other new packages declare no
build scripts. Existing libc stays at 0.2.190. Fastrand is for temporary names,
never password/key material. CLI tests reuse existing RC4 as a dev dependency.

No TableCraft unsafe, new C/FFI driver, GUI, network or runtime is introduced.
These dependencies own platform calls. Temporary output publication is separate
from a recoverable native transaction; cleanup after forced termination and
Windows console/filesystem/native Access qualification remain explicit limits.

## Legacy native encryption interoperability (20 October 2026)

`tablecraft-ace` adopts `rc4` 0.2.0 and `md-5` 0.11.0, both MIT OR Apache-2.0,
Rust 1.85, defaults disabled and zeroize enabled. These maintained RustCrypto
primitives consume the existing cipher/digest dependency families. No new
transitive registry package or build script is introduced. Manifests, APIs and
license notices were reviewed; RC4 forbids unsafe code, while MD5 uses the
existing upstream digest/CPU infrastructure. No TableCraft unsafe/FFI, external
provider, runtime, GUI or network dependency is added. RC4 and MD5 are only for
legacy format interoperability; new encryption must not choose them. Engine
vectors and independent file comparisons are described in [encryption coverage](NATIVE_ENCRYPTION.md#profile-and-codec-coverage).

## Hidden CLI password input (20 October 2026)

`tablecraft-cli` adopts workspace-owned `rpassword` 7.5.4 (Apache-2.0,
declared Rust 1.85) for hidden controlling-terminal password prompts. It adds
`rtoolbox` 0.0.6 (Apache-2.0), with existing libc on Unix and windows-sys 0.61.2 /
windows-link 0.2.1 (MIT OR Apache-2.0, Rust 1.71) on Windows. Source manifests,
license notices and console APIs were reviewed; none declares a build script.
These libraries own platform console/unsafe calls; TableCraft adds no unsafe
code, GUI, network or runtime dependency. Returned passwords use existing
Zeroizing<String>. Noninteractive invocations retain explicit `-p`.
MacOS terminal behavior is checked separately from cross-platform builds;
Windows console behavior still needs execution on Windows.
API reference: [rpassword](https://docs.rs/rpassword/7.5.4/rpassword/).

## Native Agile encryption (20 October 2026)

`tablecraft-ace` now uses the following workspace-owned normal dependencies for
working descriptor validation, password verification and explicit page decoding.
Native row/query connections and CLI password input now consume this codec;
explicit remove/change-password copies now consume it. CLI uses the existing zeroize
dependency with its alloc feature to erase its owned password String at dispatch
completion. Test-only CLI encryption derivatives directly reuse workspace AES,
CBC, SHA-256 and fixed-buffer Base64; no registry package/version changed here.

| Package | License / declared minimum Rust | Selected features and role |
| --- | --- | --- |
| quick-xml 0.42.0 | MIT / 1.86 | Defaults off; namespace-aware UTF-8 events, bounded descriptor grammar; no Serde, async or encoding support |
| base64 0.23.1 | MIT OR Apache-2.0 / 1.71 | Defaults off; fixed-buffer field decoding; no allocating/SIMD-unsafe backend |
| aes 0.9.3 | MIT OR Apache-2.0 / 1.89 | Defaults off, zeroize enabled; AES-128/192/256 primitives |
| cbc 0.2.1 | MIT OR Apache-2.0 / 1.85 | Defaults off, block-padding/zeroize enabled; CBC with explicit NoPadding |
| sha1 0.11.0 | MIT OR Apache-2.0 / 1.85 | Defaults off, zeroize enabled; legacy native compatibility profiles |
| sha2 0.11.1 (existing) | MIT OR Apache-2.0 / 1.85 | Existing defaults-off workspace dependency, now normal in ACE with zeroize enabled for SHA-256/384/512 |
| zeroize 1.9.1 | Apache-2.0 OR MIT / 1.85 | Defaults off; owned fixed password/key/scratch buffers erased on drop; no derive/alloc runtime |
| subtle 2.6.1 | BSD-3-Clause / no manifest MSRV | Defaults off; constant-time equality for verifier hash bytes |

New transitive packages are cipher 0.5.2, inout 0.2.5, block-padding 0.4.2 and
cpubits 0.1.1 (all MIT OR Apache-2.0, Rust 1.85). Existing digest/hybrid-array,
cpufeatures and memchr are reused. Cargo.lock retains exact versions/checksums.
Source manifests and license notices were reviewed locally; no newly locked
package declares a build script or native-library link. AES/CPU detection and
zeroize use upstream target-specific/unsafe internals; TableCraft adds no unsafe
code, FFI, network, GUI or executor dependency. This is dependency/license/API
review and independent tests, not an independent cryptographic audit.

The password verifier does not authenticate file/page ciphertext. `Zeroizing`
and upstream zeroization features cover supported owned buffers/state; they do
not promise erasure of compiler-generated copies, host strings/arguments,
registers, swap or backups. Hosts retain credential acquisition/lifetime policy.

The engineering comparison alone uses Jackcess Encrypt 5.0.0 (Apache-2.0) and
Bouncy Castle bcprov-jdk18on 1.86 (MIT); neither is a Rust/application dependency.
Artifact hashes and current limits are in [the encryption record](NATIVE_ENCRYPTION.md).

## Existing dependency baseline

The [complete dependency/reference review](DEPENDENCY_REVIEW.md) separately assesses
all libraries/tools in the supplied proposals. Candidate versions/licenses there
are review metadata, not adoption records. This document and Cargo.lock remain
the source for installed dependencies. The initial review only regrouped root
declarations; subsequent working adoption is recorded in the sections below.
VBA/Windows/COM/Wine and RustVBA integration dependencies are deferred under the
[current scope decision](VBA_COMPATIBILITY.md).

The 19 October provider-layout update relocates DB contracts to foundation and
built-in Access/SQLite integrations to backends. Dependency edges, registry
versions/features and Cargo.lock are unchanged. External connectivity candidates
remain [unadopted selections](DEPENDENCY_REVIEW.md#external-connectivity-direction).

The local ACE `ace16-write-preview` feature adds only an opt-in engineering API
and required-feature example, with no registry dependencies or feature changes.
It is disabled by default and not requested by normal CLI/Access consumers.

CLI presentation adopted 19 October 2026: [comfy-table 8.0.1](https://docs.rs/comfy-table/8.0.1/)
(MIT, Rust 1.88 minimum), workspace-owned and used only by `tablecraft-cli`.
Default features are disabled: no crossterm/TTY runtime or styling dependencies.
The host uses standard-library terminal detection and a fixed wrapping width.
New transitive packages are unicode-segmentation 1.13.3 (MIT OR Apache-2.0,
Rust 1.85) and unicode-width 0.2.2 (MIT OR Apache-2.0, Rust 1.66).
Source manifests/licenses were reviewed locally; Cargo.lock records checksums.
They calculate grapheme boundaries/display widths for presentation, without
changing SQL Unicode/collation semantics. No library/network runtime gains an edge.

CLI TSV encoding adopted 19 October 2026: [csv 1.4.0](https://docs.rs/csv/1.4.0/)
(Unlicense OR MIT, Rust 1.73 minimum), workspace-owned and CLI-only. It has no
optional feature selection. `WriterBuilder` supplies tab delimiters, LF endings,
standard necessary quoting and flexible inspection record widths; the CLI uses
record APIs without adopting Serde serialization. New locked transitive packages
are csv-core 0.1.13 (Unlicense OR MIT; no declared MSRV) and ryu 1.0.23
(Apache-2.0 OR BSL-1.0, Rust 1.71). Existing memchr, itoa and serde_core are reused.
Local upstream manifests/licenses were reviewed. Its buffered writer feeds a
fallibly reserved sink under the existing 1 MiB output limit, including quote
expansion; dependency-internal allocations remain infallible APIs. TSV carries
escaped display cells as described in [CLI browsing](CLI_BROWSING.md).

Recorded on 17 October 2026. The repository's existing source license
is MIT, copyright 2026 RepliCraft. No source license change is introduced.

The saved SELECT adapter promotes the existing local `tablecraft-sql` dependency
from Access's development graph to its normal graph, with unchanged workspace
versions/features. It also reuses workspace Proptest for the adapter's generated
tests. No registry package or feature is newly adopted; native ACE and reusable
query packages still do not import the Access adapter.

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
| proptest | 1.12.0 | MIT OR Apache-2.0; Rust 1.88 minimum. Core/query development dependency, defaults disabled, std enabled; no fork/timeout subprocess dependencies. |
| serde_json | 1.0.154 | MIT OR Apache-2.0; Rust 1.71 minimum. Core development dependency, std enabled; direct typed serialization tests only. |
| actions/checkout | v4 tag | MIT; CI-only source checkout. Uses a version tag per project preference; credentials are not persisted. |

The core's normal minimal dependency is thiserror; Serde is opt-in. Proptest and
serde_json are development dependencies and do not enter release builds. The CLI
uses clap, tablecraft-sql, core values/budgets and tablecraft-expressions with its
`sql` feature for scalar evaluation. Core defaults are disabled; evaluation adds
no new registry packages, Serde, GUI, network or database runtime.

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

## SQLite reader

The synchronous `tablecraft-sqlite` adapter adopts rusqlite 0.40.2 with defaults
disabled and `bundled`, `hooks`, `limits`. libsqlite3-sys builds its bundled SQLite
3.53.2 with a native C compiler and pre-generated bindings; no buildtime bindgen,
SQLCipher, load-extension API, async runtime or network stack is enabled.
SQLite core is public domain; the Rust wrappers/build tooling carry the licenses
below. Project unsafe code remains forbidden; dependency FFI/C is outside that lint.

| New registry package | Version | Declared license | Declared Rust minimum |
| --- | --- | --- | --- |
| rusqlite | 0.40.2 | MIT | Not declared; checked with Rust 1.99 |
| libsqlite3-sys | 0.38.2 | MIT | Not declared; checked with Rust 1.99 |
| fallible-iterator | 0.3.0 | MIT/Apache-2.0 | Not declared |
| fallible-streaming-iterator | 0.1.9 | MIT/Apache-2.0 | Not declared |
| smallvec | 1.16.3 | MIT OR Apache-2.0 | Not declared |
| cc | 1.8.0 | MIT OR Apache-2.0 | 1.65 |
| find-msvc-tools | 0.1.14 | MIT OR Apache-2.0 | 1.65 |
| shlex | 2.0.1 | MIT OR Apache-2.0 | 1.46 |
| pkg-config | 0.3.34 | MIT OR Apache-2.0 | 1.63 |
| vcpkg | 0.2.15 | MIT/Apache-2.0 | Not declared |

These ten packages are the complete added registry lockfile set. Existing bitflags
is reused. Only libsqlite3-sys declares a build script/native link (`sqlite3`).
Bundled builds require target C tools; system-SQLite/browser/Wasm and other targets
are not qualified by local macOS execution. No third-party fixture/code was copied.
Metadata/license/feature review is not a full dependency security audit.

The [SQLite reader contract](SQLITE.md) records snapshots, busy handling, interruption,
carrier checks and resource limitations. SQLite behavior is independent evidence,
not native Microsoft Access conformance. Normal CLI builds include synchronous
SQLite; async integration is a separate optional consumer.

## Optional async SQLite worker

`tablecraft-sqlite/async` adopts tokio-rusqlite 0.8.0 (defaults disabled) and Tokio
1.53.3 (defaults disabled, `sync`). The feature owns working bounded-admission
worker queries; Tokio's semaphore is a direct consumer. Development checks add
Tokio `rt`, `macros`, `time` only. No networking, multithreaded Tokio runtime or GUI
features are requested. CLI does not enable `async` and its observed normal
dependency closure excludes Tokio, tokio-rusqlite, crossbeam, hashlink and Wasm.

Upstream tokio-rusqlite depends on compatible rusqlite 0.40.1 with defaults enabled;
Cargo resolves the same 0.40.2 as our synchronous base. Its defaults add the native
statement cache/hashlink closure and target-specific `ffi-sqlite-wasm-rs` feature.
The latter adds lockfile packages without qualifying a browser backend. We do not
patch upstream to suppress those defaults. Normal native async builds do not
compile the Wasm closure. The previous synchronous-only record remains the baseline.

| Additional registry package | Version | Declared license | Declared Rust minimum / role |
| --- | --- | --- | --- |
| tokio-rusqlite | 0.8.0 | MIT | Not declared; checked with Rust 1.99; worker connection |
| tokio | 1.53.3 | MIT | 1.71; async admission/oneshots |
| tokio-macros | 2.7.2 | MIT | 1.71; development async tests only |
| crossbeam-channel | 0.5.17 | MIT OR Apache-2.0 | 1.60; upstream worker queue |
| crossbeam-utils | 0.8.23 | MIT OR Apache-2.0 | 1.60; build script |
| hashlink | 0.12.2 | MIT OR Apache-2.0 | 1.85; upstream statement cache |
| hashbrown | 0.17.1 | MIT OR Apache-2.0 | 1.85; cache hashing |
| foldhash | 0.2.0 | Zlib | 1.60; hashing |
| sqlite-wasm-rs | 0.5.5 | MIT | 1.81; Wasm only, build script/native link `wsqlite3` |
| rsqlite-vfs | 0.1.1 | MIT | 1.81; Wasm only |
| hashbrown | 0.16.1 | MIT OR Apache-2.0 | 1.65; Wasm closure |
| js-sys | 0.3.106 | MIT OR Apache-2.0 | 1.81; Wasm only |
| wasm-bindgen | 0.2.129 | MIT OR Apache-2.0 | 1.81; Wasm only, build script |
| wasm-bindgen-macro | 0.2.129 | MIT OR Apache-2.0 | 1.81; Wasm closure |
| wasm-bindgen-macro-support | 0.2.129 | MIT OR Apache-2.0 | 1.81; Wasm closure |
| wasm-bindgen-shared | 0.2.129 | MIT OR Apache-2.0 | 1.81; build script/link `wasm_bindgen` |
| bumpalo | 3.20.3 | MIT OR Apache-2.0 | 1.71.1; Wasm closure |
| rustversion | 1.0.23 | MIT OR Apache-2.0 | 1.31; Wasm closure, build script |

These eighteen packages are the complete added registry lockfile set. Native
async metadata/licenses/features and existing-package reuse were reviewed, not
independently security-audited. Tokio/crossbeam/cache/SQLite internals use unsafe
code outside project lints. Local native checks do not qualify Windows/Linux,
browser storage, crash recovery or Microsoft Access compatibility.

## Query parser adoption

The parser package is now `tablecraft-sql` under `crates/query/`; core lives under
`crates/foundation/`. This is a workspace relocation and parser-package rename,
with the CLI dependency updated. Registry dependencies, versions and enabled
features are unchanged. See [ARCHITECTURE.md](ARCHITECTURE.md) for ownership.

`tablecraft-sql` uses sqlparser 0.63.0 (Apache-2.0; no declared MSRV, verified
with Rust 1.99.0), default features disabled and std enabled. Its registry package
includes LICENSE.TXT and no separate NOTICE. The only added transitive package
is log 0.4.34 (MIT OR Apache-2.0, Rust 1.71.0; no logger enabled). Thiserror and
proptest reuse the existing production/development boundaries. The core does not
depend on the query crate; the CLI uses it for syntax parsing. SQLParser Serde, bigdecimal, visitor, derive-dialect,
JSON example and recursive-protection features are disabled.

The first slice used upstream expression parsing and needed recursive-protection
after a stress-test stack overflow. Access precedence now uses a TableCraft Pratt
parser with a shared per-invocation recursion budget. The recursive/stacker/psm
closure, its native build helpers, syn 2 and platform Windows imports have been
removed from Cargo.lock. Current query production dependencies introduce no
native stack-switching code or C/assembly toolchain requirement. Project unsafe
code remains forbidden; this is not an independent dependency source audit.
No GUI, network, async executor or database runtime is introduced.

Cargo.lock records exact registry checksums. Parser tests include cross-subquery
recursion and wide UNION inputs with this minimal feature closure. Browser/Wasm
and Windows/Linux execution have not been qualified locally. Upstream feature
metadata: [sqlparser manifest](https://github.com/apache/datafusion-sqlparser-rs/blob/v0.63.0/Cargo.toml).

## Expression scalar package

`tablecraft-expressions` uses the local core library with default features disabled,
thiserror 2.0.21, and development-only proptest 1.12.0 with std
and defaults disabled. These registry versions, licenses, target/MSRV and unsafe
boundaries are already reviewed above; no new registry packages or features are
adopted. Serde is not enabled or added to evaluation state. The CLI now consumes
core values/budgets and SQL-enabled expression evaluation; its registry versions
and runtime dependency boundary are unchanged. [EXPRESSIONS.md](EXPRESSIONS.md) records the supported
scalar profile and references.

The optional `sql` feature enables the local `tablecraft-sql` dependency (also
with defaults disabled) for owned-expression tree evaluation. Minimal expression
builds exclude SQLParser/log; SQL-enabled builds reuse the parser's reviewed
dependency closure. No evaluation state or bindings receive Serde derives.

## Host and fixture boundaries

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

## Schema and retrieval packages

`tablecraft-schema` consumes core with defaults disabled and thiserror.
`tablecraft-query` consumes local schema/core/SQL plus expressions with `sql`,
thiserror and development-only proptest (std, defaults disabled). All registry
versions/features/licenses/MSRVs/targets and unsafe boundaries reuse the baseline
above. The CLI now consumes the local query package for inline `query run`;
Cargo.lock records that edge. No new registry package, Serde derive, GUI, native provider, async/network
runtime or database library was adopted. Cargo.lock adds only the two local
packages. Schemas and bound execution/result state do not implement Serde.
The Source visitor contract is synchronous; host I/O owns its cancellation and
resource behavior. Production libraries depend on no application packages.

## Native reader and shared object model

`tablecraft-model` consumes core/schema with defaults disabled and thiserror.
`tablecraft-ace` consumes core/schema/model with defaults disabled, thiserror and development-only
proptest (std, defaults disabled), reusing the existing reviewed registry closure.
The CLI consumes ACE and model for native header/catalog inspection. No provider, C
library, unsafe code, codec, GUI/network runtime or new registry package is adopted.
[NATIVE_READING.md](NATIVE_READING.md) records pinned reference provenance and the
LGPL/Apache source-review boundary; upstream implementations are not dependencies.

The temporary independent comparison used Jackcess 5.0.4 (Apache-2.0), outside
the repository and production/CI dependency closures. Its exact artifact hash,
read-only profile and result are recorded in [NATIVE_READING.md](NATIVE_READING.md).
No Java dependency or upstream implementation was vendored into the product.

## Native scalar Source adapter

`tablecraft-access` consumes local ACE/core/model/schema/query with defaults disabled;
its tests additionally use local SQL. The CLI consumes this adapter for explicit
`--database` queries. No new registry package, feature, native dependency, async
executor, GUI or network runtime was adopted. Cargo.lock adds only the local
adapter and dependency edges. ACE and query import no adapter/application package.
[Native query contracts](NATIVE_QUERIES.md) document loading/caching and budgets.

## Calendar conversion and ISO date literals

Historical `time` baseline, superseded by the Jiff migration below.

Adopted 18 October 2026: core uses `time` with defaults disabled and `std` for
checked calendar validation/conversion. SQL-enabled expressions additionally enable
`parsing` and `macros` for compile-time ISO format descriptions. Standalone
expressions remain parser-free; local-offset, large-dates, serde and clock/provider
integration are not enabled. No CLI direct dependency is needed: its output uses
the core calendar conversion API. Legacy stored bits/Serde representation stay unchanged.

| Package | Locked version | License / minimum Rust |
| --- | --- | --- |
| time | 0.3.55 | MIT OR Apache-2.0 / 1.88 |
| time-core | 0.1.9 | MIT OR Apache-2.0 / 1.88 |
| time-macros | 0.2.32 | MIT OR Apache-2.0 / 1.88; compile-time proc macro |
| deranged | 0.5.9 | MIT OR Apache-2.0 / 1.85 |
| num-conv | 0.2.2 | MIT OR Apache-2.0 / 1.57 |
| powerfmt | 0.2.1 | MIT OR Apache-2.0 / 1.79 |

These versions have no build scripts. Calendar arithmetic uses the portable Rust
implementation without local-offset's libc/num_threads or JS integrations. time
and deranged contain internal unsafe/ranged-value primitives; TableCraft calls
checked safe APIs and retains its own unsafe-code prohibition. This dependency
review is not an independent upstream unsafe-code/security audit. Existing
Serde and proptest requirements remain unchanged. See [time 0.3.55](https://docs.rs/time/0.3.55/time/).

## GUID parsing, formatting and native byte order

`uuid` 1.28.0 (MIT OR Apache-2.0, Rust 1.89 minimum, no build script) is adopted
with defaults/features disabled. ACE uses its checked/safe mixed-endian byte
conversion, SQL-enabled expressions use canonical parsing, and CLI uses canonical
formatting. No RNG, generation, version algorithm, serde, JS/wasm binding,
getrandom, atomic or native-provider feature is enabled. No normal transitive
package is added by that feature selection. Core Guid remains its own canonical
byte carrier; semantic ObjectIds are not UUIDs. Upstream uses internal unsafe
formatting primitives; TableCraft uses safe APIs and keeps unsafe forbidden in
its own code. No independent upstream unsafe audit was performed.
Reference: [Uuid API](https://docs.rs/uuid/latest/uuid/struct.Uuid.html).

## Platform I/O extraction

`tablecraft-io` is a local, unpublished package under `crates/platform/`. Its normal
dependencies are core with defaults disabled and the already reviewed thiserror
2.0.21 with its default std feature. It introduces no new registry packages or
registry feature selection, build scripts, unsafe code, FFI, native dependency or
async/network runtime. Its synchronous helpers reuse std Read/Seek and core's
shared budget rather than defining another I/O runtime. Existing transitive core
calendar dependencies remain part of that closure. It has no development-only
registry dependencies or optional features.

ACE now depends on I/O with defaults disabled and maps its typed failures to native
inspection errors. The new local package/edge is recorded in Cargo.lock. I/O imports
no native engine, query, storage adapter or application. [IO.md](IO.md) records the
actual API and limits; file locks/durability are not introduced by the extraction.

## Workspace-owned dependency declarations

Registry version requirements, local crate paths and shared feature/default
selections now live in root `[workspace.dependencies]`. All member dependency and
development-dependency entries inherit those declarations. Optional flags remain
in the owning package, as do consumer-specific features: core's time/std,
SQL-enabled expressions' time/parsing+macros, and callers enabling expressions/sql.
The root time declaration adds no features by itself. No dependency, registry
version, resolved graph or standalone feature selection changed, and Cargo.lock
is unchanged. Minimal expression, SQL-enabled expression and all-feature core
feature trees matched before/after migration. See the
[Cargo workspace reference](https://doc.rust-lang.org/cargo/reference/workspaces.html#the-dependencies-table)
for inheritance and additive features.

`sys-locale` remains a future-use candidate in [LIBRARY_POLICY.md](LIBRARY_POLICY.md).
It is not added to manifests or Cargo.lock; this is not an adoption/target review.

## Structured tracing

Workspace declarations add tracing 0.1.44 (std only, defaults disabled) to CLI,
SQL, query, ACE, Access adapter and I/O. CLI alone adds tracing-subscriber 0.3.20
(fmt, defaults disabled). Libraries emit spans/events without installing subscribers.
No Tokio, exporter, network, environment filter, log bridge, ANSI, JSON or instrument
attribute feature is enabled. Manual spans record only explicitly selected counts,
cache state and outcome; function arguments are not automatically recorded.

Added locked packages (metadata reviewed locally): tracing 0.1.44, tracing-core
0.1.36 and tracing-subscriber 0.3.20 (MIT, Rust 1.65); once_cell 1.21.4
(MIT OR Apache-2.0, Rust 1.65); pin-project-lite 0.2.17 (MIT OR Apache-2.0,
Rust 1.37); sharded-slab 0.1.7 (MIT, Rust 1.42); thread_local 1.1.9
(MIT OR Apache-2.0, Rust 1.63); lazy_static 1.5.1 (MIT OR Apache-2.0,
no declared MSRV). None declares a build script or native link. Upstream synchronization,
thread-local and pinning internals contain unsafe code; project code remains safe.
This is feature/license/metadata review, not an independent unsafe/advisory audit.
See [TRACING.md](TRACING.md) and upstream [tracing](https://docs.rs/tracing/0.1.44/tracing/)
and [subscriber](https://docs.rs/tracing-subscriber/0.3.20/tracing_subscriber/).

## Civil calendars, native layouts and measured byte processing

Adopted 18 October 2026 with working consumers and
[Hyperfine workloads](testing/NATIVE_BENCHMARKS.md):

| Package | Locked version | License / declared minimum Rust | Consumer and features |
| --- | --- | --- | --- |
| jiff | 0.2.38 | Unlicense OR MIT / 1.70 | Core civil validation/calendar conversion, native ACE extended-date decoding and SQL-enabled expression parsing. Defaults disabled, std and perf-inline only. |
| jiff-core | 0.1.1 | Unlicense OR MIT / 1.70 | Jiff's internal calendar primitives; host build uses alloc/std. |
| wide | 1.7.1 | Zlib OR Apache-2.0 OR MIT / 1.89 | ACE raw-byte ASCII classification. Defaults disabled; portable scalar/target SIMD implementations selected upstream. |
| bytemuck | 1.25.2 | Zlib OR Apache-2.0 OR MIT / undeclared | ACE by-value fixed byte-array/SIMD casts; no optional features. All patterns valid; no reference alignment assumption or semantic-value casts. Also used by wide. |
| zerocopy | 0.8.63 | BSD-2-Clause OR Apache-2.0 OR MIT / 1.56 | ACE borrowed index header/long-value descriptor layouts. Defaults disabled; derive only. |
| zerocopy-derive | 0.8.63 | BSD-2-Clause OR Apache-2.0 OR MIT / undeclared | Compile-time layout derives; no handwritten unsafe implementation. |
| syn | 2.0.119 | MIT OR Apache-2.0 / 1.71 | Zerocopy procedural-macro dependency; coexists with the existing syn 3 derives. |
| safe_arch | 1.2.0 | Zlib OR Apache-2.0 OR MIT / 1.89 | wide's x86/x86_64-specific intrinsic wrapper; absent from the macOS ARM normal/build tree. |

Cargo.lock also resolves inactive/target-specific jiff-static 0.2.38
(Unlicense OR MIT, Rust 1.70), portable-atomic 1.15.0 (MIT OR Apache-2.0,
Rust 1.34), portable-atomic-util 0.2.8 (same license, Rust 1.36), defmt 1.2.0
and defmt-parser 1.1.0 (MIT OR Apache-2.0, Rust 1.81), defmt-macros 1.2.0
(same license, undeclared MSRV) and bitflags 1.3.2 (MIT/Apache-2.0, undeclared MSRV).
These are not in the observed macOS ARM CLI normal/build dependency tree.
Jiff's cfg(any()) static-version constraint contributes lockfile packages without
enabling a static timezone runtime. Actual host compilation includes no timezone
database, system-zone discovery, JS, Jiff Serde, logging or clock-provider feature.
Qualify other target graphs separately; target-specific atomic fallbacks and x86
intrinsics are upstream implementation boundaries, not local execution evidence.

Zerocopy has a build script for compiler/cfg selection. The lockfile-only
portable-atomic/util and defmt/macros have build scripts; defmt declares links=defmt
for its embedded logging metadata. No linked native database/GUI/network library
is introduced in the observed host closure. Upstream SIMD, layout/derive and calendar
internals contain unsafe code; safe checked/by-value APIs are used by TableCraft,
whose own unsafe prohibition remains. This is metadata/feature/source-interface
review, not an independent unsafe-code or advisory audit.

`time`, time-core, time-macros, deranged, num-conv and powerfmt are removed from
manifests/Cargo.lock. Core's existing scalar Serde representations remain unchanged;
the unpublished calendar conversion API now accepts/returns Jiff civil DateTime.
Literal grammar, Access field year/precision bounds and negative Automation DATE
rules remain TableCraft-owned. General timezone-aware services and date functions
are not implemented by this migration.

ACE and the Access source adapter use existing clap as a development-only
dependency for Parser-derived benchmark arguments. It does not enter either
library's normal dependency closure. The adapter query harness compares identical
current planning/execution with native constraints honored versus declined; no
new registry package or shared feature was added for predicate pushdown.
Hyperfine is an external engineering tool, observed as 2.0.0, with no Cargo/runtime
dependency. Benchmarks use original Rust code and registered/disposable inputs;
no upstream implementation is vendored.

Primary APIs: [Jiff civil](https://docs.rs/jiff/0.2.38/jiff/civil/index.html),
[zerocopy](https://docs.rs/zerocopy/0.8.63/zerocopy/),
[wide](https://docs.rs/wide/1.7.1/wide/),
[bytemuck](https://docs.rs/bytemuck/1.25.2/bytemuck/).

## Optional image-journal integrity

The working [image transaction protocol](IMAGE_JOURNAL.md) adopts sha2 only behind
tablecraft-io/journal. Root workspace ownership sets sha2 0.11.1 with defaults
disabled; the I/O package's default features remain empty. The normal CLI/native
reader dependency tree excludes sha2 and digest. I/O adds the existing proptest
development edge for two fixed-seed journal properties; no new registry test
package or application/network runtime is introduced.

Resolved all-feature metadata on the local Rust 1.99.0 host:

| Package | Version | Declared license | Declared MSRV | Enabled features |
| --- | --- | --- | --- | --- |
| sha2 | 0.11.1 | MIT OR Apache-2.0 | 1.88 | none |
| digest | 0.11.3 | MIT OR Apache-2.0 | 1.85 | default, block-api |
| block-buffer | 0.12.1 | MIT OR Apache-2.0 | 1.85 | none |
| crypto-common | 0.2.2 | MIT OR Apache-2.0 | 1.85 | none |
| hybrid-array | 0.4.15 | MIT OR Apache-2.0 | 1.85 | none |
| typenum | 1.20.1 | MIT OR Apache-2.0 | 1.41 | const-generics |

Existing cfg-if 1.0.5 and target-selected cpufeatures 0.3.1 supply backend selection;
cpufeatures declares MIT OR Apache-2.0 / Rust 1.85, with no crate features enabled.
Its Unix detection can use the existing libc 0.2.190. The six packages above are
the new lockfile entries; existing package versions were not upgraded. SHA-2's
optional alloc/OID/zeroize features are not enabled, and this protocol needs no
randomness, credentials, native crypto library or external build tool.

The safe incremental Digest API supplies SHA-256; no hash implementation is copied
or written locally. Upstream hardware backends/detection contain unsafe code and
architecture-specific instructions; TableCraft retains its own unsafe prohibition.
Defaults use supported runtime-detected CPU implementations with software fallback
as described in [RustCrypto's backend documentation](https://docs.rs/sha2/0.11.1/sha2/#backends).
Windows/Linux/other CPU execution and independent unsafe review remain unrun.

The current [RustSec sha2 listing](https://rustsec.org/packages/sha2.html) identifies
[RUSTSEC-2021-0100](https://rustsec.org/advisories/RUSTSEC-2021-0100.html) for buggy
0.9.7 AVX2 results; the adopted 0.11.1 is outside that affected release. This
targeted advisory lookup is not a complete lockfile scan: cargo-audit is unavailable
locally. No clean security audit is claimed. Golden framing/digest, corruption,
callback-failure, arbitrary journal and real-file tests qualify the working
consumer; SHA-256 integrity is not authorization, encryption or Access recovery.

## Native validation consumer

The ACE constraint audit now consumes the existing workspace `tablecraft-sql`
and `tablecraft-expressions` with its `sql` feature. Owned syntax and pure scalar
semantics remain independent of native storage and application hosts. This adds
no registry package, async runtime, GUI or network dependency. Versions, licenses
and feature reviews above remain applicable; the lockfile records the two local
dependency edges. Standalone ACE checks and workspace tests/Clippy exercise them.

## Optional native journal consumer

`tablecraft-ace/journal` now enables the existing `tablecraft-io/journal` feature
for validated native edit plans and recovery. It reuses the reviewed optional
SHA-256 dependency; no registry package is added. Default engine/CLI readers keep
this feature disabled. Standalone minimal/all-feature engine and I/O checks
exercise both dependency closures. Hosts retain durable namespace/locking duties.

## Working database contract package

`tablecraft-db` adds only existing core/schema/thiserror dependencies. The Access
adapter consumes it for snapshot-issued row handles and native edit planning, and
uses existing thiserror for typed adapter errors. No registry package or platform
runtime is added. The standalone native engine does not depend on DB or Access.
Locked standalone checks, focused tests and all-target Clippy verify this split.

## Bounded UPDATE consumers

Query editing now consumes `tablecraft-db` alongside existing scalar semantics.
The CLI directly consumes the existing `tablecraft-io` for shared input locks and
bounded fresh-copy output. No registry package, GUI/network/async runtime or
journal feature is added to the default CLI. SQLite production code is unchanged;
its optional tests verify shared query-runtime changes without expanding scope.

## Engineering dependency-boundary checks

The [workspace policy guard](architecture/DEPENDENCY_BOUNDARIES.md) uses the
Python 3.11+ standard library and installed Cargo metadata, without package
installation or Cargo dependency changes. CI selects Python 3.12 through
`actions/setup-python@v5`; it runs the guard and ten policy regression tests on
its existing platform matrix. This engineering interpreter is separate from the
planned CPython/PyO3 application host. No Python scripting, `.tcraft` writer or
new runtime/registry dependency was adopted by the architecture decision.

## Native attachment compression

`tablecraft-ace` adopts `flate2` 1.1.10 with default features disabled and only
`rust_backend`. It owns working bounded FileData zlib decoding alongside direct
uncompressed wrappers. Normal scalar/complex-child reads preserve encoded Binary;
decoding is explicit. No archive extraction, native C/zlib linkage, network or
GUI runtime is enabled. The codec does not open file contents or choose paths.

| Added registry package | Version | Declared license | Declared Rust minimum |
| --- | --- | --- | --- |
| flate2 | 1.1.10 | MIT OR Apache-2.0 | 1.67.0 |
| miniz_oxide | 0.9.1 | MIT OR Zlib OR Apache-2.0 | Not declared |
| adler2 | 2.0.1 | 0BSD OR MIT OR Apache-2.0 | Not declared |
| crc32fast | 1.5.2 | MIT OR Apache-2.0 | 1.63 |
| simd-adler32 | 0.3.10 | MIT | Not declared |

These five are the complete lockfile addition. Existing cfg-if is reused.
Only crc32fast declares a build script; none declares a native-library link.
Flate2's runtime_detection feature is disabled. The selected miniz_oxide backend
enables with-alloc and SIMD Adler support; dependency internals are outside the
workspace unsafe-code lint. Other targets remain subject to their own checks.

The [upstream API](https://docs.rs/flate2/1.1.10/flate2/struct.Decompress.html)
provides bounded output slices, progress counters, checked zlib streams and end
status. TableCraft requires exact input/output completion, caps input/output and
charges copy/decode blocks plus a conservative 128 KiB logical codec workspace.
The selected boxed InflateState was observed as 43,296 bytes on this ARM host;
upstream allocations are not intercepted by ResourceBudget. Project output
buffers reserve fallibly. This is not a process allocator quota or a source-level
security audit. Recheck backend allocation/features when upgrading.

Metadata/licenses/features/build scripts were inspected from locked Cargo
metadata and the selected cached manifests. No upstream implementation was
copied. Format research uses the separately reviewed Jackcess attachment wrapper;
[advanced field documentation](features/ACCESS_ADVANCED_FIELDS.md) records the
implemented bounds, independent fixture comparison and remaining qualification.


## Shared Gregorian date component kernels

Expressions now inherits Jiff as a normal parser-independent dependency rather
than enabling it only through the sql feature. Year/Month/Day/Hour/Minute/Second
have working scalar/query consumers: legacy DATE reuses core calendar conversion
and Jiff civil nearest-second rounding; Extended Date/Time reads exact validated
components. The sql feature still owns tablecraft-sql/UUID parsing. Root version,
std/perf-inline features, lockfile and licenses are unchanged; no timezone, clock,
provider or platform runtime is enabled. Core date carriers/Serde remain unchanged.
[Expression contracts](EXPRESSIONS.md#gregorian-date-component-built-ins) retain
explicit rounding/coercion/native-qualification limits. Dependency boundary checks
and their ten tests passed.

## Explicit Currency conversion dependency

rust_decimal 1.43.0 (MIT, declared MSRV 1.67.1) now has a working private consumer
in tablecraft-expressions' CCur kernel. The root declaration disables defaults
and enables only std; the member inherits it. Serde/JSON, database integrations,
random, macros, maths, legacy-ops, bytemuck and rkyv integrations stay disabled.
The normal/build closure adds arrayvec 0.7.8 (MIT OR Apache-2.0, MSRV 1.51) and
reuses existing num-traits/autocfg. rust_decimal's build script reads its upstream
README and writes generated documentation into OUT_DIR; no native library or
network operation is introduced. arrayvec has no build script. This is a license,
manifest, feature and build-script review, not a complete unsafe/advisory audit.
Existing domain constructors and native codecs retain their Access-specific rules.

The expression package additionally inherits existing clap 4.6.7 as a development
dependency for the Parser-based benchmark example; clap is absent from its normal
library graph. No existing locked package version was removed or upgraded. Cargo
resolution adds 26 package/version entries; only rust_decimal and arrayvec are new
active normal packages in the observed host closure. The other 24 entries are
optional-resolution lockfile entries, not enabled integrations or runtime adoption.
All added entries were checked against cached upstream package metadata:

| Added package/version | License expression | Declared MSRV | Active addition |
| --- | --- | --- | --- |
| arrayvec 0.7.8 | MIT OR Apache-2.0 | 1.51 | Normal |
| borsh 1.8.1 | MIT OR Apache-2.0 | 1.77.0 | Lockfile only |
| borsh-derive 1.8.1 | Apache-2.0 | 1.77.0 | Lockfile only |
| bytes 1.12.1 | MIT | 1.57 | Lockfile only |
| cfg_aliases 0.2.2 | MIT | undeclared | Lockfile only |
| equivalent 1.0.3 | Apache-2.0 OR MIT | 1.6 | Lockfile only |
| getrandom 0.2.17 | MIT OR Apache-2.0 | undeclared | Lockfile only |
| getrandom 0.3.4 | MIT OR Apache-2.0 | 1.63 | Lockfile only |
| indexmap 2.14.2 | Apache-2.0 OR MIT | 1.85 | Lockfile only |
| ppv-lite86 0.2.21 | MIT OR Apache-2.0 | 1.61 | Lockfile only |
| proc-macro-crate 3.5.0 | MIT OR Apache-2.0 | 1.82.0 | Lockfile only |
| r-efi 5.3.0 | MIT OR Apache-2.0 OR LGPL-2.1-or-later | 1.68 | Lockfile only |
| rand 0.8.8 | MIT OR Apache-2.0 | undeclared | Lockfile only |
| rand 0.9.5 | MIT OR Apache-2.0 | 1.63 | Lockfile only |
| rand_chacha 0.3.1 | MIT OR Apache-2.0 | undeclared | Lockfile only |
| rand_chacha 0.9.0 | MIT OR Apache-2.0 | 1.63 | Lockfile only |
| rand_core 0.6.4 | MIT OR Apache-2.0 | undeclared | Lockfile only |
| rand_core 0.9.5 | MIT OR Apache-2.0 | 1.63 | Lockfile only |
| rust_decimal 1.43.0 | MIT | 1.67.1 | Normal |
| toml_datetime 1.1.2+spec-1.1.0 | MIT OR Apache-2.0 | 1.85 | Lockfile only |
| toml_edit 0.25.17+spec-1.1.0 | MIT OR Apache-2.0 | 1.85 | Lockfile only |
| toml_parser 1.1.5+spec-1.1.0 | MIT OR Apache-2.0 | 1.85 | Lockfile only |
| wasi 0.11.1+wasi-snapshot-preview1 | Apache-2.0 WITH LLVM-exception OR Apache-2.0 OR MIT | undeclared | Lockfile only |
| wasip2 1.0.4+wasi-0.2.12 | Apache-2.0 WITH LLVM-exception OR Apache-2.0 OR MIT | 1.87.0 | Lockfile only |
| winnow 1.0.4 | MIT | 1.65.0 | Lockfile only |
| wit-bindgen 0.57.1 | Apache-2.0 WITH LLVM-exception OR Apache-2.0 OR MIT | 1.85.0 | Lockfile only |

Dependency boundary checks and all ten policy tests passed. Workspace all-feature
checks still do not activate upstream optional integrations. [CCur contracts](EXPRESSIONS.md#explicit-currency-conversion)
and [benchmarks](testing/CURRENCY_CONVERSIONS.md) record independent numerical
checks and measured cost; no native Access compatibility claim follows from adoption.

## Decimal expression range and CDec

CDec reuses the existing private rust_decimal 1.43.0 dependency with unchanged
std-only features and lockfile. The normal conversion boundary now shares exact
numeric import with CCur, choosing normalized FromPrimitive floating conversion
for CDec and retained conversion for CCur. Core remains independent of the library
and represents the full 96-bit coefficient/scale-28 scalar range. Schema/native
field limits stay precision 28. No dependency, serialization integration, runtime
or native codec dependency was added. Conversion and native Access qualification
limits are recorded in [EXPRESSIONS.md](EXPRESSIONS.md#explicit-decimal-conversion-and-expression-range).

## Checked Decimal arithmetic consumer

The existing private rust_decimal 1.43.0 std-only dependency now supplies checked
add/subtract/multiply/divide when a Decimal operand participates. This also enables
the shared query Sum/Avg fold for Decimal inputs. Existing conversion imports and
validated core result construction are reused; no package, feature or serializer
integration was added. Exponentiation uses the existing finite Double conversion/
power path. [Contracts](EXPRESSIONS.md#checked-decimal-arithmetic) and
[measurements](testing/DECIMAL_ARITHMETIC.md) distinguish the bounded engineering
profile from pending native Access promotion/rounding/aggregate qualification.
