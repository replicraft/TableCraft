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

The later [VBA deferral](../architecture/VBA_COMPATIBILITY.md) supersedes interpreter, editor/debugger,
VBA-specific DAO/ADO, Windows emulation, COM/Wine and immediate RustVBA integration
recommendations. Existing VBA preservation and inert dependency detection remain
required; Access table data macros remain in database scope. Normal OS I/O is
independent of this deferral.

## Adopted now

Cryptographic primitives are integrated through the working
[`tablecraft-crypto`](../platform/CRYPTO.md) package; native policy remains ACE-owned.
ZIP/OPC/XML/CAB codecs belong to
[`tablecraft-formats`](../formats/ACCESS_PACKAGES.md), with native/query adaptation
in Access. These are local ownership changes using the existing selected
libraries and feature choices, not adoption of new algorithm implementations.

- Reviewed Jackcess General weight data and an Apache-2.0 Rust encoder adaptation:
  native General legacy/General 2010 index payloads, with static borrowed zerocopy
  lookups and independent key checks. Retain the bundled licenses/notices. This
  does not select a generic Unicode/OS sort order or implement SQL collation.

- `cab`: formats-owned CAB deployment metadata/decompression after bounded
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
  disabled. Hosts own executors and request-specific cancellation. See [SQLite](../backends/SQLITE.md).

- `sha2`: SHA-256/384/512 for native Agile password/page decoding, plus optional
  SHA-256 integrity behind tablecraft-io/journal. Defaults remain disabled; native
  crypto enables its zeroization feature. Journal hashing does not implement
  encryption or native Access recovery. See [encryption](../access/storage/NATIVE_ENCRYPTION.md)
  and [journal scope](../foundation/IMAGE_JOURNAL.md).

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
  Keep upstream AST types private. See [SQL_PARSER.md](../query/SQL_PARSER.md) for the
  implemented scope and rejection boundaries.

- `jiff`: checked civil calendar validation/conversion, bounded ISO date input and
  Gregorian date functions/checked interval arithmetic,
  and explicit Gregorian CStr date/time output, replacing `time`.
  Libraries enable std/perf-inline without system-zone or timezone-database
  discovery. The CLI [clock host](../expressions/dates/CLOCK_FUNCTIONS.md) additionally enables tz-system,
  tzdb-zoneinfo and tzdb-bundle-platform, using fallible discovery and explicit
  fixed-reading overrides. TableCraft retains exact Automation DATE bits and extended 100 ns units.
- `zerocopy`: borrowed, explicitly little-endian index headers and long-value descriptors
  inside ACE. Layout validity does not replace format/owner/count validation.
- `wide` and `bytemuck`: private raw-byte ASCII classifiers for native compressed
  text and UTF-16LE, avoiding an intermediate UTF-16 allocation on the qualified
  ASCII path. No validated Value/Decimal/date type implements POD. Bitmap
  benchmarks selected the faster word iterator; SIMD remains a measured contender
  in engineering code. See [native benchmarks](../testing/NATIVE_BENCHMARKS.md).

- `uuid`: native GUID byte-order conversion, canonical parsing and formatting;
  defaults disabled, with no random generation or semantic-ID substitution.

The [numeric Text conversion boundary](../expressions/formatting/NUMBER_FORMAT.md) reuses exact lexical
rounding before constructing bounded carriers; rust_decimal remains the selected
numeric/decimal arithmetic kernel. This preserves guard digits beyond scale 28
without introducing an additional general decimal arithmetic library.

## Use when the corresponding feature is implemented

| Library and reference | Intended use and TableCraft boundary |
| --- | --- |
| [rayon](https://docs.rs/rayon/latest/rayon/) | CPU-parallel query/validation work over suitable inputs. Keep backend transaction/thread-affinity rules and operation bounds explicit. |
| [tokio](https://docs.rs/tokio/latest/tokio/) | Tokio sync is adopted for optional SQLite worker admission; broader orchestration/I/O/networking remains future scope. Keep blocking work off executor threads. |
| [fst](https://docs.rs/fst/latest/fst/) | Adopted for the ACE engine's immutable derived General LIKE element dictionary. Broader search dictionaries/prefix indexes remain prospective; native Access index storage remains separate. |
| [reqwest](https://docs.rs/reqwest/latest/reqwest/) | HTTP clients in features that require HTTP. Specify timeouts, response-size limits and target-appropriate TLS/features. |
| [tokio-rusqlite](https://docs.rs/tokio-rusqlite/latest/tokio_rusqlite/) | Adopted behind tablecraft-sqlite/async for worker-owned read snapshots with bounded admission and cancellation. Writes and browser storage require separate work. |
| [unicode-normalization](https://docs.rs/unicode-normalization/latest/unicode_normalization/) | Adopted for qualified character composition/decomposition in explicit Japanese StrConv width conversion. Derived search/display normalization remains prospective. Preserve stored text; normalization does not implement Access collation. |
| [moka](https://docs.rs/moka/latest/moka/) | Derived-data caching with capacity policy and source-generation invalidation. Its best-effort eviction is not a strict memory quota or native cache-coherence protocol. |
| [sys-locale](https://docs.rs/sys-locale/0.3.2/sys_locale/) | Preferred host/application language discovery for localization. Own the integration in `tablecraft-platform` when a localization consumer exists; respect explicit user language choices. Discovery does not implement formatting, translation or Access/database collation. |
| [fluent-rs](https://github.com/projectfluent/fluent-rs) | Fluent localization, selecting the needed crates (for example fluent-bundle) when localized messages/UI exist. Keep message IDs separate from stable machine error codes. |
| [pollster](https://docs.rs/pollster/latest/pollster/) | Small synchronous bootstrap boundaries that must await suitable async initialization. It does not replace a required Tokio I/O runtime or belong inside an async/UI hot path. |

## Decimal library assessment

`rust_decimal` is adopted privately by CCur/CDec conversion, checked Decimal arithmetic
and [named numeric formatting](../expressions/formatting/STYLED_NUMBER_FORMATTING.md), with only its std
feature enabled. It supplies retained-IEEE conversion, checked arithmetic and
explicit midpoint rounding. Existing conversions/arithmetic retain their half-even
contracts; named numeric output has an explicit current half-away profile awaiting
native qualification. It does not replace the stored carrier.
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
[measured conversion costs](../testing/CURRENCY_CONVERSIONS.md), but no native
Access conformance result. No candidate's rounding, byte encoding or equality
semantics is adopted implicitly.

## Additional decisions from the review

- Prototype `binrw` for a real fixed-layout codec and `winnow` for irregular streams
  before adoption. Preserve budgets, cancellation and unknown bytes. `deku` is an
  alternative, not a second default; evaluate `bilge` only for useful dense bitfields.
- Prefer `bitflags`, `bytes`, `encoding_rs`, `indexmap` and `tempfile` when a real
  named-flags, shared-buffer, supported-code-page, ordered-property or temporary-file
  consumer appears. `encoding_rs` is now adopted for [explicit scalar character-code pages](../expressions/text/CHARACTER_CODE_PAGES.md), with defaults disabled and non-replacing stack decoding. Existing bounded parsers/arrays need no mechanical rewrite.
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
Calendar-name profiles and caller-issued FirstDayOfWeek do not change that boundary:
sys-locale supplies identifiers, not localized labels or first-day preferences. A
future host selection flow also needs a locale-data provider and documented fallbacks.
No current consumer needs discovery, so the crate is recorded rather than added
as an unused dependency. Review targets, features and native boundaries on adoption;
browser discovery requires the upstream js feature.

References: upstream [preferred locale API](https://docs.rs/sys-locale/0.3.2/sys_locale/fn.get_locale.html)
and [platform/feature documentation](https://docs.rs/crate/sys-locale/0.3.2/source/README.md).

## Native-project scripting selection

The [accepted project decision](../decisions/0001_PROJECT_FORMAT_AND_EXECUTION.md)
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
can run code. [MCP direction](../interfaces/MCP.md) and
[application scenarios](../testing/APPLICATION_SCENARIOS.md) own requirements.

## Working Unicode case mapping

`icu_casemap` now supplies LCase/UCase with compiled Unicode data;
`icu_locale_core` validates explicit host language identifiers and `writeable`
streams results into our bounded sink. The [casing contract](../expressions/text/TEXT_CASING.md) keeps
this separate from collation, formatting and future sys-locale discovery. Do not
replace native Access compatibility evidence with ICU output, or call allocating
mapping convenience APIs before applying output budgets. Feature/notice/closure
records are in [DEPENDENCIES.md](DEPENDENCIES.md#unicode-case-mapping).

The optional [ICU text profile](../expressions/text/ICU_TEXT_PROFILE.md) now uses icu_collator with
compiled data for explicit host-selected comparison/search. It does not replace
native General keys, database field policies or NLS qualification. Keep exact
LCID registration, input/call ceilings, per-provider pools and ICU scratch
allocation limits explicit. SmallVec in this optional production closure is an
upstream scratch implementation, separate from its index benchmark consumer.

### Access Rnd sequence

[Query Rnd](../expressions/numeric/RANDOM_FUNCTIONS.md) owns an explicit Basic-compatible 24-bit sequence.
OS entropy from getrandom remains appropriate for salts/verifiers and a future
explicit fresh-seed host mode. It cannot replace Rnd's repeat/advance/reseed rules.
No general RNG dependency was added; Randomize/runtime execution remains deferred.

## Working localized query messages

`fluent-bundle` and `fluent-syntax` now serve the optional bounded flat
[Error(number) catalog](../expressions/ERROR_MESSAGES.md). `unic-langid` validates explicit host
language identifiers. The CLI selects English reference data; libraries never
infer a language. Keep this separate from machine error codes, UI localization,
latest VBA runtime state and future sys-locale discovery. Minimal expression
builds retain no Fluent dependency. Feature/license/source records are in
[DEPENDENCIES.md](DEPENDENCIES.md#localized-error-messages).


Access template package inspection now uses `zip` with defaults disabled and
`deflate-flate2`, sharing the existing pure Rust `flate2` backend. `quick-xml`
parses bounded OPC controls. Keep package/relationship work above the ACE page
engine; do not enable archive encryption, extra codecs, extraction or runtime
features without a working need and review. [Template scope](../access/containers/ACCESS_TEMPLATES.md)
keeps schema/row reading distinct from the package inventory.


## Exact binary floating means

The working query mean accumulator uses num-bigint/num-rational with num-traits
for bounded final binary-rational rounding. This is distinct from stored Access
Decimal, which retains the existing core carrier and rust_decimal arithmetic.
Do not expose arbitrary-precision values as stored field carriers or enable
serialization/random/provider features for this math consumer. Keep exponent/
count bounds, scratch budgets and independent rational controls explicit. Exact
versions/features and measured costs are in [DEPENDENCIES.md](DEPENDENCIES.md#bounded-exact-floating-means-22-october-2026)
and [the verification record](../testing/FLOATING_MEANS.md).


The [wider Decimal mean fallback](../query/aggregates/AVERAGES.md) also uses the existing BigInt
and directly inherited num-integer quotient/remainder/parity operations. This
bounded accumulator does not require another Decimal library or change the
stored Decimal carrier. Mixed-type and native rounding qualification remain open.


[Fixed-point statistical offsets](../query/aggregates/STATISTICAL_AGGREGATES.md#numerical-work-and-resource-ownership)
also reuse BigInt only when exact scale alignment exceeds i128. Narrow differences
use checked primitive arithmetic and a stack spelling before binary64 conversion.
This does not introduce another Decimal library or widen stored values.

The [wider Decimal sum](../query/aggregates/DECIMAL_SUMS.md) now shares the existing exact total with
means. Checked primitive/rust_decimal fast paths and existing BigInt/rational
fallbacks remain the chosen libraries; no additional Decimal implementation is
introduced.

## Collection and search decisions

Native index metadata uses ArrayVec under the existing ten-column ceiling.
SmallVec is already used transitively by Fluent, ICU and rusqlite in normal CLI
builds; its direct ACE dependency supports the retained comparison benchmark.
It is not selected as a replacement for native index storage.

The tested memchr replacement for InStrB is not selected: prepaid work changes
early-match budget admission. Keep the production search contract and retain the
benchmark as evidence for that decision. Neither experiment is an outstanding
implementation task. The [dependency review](DEPENDENCY_REVIEW.md#targeted-backend-review-24-october-2026)
records the completed evaluations; [dependency records](DEPENDENCIES.md) describe
actual consumers and features. Selected future Moka/shared-buffer uses still
require working consumers before adoption.
