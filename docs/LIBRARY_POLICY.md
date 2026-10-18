# Library choices

The user requested these libraries for matching future functionality. This is a
dependency selection policy, not evidence that those subsystems exist. Adopt each
library with actual working code, relevant features, license/target review and
tests. Record exact adopted versions in DEPENDENCIES.md and Cargo.lock. Existing
headless and native data-safety boundaries still apply.

## Adopted now

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

## Use when the corresponding feature is implemented

| Library and reference | Intended use and TableCraft boundary |
| --- | --- |
| [rayon](https://docs.rs/rayon/latest/rayon/) | CPU-parallel query/validation work over suitable inputs. Keep backend transaction/thread-affinity rules and operation bounds explicit. |
| [tokio](https://docs.rs/tokio/latest/tokio/) | Async orchestration, channels, I/O and networking. Keep CPU-heavy and blocking database operations off executor threads. |
| [wide](https://docs.rs/wide/latest/wide/) | SIMD acceleration for measured numeric/filtering hot paths; retain equivalent scalar behavior and target tests. |
| [bytemuck](https://docs.rs/bytemuck/latest/bytemuck/) | Checked byte/POD conversions where layout and valid bit patterns are established, such as renderer buffers. Validated semantic values are not automatically POD. |
| [zerocopy](https://docs.rs/zerocopy/latest/zerocopy/) | Checked views of native-format byte structures, with explicit endianness, size, alignment and format validation. No assumption that a Rust struct is an MDB/ACCDB encoding. |
| [fst](https://docs.rs/fst/latest/fst/) | Compact derived search dictionaries or prefix indexes. Native Access indexes and their collation remain separate. |
| [reqwest](https://docs.rs/reqwest/latest/reqwest/) | HTTP clients in features that require HTTP. Specify timeouts, response-size limits and target-appropriate TLS/features. |
| [tokio-rusqlite](https://docs.rs/tokio-rusqlite/latest/tokio_rusqlite/) | Native async access to worker-owned SQLite connections. Bound requests and make transaction ownership explicit; qualify browser storage independently. |
| [unicode-normalization](https://docs.rs/unicode-normalization/latest/unicode_normalization/) | Unicode normalization for explicitly derived search/display helpers. Preserve original text; normalization alone does not implement Access collation. |
| [moka](https://docs.rs/moka/latest/moka/) | Derived-data caching with capacity policy and source-generation invalidation. Its best-effort eviction is not a strict memory quota or native cache-coherence protocol. |
| [fluent-rs](https://github.com/projectfluent/fluent-rs) | Fluent localization, selecting the needed crates (for example fluent-bundle) when localized messages/UI exist. Keep message IDs separate from stable machine error codes. |
| [pollster](https://docs.rs/pollster/latest/pollster/) | Small synchronous bootstrap boundaries that must await suitable async initialization. It does not replace a required Tokio I/O runtime or belong inside an async/UI hot path. |

## Decimal library assessment

No replacement decimal dependency is adopted yet. The current carrier enforces
Access field bounds, retains scale and rejects inexact rescaling. Any dependency
must sit behind this domain contract; its native serializer is not an Access codec.

| Candidate | Documented model | Assessment for TableCraft |
| --- | --- | --- |
| [rust_decimal](https://docs.rs/rust_decimal/latest/rust_decimal/) | 96-bit magnitude and scale up to 28; checked arithmetic and rounding operations. | Closest starting point for bounded Access/VBA numeric work, but Access field precision is narrower. Preserve explicit scale/equality and rounding rules in the wrapper. |
| [bigdecimal](https://docs.rs/bigdecimal/latest/bigdecimal/) | BigInt coefficient and signed 64-bit decimal scale; configurable arithmetic context. | Useful for larger exact intermediates or an independent test oracle. Storage still needs Access bounds; input precision, scale and operation cost need explicit limits. Arbitrary precision does not make nonterminating division exact. |
| [decimal-rs](https://docs.rs/decimal-rs/latest/decimal_rs/) | Maximum precision 38, checked arithmetic and precision/scale rounding operations. | A plausible candidate for broader SQL DECIMAL needs. Still requires Access profile limits and tests for retained scale, rounding and failure behavior. |

For the current bounded storage carrier, rust_decimal remains the first candidate
to evaluate. Bigdecimal has a clearer role for larger intermediates/test oracles;
decimal-rs is attractive when a 38-digit SQL decimal requirement becomes concrete.
These are design assessments from upstream APIs, not measured performance or
Access conformance results. No candidate's rounding, byte encoding or equality
semantics is adopted implicitly.
