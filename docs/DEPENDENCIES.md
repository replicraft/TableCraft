# Adopted dependency and licensing baseline

The [complete dependency/reference review](DEPENDENCY_REVIEW.md) separately assesses
all libraries/tools in the supplied proposals. Candidate versions/licenses there
are review metadata, not adoption records. This document and Cargo.lock remain
the source for installed dependencies. The initial review only regrouped root
declarations; subsequent working adoption is recorded in the sections below.
VBA/Windows/COM/Wine and RustVBA integration dependencies are deferred under the
[current scope decision](VBA_COMPATIBILITY.md).

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
| jiff | 0.2.38 | Unlicense OR MIT / 1.70 | Core civil validation/calendar conversion and SQL-enabled expression parsing. Defaults disabled, std and perf-inline only. |
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

ACE adds existing clap as a development-only dependency for Parser-derived
benchmark arguments. It does not enter standalone ACE's normal dependency closure.
Hyperfine is an external engineering tool, observed as 2.0.0, with no Cargo/runtime
dependency. Benchmarks use original Rust code and registered/disposable inputs;
no upstream implementation is vendored.

Primary APIs: [Jiff civil](https://docs.rs/jiff/0.2.38/jiff/civil/index.html),
[zerocopy](https://docs.rs/zerocopy/0.8.63/zerocopy/),
[wide](https://docs.rs/wide/1.7.1/wide/),
[bytemuck](https://docs.rs/bytemuck/1.25.2/bytemuck/).
