# Genuine ordinary target sources and literal edits

Two immutable Jackcess public-API originals, h5/h6-distinct, provide the first
genuine ordinary target expectations. Their complete hashes are
`72e7ba5480d95d0be0d4ea6f2c75b015062970599da3fde620cc9f13c60b028d` and
`5cfbf6d4b6224b989f00f0abd63ec362846ff60240a91b058e1442c3010305c5`.
They were already admitted for independent
[secondary-index source reads](results/ace16-secondary-index-genuine-source-admission-2026-10-30.json).
No system table, property, VBA content or bootstrap byte is removed to create a
simpler input. The observed complex foreign-key carrier belongs to system
inventory; Items itself has three plain manual Int32 fields.

Original Items rows are `(Id, Score, Guard) = (1, -1, 0), (2, 0, 1), (3, 1, 2)`.
Each header has five source-derived outcomes: Score −1→17, Score −1→NULL,
Score −1→−1, one empty Score group, and manual Id 1→7. Clearing Score changes
only its presence bit and preserves the inactive `ffffffff` payload. Noops and
empty requests preserve the entire file. Intended duplicate-Id and stale-Score
requests have no expected AFTER and have not been executed by these source tests.

Items definitions are pages 89/106, with direct DATA pages 93/110. Its primary
Id index has two logical aliases sharing leaf 91/108; the two Guard aliases share
leaf 92/109. Both physical leaves have prefix zero, three nine-byte entries and
27 active bytes. Id 1→7 independently reorders the primary entries to
Id 2/ROW1, Id 3/ROW2, Id 7/ROW0 within that same span. Only the selected fixed
payload/mask and, for Id, the primary active entries change. Complete byte
complement checks preserve all other headers, metadata, properties, maps,
history, index capacity, Guard keys, system/VBA bytes and EOF.

The native source gate passes one test covering the two originals and all ten
literal outcomes through real scalar, constraint and index reads. It also
records inert target payload/child observations; these do not establish a
complete write-admission certificate. Two earlier native test compilations
failed on digest formatting and column-name assertion plumbing; their logs are
retained. No original request, model, expected image or observer was retuned.

Independent SDK reads pass all twelve complete inventories and 48 logical index
cursors, with exact stdout, empty stderr and unchanged frozen inputs/classes/JAR.
The full table iterator retains physical ROW0/1/2 order after Id changes; explicit
primary cursors return Id order 2/3/7. Expected reports transform only original
observations, recomputing page/record hashes from literal image bytes. They never
consume new reader output or a native producer image as an oracle.

The maintained pure tools are under
[`scripts/compatibility/genuine_ordinary_targets`](../../scripts/compatibility/genuine_ordinary_targets/create_expected.py).
The model and report helper retain the executed private helpers' ASTs, with
readable formatting; the request inventory remains byte-exact. The command below
requires the two pinned originals and their two pinned carrier-third reports:

```sh
python3 -B scripts/compatibility/genuine_ordinary_targets/create_expected.py \
  --h5-original ORIGINAL_H5 --h6-original ORIGINAL_H6 \
  --h5-report ORIGINAL_H5_REPORT --h6-report ORIGINAL_H6_REPORT \
  --output FRESH_EXPECTED_DIRECTORY
```

The parent directory must exist and the output directory must be absent. The
tool validates inputs and constructs all artifacts before creating output. It
writes twelve `.accdb` views, `proofs.json`, `views.json`, `native-views.tsv` using
the standard CSV module, and sixty expected inventory/cursor reports under
`reports/`. It invokes no database runtime, SDK, network or native planner and
never opens an existing database for writing. A fresh replay of the maintained
command passes: all 75 files match the frozen pre-reader expectations byte for
byte, including twelve images, sixty reports and three manifest/proof files.
The model and report ASTs remain unchanged. The replay inventory and verification
are recorded separately from the original construction and reader receipts.

These are source and expected-image read results. They grant no creator-based
authority, macro-absence certificate, normal save or recovery/session permission.
Genuine native writer calls were zero throughout this source-qualification and
pure-tool replay phase. Later write integration has separate acceptance evidence
and does not change those historical counts. Microsoft Access save/reopen and
Windows qualification remain unrun. See the
[source result](results/ace16-genuine-ordinary-target-sources-2026-10-31.json).
