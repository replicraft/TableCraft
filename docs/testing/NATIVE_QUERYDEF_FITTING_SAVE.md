# Existing QueryDef fitting-expression reference

On 8 November 2026, an original Jackcess 5.0.4 fixture retained one native SELECT
while its stored projection changed from `42` to `43`. The complete independent
raw AFTER image equals the complete SDK-produced AFTER image. This establishes
an expression-only byte oracle. The engine now has an unpublished candidate
planner for the closed profile below; **no runtime, CLI or GUI QueryDef save route
is supplied by this checkpoint**.
Native Microsoft Access save/reopen remains unrun.

[The observation record](results/querydef-fitting-expression-2026-11-08.json)
retains hashes, raw field/record spans, exact native fragments, independent
rendering and explicit TableCraft execution. The original
[fixture recipe](ConstantQuerySaveFixture.java) creates fresh V2016 outputs
using the existing reviewed system-table/`QueryImpl.Row` recipe. Expression
evaluation stays disabled. Saving stored metadata executes no query, macro, VBA,
UDF or external provider. Explicit read-only TableCraft retrieval subsequently
returns `42` and `43`; those executions are separate observations.

## Observed physical change

The source has 90 pages of 4,096 bytes (368,640 bytes). Its `Constants` query has
native ID `-2147476648`, catalog Type 5 and Flags 0. Five MSysQueries fragments
retain their original identity and four-byte ordering keys: START, TYPE 1, FLAGS
0, one COLUMN with alias `Answer`, and END. The query has no parameters, table
sources, owner-access option or stored provider selectors.

| Observation | BEFORE | AFTER |
| --- | --- | --- |
| Reconstructed SQL | `SELECT 42 AS Answer;` | `SELECT 43 AS Answer;` |
| SHA-256 | `83f9d69408948d64eab4dbeafcca468116b5624bdf6b9510569640497b487779` | `6f4269175635cccea5d61fae371899d1efa06a1f79a78172efa75539792f6693` |
| Projection row, half-open file span | `[368492, 368550)` | unchanged |
| Expression field, half-open file span | `[368521, 368537)` | unchanged |
| Inline Memo field | `04000080000000000000000034003200` | `04000080000000000000000034003300` |
| Changed file byte | — | offset 368535: `0x32` → `0x33` |

The 16-byte Expression carrier contains a little-endian `0x80000004` inline
length/tag, eight zero bytes and four bytes of uncompressed UTF-16LE text.
Its variable-field ordinal is 3. The complete 58-byte row includes the original
fixed fields, ordering key, alias, variable-offset trailer and NULL mask.
Independent raw construction requires the exact original field bytes before
replacing that literal. It changes exactly one byte on DATA page 89. No other
file byte changes, including catalog records, dates, index pages, allocation
maps, row counts, record directories and protected tails.

MSysQueries remains definition page 4 with its required unique
`ObjectIdAttribute` index rooted at page 12, ordered by ObjectId/Attribute/Order.
All five fragment keys and row references remain unchanged. The catalog's Id and
ParentIdName indexes likewise retain every byte. This observation avoids index
mutation only because identity, clause membership, order and row placement are
identical; it does not qualify arbitrary expression growth, fragment insertion,
query creation or index rewriting.

## Independent raw fixture

The testkit's
[`saved_queries::fitting_select`](../../crates/engineering/tablecraft-testkit/src/fixtures/access/saved_queries/fitting_select.rs)
constructs a complete synthetic BEFORE/AFTER pair through the existing raw
saved-query builder. Its query retains that builder's independent native ID and
name. A literal 58-byte golden matches the SDK-observed projection record after
explicitly substituting the synthetic ID; the pair differs only in the literal
payload byte. No production serializer, mutation planner or query evaluator
constructs the expected AFTER.

These synthetic images intentionally lack the SDK source's native system
indexes. They are byte oracles for the fitting expression, not imported-source
eligibility witnesses. The private SDK pair retains the real system indexes;
no complete database bytes are tracked. Fixture provenance is recorded in
[FIXTURES.md](FIXTURES.md#existing-querydef-fitting-expression-reference).

## Unpublished native candidate checkpoint

With the native engine's `journal` feature, the sealed
`NativeImage::plan_query_literal_expression` returns an owning
`NativeQueryExpressionPlan`. It retains complete BEFORE/AFTER images, original
name/native ID and expected/replacement literal. This candidate alone has no
journal preparation or mutation authority.

The separate engine-only `plan_recoverable_query_literal_expression` captures a
closed `TCQE0001`/profile 1 original intent. Its eighty-byte header retains the
exact signed query ID, Tables parent, catalog page/slot, full original length and
source digest; bounded trailing fields retain the original query name and
expected/replacement literals. It stores no final fragments or patched AFTER.
Other action prefixes and unknown versions/profiles refuse without fallback.
Fresh requalification spends independent cumulative native/proof pools and
reconstructs the complete candidate through the same strict planner. The same
retained checked V1 journal is qualified against both complete rebuilt images
before recovery or settled confirmation. Generic prepare/commit/rollback require
caller-owned exclusive offline resources and durable journal namespaces; no
runtime, CLI or GUI query-save consumer is implemented. The shared offline
adapter below now composes the retained filesystem lifecycle.

The initial source profile is unencoded ACE 16 code 5/subversion zero with one
saved query and five fragments, the observed `Answer` alias, and a source-free
canonical nonnegative Int32 decimal literal. Replacement must have the same
UTF-16 payload length. Parameters, other expressions/aliases, multiple query
objects, compiled/cache/property/owner payloads, unknown catalog carriers,
forwarded/deleted rows and unqualified storage refuse. The full seventeen-field
catalog must explicitly represent nullable query bodies; a shortened catalog
does not establish their absence. Exact no-ops undergo the same qualification.

The planner checks complete original catalog/query index membership using the
retained index checker. It checks native DATA/definition/index/map/LVAL roles
against the complete local catalog, global-free polarity and live LVAL links.
It replaces only the direct inline Memo payload and checks the entire byte
complement. A fresh complete candidate capture repeats semantic/storage/index
qualification and must resolve the same physical span, name and native ID.
Neither stored query functions nor active content execute.

A new raw nine-page fixture supplies full nullable catalog carriers and distinct
represented map cells while retaining the independent one-byte AFTER. Focused
cases cover that complete image, no-op/source qualification, literal/original/
capacity refusals, another table's DATA claim, cancellation and exhausted budgets.
An explicit ignored SDK test pins the reference BEFORE/AFTER hashes. Seven
original-intent cases additionally cover independent wire bytes, malformed
records before allocation, cross-action refusal, stale/tampered requests,
separate quotas/cancellation, pending/committed/torn-marker recovery and rejected
patched AFTER images without target writes. The focused engine gate passes five
candidate cases, seven original-intent cases and the explicit hash-pinned SDK
BEFORE/AFTER comparison. These results do not qualify Microsoft Access saves or
broader source eligibility.

## Shared offline adapter checkpoint

`OfflineWriteSession::stage_reviewed_query_expression_with_budgets` accepts the
engine's reviewed original-request plan, captures fresh bytes on its retained
exclusive target and reconstructs under separate native/proof pools. Hosts finish
review/confirmation before acquiring that lease. Staging creates no journal;
consuming apply encodes TCQE and uses the same retained target/pair lifecycle.
Failures keep that owner and its original phase/outcome information.

The adapter dispatches TCQE before decoding other actions and never retries a
failed query decoder as a row or schema action. Active, cold and settled recovery
use the same checked V1 session after complete native reconstruction. Completion
uses a distinct `QueryChange::ExpressionSaved` fact, with no matched/changed row
counts or table-schema change. Rollback and Original-image confirmation report no
query change; committed/confirmed Replacement can retain the acknowledged fact.
The shared apply path selects mutually exclusive row, schema or query facts.

Five focused adapter cases are authored for apply/no-op, source drift and
independent quotas/cancellation, retained/cold/settled outcomes, closed decoder
selection and rejected patched AFTER. At this checkpoint those adapter cases are
**unrun**. Runtime query review/save, scratch SQL lowering, generic query creation,
GUI Save/Save As and broader native profiles remain later work.

## Required proof before a normal native save

The candidate requires the following source qualification; broader native saves
must retain and extend it. An engine-owned operation must first qualify the actual catalog query,
complete selected fragments, original inline Memo framing and physical owners.
It must retain the exact signed native ID, Tables parent, original name and
properties. This fixture does not supply a complete audit for hostile sources,
shared/forwarded rows, overlapping DATA/LVAL/index roles, unrepresented maps,
unknown properties, permission defaults or Name AutoCorrect behavior.

For this narrow topology-preserving operation, proof must show that the resolved
projection's ObjectId/Attribute/Order key and physical ROW reference do not change,
that the new Expression fits its admitted existing span, and that all field/row
framing outside the exact payload remains unchanged. Independently reconstruct
and compare the complete AFTER, verify unaffected fragments and all original
index membership/counts, and reject corrupt or unsupported sources before a
candidate is published. A similar-looking or equal-length payload alone cannot
authorize a write.

An eventual normal save needs its own closed original intent: exact query
identity, original definition and requested replacement, bound to the complete
BEFORE length/digest. Fresh apply and active/retired recovery must rebuild the
same native plan from original bytes through the same checked admission. Stored
final rows or arbitrary AFTER bytes are not authority. Keep existing update,
insert, delete, table-create and rename intent formats unchanged.

The Access adapter owns checked SQL-to-native lowering and model round-trip
comparison; ACE owns storage qualification. Saving must retain declared
parameters, query-callable expression syntax and definition ownership without
binding parameter values, scanning sources or changing external/named/domain
namespaces. The constant-only reference does not qualify those broader shapes.
Normal Save/Save As, new QueryDef creation and cross-profile qualification
remain separate implementation work. Creating a query in an existing SDK source
also inserts catalog/fragment records and maintains three native indexes; the
one-byte save observation does not remove that requirement. The current
TableCraft-authored canonical creator does not yet include MSysQueries.

## Reproduce the independent reference

Use the already reviewed Jackcess 5.0.4 JAR and two fresh private output paths:

```sh
java -cp path/to/jackcess-5.0.4.jar docs/testing/ConstantQuerySaveFixture.java \
  before.accdb after-sdk.accdb
```

The recipe preserves the original source hash across the explicit SDK metadata
update and uses read-only observation for reconstruction. Its fixture is
independently generated, not Access-authored. No native TableCraft writer, normal
journal, GUI save or native Access interaction is exercised by that command.
