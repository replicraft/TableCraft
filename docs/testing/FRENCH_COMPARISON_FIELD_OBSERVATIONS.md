# French comparison field-data preparation

The [compact field receipt](results/french-comparison-fields-2026-10-26.json)
records two fresh ACE16 V2016/V2019 originals, 76 rows and 304 complete Text/Memo
values. This receipt captures actual stored data and descriptors before any query
or string comparison runs. It is not a native Access SQL or NLS ordering oracle.

[FrenchComparisonFixture.java](FrenchComparisonFixture.java) is original
repository-owned code. Public Jackcess builders create one Comparisons table
with ID Int32, FrenchShort TEXT255, FrenchMemo MEMO and matched GeneralShort/Memo
fields. French fields receive exact `1036/0/0`; General fields receive `1033/0/0`.
The reviewed JAR is Jackcess 5.0.4, SHA-256
`19a7494a56f439a57fda85c059b019b665802085cec5ae8186d1e028cc1bdfbc`;
its existing provenance is in [the fixture ledger](FIXTURES.md). No upstream
code or database bytes were copied. Expressions are disabled, UTC is explicit
and linked opens throw. No header, descriptor or flag is patched.

Actual raw/getter descriptor bytes are `0c040000` for both French fields and
`09040000` for both General fields in each profile. Short Text is native type 10,
510 storage bytes; Memo is native type 12, storage length zero. All text flags
are 2 with extension 0; ID is native LONG 4, length 4, flags 3. No AutoNumber,
calculation, history or stored table/column properties are present. The ID
primary and nonunique FrenchShortAsc indexes have separate physical root leaves
with 38 entries each. No Memo index exists, and Memo index qualification is not
inferred from successful scalar field storage.

Twenty base rows cover NULL, empty, `cote/coté/côte/côté`, case, composed versus
decomposed accents, expansions, apostrophe/hyphen/space/punctuation and non-BMP
text. French and General fields store identical actual values. For each of
256, 1,024 and 20,000 UTF-16 units, six further Memo rows contain:

- Identical 255-unit prefixes followed eventually by primary `a` versus `b`.
- An all-e primary sequence with an acute accent first versus last.
- An all-e primary sequence with first grave/last acute versus first acute/last grave.

The short fields on these rows contain legal constant controls. Every actual
getter code unit matches the original construction input, including non-BMP and
all 18 long Memo controls; there were zero input/storage differences. This is an
observed identity, not an assumed normalization rule for arbitrary public-writer
inputs. Complete actual UTF-16LE values are retained as Base64 with lengths and
SHA-256 in the ignored getter captures. NULL and empty remain distinct even
though their empty byte sequence has the same digest.

[capture_french_comparison.py](capture_french_comparison.py) is import-safe and
uses bounded subprocesses. It compares an independent construction model with
actual values, recording any public-writer storage differences without silently
substituting input text. It verifies field/index inventories, raw descriptor
bytes, properties and source hashes before/after. No SQL, comparator, expression
or stored active content runs. These long values can test a separately defined
comparison model; their storage alone supplies no expected comparison result.

Database bytes, compiled helper and full transcripts stay ignored. The receipt
pins exact sources/JAR/class/original/getter hashes. Fresh reproduction uses new
paths and does not promise byte-reproducible database generation:

```sh
mkdir -p .cache/french-comparison-reproduction/classes
javac -encoding UTF-8 -cp .cache/tablecraft-fixtures/jackcess-5.0.4.jar \
  -d .cache/french-comparison-reproduction/classes docs/testing/FrenchComparisonFixture.java
python3 docs/testing/capture_french_comparison.py \
  --directory .cache/french-comparison-reproduction/originals \
  --jar .cache/tablecraft-fixtures/jackcess-5.0.4.jar \
  --classes .cache/french-comparison-reproduction/classes \
  --report .cache/french-comparison-reproduction/fields.json
```

## Frozen native-field comparison replay

The subsequent [112-control receipt](results/french-comparison-fields-replay-2026-10-26.json)
uses frozen CLI `596d40ea41a4bafefbd5b34af4ef35a8a4e2cde4`, SHA-256
`bcff00ad245c7a70582db222fa48e5eb2f796bd944d5ccbeefe759fbd010a576`.
All 52 compiled source objects were verified against immutable Git objects,
independently of later source work. The [build receipt](results/french-comparison-cli-build-2026-10-26.json)
and raw commands/results are hash-pinned. Ordinary and declared arithmetic modes
run separately on both unchanged originals: 96 successes and 16 intended refusals,
zero disagreements, 1,204 rows and 1,548 cells compared.

Expected Short results use 42 independent complete keys for 21 distinct actual
non-NULL values under exact French and General Legacy reference encoders.
[ShortKeysOracle.java](ShortKeysOracle.java) is an original bounded caller of the
reviewed JAR, not copied encoding logic. Every French physical key/reference in
the originals is additionally checked by independent raw framing. No Short value
here needs prefix truncation or overflow digestion. Long Memo expectations use
the pinned [Latin model](french_comparison_latin_model.py), restricted to measured
e/a/b/é/è primitive streams. Matched General Legacy Memo siblings use the forward
secondary stream separately. These model results do not qualify arbitrary long
Unicode or native Access SQL/NLS comparison; serialized unprintable positions
are outside this long oracle.

The 28 cases per profile/mode cover Short sort/group/distinct/extrema/equality
join, retained aliases through two derived layers, explicit ordinal override,
Memo sort and later primary/accent differences at 256/1,024/20,000 units,
NULL/empty/ignored-key classes, full Len results, policy conflict, LIKE refusal,
missing LCID registration, operation-default and explicit Binary search controls,
and a comparison-byte quota failure. The originals have no saved query definitions;
a positive native named-source control is not claimed by this corpus. Existing
named-source coverage elsewhere is unchanged.

Actual read-only header defaults differ: V2016 is General2010 `1033/0/1`, V2019
General Legacy `1033/0/0`; field descriptors are unchanged. InStr's explicit
Database mode uses that operation/default provider, so the ASCII `cote`/`ot`
control returns 2 even when its argument is a French field. French argument policy
is not mistaken for an operation-wide French matcher. An unregistered explicit
LCID 1036 still refuses; French field LIKE also refuses. General constructor/API
recognition remains distinct from the separate French comparison provider.

The first 112 attempts retained 60 native Step refusals before modeled long results;
these were neither value passes nor wrong value/order answers. An identical
Len-only query, which performs no text comparison, succeeded after changing only
native steps to 10 million. The accepted retry kept all SQL, data, selectors,
expected values and comparison allowances unchanged. Native bytes 64 MiB and
native steps 10 million are explicit; comparison bytes 64 MiB and comparison
steps 10 million form a separate pool. The quota-negative changes only comparison
bytes to 1 MiB and reports the intended Bytes refusal after native materialization.
No defaults were changed. The complete initial report, executed source snapshots
and causal probe remain pinned.

[compare_french_comparison_fields.py](compare_french_comparison_fields.py) and its
[expected model](french_field_comparison/model.py) are import-safe. They use fresh
output/report paths and preserve every command/stdout/stderr and initial failure.
These are headless engineering model comparisons over actual native field data,
not Windows/native Access execution or save qualification.
