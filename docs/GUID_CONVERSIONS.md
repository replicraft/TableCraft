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

Predicate equality now admits a stored Guid beside exactly sixteen Binary
GUID-structure bytes, including GUIDFromString output; see the predicate contract
below. The existing SQL GUID literal remains a separate typed expression and
strict relational keys retain their carrier distinctions. The function stays
**Partial** in the [built-in tracker](ACCESS_BUILTINS.md).

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

This enables explicit GUID-field-to-Text conversion. GUID/Binary predicate
admission has its separate contract below; no identifier generation, COM
activation or VBA execution is introduced. NULL/error
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

## GUID-structure predicate equality

A contextual scalar predicate can now compare a canonical stored Guid with a
sixteen-byte Binary GUID structure, in either order, using `=` or `<>`. This
allows ordinary native-table filters such as:

```sql
SELECT ID FROM ScalarSamples
WHERE G=GUIDFromString('00112233-4455-6677-8899-aabbccddeeff');
```

Microsoft's [query-callable conversion contract](https://support.microsoft.com/en-us/access/functions/guidfromstring-function)
defines GUIDFromString output as a Byte array and describes stored GUIDs as Byte
arrays as well. TableCraft uses its existing GUID-structure representation to
bridge those expression/storage carriers at the predicate boundary. The first
three canonical groups map to little-endian Data1/Data2/Data3; the final eight
bytes retain order. The existing `uuid` library performs the fixed conversion,
without allocation, Text parsing, providers or a new dependency. Exact bytes and
native Access comparison/error precedence still require Windows qualification.

The ordinary and explicit declared profiles share this predicate service.
Membership, equality quantifiers, joins, HAVING, domains and eligible progressive
filters reuse it. Both operands evaluate before admission; NULL keeps Unknown
before width validation, matching the existing scalar NULL boundary. Wrong byte
widths fail TypeMismatch. GUID ordering remains unsupported. Binary/Binary,
GUID/Text, raw String-byte ranges and arbitrary Variant arrays keep their existing
refusals; a sixteen-byte width alone does not grant a stored Guid type.

Pure `comparison::compare`, strict set/group/sort/extrema keys, result metadata,
source schemas, parameter carriers and stored values do not change. A predicate
match does not rewrite Binary into Guid or enable mixed-key equality. No new
native index hint is extracted from the conversion call. Original filters remain
eager, so false predicates cannot hide a failed GUIDFromString operand.
Cancellation/work checks run before the fixed-size comparison, which allocates
no payload storage. The conversion call retains its own allocation/length budget.

Four expression tests, including a generated independent group-reversal/bit-flip
model, cover bytes, widths, NULL, strict-key separation, work and cancellation.
Five query tests cover composed consumers, parameter reuse, strict group refusal,
eager errors, progressive parity and every observed filter cancellation point.
CLI checks cover eval/run equality and buffered refusals. See the
[verification record](testing/results/guid-byte-predicates-2026-10-25.json).
This closes a concrete predicate-coercion gap; broader Byte-array semantics,
native qualification and full P0 acceptance remain open.

## Native predicate replay — 25 October 2026

The original [GUID predicate runner](testing/compare_guid_predicates.py) passes
192 CLI controls: 156 successful queries and 36 buffered refusals. Eight
independent read-only getter invocations supply the original stored GUIDs. Four
small scalar originals cover Jet 4, ACE 2007, ACE 2016 and ACE 2019; four registered
20,000-row indexed originals cover the same families. Python UUID supplies
identity expectations and Python integers supply final ID ordering. Only
TableCraft executes SQL.

Small-file queries exercise equality in both directions, inequality, list and
subquery membership, equality quantifiers, parameters, runtime domains and
progressive filters in both arithmetic profiles. Malformed bytes, unsupported
ordering, eager conversion errors and NULL constructor calls retain explicit
empty-stdout refusals. The earlier executable retains twenty expected type
mismatches across small and indexed queries. Large cases compare sequential,
required ascending and required descending GUID-index reads under explicit
native/query allowances; they retain the complete original predicate.

The [audited report](testing/results/guid-predicate-replay-2026-10-25.json) records
64,140 returned ID observations, a complete unique command grid, eighteen
unchanged artifact pins and four selected committed/live source snapshots. The
large getters read 80,000 original rows across four files; repeated returned rows
are query observations, not additional authored records. Scalar getter hashes
match the preceding independent numeric replay. Native Access comparison and
Byte-array/type inference remain unqualified. No new fixture, dependency or
performance claim is introduced.

The candidate was built at `819980ce` with SHA-256
`2be6ed81c1839ce7235bf29ce0b6d6e8f5d6aea70da5ac4e20109ff62bf5b9ed`.
The baseline was built at `8c1b3004` with SHA-256
`5a73870b888d58a22314096e761ec3ddec8cfc4ea6bbf7a7e5acda868b46d10c`.
The runner downloads/builds nothing, verifies registered original/tool hashes,
requires a fresh report path and caps CLI observations at 400. Getter heaps are
capped at 256 MiB and individual commands have timeouts. A private diagnostic
report preserves the initial missing TABLE argument to `--index`; the runner
correction did not change runtime code or expected values.

```sh
python3 -B docs/testing/compare_guid_predicates.py \
  --cli .cache/tablecraft-bin/guid-predicates-cli-20261025 \
  --baseline .cache/tablecraft-bin/agreement-result-types-cli-20261025 \
  --jar .cache/tablecraft-fixtures/jackcess-5.0.4.jar \
  --directory .cache/tablecraft-fixtures/scalar-seek-20261023 \
  --indexes .cache/tablecraft-fixtures/large-indexes \
  --revision 819980ce \
  --report FRESH_REPORT.json
```
