# Generated preserved joint Text and Boolean copies

This repository-authored qualification reuses the exact unchanged V2016/V2019
originals, creation recipe and read-only getter from the
[Short Text corpus](../preserved_short_text_genuine/README.md). No database,
getter or compiled class was recreated or changed for this tranche. Generated
files, runtime JARs, classes and raw logs remain private; the
[result record](../results/ace16-preserved-joint-text-fixed-genuine-images-2026-10-31.json)
pins the actual observations.

## Scope and cases

Each original has an ordinary, unindexed `Short_Text` target and Boolean `Flag`.
The other Int32 fields are indexed, calculated or calculation inputs; this
corpus does not qualify joint Text/Int32 or eight-byte targets. Their authored
coverage remains in the [joint fitting contract](../ACE16_PRESERVED_JOINT_TEXT_FIXED.md).

| Request per original | Transport | Changed logical roots |
| --- | --- | ---: |
| Shorten Text and set Flag on the same row | Typed | 1 |
| Exact encoded Text no-op and set Flag | Typed | 1 |
| Clear Text and set Flag | Typed | 1 |
| Promote NULL to present empty Text and clear Flag | Typed | 1 |
| Change Text and Flag on different original rows | Typed | 2 |
| Exact Text/NULL and Boolean no-ops | Typed | 0 |
| False SQL selection with genuinely empty groups | SQL | 0 |

Text and Flag share one mask byte. The original NULL Text slot has zero payload
capacity: present empty Text must use zero bytes and change its presence bit;
the compression marker cannot fit. Boolean has raw declared length one and zero
physical payload bytes. Only its value bit changes.

The pure [model](model.py) reconstructs Text payloads, reverse offsets, sibling
spans and individual mask changes from original framing and [literal requests](recipes.py).
It consumes no writer output. Every fixed-prefix byte, opaque sibling, retained
padding, unselected mask bit and byte outside the requested records stays exact.
Changed rows are the union of original logical roots.

## Observed qualification

All fourteen expectations passed complete native Source admission before any
candidate call. Their model, recipes and native expected-source helper were
committed separately as `b2f9bf62`. A fresh isolated release build of committed
producer `510b4824` then matched all fourteen complete images. Twelve typed
batches and two SQL calls produced ten changed files and four exact no-ops.
Fourteen fresh unchanged read-only SDK getter processes matched the full
expected inventories, including stored caches, Memo/OLE, attachments and Sentinel.
The first saved-only replay passed without child processes.

The initial reader wrapper rejected the unchanged getter's twelve-token COLUMN
format during preparation, before starting a process. Its arity correction
changed no image, request or getter. A separate initial executable-retention copy
failed for disk space; its incomplete copy remains separate, and the retained
replacement matches the executed artifact exactly. Both diagnostics remain in
the result record. No candidate or SDK outcome was retuned.

To replay retained evidence with the pinned private artifacts restored:

```sh
python3 docs/testing/preserved_joint_text_fixed_genuine/verify.py --root .
```

The verifier checks source/tool provenance, the complete committed source archive,
fresh owning compiler events, literal requests, independently reconstructed
whole images, exact CLI reports and complete reader inventories. It launches no
build, JVM, database writer or reader. Historical external tool observations
remain data; repository artifact checks stay scoped to the repository.

These engineering observations grant no ordinary save, journal, required-data-macro
coverage or Microsoft Access save/reopen qualification.
