# Passive display metadata in ACE 16 write previews

The engineering write gate now qualifies existing column `Format` and numeric
`DecimalPlaces` declarations without rendering, rounding or replacing stored
values. This applies to the ACE 16 layouts recognized as Access 2016 and 2019.
Normal Jet 4 writes and normal ACE saves retain their existing admission rules.

The declaration must name an actual field. `Format` requires native TEXT (`0x0a`)
with valid decoded text. `DecimalPlaces` requires native BYTE (`0x02`), exactly one
byte, containing 0 through 15 or 255 for Auto. Only property flags zero and the
known DDL flag (`0x01`) are admitted. Table, index, unknown-field and unsupported
carrier/value/flag combinations remain refused. The raw property map remains
unchanged in every candidate.

The current `Format` profile covers Byte, Int16, Int32, Int64, Currency, Single,
Double, Decimal, Boolean, Short Text and legacy Date/Time fields. Numeric
`DecimalPlaces` covers the numeric families in that list. Decimal values retain
their declared physical precision and scale; display places never determine the
stored coefficient or permit a mismatched carrier. A retained Format declaration
on a generated Int32 field does not identify its generation mode or change the
explicit incrementing policy.

## Basis and boundaries

Microsoft documents field Format as presentation that does not change stored
data. The engine preserves that declaration and never invokes a formatting
function during a write. [Format property](https://support.microsoft.com/en-us/access/format-property)

Microsoft separately documents numeric DecimalPlaces as a Byte with values
0–15 or 255, inherited from table fields by bound controls. It affects displayed
digits rather than stored precision. [Numeric DecimalPlaces](https://learn.microsoft.com/en-us/office/vba/api/access.textbox.decimalplaces)

Extended Date/Time has a separately documented Decimal Places precision setting
of 1–7. It is excluded from this numeric display profile. Memo and Extended
Date/Time Format await their own source controls; Binary, GUID, OLE, complex and
unknown-field display declarations remain outside this slice.
[Field properties by data type](https://support.microsoft.com/en-us/access/introduction-to-data-types-and-field-properties)

The pinned independent writer's `PropertyMapImpl` binds `DecimalPlaces` to BYTE
and `Format` to TEXT, both with the DDL flag. This carrier evidence supports the
exact MR2 profile; it does not establish native Access save compatibility.

Required, AllowZeroLength, validation, generated-column, relationship, index and
active-content qualifications still apply. NewValues remains unqualified; no
numeric enum, counter sign, property absence or Format declaration proves an
Increment/Random mode. InputMask, UnicodeCompression, Lookup/RowSource and other
unqualified properties remain refused. No defaults or omitted values are
evaluated, and no UI formatting behavior is implemented by this admission.

## Independent controls and verification

The [rich scalar source owner](../../crates/engines/tablecraft-ace/src/table/mutation/insert/tests/display_metadata.rs)
reuses the actual independent MR2 and ordinary-row fixture builders. It constructs
complete expected INSERT, fixed UPDATE, text-slot UPDATE and DELETE images before
any new API call. Cases include both layouts, property flags zero/one, all four
numeric display settings 0/2/15/Auto, exact supplied values, NULL, empty text and
empty inserts. Text Format `>` must preserve mixed-case stored text. DELETE
expectations preserve raw record bodies, physical slot identities, inactive key
tails, historical counters and property bytes.

The generated-column controls use the actual non-first primary field and LAST
260. The [Decimal controls](../../crates/engines/tablecraft-ace/src/table/mutation/insert/tests/decimal_index_inserts/display_metadata.rs)
retain precision 28 and scale 28 under display places 0/15/Auto, including exact
coefficient/NULL records and wrong-scale refusal. Separate API controls cover
negative scopes/carriers/flags/values, unrelated properties, exhausted budgets
and cancellation at the last poll of a measured healthy run.

All three source/expected qualification functions passed on their first actual
run before the qualifier or API controls were registered. The seven API controls
then passed on their first actual runtime, together with the three source
functions. The first API build stopped before tests because a DELETE assertion
used the UPDATE result accessor; only that assertion changed to the actual
`deleted_rows()` accessor. No source, expected image or engine behavior changed
for that correction.

The INSERT library regression passes 306 functions and five existing shared
integration targets pass 43 tests. Strict Clippy initially identified a repeated
test fixture module and an unnecessary one-element clone. Both display consumers
now import the actual existing fixture owner through narrow test visibility;
the clone became `slice::from_ref`. Named display constants and a direct error
import completed the mechanical cleanup. All ten focused functions pass again
after those changes, with unchanged fixture/golden bodies.

Two representative executable source gates pass before the new CLI actions.
Their initial build stopped before tests because integration-root child modules
needed explicit paths; only those registrations changed. Both new executable
API functions then pass, comparing three complete candidate images: a non-first
generated INSERT with Format, a mixed-case text UPDATE with Format `>`, and a
DELETE preserving property and inactive record bytes. These ACE2019 controls
reuse the actual existing SQL INSERT and C4/V2 fixture owners. The engine matrix
separately covers both ACE16 layouts and Decimal scale 28.

The full CLI round passes 31 unit tests and 33 integration tests. Strict ACE/CLI
Clippy over all targets/features and minimal ACE/Access builds pass. No source or
golden was repaired following a producer result. The retained compile/lint
corrections do not represent failed mutation runtimes.

No native Access open/save comparison or original-file mutation qualification is
claimed by these literal engineering checks.
