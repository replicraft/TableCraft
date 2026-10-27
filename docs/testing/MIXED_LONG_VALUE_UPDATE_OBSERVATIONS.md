# Mixed indirect-map Memo replacement acceptance

The first committed-consumer replay passes twelve ordinary Memo replacements on
the two unchanged [mixed-map originals](INDIRECT_LONG_VALUE_MAP_OBSERVATIONS.md).
Each candidate matches its independently frozen complete image and fresh
read-only getters/maps. All twenty-five fixture commands succeed without retries,
regeneration or public UPDATE calls. This remains engineering preview evidence,
not native Access save/reopen or normal-save qualification.

The [acceptance receipt](results/ace16-mixed-long-value-originals-2026-10-27.json)
separates the source observations, implementation checks, immutable build and
actual first candidate replay. The
[build receipt](results/ace16-mixed-long-value-example-build-2026-10-27.json)
binds revision `c609d94e1f1384e2d8a431922f7bba5d34ebc35b`, 219 selected Git objects,
the complete 2,561-file export inventory, driver/log and frozen executable.
Concurrent working-tree changes are not compiled-source authority.

## Exact inputs and outcomes

Each V2016/V2019 source has 702 user rows. Selected ID1/original ordinal zero has
8,192 U+0100 Memo units. Its owned map is indirect and its candidate map inline;
the table-owned and global maps also use existing indirect bitmap storage.
The sibling ID2 Memo, both OLE values, 700 padding records and all original
index pages remain protected. Each case starts from the same unchanged original.

| Case, per format | Replacement | Parent placement | EOF growth |
| --- | --- | --- | --- |
| No-op | Original 8,192 units | Unchanged | 0 |
| NULL | Stored NULL | Fitted original record | 0 |
| Inline | 32 units / 64 bytes | Hidden append on existing ordinary page; original root forwards | 0 |
| Fitted single | 33 units / 66 bytes | Fitted original record; existing LVAL candidate append | 0 |
| EOF single | 2,037 units / 4,074 bytes | Fitted original record | 1 page |
| EOF chain | 4,073 units / 8,146 bytes | Fitted original record | 3 pages |

All non-NULL replacements contain U+0100. The 39-byte parent becomes a 103-byte
hidden record only for the inline case. Both logical root references stay fixed.
The one-page and three-page cases prove the old 3,980-byte candidate cannot fit,
clear its hint and allocate through already represented owned/hint/global cells.
Transient set-then-clear hint changes and same-byte EOF edits match the independent
ledger. There is no map resize, pointer creation, window shift or free-page reuse.

Across twelve cases, the independent proof checks 8,412 protected original
record-body/slot comparisons, 144 original LVAL-body/slot comparisons and 312
unchanged index-page comparisons. These are repeated comparisons, not counts of
distinct stored objects. All old chunks remain allocated and byte-exact, even
when no longer live. Bitmap prefixes, pointer rows, unrelated bits and every byte
outside the explicit record/page/map spans remain unchanged. Total file growth
across the eight no-growth and four growing cases is 32,768 bytes.

## Frozen model and actual fresh getters

The twelve-case plan, quoted TSV requests and whole-image proof were frozen
before the new consumer build. The original raw placement model is published
[byte-identically](mixed_lval_originals/raw_placement.py); the
[mixed adapter](mixed_lval_originals/proof.py) is a separately identified path
derivative of the executed adapter. Neither derives expected bytes from production
serializers or successful candidates.

The actual first replay compiles one new
[read-only probe](mixed_lval_originals/MixedLongValueCandidateProbe.java), then
executes twelve native attempts and twelve fresh combined inspections. The probe
has no mutation entrypoint, opens read-only, disables expression evaluation and
refuses linked opens. It reports NULL explicitly, independently of empty bytes.
Its Java source is byte-identical to the executed source. Native allowances stay
256 MiB/100,000 rows/100 million steps; Java/javac heaps are 512 MiB, with 30-second
process, 8 MiB stream and 8 MiB getter-file ceilings.

Preparation review caught a draft adapter delegating to the original-source
getter parser, which assumed unchanged Memo values and could not decode NULL.
That draft ran no fixture commands. Its preserved replacement was checked purely
against all twelve expected images, including NULL and forwarded parents, before
the first actual replay. Accepted source helpers, model, selectors and inputs
were not modified to satisfy the new checks.

## Saved verification

With the recorded ignored artifacts retained, run from the repository root:

```sh
python3 docs/testing/mixed_lval_originals/verify.py \
  --report .cache/ace16-mixed-lval-acceptance-first-20261027/report.json
```

The verifier rehashes saved artifacts, checks compiled Git objects, reconstructs
all twelve complete images and reanalyzes the saved fresh getter/map output. It
does not invoke the engine, JVM, generation or a database UPDATE. The public proof
and parser have explicit path/import adaptations; their historical executed
identities remain pinned separately. The first derived verifier had an unrelated
Python import error before verification; its source/log are retained. The corrected
verifier and separate independent saved-copy verification pass without repeating
any fixture command. The raw first replay report remains unchanged.

Indirect candidate maps, map/pointer growth, reclamation, broader fields and
relationship actions, required data macros, locking/recovery and native Access
qualification remain separate work. This acceptance does not enable ordinary
backend or CLI saves.
