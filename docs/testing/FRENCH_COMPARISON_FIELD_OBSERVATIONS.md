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
