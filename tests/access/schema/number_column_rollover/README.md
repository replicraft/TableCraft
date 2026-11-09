# Number property-page rollover: independent SDK evidence

The unchanged first-party [Number helper](../fields/CreatedNumberColumnOracle.java)
added `N2` through `N14` to fresh copies of the pinned `Table1(ID, Score)` AFTER
from the [Number declaration record](../../../results/schema/add-column-number-sdk-source-2026-11-08.json).
The reviewed Apache-2.0 Jackcess 5.0.4 JAR remains unchanged. Expression evaluation
was disabled; every result was reopened read-only. Every original stayed exact.

All twelve complete SDK AFTERs through `N13` match an independent same-page raw
model. This extends the observed history beyond the earlier second-addition pins;
it does not change those earlier observations. Before `N14`, page 10 contains
13 live historical bodies and 71 free bytes. The new complete MR2 body needs
560 bytes plus a slot, so it cannot fit on that page.

`N14` grows the image from 45,056 to 49,152 bytes. The SDK preserves page 10
completely, allocates immediate EOF page 11, and stores the new single MR2 body
as row 0 there. The new page has 3,520 free bytes. Only existing pages 3, 4 and 7
change. The catalog forwarding root, physical slot 8 and its 3,359–3,444 span
stay exact; its descriptor changes from 523 bytes at page 10/row 12 to 560 bytes
at page 11/row 0. The selected owned map retains page 10 and adds page 11; its
candidate map drops page 10 and includes page 11. The global-free map stays exact.
The [record](../../../results/schema/number-column-rollover-sdk-2026-11-08.json) pins the complete
images, actual map bytes, helpers and all intermediate comparisons.

The [independent model](model.py) constructs the entire expected image from
BEFORE bytes, the requested name, prior first-party raw declaration/property
owners, and literal observed page/map framing. Its complete `N14` AFTER matches
the SDK. It does not import a production codec or SDK serializer. The portable
model replay also matches all thirteen retained SDK outputs. An initial raw-model
assertion selected catalog-owned map `3:0` instead of global-free map `3:1`;
that failed draft, its hash and correction remain separately recorded. No SDK
operation was repeated for the correction or portable replay.

This is SDK/raw-model evidence, separate from native engine, journal, runtime and
GUI verification. It does not qualify page rollover in a native writer, another
allocation-map shape, another source class, Microsoft Access saves, events or
compiled caches. Table dates remain zero native days, rendered by the SDK as
1899-12-30; this does not establish Access timestamp behavior. Rows remain zero,
there is one primary index, and `ID` remains generated. All Number fields retain
explicit Required false and DefaultValue zero. No VBA or active content ran.

Reproduce from the repository root using a fresh output directory and the exact
pinned `Score` source with SHA-256
`a6a9e18cd35dc2bf018e5cd554a68b5b60d56023fd78a4a0ecba627ab9041d72`:

```sh
python3 tests/run.py tests/access/schema/number_column_rollover/probe.py \
  --jar /path/to/reviewed-jackcess-5.0.4.jar \
  --source /path/to/id-only-number-sdk-after.accdb \
  --output-dir /path/to/fresh-sdk-output
python3 tests/run.py tests/access/schema/number_column_rollover/model.py \
  --sdk-dir /path/to/fresh-sdk-output \
  --output-dir /path/to/fresh-model-output
```

The maintained portable SDK driver is syntax-checked; the original observed
execution used its retained first-party predecessor with the same unchanged Java
helper and JAR. Raw same-page and corrected rollover function bodies are AST
identical after relocating their existing raw-owner import. The portable model
was executed against the retained SDK outputs. Generated databases, classes, JARs
and local output directories remain untracked.
