# External Memo/OLE INSERT saved proof

Sixteen engineering candidates on four immutable ACE 16 originals match complete
source/request-derived images. Sixteen subsequent fresh, inert Jackcess calls
verify typed rows, raw records, both physical index streams and their aliases,
maps and catalog-wide long-value ownership. Each inspection emits both rows and
maps; this is sixteen calls, not thirty-two separate getter/map calls.

The four originals cross V2016/V2019 with raw/compressed Memo declarations. Each
contains `Items(ID, Group, Blob, Memo)` and an untouched `Sentinel`. ID is a
required Int32 primary; Group is a nullable ordinary Int32 secondary. Two true
logical aliases share each physical owner. Every request supplies all four fields.

The public creator adds seven rows and publicly deletes ID90. The saved gate
observes six live parents, primary historical count 7, secondary historical count 5,
and one intact orphan chunk per long column. Each column owns five LVAL pages;
Blob has six nondeleted physical chunk rows and Memo five. Orphans are included
in those physical counts. No deleted LVAL chunk is observed. These are measured
facts, not a source-generation predicate or permission to rewrite orphan bytes.

| Request | OLE payload | Memo payload | New LVAL pages per case |
| --- | --- | --- | ---: |
| single | 65 and 4,076 bytes | 66 and 4,076 encoded bytes | 4 |
| chain | 4,077 bytes | 4,078 encoded bytes | 4 |
| carriers | NULL, empty, 64 bytes | NULL, empty, `ABC` | 0 |
| mixed | 4,077 and 65 bytes | `ABC` and 4,078 encoded bytes | 4 |

`ABC` uses six raw UTF16 bytes or five compact bytes according to the declaration.
The longer Memo values are wide UTF16. NULL and present empty values remain
distinct. The 4,077/4,078-byte values have independently derived chunk links.
Both columns allocate through one LVAL cursor before DATA/index work. All new
ordinary rows fit the existing DATA page and both original index roots remain
fitted leaves. This authentic corpus does not exercise new DATA/INDEX pages,
map promotion or missing declarations; separate literal composition controls do.

Across the sixteen candidates, the proof checks 32 inserted parents, 96 preserved
old parent ROWs, 144 final user rows including Sentinel, 256 physical index key/ROW
pairs, 52 new external chunk rows and 48 new LVAL pages. Each source's two orphan
bodies remain byte-exact in every case. Actual new ROWs are 93:7 onward for V2016
and 110:7 onward for V2019. The separate public outputs match all sixteen
previously frozen engineering images byte-for-byte in this corpus. This observed
agreement does not establish a general public-writer scratch or placement policy.

## Evidence history

SOURCE9 passed on its first run: one compilation, four public creates and four
inert inspections. Capture explicitly left source qualification false. The later
saved-only gate binds C4/V2, fixed spans, variable ordinals, actual properties,
full old typed records, both column-map pairs and future cells, aliases/counters,
all catalog map roles and live long references. Legacy system long-value holders
without declarations are reported explicitly; they are not fabricated empty maps.
No old record is admitted by re-encoding it with new-record defaults.

The public reference phase first completed two successful children, then stopped
on a Python reverse-footer assertion: mask-8 contains END, while START is mask-4.
The original proof, freeze, image and report remain intact. A one-offset derivative
passed the complete saved first transition; exactly thirty previously unattempted
children then passed. There are 32 unique public calls and 16 transition proofs,
with no repeated completed call and no first-attempt whole-phase claim.

The first consumer build exited zero but reused an older shared-target binary
because archived source timestamps preceded its Cargo fingerprint. That result
was declared unqualified before any native candidate execution. Its artifact,
report and byte-identical relocated receipt remain retained. The corrective build
used a previously absent target directory, actually compiled the engine from
committed `8374bad266ea9feebbf128c87fc2fec50ea36402`, and verified all 4,526 exported
files before/after compilation. Its compact receipt pins the full archive/manifest
and the correction lineage. The sixteen native calls and sixteen fresh getter
calls then passed on their first attempts with the corrected artifact. This was
a build-freshness qualification
failure, not a failed native producer call or a reason to alter expected images.

## Saved verification

```sh
python3 -B docs/testing/ace16_insert_external_long_values/verify.py
python3 -B docs/testing/ace16_insert_external_long_values/verify.py --verify-git
```

The default launches no subprocess. It validates current evidence pins, checks
exact maintained/private implementation identity, qualifies all four originals,
reconstructs all sixteen whole images before reading any public/native AFTER,
and replays public, build and native/getter evidence. The optional Git mode
launches one metadata-only batch to verify 4,526 committed archive blobs and the
committed corrected build receipt. It never runs Cargo, the producer or Java.
Missing private evidence returns `available:false` and status 2; contradictions
return status 1. There is no cached-success fallback.

The actual source/census/request/model/reference implementations live here;
imports use their owning modules directly. Public promotion changes only the
repository bootstrap/import paths. The Java helper is byte-identical to the
executed helper and uses the existing
`parent_index_split_originals/BoundedFileChannel.java` implementation. The verifier
executes no cached Python code or database content. Private originals, classes,
archives and raw streams are deliberately not distributed in this directory.

The [owning contract](../ACE16_LONG_VALUE_INSERTS.md) and
[compact result](../results/ace16-insert-external-long-values-2026-10-28.json)
retain the distinction between engineering evidence and compatibility. No native
Microsoft Access/Windows save or reopen has run; validation/data macros and
production save admission remain separate gates.
