# Original-file compressed Memo candidate checks

The first frozen-consumer replay matched all **148 fixed controls**: **132
candidates** matched complete independent expected images and read-only getter
checks, and **16 leading-FEFF requests** produced the expected typed atomic
refusal. There were no retries, selector changes, budget changes or unmet
controls. These are engineering candidate checks, not Microsoft Access
save/reopen or normal backend write qualification.

The [acceptance receipt](results/ace16-compressed-memo-originals-2026-10-27.json)
pins the unchanged sources, requests, model, compiled artifact, execution and
saved-output verification. The preceding [raw 60-case acceptance](LONG_VALUE_UPDATE_ACCEPTANCE_OBSERVATIONS.md)
and its preserved initial census refusals remain separate.

## Unchanged inputs and independent expectations

The exact selectors and requested strings come from the
[92 plus 56 public Memo observations](COMPRESSED_MEMO_UPDATE_OBSERVATIONS.md).
Their public-writer outcomes are distinct: all 148 public updates completed,
but 16 reopened with normalized text. This candidate replay instead requires
132 exact requested values and refuses those 16 ambiguous requests without
publishing a file. No public update or source creation was rerun.

There are 22 source paths: unchanged V2016/V2019 originals and their fixed public
history descendants. No source is a prior kernel candidate. Each request still
selects ID 91, live ordinal 0, in `CompressedMemos`; ID 92 and the other long
fields remain untouched. Full raw/getter/map facts are revalidated before each
call. The source schema has four fields, three variable slots, an eleven-byte
footer and six-byte fixed prefix. Selected metadata remains exact Memo type 12,
flag 2, with extended flag 1 for CompactMemo or 0 for RawMemo. Neither Memo field
has an index or stored properties; the three ID declarations and actual alias
remain intact.

The independent image model adapts the earlier independently authored raw-LVAL
model to this actual schema. It rebuilds all variable offsets and the selected
presence bit while preserving the fixed prefix, other variable fields and
opaque padding. Existing forwarded-root residue and orphan hidden targets
remain protected. Per-column LVAL and ordinary parent placement share EOF and
an aggregated map-byte ledger; old chunk bodies/slots remain unchanged.

Canonical wire follows the measured field policy: compression requires the
exact flag and 3–1,024 UTF-16 units, every unit in 1..255; other values remain raw.
Semantic equality is checked before serialization admission. Refusal is derived
from the requested canonical stream's independent decoder roundtrip, rather
than from public-writer mismatch labels. All 132 predicted images are decoded
again for all three long fields before the first native call.

A separate reviewer recomputed the 132 whole-image hashes and 16 refusals,
checked all 148 request files, and verified source footer and page geometry.
The complete image model, input freeze and later consumer/runner binding have
separate retained identities. Expected images never come from kernel output.

## Frozen consumer and actual outcomes

The artifact is compiled from Git `452f2bde94a2580bd0a64e166cc11384f1ff8a4a`;
its [build receipt](results/ace16-compressed-memo-example-build-2026-10-27.json)
records the isolated committed export. The replay verifies all 197 historical
Git source objects against that revision, rather than current working files.
The consumer retains its 256 MiB/100,000-row/100-million-step budget and the
64 KiB value-input profile. No allowance or selector was adapted.

The replay used **280 native/JVM fixture commands**: 148 native candidate calls
and 132 combined read-only Java metadata/getter inspections, each bounded by a
30-second timeout. The getter uses the exact prior class/JAR with `-Xmx512m`.
The 197 Git source-object preflight processes are separate from those 280 fixture
commands. No public UPDATE, compilation, database creation or native Access
execution occurs in this replay.

| Candidate storage | Matched controls |
| --- | ---: |
| NULL | 8 |
| Inline | 36 |
| Single external row | 70 |
| Chained external rows | 18 |

There are 104 fitted and 28 forwarded parent replacements. All 132 report one
changed row; **none is a semantic no-op**. No-op preservation remains separately
covered by implementation tests and is not claimed as original-file evidence
from these selectors.

Every candidate matches its complete independently modeled byte image before
getters run. The cached full getter/metadata transcripts then verify all table
values, selected and untouched descriptors/streams, index encounter order,
flags/properties, historical counters and allocation memberships. There are
884 explicit before-live-chunk body/slot comparisons and 1,188 exact stable-map
wire proofs. Complete image comparison also protects old orphan storage.
Sixty new owned EOF pages match sixty global free-bit clears; no old ownership
is retired. Original-source, request, artifact and both manifest pins remain
unchanged after execution.

All 16 refusals have exit status 1, exact `Unsupported(Column)`, empty stdout,
no output file and no getter call. They cover FEFF/FEFFAB and the long
FEFF-prefixed boundary cases under both field policies/profiles. FFFE controls
remain exact successful values. The engine does not silently reproduce the
public writer's character loss.

## Scope and saved-proof verification

The placement model explicitly repeats highest-hint attempts before represented
EOF allocation, matching the implemented multiple-candidate policy. These
sources have at most one hint per family, so they do not discriminate ordering
among multiple hints. That behavior retains separate focused implementation
checks. Broader map growth, in-file free reuse, old-chain retirement,
mode-switch authoring, indexed/history Memo, active-content eligibility and
native Access qualification remain explicit gaps toward full ACE 16 support.

The [saved-proof verifier](compressed_memo_observations/verify_acceptance.py)
reconstructs every expected image from the frozen before sources and requests,
rehashes actual output and all recursive live artifacts, reparses cached
getters/maps, and re-derives each atomic refusal. Its accepted run verified
**1,111 live artifact pins** and separately reverified the **197 historical Git
objects**. It performs no native/JVM or database-write calls:

```sh
python3 docs/testing/compressed_memo_observations/verify_acceptance.py \
  --report .cache/ace16-compressed-memo-acceptance-first-20261027/report.json \
  --output .cache/compressed-memo-acceptance-verify-fresh.json \
  --verify-git
```

Its initial read-only run used an incorrect report key for the archived request
path; source and failure are preserved. The correction uses the actual native
argv path and requires byte equality with the frozen original TSV. This did not
rerun or change any candidate call, expected image or result. The corrected
verifier's source is distinct from the exact executed native replay driver.
