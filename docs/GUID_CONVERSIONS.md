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

StringFromGUID remains unimplemented. Its
[Microsoft method reference](https://learn.microsoft.com/en-us/office/vba/api/access.application.stringfromguid)
defines the inverse application operation, but the reviewed examples do not
establish its direct query admission or exact output envelope. Do not manufacture
that evidence from a synthetic saved query or generic UUID formatting. This does
not assert that the function is unavailable in Access queries.

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
