# Engineering INSERT request input

The `preview_ace16_insert` example takes four positional arguments:

```text
SOURCE OUTPUT TABLE REQUESTS.tsv
```

Its headerless, quoted TSV contains one complete row per record. Each column
occupies a `KIND` and `DATA` pair, in the table's native column order. Every
record must have the same number of pairs. There are no column names, row
ordinals, omitted values, implicit NULLs or default-value substitution in this
input protocol. The engine independently checks the complete native row shape
and its current mutation profile.

The first Extended Date/Time consumer batch passes all 16 example functions,
including three strict civil-input and validation-before-database-open controls.
This parser evidence does not qualify a new original file or native save.
The later [original-file replay](ace16_insert_extended_datetime/README.md) uses
the same strict token in a separately qualified V2019 source/request/native/read
sequence; it retains distinct phase receipts and does not qualify normal saves.

For an Int32 `ID` followed by Binary `Bytes`, a two-column request is:

```text
int32	10001	binary	00ff
```

The accepted kind tokens are lowercase and exact:

| Kind | Data |
| --- | --- |
| `boolean` | `true` or `false`; NULL Boolean is not a native stored value |
| `byte` | Unsigned decimal in 0..255 |
| `int16`, `int32`, `int64` | Signed decimal within the exact carrier range |
| `decimal` | Signed coefficient and scale as `COEFFICIENT:SCALE`; must exactly match the table declaration |
| `currency-scaled` | Signed Int64 coefficient in units of 1/10,000; no decimal/float parsing |
| `single-bits`, `double-bits` | Unprefixed hexadecimal IEEE bits within 32/64-bit width |
| `date-bits` | Unprefixed hexadecimal 64-bit legacy Date/Time bits, validated by the carrier |
| `extended-date` | Exactly `YYYY-MM-DDTHH:MM:SS.fffffff`, years 0001..9999, seven fraction digits and no timezone |
| `guid` | UUID text accepted by the shared exact GUID parser |
| `binary` | Even-length hexadecimal; empty means present empty Binary |
| `text` | UTF-8; empty means present empty Text |
| `null` | Empty data only |

The other exact scalar kinds use the actual shared `fixed_values` parser. The
`extended-date` boundary parser validates its 27 ASCII bytes and the Core civil
constructor directly; it has no optional fractions, timezone handling or implicit
Date/Time/Text coercion. The backend independently requires the qualified
[ACE2019 Extended Date/Time field](ACE16_EXTENDED_DATETIME_INSERTS.md); parsing the
carrier alone does not admit another source profile. Decimal retains its
coefficient/scale without rounding or conversion. NULL always uses its own
`null`/empty pair; `int32` or `extended-date` with data
`null` is invalid. A `text` pair whose data is the literal `null` remains Text.
The [ordinary-row profile](ACE16_ORDINARY_INSERTS.md) independently qualifies each
carrier, field width and constraint, including explicit empty-Text declarations.

Text and Binary remain distinct from NULL. Standard CSV quoting with a tab
delimiter preserves embedded tabs, line breaks and doubled quotes. Input kind
support does not expand the engine's admitted INSERT field profiles.

The loader captures and validates the complete request file before opening the
source database. Limits are 16 MiB of input, 4,096 complete rows and 1,024 encoded
bytes per value (hex digits count toward this limit). Parsing and native preview
share operation budgets of 256 MiB, 100,000 rows and 100,000,000 steps.

Publication uses the existing engineering example's fresh-output publisher.
An existing output is never overwritten; a failed owned partial output is
removed, with cleanup failure reported. The source is opened for reading.
The result is a separate engineering candidate, with data-macro semantics and
native Access qualification pending. This example adds no normal CLI write path.
