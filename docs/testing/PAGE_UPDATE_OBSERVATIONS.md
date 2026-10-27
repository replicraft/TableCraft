# Existing-page UPDATE candidates on original files

The first frozen PAGE replay passes all **180 controls**: 156 candidates and 24
atomic typed refusals, with zero disagreements across 336 commands. All 382
immutable inputs/tool pins remain unchanged. The [compact receipt](results/ace16-page-originals-2026-10-26.json)
links the raw report, command log, 180 requests, actual candidate hashes and
executed-source archives. No failed candidate attempt or corrected golden is
hidden: the single original-file run exited zero on its first attempt.

The executable was compiled at `fd2a23cd` and frozen with SHA-256
`32c2ca3d9b4fbcfad5909a8fbe0d9fadcca5047b68709c9708e9075bce346d82`.
Its [build receipt](results/ace16-page-example-build-2026-10-26.json) records the
94 historical Git source pins. Later working allocator code is not the code
qualified by these candidates. Every candidate command explicitly includes
`--page-updates`; the existing Slot default and its earlier 162 outcomes remain
separate and unchanged.

## Fixed selectors and independent admission

| Source series | Controls | Candidates | Refusals |
| --- | ---: | ---: | ---: |
| Same original Slot162 assignments, through the separate PAGE API | 162 | 138 | 24 |
| Actual public growth-copy semantic no-ops | 14 | 14 | 0 |
| Four actual forwarding-history descendants | 4 | 4 | 0 |
| Total | 180 | 156 | 24 |

Admission was frozen before the artifact was supplied. It uses complete physical
record capacity and the rebuilt variable footer, including all unselected
payloads. Changed Text follows metadata-preferred faithful serialization: raw
UTF16LE, except eligible compressed nonzero Latin1 strings longer than two units.
The two-unit tie stays raw. NULL and empty remain distinct. A semantic no-op
preserves the original whole image, including noncanonical stored encodings.

The resulting candidates comprise 122 fitted records, 16 same-root-page hidden
appends and 18 semantic no-ops. Fourteen appends grow ordinary original records;
two grow already-forwarded histories again. The other two history actions shrink
within retained physical capacity. A new hidden append requires a true represented
hint, complete record plus slot-word space, an exclusive physical target and the
observed 255-slot ceiling. No allocation, alternative-page fallback, deleted-slot
reuse or compaction is exercised.

The 24 refusals retain the exact original Slot diagnostics: two unfaithful
leading-FEFF requests, eight unique duplicates, two Required violations, two
AllowZeroLength violations, four Len/StrComp0 rule violations, two indexed French
profiles and four ignore-NULL profiles. The two contrary public FEFF writes remain
public discrepancies, not replacement-value goldens. Expressions-disabled public
updates do not establish ValidationRule execution.

## Preservation proof

The independent proof reconstructs the complete selected physical record, mask,
variable offsets, footer and retained padding. It then reconstructs the exact
data-page image: target body, hidden slot, count/free words, root pointer and
permitted root-stub changes. Obsolete hidden bodies and every other data byte are
preserved. As a separate read-only check, this reconstruction reproduced all
fourteen existing public ordinary-growth images byte for byte.

All 156 accepted candidates also pass complete read-only getter checks. Original
logical row/root references, unselected raw fields, declarations, table counters,
maps and unrelated payload bytes remain intact. Changed index keys use recorded
public key bytes only as semantic references; original row references are retained.
The proof checks every logical alias/order, full child maxima, original topology,
retained key/row entries and exact bytes outside narrowly authorized index spans.
Fitted padding preservation is an explicit engineering contract; general public
writer byte identity is not claimed.

## Public reproduction and read-only verification

The [parameterized model](page_preview_originals/model.py),
[proof](page_preview_originals/proof.py) and
[runner](page_preview_originals/replay.py) are new public wrappers. They are
separate from the exact private files executed historically. Source/archive
mappings in the receipt preserve both identities. The public model rebuilds the
same 180 selectors, and `--verify-existing` revalidates the accepted 156 candidate
images and 24 refusals without launching the native consumer or Java getters.
These verification passes do not add candidate executions to the 180 count.

Run the [entry point](compare_page_updates.py) from the repository root, supplying
preserved ignored inputs explicitly. For read-only verification:

```sh
python3 docs/testing/compare_page_updates.py \
  --example .cache/tablecraft-bin/ace16-page-20261026 \
  --sha256 32c2ca3d9b4fbcfad5909a8fbe0d9fadcca5047b68709c9708e9075bce346d82 \
  --compiled-revision fd2a23cd1f4b50157531c28ab50a77b3e45eb405 \
  --build-receipt docs/testing/results/ace16-page-example-build-2026-10-26.json \
  --plan .cache/ace16-page-originals-plan-20261026/plan.json \
  --plan-sha256 f252de8e18e80e966aa23192bab39c7cf8bd6303a50cd0465ca5adb504de963c \
  --facts .cache/ace16-slot-originals-next-20261026/original-facts-initial.json \
  --initial-report .cache/ace16-slot-originals-next-20261026/reference-updates-initial/report.json \
  --null-report .cache/ace16-slot-originals-next-20261026/null-supplement-initial/report.json \
  --jar .cache/tablecraft-fixtures/jackcess-5.0.4.jar \
  --classes .cache/ace16-slot-originals-next-20261026/classes \
  --request-byte-pins .cache/ace16-page-originals-plan-20261026/request-byte-pins.json \
  --request-pins-sha256 22c7907869e45f75be4456aa2b6ba4e3c85e50ba76dbf66111f42b31fb1e5006 \
  --workdir .cache/page-readonly-fresh \
  --report .cache/page-readonly-fresh-report.json \
  --verify-existing .cache/ace16-page-originals-initial-20261026.json
```

Omitting only `--verify-existing` explicitly runs the separate PAGE consumer on
fresh outputs; both work directory and report must be fresh. It never rebuilds
or chooses a mutable binary. Full stdout/getters and raw proof arrays remain
ignored and hash-pinned. [Original Slot provenance](results/ace16-slot-original-references-2026-10-26.json)
and [separate cross-page public observations](CROSS_PAGE_UPDATE_OBSERVATIONS.md)
retain their own scopes. No Microsoft Access/native save, data-macro, allocation,
arbitrary free-page reuse or broader write compatibility is qualified here.
