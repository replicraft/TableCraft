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

For an Int32 `ID` followed by Binary `Bytes`, a two-column request is:

```text
int32	10001	binary	00ff
```

The accepted kind tokens are lowercase and exact:

| Kind | Data |
| --- | --- |
| `int32` | Signed decimal within the Int32 range |
| `binary` | Even-length hexadecimal; empty means present empty Binary |
| `text` | UTF-8; empty means present empty Text |
| `null` | Empty data only |

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
