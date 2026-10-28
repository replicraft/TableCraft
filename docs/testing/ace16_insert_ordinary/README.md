# Ordinary-row INSERT candidate proof

Four engineering candidates on two unchanged ACE 16 originals passed complete
source/request-derived byte comparisons. Eight subsequent fresh, read-only
Jackcess calls passed the model-derived typed, raw-record, ROW, index and map
checks. A separate twelve-call public Jackcess INSERT phase also passed four
transition proofs. These are separate copies, not native Access save/reopen
qualification or a normal CLI write path.

The V2016 and V2019 originals each contain `FixedRows`, `MixedRows` and a
`Sentinel` table. Each target starts with 451 rows and one physical required,
unique ascending Int32 primary index, shared by two logical aliases. The actual
primary is declared field 2, after an unindexed nullable Int32 and a Boolean.
The source gates establish this physical identity; equivalent key declarations
alone do not authorize alias deduplication.

`FixedRows` has thirteen fields and 71-byte records. Its new record fits the
highest existing DATA hint. `MixedRows` has seventeen fields, including four
variable fields in actual ordinal order; its 97-byte source records fill eleven
41-row DATA pages. The new 1,106-byte record needs one fresh DATA page. Both
requests insert explicit ID 226 and split the original full primary root leaf
into two fresh leaves under the retained root.

| Case | New logical ROW | New DATA | Fresh INDEX creation order |
| --- | --- | --- | --- |
| V2016 FixedRows | 101:11 | none | 121, 122 |
| V2016 MixedRows | 121:0 | 121 | 122, 123 |
| V2019 FixedRows | 119:11 | none | 139, 140 |
| V2019 MixedRows | 139:0 | 139 | 140, 141 |

Every candidate preserves 451 old selected ROWs, the other target's 451 rows and
the Sentinel row. Each fresh getter checks 904 typed rows and 452 selected
primary key/ROW pairs. Across the four cases, that is 3,616 typed rows, 1,808
selected pairs and 1,804 preserved old selected ROWs. The two logical primary
aliases increment one physical historical counter once per inserted row; the
historical and table counters are read independently before reconstruction.

## Record and preservation authority

The model derives fixed spans, physical widths, mask size and variable ordinals
from the actual native metadata. The packed fixed extent is 69 bytes. It does
not reuse the separate literal fixture's 70-byte extent or its variable order.
Requests supply every field explicitly. New records include Boolean mask bits,
nullable fixed scalars, scaled Currency, exact Single/Double and legacy DATE
bits, mixed-endian GUID storage, preferred ShortText and ShortBinary payloads.
NULL and empty remain distinct through their presence bits.

Raw Boolean declarations in the originals report length 1, while their physical
payload width is zero. Rust already normalizes this distinction. The prospective
Python metadata binder was corrected before candidate construction; no source
or production bytes were repaired. Existing NULL payload bytes, padding and
reader-valid alternate Text encodings are interpreted from raw masks and
footers, then preserved. They are never admitted by reserializing an old row
with the canonical new-record encoder.

The raw decoder checks every original record against its typed getter and every
new record against its original request. BYTE storage `ff` represents Core Byte
255, while the pinned Java getter renders signed `Byte:-1`; this explicit
projection does not authorize general numeric coercion. Getter JVMs pin UTC and
`LOCAL_DATE_TIME`. The observed legacy DATE values are the epoch and
2000-01-01 noon; the maintained projection deliberately makes no general
negative-date or NaN-payload inference.

All four complete engineering images were frozen before the isolated committed
producer build. DATA allocation precedes INDEX allocation. Retained root residue
and fresh-page zero padding are independent engineering expectations. Complete
byte equality protects unrelated records, metadata, counters and all other
bytes; fresh readers provide additional checks. Public-writer after-images or
their scratch padding do not construct these expectations.

## Saved verification

Run from the repository root:

```sh
python3 -B docs/testing/ace16_insert_ordinary/verify.py
python3 -B docs/testing/ace16_insert_ordinary/verify.py --verify-git
```

The default command launches no child process. It verifies current evidence
pins, replays every source gate, reconstructs all four complete expected images
before replaying the public reference proofs or interpreting cached fresh getter
streams, and checks the actual command
inventories, limits and outcomes. It does not rerun the producer or Jackcess.
The optional Git check launches one metadata-only `git cat-file --batch` call
for 766 compiled source objects and the committed build receipt. It does not
compare those historical sources with the mutable working tree.

Private originals, classes and raw reports are intentionally not distributed in
this directory. Missing required evidence returns JSON with `available:false`
and exit status 2; a proof contradiction returns status 1. There is no cached
success fallback. For example, an absent `--producer-report` exercises the
unavailable-evidence contract without launching any child.

The maintained modules own record encoding, separate raw decoding, requests,
source qualification, image composition, fresh-reader interpretation and the
separate public reference proof. The six record/request/source/image modules
retain the frozen private functions and classes: AST identity and the exact
permitted repository-root/import transformations are checked. The reference
owner separately verifies its promotion transformation. The existing recursive
tree framing and map primitives are imported from their actual owner.

## Retained setup history

The source-only capture completed all nine calls on its first attempt; source
qualification followed as a separate pure saved-byte phase. The prospective
codec's Boolean declaration correction is recorded separately from this actual
capture and from the already-correct native normalization.

The first four-image preparation completed, then a pre-build pin check detected
a source-gate receipt changing from 4,482 to 4,670 bytes when scope metadata was
added. That original bundle, report and images remain intact. A separate
preparation bound the final source receipt, confirmed that it was the sole input
drift, reran all source gates and required all four image bytes and details to
remain identical. The producer build and calls followed this second freeze.
This receipt rebinding is not presented as an unchanged first bundle.

The actual four producer calls and eight fresh getter calls each passed on their
first attempt. The separate public-writer phase then completed four INSERT
copies and eight read-only inspections on its first attempt. Its transition
proofs preserve the original rows, metadata and unrelated payloads while
classifying the public writer's actual index framing and padding separately
from the engineering byte policy.

The initial candidate-only maintained verifier and its successful default,
optional-Git and missing-evidence results are preserved before this reference
scope expansion. The expanded verifier rederives all four engineering images
before calling the reference proof and never uses public after-bytes to build
an expected candidate.
