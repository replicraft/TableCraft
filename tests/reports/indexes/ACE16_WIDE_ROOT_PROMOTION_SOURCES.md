# Wider numeric root-promotion sources

Six independently constructed files now pass eight native table contexts and six
complete Jackcess reader reports. They cover Int64 and Currency tables under both
frozen modern header variants. The source phase preceded matching writes; the
[normal writer](../../../docs/access/writes/NORMAL_WIDE_ROOT_PROMOTION.md) now passes its
separate mutation acceptance. The source receipt retains its historical phase
status.

Each changed image adds one complete row to an original 400-row table. The Int32
primary still fits its original leaf. The ascending and descending wide indexes
each retain their root as a NODE and acquire two leaves, with the logical alias
sharing its actual physical owner. Four INDEX pages are appended in total. DATA
placement uses existing owned capacity. Original rows, inactive root tails,
unrelated objects and all bytes outside the declared changes remain protected.

The original request and allocation laws preceded the independent Python image
model. A separate Rust test replays all four changed images from original bytes,
literal keys and allocation claims, then checks complete Source capture, rows,
all four aliases, constraints, historical counters and owned-page membership.
Neither replay uses a TableCraft mutation planner or production encoder.

The first SDK comparison stopped after two passing originals. Its third child
returned successfully with matching rows and index order, but exposed a report
prediction error: the three affected INDEX lines still reported one owned page
instead of the original root plus two new leaves. A report-only successor derives
three from the already-frozen allocation law. Images, requests, models, original
predictions and the failed receipt remain unchanged. The third raw observation
was reused; only the three remaining reader commands were issued. A strict saved
verifier validates the combined six reports and complete input closure.

The first native build also stopped before running a test when its default build
directory disappeared. A separate build cache completed the unchanged source
check. Both attempts remain recorded.

Run the ignored source check with an absolute fixture directory:

```sh
TABLECRAFT_WIDE_ROOT_PROMOTION_SOURCE_DIR=/absolute/path/to/expected-first \
  cargo test -p tablecraft-ace --all-features --lib \
  wider_root_promotions_source_eight_contexts_replay_complete_images_before_mutation \
  -- --ignored
```

See the [execution record](../../results/indexes/ace16-wide-root-promotion-sources-2026-11-02.json)
and [original source observations](INTEGRAL_INDEX_SPLIT_SOURCE_OBSERVATIONS.md).
Independent reader agreement does not establish native Microsoft Access save or
reopen compatibility, or grant mutation permission.
