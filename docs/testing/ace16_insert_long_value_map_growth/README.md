# Original-file column-LVAL map growth proof

Sixteen native candidates on eight immutable ACE 16 originals match complete
source/request-derived images. Sixteen fresh inert inspections subsequently
validate full typed rows, raw LVAL chains, physical index key/ROW pairs and aliases,
maps and catalog-wide ownership. The combined helper emits both rows and maps,
so the reader phase has sixteen calls. A separate public reference phase has
thirty-two successful calls and sixteen transition proofs. All actual phases
pass on their first attempts, without retries or regeneration.

The originals cross V2016/V2019, raw/compressed Memo and fixed 430/700 Padding
rows. Items first receives the same explicit six-live-parent Memo/OLE source as
the earlier [external payload corpus](../ace16_insert_external_long_values/README.md).
Public deletion leaves one observed orphan chunk per column. An independent
Padding table then advances EOF through ordinary public writes: seven raw
255-unit Text fields plus ID give actual 3,595-byte records, one DATA page per row.
Padding never changes selected membership or map references. There are no source
byte patches, map setters, padding searches or retargeted selectors.

| Layout | Padding | Actual source EOF | Source-derived owned-map transition |
| --- | ---: | ---: | --- |
| V2016, both Memo modes | 430 | 543 | Rebase Blob to 88, Memo to 96 |
| V2016, both Memo modes | 700 | 813 | Promote both owners |
| V2019, both Memo modes | 430 | 561 | Rebase both owners to 112 |
| V2019, both Memo modes | 700 | 831 | Promote both owners |

All four selected map rows begin as real 69-byte inline records with base zero.
Actual early minima, complete owned members and aligned spans determine which
transition fits; literal page 16 boundaries are not imposed on original files.
The saved gate checks all selected records/descriptors/chains and Padding values,
complete declarations and aliases, stored historical counters, raw catalog roles,
all live long references and future global/foreign claims before any append/image.
It decodes existing wire rather than reserializing old rows with new defaults.

Each source receives either one Single request (OLE 65/Memo 66 encoded bytes) or
one chained request (OLE 8,145/Memo 8,146 bytes). The latter has three chunks per
column. The engineering model independently derives semantic map rows, bitmap
contents, links and one shared body/metadata cursor. A promoted owned bitmap
immediately follows that column's first body, is excluded from owned/hint body
membership, and is reused across subsequent chunks. Candidate hints exhaust
before EOF and rebase inline independently of owned promotion.

For example, the V2016 upper chained case has Blob bodies 813→815→816 with owned
bitmap 814, then Memo bodies 817→819→820 with bitmap 818. All original map references
and record lengths remain unchanged. DATA fits at 93:7 (V2016) or 110:7 (V2019);
both index roots remain fitted leaves. Table rows grow 6→7, physical primary
history 7→8 and ordinary secondary history 5→6, each once despite aliases.

Across the sixteen candidates there are 64 new LVAL bodies and 16 owned bitmap
pages, sixteen inserted parents, 96 old selected ROW preservation checks and
224 physical key/ROW pairs. Fresh getters check 9,168 final user rows, including
unchanged Padding and Sentinel. No fresh DATA or INDEX page is required here.
The public outputs match all sixteen previously frozen engineering images exactly
in this corpus; that observation never supplies golden bytes or a general native
scratch policy.

## Phases and saved verification

SOURCE17 is one javac, eight creates and eight complete inert inspections. Capture
left qualification false; all eight saved raw gates passed separately. Sixteen
complete engineering images were then frozen before public references or the
producer build. The public 32-call phase passed independently. The consumer build used
committed `18bbfee7cd6bb229c68a936f3c023d04b111bdd5` and an initially absent dedicated
target, recorded actual engine compilation, and retained a compact receipt for
the complete 4,542-file Git-bound archive/manifest. All sixteen native image
comparisons precede sixteen fresh reader calls. No active database content runs.

```sh
python3 -B docs/testing/ace16_insert_long_value_map_growth/verify.py
python3 -B docs/testing/ace16_insert_long_value_map_growth/verify.py --verify-git
```

The default launches no subprocess. It checks immutable pins, exact maintained
implementation identity, all eight source gates and all sixteen reconstructions
before interpreting any public/native AFTER, then replays public and fresh
reader proofs. It imports no cached Python code. The optional Git mode launches
one metadata-only batch for 4,542 committed archive blobs and the committed build
receipt. Neither mode runs the producer, Java or Cargo. Required private evidence
missing from a checkout returns `available:false` and status 2; contradictions
return status 1, with no cached-success fallback.

The first maintained replay stopped on comparing raw census tuples with their
JSON list representation. Its code/output remain retained; the correction only
normalizes that comparison boundary. The corrected default and first Git replay
pass. No original, model, expected image, runtime call or physical proof changed.

The [result](../results/ace16-insert-long-value-map-growth-2026-10-28.json) and
[owning contract](../ACE16_LONG_VALUE_INSERTS.md) retain current limits. This
small-file corpus does not reach a new 32,736-page indirect bitmap block near
128 MiB. Authentic missing-pointer growth needs a separate file/native-budget
profile; literal empty-slot coverage is not promoted from these files. Missing
declaration initialization, production saves and Microsoft Access/Windows
save/reopen also remain outside this qualification.
