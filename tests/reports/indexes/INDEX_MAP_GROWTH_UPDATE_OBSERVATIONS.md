# Index-map growth original-file observations

The first candidate replay passed both fixed original requests: V2016 and V2019
ID 1 KeyValue zero to Int32 maximum. Two native preview commands and two fresh
read-only Jackcess inspections succeeded, with zero unmet controls and unchanged
inputs. Both complete candidate images match the predeclared independent byte
model. The [compact receipt](results/ace16-index-map-growth-originals-2026-10-27.json)
pins the first terminal, raw report, source/runner archives and output files.

The [frozen consumer](results/ace16-map-growth-example-build-2026-10-27.json) was
compiled at `ce230b92adb192199ff273300b52b9acc29f3896`; its receipt pins 241
historical Git source objects. Later worktree state is not treated as compiled
source. This is bounded engineering preview evidence, separate from normal
backend editing or native Access save compatibility.

Each unchanged original contains 900 GrowthKeys records and 700 Padding records.
The two logical KeyValue indexes, Ascending and Twin, share one physical root and
one 69-byte inline owned-map row. The primary-ID index is a separate physical
owner and stays byte-identical. Changing ID 1 overflows the KeyValue root leaf,
and its existing inline map cannot represent the necessary child span.

The independent [model](usage_map_growth/candidate_model.py) changes exactly the
selected four-byte field and appends three pages: RIGHT child at source EOF F,
a usage bitmap at F+1, then LEFT child at F+2. The old root identity is retained
at height one, with a full left maximum key/ROW separator and right tail. Both
450-entry leaves preserve every original ROW reference and exact intended key.
Engineering prefix lengths are 7 and 1, derived from full key/ROW bytes; fresh
unused bytes are zero and original root residual bytes are preserved.

The owned-map record changes from inline to indirect in the same 69-byte range.
Its first pointer identifies F+1, all remaining pointers are zero, and the bitmap
contains exactly the old root plus the two child pages. The bitmap is globally
allocated metadata, not an index member. Exactly three global-free bits clear;
all other original bytes, counters, ordinary map rows, index declarations and
unselected user record bodies remain exact. Each output grows by 12,288 bytes.

The source-only model preparation checks all 3,200 user records across the two
expected images and four complete physical trees, including unchanged primary
indexes. The first actual replay checks complete image equality before opening
any fresh getter. The read-only inspection then verifies typed values, complete
index key/ROW streams and order, map memberships and exact transition geometry.
Candidate hashes remain unchanged by inspection.

The earlier [reference observations](USAGE_MAP_GROWTH_OBSERVATIONS.md) retain
18 fixture commands: source construction/inspection, the failed V2016 helper
invocation and its separately checked partial mutation, and the corrected V2019
reference phase. That history is not relabeled as 18 successful actions. The two
old frozen-service MAX refusals and two separate byte-identical no-op admissions
also remain distinct from these four successful candidate/getter calls. No
source generation or public UPDATE was repeated for this acceptance.

The public model is byte-identical to the executed private proof. The maintained
[saved verifier](usage_map_growth/verify_candidates.py) is a later tool, with its
private source retained; only its repository-root location differs. It rechecks
live hashes and historical Git objects, reconstructs complete expected images
and re-parses cached getters. It issues no native/JVM fixture calls. Expanded
plans, transcripts and raw arrays remain ignored with hashes in the receipt.

```sh
python3 docs/testing/usage_map_growth/verify_candidates.py \
  --report .cache/ace16-map-growth-candidates-first-20261027/report.json \
  --report-sha256 901688a5052f7a9d0d97b1be935c68ba91e4aaffec32fa8ac05cc88afc7bdf2f \
  --plan .cache/ace16-map-growth-acceptance-next-20261027/plan.json \
  --proof docs/testing/usage_map_growth/candidate_model.py \
  --require-success
```

Saved verification passed both complete images/getters, 166 recursive live pin
occurrences and 241 historical Git objects, with zero unmet controls and zero
fixture calls. These originals qualify this Int32 first-span promotion case.
Separate Int32 literal tests cover multiple owners, late promotion and represented
peers. Existing split tests for other carriers do not establish their promotion
combinations. No map-row resizing, global bootstrap, second bitmap span, in-file
reuse or reclamation is established here. Unrelated
foreign payload health, native Access, data macros, ordinary saves, locking and
recovery remain outside this result.
