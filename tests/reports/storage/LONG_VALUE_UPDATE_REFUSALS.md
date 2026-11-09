# Initial raw Memo/OLE update refusal boundary

The first original-file candidate replay attempted all 60 frozen Memo/OLE
transitions and produced **zero candidates: all 60 controls remain unmet**.
The [compact receipt](results/ace16-long-value-first-refusals-2026-10-27.json)
pins the exact first report, terminal log, frozen requests/model and later
read-only diagnostic evidence. This records a discovered admission gap, not
replacement acceptance or native Access qualification.

## First candidate run

Session 59027 terminated with status 1 after exactly 60 native commands and no
JVM calls. Every command refused atomically, with empty stdout and no candidate
file. V2016 returned `Unsupported(Column)` for 15 Memo and 15 OLE cases; V2019
returned `Inspect(Malformed)` for the corresponding 30 cases. No selector,
expected value, budget or metadata was changed afterward.

The artifact was compiled from `d1e6d4c77523c80594584e8b9143d564cd41a56b`
and has SHA256
`3c829ba49adb25146e000ce1d1ce431919b91ac8d5b81c261950d7ec0d92e808`.
Its [build receipt](results/ace16-long-value-example-build-2026-10-27.json)
retains the compiled source identities. The exact private plan `bbcd7ad0`, model
`b35b1aa9` and runner `e938b2f2` remain unchanged. Each case used its own original
or public-history input, never a preceding kernel candidate.

## Read-only diagnosis and causal controls

A separate read-only diagnostic found that selected Sentinel field admission
and maps passed in both profiles. V2016 then encountered an unrelated system
attachment backing `FileData` field at page 80, slot 0: variable OLE type 11,
flags 2, extended flags `0x10`, declared length zero. The whole-catalog ownership
census incorrectly required an ordinary scalar result category to follow its
outer long-value descriptor.

V2019 instead encountered MSysObjects page 17, slot 32, with raw slot word
`0xc3d4`. It is a deleted, relocated, zero-length tombstone. The census validated
its record span before checking the deleted flag, unlike the catalog reader.

Two independent synthetic regression functions failed against the unchanged
implementation in session 17881. After the narrow corrections, session 70085
passed all ten independent writer functions, including these unchanged controls:

- Disjoint backing OLE storage permits an ordinary selected Memo update; an
  external alias into selected storage still refuses with `RowLayout`.
- Deleted empty slots are skipped; an otherwise identical live empty slot still
  fails with `Malformed`, including the separate long-value checker control.

Selected-field mutation admission remains strict. The backing field's default
checker status remains `UnsupportedColumn`; descriptor census does not grant
write authority. Subsequent read-only original censuses completed over 38 and
41 local tables, preserving both source hashes. These diagnostics neither
published replacement images nor qualified the original 60 requested updates.
A revised artifact must receive its own fresh acceptance run; that result cannot
be described as first-attempt acceptance.

The separate [census correction record](results/ace16-long-value-census-correction-2026-10-27.json)
owns the implementation checks and subsequent regression totals. The counts
above describe the actual causal before/after runs, rather than replacing them
with later combined totals.

## Read-only reproduction

With the pinned ignored local evidence available, run:

```sh
python3 docs/testing/long_value_preview_originals/verify_first_refusals.py \
  --output FRESH_OUTPUT.json
```

The verifier checks the exact failed report and plan, all 339 unique pre-call
pins, the 60 requests and refusal outputs, absent candidate paths, and 176
compiled Git objects. It makes no native or JVM calls and refuses to overwrite
its receipt. Its new public implementation is distinct from the historical
candidate runner. Public command examples use portable repository-relative
renderings; raw commands and tracebacks remain untouched in the pinned private
report.
