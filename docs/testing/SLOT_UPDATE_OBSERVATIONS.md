# Existing-slot Text/Binary original comparisons

The frozen engineering consumer agrees with the predeclared model on all **162
controls: 124 candidates and 38 atomic refusals**. The separate public-writer
observations are **150 successful calls and 12 refusals**; two successful FEFF
writes reopen to a different value than requested. This is not full public-action
parity or native Access qualification.

The [reference receipt](results/ace16-slot-original-references-2026-10-26.json)
and [candidate receipt](results/ace16-slot-original-candidates-2026-10-26.json)
pin the actual originals, fixed selectors, sources, commands, getters, reference
copies and 124 candidate images. The
[core profile](ACE16_WRITE_PREVIEWS.md#existing-width-short-text-and-binary-slots)
and [consumer](SLOT_PREVIEW_CONSUMER.md) remain engineering-only and do not enable
normal saves, record resizing, data macros or journals.

## Fresh independent originals

Repository-owned [SlotUpdateFixture.java](SlotUpdateFixture.java) uses the reviewed
Jackcess 5.0.4 JAR and its public builders. Expressions are disabled, foreign-key
enforcement is enabled, UTC is explicit and linked opens throw. The Java producer
is byte-identical to the source used for the actual captures; no third-party code
or database was copied. Both original files remain unchanged.

| Original | Bytes | SHA-256 |
| --- | ---: | --- |
| Slot-V2016-before.accdb | 1,044,480 | `feeeab8b1a5d51cd6d2366cdc54b895da9bd023c48c364896a9beffc7215dd46` |
| Slot-V2019-before.accdb | 1,114,112 | `2994922c9fef5302e08bfc4db66b8cbfe01e8d47b025e1a4f9b6db10d2a66092` |

Each has 17 tables and 1,125 rows. The stored fields include compact/raw General
legacy and modern Text, Short Binary, NULL/empty values, embedded NUL, non-Latin
and supplementary characters, maximum short values, noncanonical empty/AB
representations, indexed 500-row populations, physical aliases, ascending and
descending keys, supported mixed tuples, uniqueness, field properties and
validation rules. Unrelated Memo/OLE and scalar sentinels remain opaque protected
values. Indexed French and ignore-NULL tables supply explicit refusal controls.

Read-only captures establish actual flags, scalar getters, record/slot spans and
all 44 logical index orders per profile. The 36 distinct physical roots have
heights zero or one, including 29 full key/row ancestor separators per profile.
This original evidence does not qualify deeper native topologies.

## Separate public per-row observations

The original 152-action list remains immutable. A separate ten-action supplement
observes nonempty-to-NULL storage; it does not rewrite the first selector list.
All 162 calls update fresh copies. Each created copy has a read-only getter
capture, including the 12 refused calls whose copies remain byte-identical to
the originals. There are 324 action/getter commands in these two series, separate
from preparation and candidate commands.

Of the 150 successful calls, 148 reopen the requested value exactly. The two
`leading-feff-write` controls request U+FEFF followed by `AB`, but reopen as `AB`.
They remain contrary reference results, never candidate successes. The original
FEFF-derived rows likewise use their actual stored getter values as expected
preconditions, rather than the generator's intended input spelling.

Public update calls can resize/repack or canonicalize a slot even when the
engineering profile preserves its original span. In particular:

- Four raw-preferred public updates replace `ĀāĂ` with `ABCD`, growing the slot
  from six to eight raw bytes. The engineering candidate fits the requested value
  in a legal six-byte compact representation and therefore succeeds.
- Four noncanonical empty/AB no-ops canonicalize under the public writer. The
  engineering no-ops preserve their entire original files byte for byte.
- The ten nonempty-to-NULL reference updates shorten the visible span and clear
  the old payload. Engineering NULL assignments clear the presence bit while
  preserving the original slot bytes.

These are distinct preservation contracts. Reference byte canonicalization and
public width changes are not candidate admission or byte-golden rules. Public
calls run with expression evaluation disabled; their successes do not authorize
bypassing the engineering field-rule checks.

## Frozen candidate model and results

The plan is frozen before candidate execution at SHA-256
`81c449f9249ceb2af2689121e1671c1e646001de158aa4e844ac5b1e570117b4`.
It uses actual stored values and ordinary original ordinals. Admission depends on
the original slot, faithful raw UTF-16 or compact/marker-empty representation,
exact no-ops, NULL masks, native descriptors, index profiles and final constraints.
No case, budget, selector or expected outcome changes after execution.

The frozen consumer is built from committed
`59c87310ad4d25fa182336640a85477f5075455c`:
[build receipt](results/ace16-slot-example-build-2026-10-26.json).
Its SHA-256 is `ba3b1f5f80c0274d0145006e22ae60a1fc8b7b051576ae651e7a853da36e285b`.
The 75 selected build-source pins are Git-verified; they are not an exhaustive
compiler-input inventory. Candidate execution is first-attempt, 162/162 model
agreement with no corrections or omitted cases, and 286 native/getter commands.

| Actual refusal | Controls |
| --- | ---: |
| `Unsupported(RowLayout)` | 16 |
| `IndexConstraint(DuplicateKey)` | 8 |
| Required field | 2 |
| Empty Text field | 2 |
| Field validation rule | 4 |
| `Unsupported(ConstraintCoverage)` | 6 |

Every refusal publishes no output. All 124 successful images pass exact typed
rows and complete logical index orders, original record identities, exact
retained full key/row entries, original leaf populations, complete ancestor
maxima and unchanged topology/links. Selected bytes must use a faithful permitted
representation; NULL retains original payload. Whole-image checks preserve
length, allocations, historical counters and every byte outside the permitted
selected slot/mask and affected index serialization spans. All four semantic
no-ops preserve entire original images. Source files and execution tools remain
unchanged throughout.

## Current public reproduction tools

The public helpers are parameterized refactorings, **not the source versions that
executed the historical 162 candidate calls**. Exact executed private sources and
plan are archived and pinned separately. Current wrappers have real read-only
checks: both original getter streams are byte-identical to the initial captures,
metadata/geometry agree, and the public proof helper verifies all 162 recorded
outcomes and 124 existing images without relaunching candidates.

To create a separate fresh comparison identity, use the repository-owned Java
source launcher, for example:

```sh
java -Dstdout.encoding=UTF-8 -cp .cache/tablecraft-fixtures/jackcess-5.0.4.jar docs/testing/SlotUpdateFixture.java generate .cache/Slot-new-V2016.accdb V2016
java -Dstdout.encoding=UTF-8 -cp .cache/tablecraft-fixtures/jackcess-5.0.4.jar docs/testing/SlotUpdateFixture.java generate .cache/Slot-new-V2019.accdb V2019
```

Those paths must be fresh. New generation records new hashes; it is not a
byte-identical restoration of the accepted files. The publication checks below
reuse existing bytes and do not run these generation commands.

For read-only capture using existing registered originals:

```sh
python3 docs/testing/slot_preview_originals/capture.py \
  --original V2016 .cache/ace16-slot-originals-next-20261026/Slot-V2016-before.accdb \
  --original V2019 .cache/ace16-slot-originals-next-20261026/Slot-V2019-before.accdb \
  --jar .cache/tablecraft-fixtures/jackcess-5.0.4.jar \
  --producer docs/testing/SlotUpdateFixture.java \
  --workdir .cache/slot-readonly-new --report .cache/slot-readonly-new.json
```

All paths must resolve within the repository. Work/output paths must be fresh;
accepted inputs and captures are never regenerated or overwritten. Adding explicit
`--references` creates the 162 fresh public-update copies, reference reports and
a model plan for that new run. That reproduction branch is distinct from the
historical private capture and the executed read-only wrapper check.

Use `python3 docs/testing/slot_preview_originals/replay.py --help` for required
artifact hash/revision/build receipt, original preparation/facts, both reference
reports, fixed plan/hash, helper classpath, JAR and fresh work/report arguments.
`--verify-existing REPORT` performs read-only proof verification; omitting it
executes a new candidate replay. Capture/replay entry points are import-safe,
classpaths use the platform separator, artifact pins use repository-relative
labels, and outside-repository artifact paths refuse. Raw actual argv, expanded
getters, images and reports stay ignored and unchanged.

No native Access SQL/action/save, per-row timing equivalence, arbitrary encoding,
record-growth, unsupported index or normal editing profile is qualified. Later
batch, boundary and forwarding experiments are separate corpora and counts.
