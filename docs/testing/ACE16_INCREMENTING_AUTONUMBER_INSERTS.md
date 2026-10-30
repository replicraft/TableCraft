# Explicit incrementing AutoNumber INSERT previews

With `ace16-write-preview`,
`NativeImage::preview_ace16_incrementing_insert_rows` constructs a separate
`Ace16AutoNumberInsertPreview` under a caller-selected incrementing policy.
The ordinary [explicit-row INSERT API](ACE16_INSERT_PREVIEW.md) remains unchanged
and continues to refuse generated fields, including empty manual batches.
Normal ACE saves, journalable edits, required data macros and SQL INSERT execution
are not enabled.

## Explicit requests and returned keys

`IncrementingAutoNumber::new(expected_last)` selects signed Int32 increment-by-one
engineering behavior and pins the original last-issued counter. It does not detect
or certify the source's stored New Values mode. Microsoft documents both
[Increment and Random AutoNumber choices](https://support.microsoft.com/en-us/access/add-an-autonumber-field-as-a-primary-key);
a stored AutoNumber flag alone does not distinguish them.

Each `AutoNumberRowInsert::new(values)` supplies every companion value in native
column order, excluding exactly one schema-resolved generated primary column.
The generated field can occur after another Int32 field. There is no NULL or
Empty generation marker, explicit-ID override, default policy or reseeding input.
NULL and present empty companion values retain their ordinary storage meaning.

The generated primary must be a single required unique ascending Int32 key. Its
field has qualified fixed four-byte storage, raw flags `0x07` and no extended or
complex attributes. These flags qualify this selected engineering profile; they
are not an Increment-mode detector. Multiple generated fields, generated GUIDs,
complex fields and unqualified properties remain refused. In particular, an
unqualified stored `NewValues` declaration is not overridden by caller policy.
Primary Microsoft documentation identifies Increment and Random choices but does
not establish a numbered native property encoding here. The pinned independent
writer also supplies no NewValues metadata enum binding. No raw `0`/`1` carrier,
flag, counter sign or property absence is treated as proof of a stored mode.

On success, the result exposes `before_bytes()`, `candidate_bytes()`,
`inserted_rows()`, `generated_column_number()` and `generated_values()`.
Generated values follow original request order and become available only after
independent complete-image proof and final index/constraint checks. The result
has no conversion to a journalable edit plan.

## Explicit policy from the captured counter

`NativeImage::prepare_ace16_incrementing_insert_policy(&self, table, budget,
cancelled)` explicitly selects increment-by-one behavior and obtains the LAST
precondition from that same captured image. It shares the existing generated
primary and signed counter binding, and performs the complete before-INSERT
gates. The existing preview then independently revalidates the returned policy.
This avoids manual counter inspection without selecting a mode automatically.

The returned `IncrementingAutoNumber` is a counter precondition, not an image
identity certificate. It does not detect stored Increment/Random mode, infer
`MAX(live ID)`, issue IDs, reseed a table, evaluate defaults or override
`NewValues`. Required, validation, active-content and property refusals remain.
Callers that already know the original counter can continue using
`IncrementingAutoNumber::new(expected_last)` unchanged.

The main CLI exposes the explicit choice as:

```sh
tablecraft write preview-insert offline.accdb inserted.accdb Items companions.tsv \
  --incrementing
```

`--incrementing` and `--incrementing-last LAST` are mutually exclusive. Both
consume the same complete companion KIND/DATA TSV; neither supplies omitted
companion fields. The current TSV protocol cannot encode a zero-companion row,
so a table containing only the generated field still requires the engine API.
The service obtains the policy and prepares one candidate from one captured
image. Generated column/value facts become reportable only after successful
fresh-output publication. The separate `preview_ace16_insert` engineering example
retains its existing `--incrementing-last` interface.

Two source-only gate functions pass before any new helper calls: they qualify
both ACE16 layouts, signed minimum/negative counters, deleted highest/all rows,
separate high-water history, exhausted maximum and independently constructed
complete expected images. The six-function helper batch then passes, including
exact preview output, unchanged explicit counter mismatches, property/profile
refusals, all resource pools and cancellation at the measured final poll.
The CLI argument control passes, including mutually exclusive incrementing flags.
The actual CLI workflow also passes: source-counter selection and the existing
explicit-LAST policy produce identical complete candidate bytes and generated
IDs 31 and 32; a stale explicit counter publishes no output. This is consumer
parity evidence, not a new independent raw model or native Access comparison.
The first CLI unit compilation stopped before tests because two new test patterns
used a stale internal enum variant. Correcting those patterns changed no runtime
behavior or expected bytes; the complete CLI unit batch then passes all 28 tests.

## Last-issued counter and signed arithmetic

The reviewed independent writer reads the signed little-endian Int32 at table
definition offset 20 as the **last issued** Long AutoNumber. Its internal
`OFFSET_NEXT_AUTO_NUMBER` name does not mean the stored word already contains the
next value. Source metadata, the actual generated primary and the exact four-byte
before-image bind that counter transition.

A mismatched `expected_last` returns `Conflict`. For request ordinal zero, the
first generated value is `last + 1`; subsequent requests increment in order.
The full signed range is supported under the explicit policy: a last value of
-2 generates -1 and 0, and `i32::MIN` generates `MIN + 1`, then `MIN + 2`.
Negative counters neither prove nor imply Random mode.

Upper overflow returns payload-free `AutoNumberExhausted`. Values never wrap,
skip collisions or publish a successful prefix. An empty batch at `i32::MAX`
remains valid under complete source gates and produces no keys or counter patch.
The candidate changes the counter once to the last generated value. Row-count
and per-physical-index historical counters remain separate.

Generation does not use `max(live ID) + 1`, the live row count or an index counter.
Deleted highest IDs, gaps and empty live populations retain their stored counter
history. Microsoft notes that
[deleted records can leave gaps](https://support.microsoft.com/en-us/access/introduction-to-data-types-and-field-properties).
The service does not invent a live-maximum consistency rule or repair a stale
counter: an actual generated primary collision fails the existing uniqueness
checks. Source bytes at offsets 24–27 remain opaque and preserved; the separate
complex-type counter at offset 28 is not modified.

## Shared INSERT pipeline and independent authority

Only the distinct incrementing admission/layout context admits the selected
`0x07` column. The actual schema is not copied with its generated flag removed,
and companion fields retain ordinary carrier, property and geometry checks.
Text/Binary companions are copied fallibly after charging their dynamic memory
and traversal. Native UTF-16 storage limits remain separate from UTF-8 allocation
charges. Counts are bounded before normalization; normal source, allocation,
work, cancellation and final constraints continue to apply.

The normalized rows enter the existing record, Memo/OLE, DATA and physical-index
pipeline. The bound generation context also reaches long-value preparation and
its independent layout replay. Long-value containers/bodies, ordinary rows, bitmap metadata and index
splits retain their shared actual allocation cursor. The exact TD20 transition
is separate from row/index counters and from qualified table-definition length,
free-space and long-value declaration tail changes. This does not widen ordinary
UPDATE or manual INSERT generated-field admission.

The independent audit receives the **original companion requests and policy**.
It rereads source metadata and TD20, independently derives signed values and
column placement, reconstructs complete records, and replays the counter into its
own expected image. Expanded producer rows or their generated key vector are not
authority. Returned keys, staged descriptors and the whole final image must agree
with that original intent before publication.

## Independent literal coverage and verification state

The [source/request model](../../crates/engines/tablecraft-ace/src/table/mutation/insert/tests/incrementing_autonumber/model.rs)
and [public API tests](../../crates/engines/tablecraft-ace/src/table/mutation/insert/tests/incrementing_autonumber.rs)
construct complete expected images before preview calls. They cover both ACE 16
headers, a generated column after another Int32, deleted highest/all-deleted
populations, a higher historical counter, signed counters, upper exhaustion,
collisions, empty requests, aliases and shared DATA/index growth.

[Private audit controls](../../crates/engines/tablecraft-ace/src/table/mutation/insert/autonumber/tests.rs)
first match a complete independent healthy image, then test correlated wrong IDs,
DATA/index ledgers and counter values that remain reader-valid. Original request
arithmetic still rejects them. Missing counter updates and incorrect returned key
order, length or values are checked separately.

[Companion integration tests](../../crates/engines/tablecraft-ace/src/table/mutation/insert/autonumber/companions.rs)
reuse actual independent ordinary-row and missing-long-value-declaration models.
Their 14 source/expected cases cover large, empty and NULL Text/Binary companions,
a non-first generated primary, and external Memo/OLE values with single/chained
bodies plus genuine TD declaration growth. Separate controls exercise exact
copy-allocation limits and a parsed, unqualified `NewValues` property. The opaque
synthetic property value does not establish native mode encoding.

The first compilation stopped before tests on private test-helper visibility;
that correction changed no source or expected database bytes. The companion
source gate initially expected the zero-pair status for a genuinely missing
declaration. Its exact assertion was corrected to `Absent` with no map references,
retaining the distinction from an explicit zero pair. The first combined INSERT
batch passes 278 of 279 functions; its generated external-value case exposes a
missing generation context in long-value preparation. Passing that context through
the bound profile and independent audit corrects the refusal without changing any
source or expected image. A separate fixed 42-byte timestamp plus external-value
regression verifies the checked preflight buffer and rejects a wrong carrier.

Final library verification passes 641 tests with two existing ignored tests,
including 280 INSERT functions. All 16 AutoNumber functions pass, covering signed
counters, companions and private correlated audits; the fixed-42 regression also
passes. The separate example batch passes all 22 tests again after the context
correction; its [input contract](ACE16_INSERT_INPUT.md#explicit-incrementing-autonumber-requests)
describes the explicit flag and companion ordering. Strict all-target/all-feature and
minimal-library Clippy pass. The 76 shared write regression tests also pass.

The original-file SOURCE17 capture and all eight saved raw source gates now pass.
They qualify those immutable source populations and metadata, while recording the
stored mode as unclassified. Eight complete SOURCE/request-only images are frozen
with expected-freeze SHA-256
`9403f2c10b6e581cd6e64013773ff74e498d7a8cb5e966e1653193ae127896ae`.
All 16 public-reference calls pass. The committed producer's eight candidates
match all 3,571,712 expected bytes; eight separate fresh independent reads verify
the resulting rows, counters, aliases, maps and 20 generated IDs. The
[original-file proof](ace16_incrementing_autonumber/README.md) supplies maintained
saved-evidence verification. Public-reference
images remain observations, not expected-image authority. Stored mode qualification
and Microsoft Access save/reopen remain separate from this independent evidence.

Before any fresh-reader call, review caught a report-format mismatch in the
planned checker: it expected the independent writer's returned-value format
instead of the CLI's actual generated-key report. A separately frozen derivative
checks the genuine CLI stream; the original protocol and successful eight native
outputs remain unchanged. No native call was repeated for this correction.

The [committed-producer build receipt](results/ace16-incrementing-autonumber-example-build-2026-10-28.json)
binds the copied engineering example to commit `9e77ffcf`. Its initially absent
isolated release target compiled the owning engine; all 4,702 exported files match
the committed Git blobs. This establishes producer provenance separately from
candidate comparisons and native Access qualification.
