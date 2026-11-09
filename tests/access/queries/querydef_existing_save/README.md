# Existing saved-query expression growth and shrink reference

On 8 November 2026, six explicit Jackcess 5.0.4 metadata updates on fresh
copies matched six independently authored complete AFTER images. This is
reference evidence for a future versioned expansion of existing saved-query
Save. The production writer and frozen `TCQE0001` admission remain unchanged.
Microsoft Access application save/reopen was not run.

The [observation record](../../../results/queries/querydef-existing-fragment-save-2026-11-08.json)
pins source, AFTER, independent-model, recipe, SDK and observation hashes. The
[Java recipe](ExistingQuerySaveFixture.java) is original repository-authored
engineering code extending the earlier [constant-query recipe](../execution/ConstantQuerySaveFixture.java)
in a new file. The earlier recipe and its outputs remain unchanged. The
[raw model](model.py) calls no TableCraft codec/planner/constructor or SDK encoder.
No third-party database bytes or source code are copied into this fixture.
Generated databases, classes and raw observations remain private under ignored
`.cache/querydef-existing-save-sdk-20261108/`.

## Sources and changes

Both sources contain indexed `LocalData(ID LONG, Score LONG)` with two records,
a `Constants` SELECT and another saved constant SELECT. The larger source adds
`ManyLocalColumns`, a 96-column SELECT from `LocalData AS p` with a WHERE clause.
Expression evaluation is disabled during every SDK open; no query, macro, VBA,
UDF or provider executes. The helper updates one exact ObjectId/Attribute row
and independently reopens metadata read-only.

| Source | Saved queries | MSysQueries fragments | Fragment DATA pages | Bytes |
| --- | ---: | ---: | --- | ---: |
| Shared page | 2 | 10 | 94 | 389,120 |
| Multiple pages | 3 | 112 | 94, 95 | 393,216 |

The selected constant query is inserted last, after another query's COLUMN
fragment. Its exact native ID is `-2147476548`; its projection logical ROW is
`94:8` or `95:42`. Fresh copies separately change `42` to `1000` and `7` in each
source. The larger query's native ID is `-2147476546`; fresh copies separately
change its Attribute 8 WHERE expression at logical ROW `95:32` from
`[p].[ID] >= 2` to `[p].[ID] >= 1000` and `[p].[ID]>0`.
These selections deliberately require exact native object and fragment identity,
rather than the first COLUMN row or the only query in the database.

## Observed physical behavior

All six updates preserve file length, logical MSysQueries row count, other
original fragment records and slot words. Complete independent AFTER images
match the SDK output with zero differing bytes. SDK ordered key/logical-ROW
fingerprints remain equal for five indexes across MSysObjects, MSysQueries and
LocalData. The 24 source pages with INDEX signatures remain byte-identical;
this signature observation does not replace full engine ownership/index proof.

Growth retains the original indexed logical ROW. Its directory word gains
`0x4000`; the original span becomes a four-byte packed target ROW pointer followed
by zeros. A newly appended physical slot on the same DATA page carries `0x8000`
and the larger inline Memo record. The physical row count increases by one and
the page free-space count decreases by the new record size plus its two-byte
directory entry. The constant grows from 58 to 62 record bytes; the WHERE record
grows from 68 to 74 bytes. Logical table count, index keys and logical ROW
references remain unchanged. No dense repack or index rewrite is observed.

Shrink retains the original span and directory unchanged. The inline Memo length
and used-data-end decrease; the unused region before the footer is zero padded.
The constant retains its 58-byte span with two padding bytes; WHERE retains 68
bytes with six padding bytes. There is no free-space/count change or relocation.

The independent model starts from the complete original image and independently
reads the selected eight-field/four-variable row. It checks the signed ObjectId,
Attribute, four-byte Order, original expression, original offsets and inline Memo
framing before authoring record bytes, footer, relocation pointer and affected
page counters. Every unrelated byte is retained. No SDK-written AFTER is used as
input to construct the expected AFTER; comparison happens after construction.

## Reproduction and provenance

The reference uses the previously reviewed Apache-2.0 Jackcess 5.0.4 JAR with
SHA-256 `19a7494a56f439a57fda85c059b019b665802085cec5ae8186d1e028cc1bdfbc`.
No download, dependency adoption or third-party fixture import was performed.
Run from the repository root with the existing reviewed JAR. The helper refuses
existing output files so originals cannot be overwritten.

```sh
javac -cp .cache/tablecraft-fixtures/jackcess-5.0.4.jar \
  -d .cache/querydef-existing-save-sdk-20261108/classes \
  docs/testing/querydef_existing_save/ExistingQuerySaveFixture.java
java -cp .cache/querydef-existing-save-sdk-20261108/classes:.cache/tablecraft-fixtures/jackcess-5.0.4.jar \
  ExistingQuerySaveFixture .cache/querydef-existing-save-sdk-20261108/files \
  > .cache/querydef-existing-save-sdk-20261108/sdk-observations.tsv
java -cp .cache/querydef-existing-save-sdk-20261108/classes:.cache/tablecraft-fixtures/jackcess-5.0.4.jar \
  ExistingQuerySaveFixture --extend-where .cache/querydef-existing-save-sdk-20261108/files \
  > .cache/querydef-existing-save-sdk-20261108/sdk-where-observations.tsv
python3 tests/run.py tests/access/queries/querydef_existing_save/model.py
```

The actual first run used the four-edit recipe archived under `first-run/`; the
second run extended the new helper with the two WHERE cases. The maintained Java
and model files are byte-identical copies of the final executed recipes.
`first-run/` retains original recipe, compiled helper, raw observations and
four-image report. The six-image report verifies both source hashes still match
that first report. Reproduction at the recorded location requires a fresh output
bundle; do not delete or overwrite the retained evidence merely to rerun it.
The model writes its raw report only under ignored `.cache`; the maintained JSON
adds provenance/status boundaries and must not be silently replaced by that raw
report. Both javac/java runs exited zero with empty stderr. No Cargo gate ran for
this evidence-only checkpoint.

## Proposed next implementation slice

Expand the existing reviewed Save workflow with a new sealed engine profile and
new `TCQE` intent version; keep V1's wire, admission and reconstruction exact.
Qualify complete multi-query/multi-page sources and bind every selected fragment
to its native ObjectId, Attribute, Order and indexed logical ROW. Support
replacement of existing expression leaves while retaining their fragment
membership/order/keys; the useful initial integration should include existing
projection and WHERE expressions, not a second constant-only GUI command.

Reuse the native variable-record placement, retained-index, allocation-map and
role-ownership owners for inline Memo shrink/relocation. Do not implement the
observed relocation as an isolated serializer that bypasses those proofs.
Reconstruct from full BEFORE-bound original semantic intent during apply and
recovery; final row bytes or arbitrary AFTER images are not authority. These
same-length files can retain the existing checked image-journal lifecycle.
The runtime SQL bridge must diff the complete original and submitted SELECT and
round-trip exact permitted expression-leaf changes, rejecting topology or
unsupported syntax changes before a native save begins.

The sources deliberately have absent owner/cache/property/module payloads and
absent catalog dates. They establish neither compiled-cache invalidation nor
Access-created query-property preservation. Native Access Save/events/timestamps,
external Memo allocation, exhausted-page relocation, changed fragment topology,
query creation/deletion, action queries and older file profiles need separate
qualification. The reference is not production admission or a native Access
compatibility claim.
