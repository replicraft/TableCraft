# Non-root Int32 index split observations

The two registered ACE 16 `DeleteTreeFixture` originals produced two atomic
`Unsupported(IndexedColumn)` refusals from the frozen root-leaf split consumer
and two successful public Jackcess updates. These are independent-reader/writer
observations, not native Access qualification. No non-root kernel implementation
or successful kernel candidate is established by this record.

The first verifier exited **1 after all 12 commands**. It incorrectly expected
that a public fixed-field update would preserve the selected record's Memo/OLE
references. Both public updates rewrote those references while preserving the
complete values. The first source, report, command outputs and precall archives
remain unchanged. Separate read-only analyses verified the captured results;
no action was repeated or relabeled as an initial verifier pass.

The [compact receipt](results/ace16-nonroot-index-split-originals-2026-10-27.json)
pins the originals, executed sources, frozen consumer, commands, raw reports and
subsequent analyses. The [frozen action plan](nonroot_index_split_originals/plan.json)
selects `Tree.ID=1000` at live ordinal 27 and replaces it with `Int32MAX`, without
changing the 16,384-row ceiling or any admission rule. Each source has 1,000 rows.
The public helper uses fresh copies, foreign-key enforcement, disabled expression
evaluation, UTC and a link resolver that refuses target opens. It checks the
actual table/column properties, required unique primary metadata, selected
ordinal and unused replacement before its individual `updateRow` call.

## Measured split and preservation

The original primary root is page 91/108 in V2016/V2019, at height 1. Its leaves
have 504 and 496 entries. Updating the last value reduces the affected leaf's
best common prefix from three bytes to one, requiring 3,969 bytes against the
3,616-byte entry capacity. The parent itself has room; this is not evidence for
internal-node overflow or recursive root growth.

| Fact | V2016 | V2019 |
| --- | --- | --- |
| Retained root | 91 | 108 |
| Preceding leaf, 504 entries | 144 | 161 |
| Retained overflowing leaf, now right 248 | 143 | 160 |
| New left leaf, 248 entries | 175 | 193 |
| Original file pages | 166 | 184 |
| Public result pages | 176 | 194 |
| New Memo chain | 166–170 | 184–188 |
| New OLE chain | 171–174 | 189–192 |

The public writer allocates the new **left** leaf and retains the old leaf as
its right neighbor. The preceding leaf's next link is repaired. The root retains
its height and old right tail, adding the full left maximum key plus physical row
reference as a separator; free entry space changes from 3,603 to 3,590 bytes.
All complete primary key/row-reference entries match the independently modeled
replacement. Other physical index pages and complete key/reference streams are
unchanged, including the logical aliases. Historical counters remain unchanged.

The file grows ten pages: nine LVAL pages, then one index page. The selected
49-byte record changes only at its four ID bytes and the two four-byte LVAL
references. The complete old/new Memo chain payload is 19,600 bytes and the OLE
payload is 12,288 bytes; both payloads are byte-identical. The typed getter also
verifies their UTF-8 length/digest and binary length/digest. All original LVAL
pages and every other raw record body remain unchanged. The raw report and
analyses retain complete page headers, leaf partitions, separator entries,
source/result file hashes, map payloads and byte transitions.

These public reserialization choices are not byte goldens for a fixed Int32
engineering service. Such a service can preserve the original Memo/OLE
references and payloads, allocate its one new index leaf at original EOF page
166/184, and retain its stronger unrelated-byte preservation contract. That is
an implementation target, not a candidate outcome in this observation.

## Reproduction and source identity

The public tools are maintained wrappers, distinct from the archived sources
that executed the initial observation. `NonrootSplitReference.java` and
`plan.json` are byte-identical promotions. The Python wrappers adjust repository
paths and expose saved-report verification. They do not claim historical
execution from their new public paths.

These commands recheck existing artifacts only; they do not launch a native
consumer or JVM and require the ignored raw reports:

```sh
python3 docs/testing/nonroot_index_split_originals/capture.py
python3 docs/testing/nonroot_index_split_originals/analyze.py \
  --verify-existing \
  --output .cache/ace16-nonroot-split-next-20261027/diagnosis.json
python3 docs/testing/nonroot_index_split_originals/lval.py \
  --verify-existing \
  --output .cache/ace16-nonroot-split-next-20261027/lval-chains.json
```

Explicit `capture.py --execute --workdir FRESH_CACHE_DIRECTORY --freeze MANIFEST`
repeats the fixed actions on fresh outputs only. Supply a manifest pinning the
current public wrappers; the historical private-source manifest is not a public
execution receipt. Its retained raw-body hypothesis can produce the documented
exit 1, after which the read-only analysis commands classify the captured output.
No test changes, guard relaxation, database regeneration or native Access action
is part of these saved observations.
