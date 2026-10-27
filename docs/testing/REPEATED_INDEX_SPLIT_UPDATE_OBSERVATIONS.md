# Repeated fixed-index split original-file observations

The first candidate replay passed all four exact original requests: Int64 and
Currency in V2016 and V2019, each changing ID 1 from zero to the maximum signed
64-bit value (Currency uses exact scaled units). Four native preview commands and
four fresh read-only Jackcess getters succeeded, with zero unmet controls and
unchanged inputs. The [compact receipt](results/ace16-repeated-index-split-originals-2026-10-27.json)
pins the raw report, frozen artifact, original request/model sources and outputs.

This qualifies the engineering preview on these originals. The artifact was
compiled at `ddaffcf453fc3887a0a383d40f4e0efa1dd39917`, before subsequent work;
[its build receipt](results/ace16-repeated-split-example-build-2026-10-27.json)
pins 229 historical Git objects. The replay did not regenerate sources or repeat
public updates. Native Access, normal saves, macros, locking and recovery remain
unqualified.

Each source has two physical KeyValue indexes, ascending and descending, with a
real ascending logical alias. Each holds 900 zero keys in one leaf. The independent
[whole-image model](repeated_split_originals/candidate_model.py) changes exactly
the selected eight-byte field, retains every original row reference and builds
all full key/ROW bytes from the original stream using integer arithmetic. It
replays stable allocation-order split passes without importing production encoders.
The four [selectors](repeated_split_originals/candidate_selectors.json) are unchanged.

Each candidate adds six EOF pages. Ascending leaf order relative to original EOF
is `[1,2,0]`, with populations `[450,225,225]`; descending is `[5,4,3]`, with
`[225,225,450]`. Fresh 450-entry leaves require the full key/ROW common prefix of
11. Fresh 225-entry leaves fit without compression and retain prefix zero. Roots
retain their original identities, become height one and contain two complete
maximum-key/ROW separators plus the right tail. Every sibling link, represented
index-owned bit and global-free bit matches the independent expected image.

All four outputs match the complete expected bytes. Original root residual bytes,
historical counters, unrelated indexes, sentinel values and all unselected record
bytes are preserved. Every fresh unused byte is zero. The static model selfcheck
examines 7,204 original row bodies and 24 complete physical trees across the four
images, including eight changed trees. Fresh getters independently confirm typed
values, complete index order/membership, row references, counts and map roles.
The sources contain no unchanged NULL KeyValue peer discriminator; those remain
separate literal contract coverage.

The earlier [public reference actions](REPEATED_INDEX_SPLIT_OBSERVATIONS.md) use
key-only prefix 9 on the large new leaves and leave nonzero scratch in fresh
unused bytes. Those outputs establish separate public action/topology evidence;
they are not the candidate byte golden. The old four service refusals and source
no-op admissions remain distinct historical records.

Before any candidate calls, the pure selfcheck's final authored tree tally was
corrected from 28 to 24: the preserved sentinel has no index. The original script
and failed assertion are archived. No expected image, source, selector or request
changed. The subsequent first actual replay passed without correction or retry.

The maintained [saved verifier](repeated_split_originals/verify_candidates.py)
reconstructs all four complete images, rechecks cached full getters and live pins,
and verifies compiled source pins through Git. It issues no native/JVM fixture
commands. Expanded plans, raw transcripts and executed private sources stay
ignored with immutable hashes; the public model is byte-identical to the executed
proof, and the public verifier differs from the saved private verifier only in
its repository-root location.

```sh
python3 docs/testing/repeated_split_originals/verify_candidates.py \
  --report .cache/ace16-repeated-split-acceptance-first-20261027/report.json \
  --plan .cache/ace16-repeated-split-acceptance-next-20261027/plan.json \
  --proof docs/testing/repeated_split_originals/candidate_model.py
```

The saved verifier passed: four images/getters, 341 recursive live pin occurrences,
229 historical Git objects, zero unmet controls and zero fixture calls.
