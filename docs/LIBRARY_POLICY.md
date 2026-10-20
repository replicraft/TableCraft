# Library choices

#1 Tenant: Do not reinvent the wheel when it is possible to use something better off the shelf

The user requested these libraries for matching future functionality. This is a
dependency selection policy, not evidence that those subsystems exist. Adopt each
library with actual working code, relevant features, license/target review and
tests. Record exact adopted versions in DEPENDENCIES.md and Cargo.lock. Existing
headless and native data-safety boundaries still apply.

The complete [dependency/reference review](DEPENDENCY_REVIEW.md) evaluates every
item in the three supplied library, Wine and RustVBA proposals, including alternatives
and engineering tools. It records individual decisions, sources, reviewed release
metadata/licenses, owners and limits; none of its candidates is installed merely
by being listed. [DEPENDENCIES.md](DEPENDENCIES.md) records actual adoption.

The later [VBA deferral](VBA_COMPATIBILITY.md) supersedes interpreter, editor/debugger,
VBA-specific DAO/ADO, Windows emulation, COM/Wine and immediate RustVBA integration
recommendations. Existing VBA preservation and inert dependency detection remain
required; Access table data macros remain in database scope. Normal OS I/O is
independent of this deferral.

## Adopted now

- Reviewed Jackcess General weight data and an Apache-2.0 Rust encoder adaptation:
  native General legacy/General 2010 index payloads, with static borrowed zerocopy
  lookups and independent key checks. Retain the bundled licenses/notices. This
  does not select a generic Unicode/OS sort order or implement SQL collation.

- `cab`: Access backend CAB deployment metadata/decompression after bounded
  geometry checks. Package signatures stay unverified; no installation or
  active content runs. Its transitive `time` metadata dependency does not replace
  Jiff's selected calendar role.

- `getrandom`: OS-backed fresh native password salt/verifier generation; do not
  use temporary-name RNGs for cryptographic material.
- `tempfile`: CLI staged fresh encryption outputs with non-clobbering publication;
  temporary-file cleanup is not native transaction/crash recovery.

- `rc4` / RustCrypto `md-5`: legacy native Jet/Office interoperability; use
  zeroizing implementations. Only explicit Jet 4 add operations select native
  recoverable RC4 encoding for new outputs. New ACCDB encryption uses Agile
  AES-256/SHA-512, never RC4/MD5.

- `rpassword`: CLI-only hidden terminal password acquisition; engine libraries
  accept explicit credentials and never open a terminal. Retain zeroizing owned
  strings and payload-free failures.

- `flate2`: bounded native attachment zlib decoding with defaults disabled and the
  pure Rust `rust_backend`. Native wrappers, sizes, exact stream completion and
  publication remain TableCraft's responsibility; no file content is executed.
  Other compression formats and large streaming attachments are unqualified.

- `rusqlite`: synchronous read-only SQLite snapshots feeding TableCraft query
  evaluation. `tokio-rusqlite` and Tokio `sync` provide optional worker-owned
  bounded requests behind `tablecraft-sqlite/async`; the CLI leaves that feature
  disabled. Hosts own executors and request-specific cancellation. See [SQLite](SQLITE.md).

- `sha2`: SHA-256/384/512 for native Agile password/page decoding, plus optional
  SHA-256 integrity behind tablecraft-io/journal. Defaults remain disabled; native
  crypto enables its zeroization feature. Journal hashing does not implement
  encryption or native Access recovery. See [encryption](NATIVE_ENCRYPTION.md)
  and [journal scope](IMAGE_JOURNAL.md).

- `quick-xml` and `base64`: bounded, namespace-aware Agile descriptors and fixed
  decoded fields. Defaults are disabled: no XML Serde/async/encoding runtime,
  external entity resolution, or Base64 allocation/SIMD backend is enabled.
- RustCrypto `aes`, `cbc`, `sha1`, plus `subtle` and `zeroize`: native AES-CBC
  compatibility codecs, legacy SHA-1 profiles, verifier comparison and wiping of
  supported owned key/scratch buffers. Defaults are disabled; AES/CBC/hash state
  zeroization is enabled. These primitives do not authenticate ciphertext or
  qualify native file layouts by themselves. See [adoption](DEPENDENCIES.md).

- `tracing`: explicitly selected structured operation spans/events in working libraries.
  Hosts own subscribers; omit SQL, parameters, values, object names and file paths.
  CLI uses `tracing-subscriber` with explicit verbosity and stderr output. See [TRACING.md](TRACING.md).

- `thiserror`: typed library errors with derived Error/Display implementations.
- `serde`: optional core serialization; validated types must use their constructors
  during deserialization. Currently limited to Value and its scalar carriers.
  Add support to other types only when a concrete consumer needs it.
  Format-specific application protocols remain separate.
- `proptest`: development-only generated invariants with shrinking, fixed seeds
  and saved regressions.
- `serde_json`: development-only, direct typed round-trip checks. The application
  has not adopted JSON output yet.
- `sqlparser`: lexer/parser primitives inside the bounded Access query grammar.
  Keep upstream AST types private. See [SQL_PARSER.md](SQL_PARSER.md) for the
  implemented scope and rejection boundaries.

- `jiff`: checked civil calendar validation/conversion, bounded ISO date input and
  Gregorian date functions/checked interval arithmetic,
  replacing `time`. Enable std/perf-inline without system-zone or timezone-database
  discovery. TableCraft retains exact Automation DATE bits and extended 100 ns units.
- `zerocopy`: borrowed, explicitly little-endian index headers and long-value descriptors
  inside ACE. Layout validity does not replace format/owner/count validation.
- `wide` and `bytemuck`: private raw-byte ASCII classifiers for native compressed
  text and UTF-16LE, avoiding an intermediate UTF-16 allocation on the qualified
  ASCII path. No validated Value/Decimal/date type implements POD. Bitmap
  benchmarks selected the faster word iterator; SIMD remains a measured contender
  in engineering code. See [native benchmarks](testing/NATIVE_BENCHMARKS.md).

- `uuid`: native GUID byte-order conversion, canonical parsing and formatting;
  defaults disabled, with no random generation or semantic-ID substitution.

The [numeric Text conversion boundary](NUMBER_FORMAT.md) reuses exact lexical
rounding before constructing bounded carriers; rust_decimal remains the selected
numeric/decimal arithmetic kernel. This preserves guard digits beyond scale 28
without introducing an additional general decimal arithmetic library.

## Use when the corresponding feature is implemented

| Library and reference | Intended use and TableCraft boundary |
| --- | --- |
| [rayon](https://docs.rs/rayon/latest/rayon/) | CPU-parallel query/validation work over suitable inputs. Keep backend transaction/thread-affinity rules and operation bounds explicit. |
| [tokio](https://docs.rs/tokio/latest/tokio/) | Tokio sync is adopted for optional SQLite worker admission; broader orchestration/I/O/networking remains future scope. Keep blocking work off executor threads. |
| [fst](https://docs.rs/fst/latest/fst/) | Compact derived search dictionaries or prefix indexes. Native Access indexes and their collation remain separate. |
| [reqwest](https://docs.rs/reqwest/latest/reqwest/) | HTTP clients in features that require HTTP. Specify timeouts, response-size limits and target-appropriate TLS/features. |
| [tokio-rusqlite](https://docs.rs/tokio-rusqlite/latest/tokio_rusqlite/) | Adopted behind tablecraft-sqlite/async for worker-owned read snapshots with bounded admission and cancellation. Writes and browser storage require separate work. |
| [unicode-normalization](https://docs.rs/unicode-normalization/latest/unicode_normalization/) | Unicode normalization for explicitly derived search/display helpers. Preserve original text; normalization alone does not implement Access collation. |
| [moka](https://docs.rs/moka/latest/moka/) | Derived-data caching with capacity policy and source-generation invalidation. Its best-effort eviction is not a strict memory quota or native cache-coherence protocol. |
| [sys-locale](https://docs.rs/sys-locale/0.3.2/sys_locale/) | Preferred host/application language discovery for localization. Own the integration in `tablecraft-platform` when a localization consumer exists; respect explicit user language choices. Discovery does not implement formatting, translation or Access/database collation. |
| [fluent-rs](https://github.com/projectfluent/fluent-rs) | Fluent localization, selecting the needed crates (for example fluent-bundle) when localized messages/UI exist. Keep message IDs separate from stable machine error codes. |
| [pollster](https://docs.rs/pollster/latest/pollster/) | Small synchronous bootstrap boundaries that must await suitable async initialization. It does not replace a required Tokio I/O runtime or belong inside an async/UI hot path. |

## Decimal library assessment

`rust_decimal` is adopted privately by CCur/CDec conversion and checked Decimal arithmetic kernels, with only its
std feature enabled. It supplies retained-IEEE conversion, checked multiplication
and explicit midpoint-to-even rounding; it does not replace the stored carrier.
The core carrier enforces the full 96-bit scalar magnitude, retains scale and
rejects inexact rescaling. Shared schema/native codecs enforce Access field bounds. Any dependency
must sit behind this domain contract; its native serializer is not an Access codec.

The custom carrier is a validated 96-bit coefficient/scale representation, not a general
decimal arithmetic engine. Bounded Decimal query arithmetic now uses checked library operations; broader
promotion/native qualification remains pending. Custom
Jet/ACE field and index codecs remain necessary regardless of arithmetic library.
There is no field-range obstacle to using `rust_decimal`: its 96-bit magnitude
covers every coefficient in the current 28-digit Access field envelope, and it
preserves trailing zeros. Keep representational equality, exact rescaling and
column precision/scale validation explicit in TableCraft's wrapper. Arithmetic
adoption must also verify intermediate range, promotion, rounding and overflow;
fitting stored operands alone does not establish compatible calculation results.

| Candidate | Documented model | Assessment for TableCraft |
| --- | --- | --- |
| [rust_decimal](https://docs.rs/rust_decimal/latest/rust_decimal/) | 96-bit magnitude and scale up to 28; checked arithmetic and rounding operations. | Closest starting point for bounded Access Decimal arithmetic, but Access field precision is narrower. Preserve explicit scale/equality and rounding rules in the wrapper. |
| [bigdecimal](https://docs.rs/bigdecimal/latest/bigdecimal/) | BigInt coefficient and signed 64-bit decimal scale; configurable arithmetic context. | Useful for larger exact intermediates or an independent test oracle. Storage still needs Access bounds; input precision, scale and operation cost need explicit limits. Arbitrary precision does not make nonterminating division exact. |
| [decimal-rs](https://docs.rs/decimal-rs/latest/decimal_rs/) | Maximum precision 38, checked arithmetic and precision/scale rounding operations. | A plausible candidate for broader SQL DECIMAL needs. Still requires Access profile limits and tests for retained scale, rounding and failure behavior. |

The adopted boundary now covers 96-bit scalar range and bounded arithmetic.
Broader promotions and optimizations still require explicit error/rounding
contracts and native evidence. Bigdecimal has a clearer role for larger intermediates/test oracles;
SQLParser's optional bigdecimal integration remains disabled here.
`decimal-rs` is attractive when a 38-digit SQL decimal requirement becomes concrete.
The alternative-library assessments are API reviews. The adopted CCur path has
[measured conversion costs](testing/CURRENCY_CONVERSIONS.md), but no native
Access conformance result. No candidate's rounding, byte encoding or equality
semantics is adopted implicitly.

## Additional decisions from the review

- Prototype `binrw` for a real fixed-layout codec and `winnow` for irregular streams
  before adoption. Preserve budgets, cancellation and unknown bytes. `deku` is an
  alternative, not a second default; evaluate `bilge` only for useful dense bitfields.
- Prefer `bitflags`, `bytes`, `encoding_rs`, `indexmap` and `tempfile` when a real
  named-flags, shared-buffer, supported-code-page, ordered-property or temporary-file
  consumer appears. `encoding_rs` is now adopted for [explicit scalar character-code pages](CHARACTER_CODE_PAGES.md), with defaults disabled and non-replacing stack decoding. Existing bounded parsers/arrays need no mechanical rewrite.
- Use Jiff civil calendars now; qualify future zoned services separately. Evaluate
  `icu_collator` using actual Jet/ACE evidence before accepting native sort keys.
- Study UCanAccess, DataFusion, SQLite, redb, Fjall and LMDB at their relevant
  boundaries. Native pages, locks, transactions and recovery remain Jet/ACE-specific.
- Prefer `rusqlite` as the synchronous SQLite base and `tokio-rusqlite` for actual
  async consumers, with compatible versions and explicit connection ownership.
- Introduce AccessKit with custom UI controls; prototype Parley/krilla for shared
  report measurements. Prefer existing CSV/XML/spreadsheet libraries for actual
  interchange features rather than implementing generic formats ourselves.
- Prioritize runnable fuzzing, license/advisory checks and later concurrent/write
  failure tests. `shuttle`/`loom` need instrumented concurrent algorithms; `fail`
  and `failpoints` are distinct alternatives. A tool being evaluated is not a
  passing test or an installed CI gate.

The full review also covers every collection, synchronization, security, P2P,
plugin and binding candidate; the rules above are the concise selection direction.

## Host locale discovery

`sys-locale` is a useful candidate alongside Fluent when localized UI/messages
exist. Its get_locale/get_locales APIs discover preferred BCP 47 language tags;
they do not supply translated messages, number/date formatting or SQL comparison
rules. A host can use these preferences after an explicit user setting and select
from its supported translations with a documented fallback. A missing detected
locale is not itself an error in database evaluation.

This belongs with future platform/host language selection, outside generic I/O,
core value carriers, SQL parsing and native format interpretation. The current CLI
keeps invariant output/ISO date input and explicit query collation. Host detection
must not silently change those semantics or replace database locale metadata.
No current consumer needs discovery, so the crate is recorded rather than added
as an unused dependency. Review targets, features and native boundaries on adoption;
browser discovery requires the upstream js feature.

References: upstream [preferred locale API](https://docs.rs/sys-locale/0.3.2/sys_locale/fn.get_locale.html)
and [platform/feature documentation](https://docs.rs/crate/sys-locale/0.3.2/source/README.md).

## Native-project scripting selection

The [accepted project decision](decisions/0001_PROJECT_FORMAT_AND_EXECUTION.md)
selects Python initially, superseding JS/TypeScript-default advice. Prefer CPython
through PyO3 and uv-managed locked project environments when working application
consumers exist; exact runtime/ABI/platform/security review remains required.
Keep the interpreter outside engine/query and ordinary CLI dependency closures.
Pyodide is a reduced browser candidate; RustPython is an alternative, not another
selected runtime. Boa/rquickjs/Oxc/mlua are not needed for native Python scripting.
Optional pandas/openpyxl/SQLAlchemy packages belong to explicitly managed user
projects. The [candidate dispositions](DEPENDENCY_REVIEW.md#native-project-and-scripting-proposal-dispositions)
cover all new runtime/tool references; none is adopted or installed now.

## Agent interface and project verification tools

Prefer the official Rust MCP SDK `rmcp` for a working future `tablecraft-mcp`
adapter, not a custom protocol implementation. Select exact version/features and
qualify negotiation/extensions with that consumer. Keep ordinary CLI/engine/query
runtime closures unchanged; no SDK dependency is added by documenting it.
Ruff, a qualified type checker and pytest belong to explicitly managed project
workflows. Checking is separate from authorized execution; pytest discovery/plugins
can run code. [MCP direction](interfaces/MCP.md) and
[application scenarios](testing/APPLICATION_SCENARIOS.md) own requirements.
