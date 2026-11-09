# Repeated Number-field addition: independent SDK evidence

Three explicit second additions to the hash-pinned AFTER images from the
[Number declaration reference](../results/add-column-number-sdk-source-2026-11-08.json)
match three independently authored complete AFTER images. Both SDK and raw model
preserve every original. The unchanged first-party
[Java helper](../CreatedNumberColumnOracle.java) runs against the same reviewed
Apache-2.0 Jackcess 5.0.4 JAR; evaluation is disabled and it reopens read-only.
No database, class, JAR or third-party source is published.

The [observation record](../results/repeated-number-add-sdk-2026-11-08.json)
pins the complete images, helper and models. These are inert SDK observations,
not passing native candidate/recovery/GUI tests or Microsoft Access save evidence.
The separately versioned [native repeated-ADD contract](../../access/writes/ADD_COLUMN.md#repeated-number-addition-with-existing-external-properties)
now connects this bounded source through candidate, TCAC4 original intent, offline
staging/recovery and the same runtime/GUI Number service. These subsequent Rust
and host gates remain distinct from the original three SDK observations. The
[workflow record](../results/repeated-number-workflows-2026-11-08.json) retains
127 selected passing checks and their exact current limits.

| Prior Number source | New name | Preserved existing policy |
| --- | --- | --- |
| ID-only first addition | Score2 | Score: Required false, default zero |
| GUI Required-first inline source after addition | Extra2 | Score and Extra: Required false, default zero |
| Default-only inline source after addition | Extra2 | Score: absent Required, default zero; Extra: Required false, default zero |

All inputs and outputs are 45,056 bytes. Only pages 4 (catalog descriptor), 7
(table definition) and 10 (property LVAL) change. The SDK appends the complete new
MR2 body as row 1 on the same LVAL page, retaining row 0 and its entire old body.
The catalog physical slot/span, forwarding root, all allocation maps and primary
INDEX page remain unchanged. Only the selected property's descriptor length and
row reference change. Dictionary order, existing group order and policies remain
exact. No new LVAL page is allocated. Page-full behavior is outside these three observations; the subsequent
[ID-only rollover record](../number_column_rollover/README.md) observes its first
EOF allocation separately.

The independent [model](model.py) consumes BEFORE bytes and the explicit new
name, constructs the entire expected image and only then compares SDK AFTER.
Its [raw owner](raw.py) extracts unchanged, first-party literal framing functions
from the earlier independent Number model; it never calls a production codec or
SDK encoder. The portable model rerun also matches all three retained SDK images.
Original execution scripts and report hashes are retained in the evidence record.

Reproduce with fresh output directories and the prior reference's exact three
AFTER files named `id-only-number-sdk-after.accdb`,
`gui-number-inline-number-sdk-after.accdb` and
`existing-default-only-number-sdk-after.accdb` under `SOURCES_DIR`:

```sh
python3 docs/testing/number_column_repeat/probe.py \
  --jar "$JACKCESS_JAR" --sources-dir "$SOURCES_DIR" --output-dir "$SDK_OUTPUT"
python3 docs/testing/number_column_repeat/model.py \
  --sources-dir "$SOURCES_DIR" --sdk-dir "$SDK_OUTPUT" --output-dir "$RAW_OUTPUT"
```

The SDK wrapper refuses existing output directories, validates the reviewed JAR,
unchanged Java helper and every original hash, and retains stdout/stderr. The raw
model likewise requires a fresh output directory. Neither is a CI dependency on
private local database paths. Dates remain zero native days; no Access timestamp,
event, Name AutoCorrect, older-profile or page-exhaustion behavior is established.
