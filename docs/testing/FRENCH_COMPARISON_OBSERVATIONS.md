# French legacy complete-string engineering oracle

This evidence concerns the explicit `1036/0/0` comparison model. It does not
establish native Access SQL, Windows NLS, substring matching or LIKE behavior.
The [stored-index observations](FRENCH_LEGACY_INDEX_OBSERVATIONS.md) remain a
separate physical-key oracle with a 255-unit input prefix and native entry limits.

[FrenchComparisonOracle.java](FrenchComparisonOracle.java) is an original caller
of the reviewed Jackcess 5.0.4 JAR. It selects the exact French encoder, exports
82 measured Latin character handlers, and obtains 158 complete short keys from
that independent implementation. The alphabet contains only measured SIMPLE or
INTERNATIONAL one-character/one-primary-unit handlers, with no modifiers, kana,
unprintable or tertiary bytes. No upstream implementation is copied or modified.

The independent [Latin stream model](french_comparison_latin_model.py) checks
those handler restrictions, constructs complete primary and per-character
secondary streams, reverses the entire secondary stream, removes neutral trailing
secondary weights and compares complete keys. All 158 short outputs agree exactly
with the pinned encoder. Twenty long comparisons at 256/257/1,024/20,000 units
then distinguish suffix primaries, accents exchanged between the first and last
positions, neutral padding, trailing ASCII spaces and case equality.

The long comparisons are an engineering extension of the measured Latin weights.
Jackcess's physical encoder truncates input at 255 UTF-16 units before encoding;
feeding it longer input is not a full-string oracle. The Latin model does not
qualify arbitrary long Unicode, unprintable positional extensions or native SQL
operation-specific rules. Raw-unit and widened-position engine controls are
labelled separately from this independent Latin evidence.

The public Python model is a byte-identical promotion of its executed private
source. The public Java caller names the three descriptor constants after its
initial private capture; a fresh public-source run reproduces both accepted raw
outputs byte-for-byte. The initial private sources remain immutable and pinned.
Accepted raw stdout, long vectors and reports
remain ignored and hash-pinned by the feature verification record. Reproduction
must use fresh output paths; it does not regenerate any database or alter an
accepted fixture.

```sh
mkdir -p .cache/french-comparison-reproduction/classes
javac -encoding UTF-8 -cp .cache/tablecraft-fixtures/jackcess-5.0.4.jar \
  -d .cache/french-comparison-reproduction/classes \
  docs/testing/FrenchComparisonOracle.java
java -cp .cache/french-comparison-reproduction/classes:.cache/tablecraft-fixtures/jackcess-5.0.4.jar \
  com.healthmarketscience.jackcess.impl.FrenchComparisonOracle \
  > .cache/french-comparison-reproduction/oracle.tsv
python3 docs/testing/french_comparison_latin_model.py \
  --oracle .cache/french-comparison-reproduction/oracle.tsv \
  --vectors .cache/french-comparison-reproduction/long-pairs.tsv \
  --report .cache/french-comparison-reproduction/report.json
```

Use the platform Java classpath separator (`;` on Windows). Verify the reviewed
JAR hash from the [fixture ledger](FIXTURES.md) before invoking the commands.
Neither helper opens or modifies a database or executes stored content.

The working consumer contract and intentional resolver accessor source change
are described in [native text comparison](../access/indexes/NATIVE_TEXT.md#provisional-complete-french-legacy-comparison).

## Local implementation verification

The [scoped verification record](results/french-full-comparison-2026-10-26.json)
records 119 distinct selected runtime functions, including eighteen new controls,
and three separate compile-fail isolation checks. Default/preview strict checks,
standalone minimal checks, warnings-denied public rustdoc and formatting pass.
The evidence includes real resolver/provider query controls over explicit mock
field identities; original native-field CLI comparisons remain a separate scope.

The first independent query run retained its wrong mixed-policy error expectation
and exhausted 32 MiB comparison pool. The final expectation is the established
`CollationConflict`; a positive long workload uses an explicit 64 MiB allowance,
and a separate test preserves the typed 32 MiB failure. Defaults did not change.
Default-feature strict checks also exposed an existing infallible UPDATE-only
match; a cfg-only destructuring cleanup leaves preview branches and gates intact.
Initial/diagnostic sources and failed logs remain pinned rather than erased.
