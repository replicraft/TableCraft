# Access GUID byte-array conversion

`GUIDFromString(text)` now runs in scalar and bound read-only queries.
Microsoft's [function reference](https://support.microsoft.com/en-us/access/functions/guidfromstring-function)
includes a SELECT example and defines the result as a Byte array. TableCraft
therefore returns sixteen Binary bytes, rather than replacing that shape with
its canonical stored GUID carrier. Existing type inspection reports Byte-array
subtype 8209 and IsArray TRUE for this result.

The pure conversion kernel uses the existing `uuid` dependency without default
features. It accepts exactly a hyphenated spelling, a braced spelling or the
Access `{guid {…}}` envelope, with case-insensitive hex/prefix. Compact UUIDs,
URNs, surrounding whitespace, malformed envelopes and invalid Unicode/hex fail.
No identifier generation or ambient provider is involved.

The selected byte representation follows GUID structure order: little-endian
Data1/Data2/Data3 groups and literal final bytes. Microsoft's
[GUID layout reference](https://learn.microsoft.com/en-us/sql/odbc/reference/appendixes/c-data-types)
and `uuid`'s `to_bytes_le` provide the representation basis. This is an explicit
implementation choice pending native Access Byte-array comparison, not proof
that an Access application returned those exact bytes.

```sh
tablecraft query eval "SELECT GUIDFromString('00112233-4455-6677-8899-AABBCCDDEEFF')"
```

The Binary value displays as `0x33221100554477668899aabbccddeeff`. Stored GUIDs
retain their separate canonical representation and existing native codecs.

## Evaluation boundaries

Exactly one operand evaluates eagerly. Invalid arities, missing slots, wildcards
and qualified names fail before evaluation. The library kernel allocates nothing;
the evaluator checks cancellation/work and input ceilings, then charges sixteen
output bytes before fallible allocation. Query results and CLI stdout remain
buffered on errors. Dependency, correlation, aggregate/domain and grouping walks
traverse the explicit conversion node; binding does not parse row values.

Only Text is accepted as input; invalid spellings and wrong carriers return
TypeMismatch. Explicit NULL returns InvalidNullConversion in this profile.
Native NULL, accepted-spelling and error-precedence behavior remain unqualified.

Binary results do not gain implicit comparison/coercion with a stored Guid value.
Consequently this implementation alone does not enable filtering a native GUID
field against GUIDFromString output. The existing SQL GUID literal remains a
separate typed expression. The function stays **Partial** in the
[built-in tracker](ACCESS_BUILTINS.md).

## StringFromGUID inverse conversion

`StringFromGUID(value)` now executes in scalar and bound queries. The existing
`uuid` dependency formats mixed-endian GUID-structure bytes; the fixed kernel
returns ASCII without allocation. The evaluator accepts exactly sixteen Binary
bytes or the canonical stored Guid carrier. Stored fields are explicitly mapped
to GUID-structure bytes at this conversion boundary; their source/storage carrier
is unchanged. Text, other scalar carriers, byte strings and Variant Array inputs
are not implicitly reinterpreted as a GUID. NULL returns InvalidNullConversion;
wrong carriers/widths return TypeMismatch.

The selected output is uppercase hexadecimal inside the literal envelope, for
example `{guid {00112233-4455-6677-8899-AABBCCDDEEFF}}`. It is 45 ASCII bytes.
The [Microsoft function reference](https://support.microsoft.com/en-us/access/functions/stringfromguid-function)
and [method reference](https://learn.microsoft.com/en-us/office/vba/api/access.application.stringfromguid)
define the inverse operation and Byte-array argument. They do not establish the
exact emitted envelope/case or directly demonstrate query admission. The output
spelling and query exposure are an explicit provisional TableCraft profile;
implementation/model checks are not native evidence. No Access-generated saved
query or live Access comparison has been added to qualify those choices.

The evaluator checks cancellation/work, output Text ceilings and shared bytes
before fallible allocation. Its one argument evaluates eagerly; invalid arities,
missing slots, wildcards and qualified names fail before operand evaluation.
Binding traverses the dedicated child node in grouping, aggregate, correlation,
collection and named-query dependency walks. The result has fresh host-default
text provenance rather than inheriting a source field's text policy. CLI output
uses its existing quoted Text display, independently of the conversion kernel.

```sh
tablecraft query eval "SELECT StringFromGUID(GUIDFromString('00112233-4455-6677-8899-aabbccddeeff'))"
tablecraft query run "SELECT ID,StringFromGUID(G) FROM ScalarSamples WHERE G IS NOT NULL" --database example.accdb
```

This enables explicit GUID-field-to-Text conversion, not implicit GUID/Binary
comparison, identifier generation, COM activation or VBA execution. NULL/error
precedence, wider argument coercion, saved-query/application behavior and exact
native output remain unqualified. StringFromGUID stays **Partial**.

## Evidence

Kernel tests include an independent mixed-endian golden, accepted/refused shapes,
a Unicode boundary regression and a fixed-seed 256-case group-reversal property.
Evaluator tests exercise fixed output charges, work limits, cancellation and NULL.
Query/CLI tests cover binary shape/subtype, parameters/correlations, aggregates,
all observed cancellation checkpoints and buffered failures.

The original [independent runner](testing/compare_guid_conversion.py) compares
bytes against Python UUID's independent implementation and checks unchanged
original SavedForest-derived native files. It does not execute Microsoft Access
or add saved GUID conversion fixtures. Native qualification remains pending.

The inverse adds an independent mixed-endian output golden and a fixed-seed
256-case integer-hex/group-reversal model. Evaluator tests check exact 45-byte
charges, narrower Text ceilings, invalid Binary widths/carriers and cancellation.
Bound tests retain parameter/aggregate/group/correlated dependencies and every
observed cancellation checkpoint. CLI controls exercise both commands and empty
stdout on late failures.

The original [inverse runner](testing/compare_string_from_guid.py) passes 1594
provisional Text/byte-order/type/refusal controls. Python UUID supplies an
independent formatting and bytes_le model; reviewed getters supply GUID fields
from four unchanged original ScalarFixture files with expression evaluation
disabled. This is independent data/model evidence, not native StringFromGUID
execution. No third-party code or fixture was copied.
