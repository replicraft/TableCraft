# ICU4X ordering candidate probe

ICU4X 2.3.1 remains a candidate, **not a runtime dependency or Access comparison
provider**. The original [Rust probe](probe_icu_collation.rs) and
[isolated runner](probe_icu_collation.py) make that decision reviewable without
changing the workspace manifest or lockfile. They do not qualify Windows/NLS,
Access SQL, native index keys or a new locale provider.

## Capability and semantic evidence

The runner selects icu_collator 2.3.1 with compiled_data and no default features,
and reuses the workspace's locale-core/CLI choices. The native comparison reference
is TableCraft's existing provisional full-string General model, not an independent
Microsoft SQL oracle. ICU uses en-US secondary strength with other options left
at their defaults. These settings are an experiment, not a proposed equivalence.

Six explicit capability checks pass: case-insensitive comparison, retained accent
distinctions, canonical equivalence, Turkish dotted/dotless I distinctions and
UTF-16 surrogate replacement. All 41 sample spellings are original synthetic
inputs, including punctuation, trailing spaces, ignored units and supplementary
characters. Each family receives the same 1,681 ordered sample pairs.

| General family | Disagreements with raw ICU | Disagreements after trimming ASCII trailing spaces before ICU |
| --- | ---: | ---: |
| Legacy | 136 | 118 |
| 2010 | 124 | 106 |

For example, both General models put `ab` before `a-b`; the tested ICU
configuration reverses them. ICU treats U+0080 as equivalent to empty text while
both General models distinguish it. These are reproducible disagreements between
models; they do **not** establish which model reproduces Access SQL in every case.
Trimming is a diagnostic only, not a proposed text-mode rule.

Upstream [CollatorOptions](https://docs.rs/icu_collator/2.3.1/icu_collator/options/struct.CollatorOptions.html)
documents no prefix/substring matching API and no search-collation data by default.
[compare_utf16](https://docs.rs/icu_collator/2.3.1/icu_collator/struct.CollatorBorrowed.html#method.compare_utf16)
replaces unpaired surrogates with U+FFFD; the probe verifies this. Neither behavior
supplies TableCraft's existing raw-unit LIKE or consumed-span search contracts.
The [ICU string-search model](https://unicode-org.github.io/icu/userguide/collation/string-search.html)
also documents boundary/minimal-match rules beyond arbitrary substring equality.
An ordering API alone must not silently become a search implementation.

## Cost observations, 21 October 2026

Darwin arm64, Rust 1.99.0, release build, Hyperfine 2.0.0; five measured runs after
two warmups. Each invocation performs 600,000 comparisons over six fixed pairs.
These are **nanoseconds per comparison**, derived from whole-invocation wall time:

| Implementation | Invocation mean ± sample standard deviation | Approximate ns/comparison |
| --- | ---: | ---: |
| ICU, direct comparer | 76.972 ± 1.838 ms | 128.3 |
| General legacy, public bounded kernel | 748.269 ± 25.100 ms | 1,247.1 |
| General 2010, public bounded kernel | 754.215 ± 18.840 ms | 1,257.0 |

The checksums differ: -200,000 / 0 / 200,000 respectively. General includes its
allocation and cumulative work/cancellation checks; the raw ICU call has no
TableCraft budget wrapper. **These are different behaviors and different amounts
of work, so no replacement speedup is established.** This is not database I/O,
query throughput, provider construction or a production performance qualification.
The standalone combined executable is 4,279,536 bytes; it contains both contenders,
clap and native engine dependencies. That is not ICU's incremental binary cost.

Final Rust source SHA-256:
`3ace80cd7e461c8f080327334cea96545b0f8fe4d10ff7c5303559eea78b541c`.
Measured binary SHA-256:
`83b1c79c15a7c1a8dd52ec1711649c142acca1e653f5dfca7f1d198395598a47`.
The final retained run contains the source snapshot, external manifest/lockfile,
Cargo metadata, context JSON, report, executable, raw Hyperfine JSON/log and lint
log. Initial shorter probes/build failures are separate artifacts and are excluded
from this table. The isolated lock also resolved newer crc32fast/libc patches;
its complete metadata, rather than the production lockfile, identifies this build.

## Reproduction and disposition

Choose a fresh external directory; the runner refuses to overwrite it or write
candidate build output inside the checkout. `--offline` is useful after the
candidate packages are cached. Optional `--target-dir` shares an external Cargo
cache while retaining an immutable executable in each result directory.

```sh
python3 docs/testing/probe_icu_collation.py --output-dir "$FRESH_PROBE_DIR"
"$FRESH_PROBE_DIR/tablecraft-icu-probe" --help
hyperfine --runs 5 --warmup 2 --shell=none --export-json "$FRESH_PROBE_DIR/hyperfine.json" \
  "$FRESH_PROBE_DIR/tablecraft-icu-probe --provider icu --rounds 100000" \
  "$FRESH_PROBE_DIR/tablecraft-icu-probe --provider general-legacy --rounds 100000" \
  "$FRESH_PROBE_DIR/tablecraft-icu-probe --provider general-2010 --rounds 100000"
```

The experimental CLI uses clap::Parser and bounds its round count. Probe source,
argument/error checks, release execution and isolated warnings-denied Clippy pass.
No external fixture/source/binary was added to Git. ICU packages declare
Unicode-3.0 and Rust 1.88; the selected 1.99 toolchain built the complete isolated
closure. This is not a full adoption/security/target audit: production adoption
still requires an actual semantics-preserving consumer and the normal dependency
review. Native Windows comparisons remain deferred rather than blocking other work.

Keep Text/system locale, explicit LCID search and stored database/field policies
separate. Continue the backend roadmap without substituting ICU data for existing
General keys or silently assigning ICU behavior to an Access mode. Concrete locale
providers, broader native sort orders and native expression precedence remain
unfinished. ICU may still help an explicitly defined future consumer; this probe
rejects treating it as an automatic implementation of current Access contracts.
