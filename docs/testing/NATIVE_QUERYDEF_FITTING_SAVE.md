# Existing QueryDef fitting-expression reference

On 8 November 2026, an original Jackcess 5.0.4 fixture retained one native SELECT
while its stored projection changed from `42` to `43`. The complete independent
raw AFTER image equals the complete SDK-produced AFTER image. This establishes
an expression-only byte oracle for future native save work; **TableCraft has no
QueryDef persistence producer or normal save/recovery route from this evidence**.
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

## Required proof before a native save

A future engine-owned operation must first qualify the actual catalog query,
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
General Save/Save As, new QueryDef creation and cross-profile qualification
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
