# Create a fresh ACE database

## Normal editing after creation

Fresh output now uses the seventeen-field catalog required by ordinary normal
INSERT/DELETE admission. Eligible created schemas can use the existing
`query journal-insert` and `query journal-delete` commands through fresh content
qualification and retained offline sessions. No creator token or implicit edit
permission is passed.

A focused process test creates an Int32 file, inserts `(2,20)` and deletes that
new row, matching independent whole images and exact SELECT readbacks after each
save. The existing creation publication/report contract is unchanged. Complete
INSERT carrier limits and field policies still apply; creation of a broader
scalar schema alone does not qualify all its normal writes. Future GLOBAL FREE
metadata, property/default/AutoNumber authoring and native Access save/reopen
remain separate compatibility work. See the
[creation/session contract](../access/writes/FRESH_DATABASE_CREATION.md#current-catalog-and-normal-session-consumption).

`tablecraft create` constructs a fresh unencrypted ACE 2016 database. Explicit
`--blank` creates system metadata with no user tables, fields or records:

```sh
tablecraft create new.accdb --blank
```

This directly serializes the blank native image; no sample or template is copied.
`--blank` conflicts with `--table`, `--column`, `--field`, `--data`, `--primary-key`
and `--incrementing`. Otherwise `--table` and exactly one column/field schema mode
remain required, producing one explicitly declared user table. Existing files,
directories and symlinks are never overwritten. `--password` remains unsupported
in both modes; creation produces an unencrypted file.

Both modes use the same required-publication service. Unix acknowledges file
and captured-directory barriers; Windows uses a local-NTFS write-through rename
and retained-file flush. Unsupported locations refuse before staging. Actual
Windows execution and native Access open/save qualification remain pending.

Use `--column` for the original present-Int32 input, or the separate `--field`
schema below for typed values. The modes are mutually exclusive; both use the
same [shared creation service](../access/writes/FRESH_DATABASE_CREATION.md#typed-scalar-preparation).

```sh
tablecraft create new.accdb --table Items --column Id --column Score
```

Without `--data`, the table starts empty. To supply rows, create a tab-separated
file whose first record matches the declared column names exactly, in order:

```text
Id	Score
1	10
2	20
```

```sh
tablecraft create populated.accdb --table Items \
  --column Id --column Score --data rows.tsv
tablecraft rows populated.accdb Items
```

The displayed TSV contains literal tabs. CSV-style quoting is supported, including
doubled quotes inside quoted fields. Header spelling and case must match the
column arguments. With `--column`, every data cell is a present decimal Int32 from -2147483648
through 2147483647, with an optional minus sign. NULL, plus-prefixed numbers,
expressions and surrounding whitespace are refused. A header-only file creates
an empty table. TSV capture uses the actual file length under the cumulative
`--work-bytes`, `--work-rows` and `--work-steps` allowances; it has no separate
64 KiB input ceiling. A changing input length is refused before publication.

The current native creator uses fixed layout/key proof storage for one through
sixteen columns and at most 64 rows,
with an additional physical fit check. A wide schema can fit fewer rows; a
request that does not fit is refused before publication. Names are validated by
the native constructor and limited to 64 UTF-16 code units. Column order is the
written schema order. All values are supplied manually. Without `--primary-key`, a name such as
`Id` creates neither a primary key nor AutoNumber.

```sh
tablecraft create people.accdb --table 'People Stats' \
  --column 'Person Id' --column 'Score Δ'
tablecraft create --help
```

## Explicit manual primary key

Add `--primary-key COLUMN INDEX_NAME` once to either schema mode:

```sh
tablecraft create keyed.accdb --table Items \
  --column Id --column Score --primary-key Id PrimaryId --data rows.tsv
```

The selector matches exactly one declared column under ASCII case folding. The
index name is explicit and validated by the native constructor. Int32, Int64 and
Currency are supported manual key declarations; native constraints reject NULL
or duplicate keys before publication. A header-only file or omitted `--data`
creates an empty table with the declared key. Names and LAST values never infer
AutoNumber or generated values.

`--column` retains present decimal Int32 parsing. Use `--field` for exact Int64
or `currency-scaled` carriers; Currency input is signed integer units of 1/10000.
Typed NULL remains NULL and fails the primary key constraint. No SQL coercion,
default or generation policy is added. Repeated/incomplete key flags, missing or
ambiguous column selectors, unsupported declarations and native name/key errors
publish no file. Existing destinations are never replaced.

The CLI still creates one table. Multiple-table creation remains available
through the shared library service; this flag adds no repeated-table syntax.

## Explicit typed schema

Repeat `--field NAME DECLARATION` instead of `--column`:

```sh
tablecraft create typed.accdb --table Items \
  --field Id int32 --field TextValue text:8 \
  --field BinaryValue binary:16 --field DecimalValue decimal:28:4 \
  --field Flag boolean --data typed.tsv
```

The TSV header still matches every name exactly and in order. For example:

```text
Id	TextValue	BinaryValue	DecimalValue	Flag
1	value:null	00ff	12345:4	true
2	null	value:	null	false
```

The first Text is literal `null`; the second is stored NULL. The second Binary
is present empty. Without `--data`, the explicitly declared table starts empty.
The same input, row, column and physical-fit ceilings apply.

| Declaration | Exact cell representation |
| --- | --- |
| `boolean` | `true` or `false`; NULL refuses |
| `byte`, `int16`, `int32`, `int64` | Exact integer carrier |
| `currency-scaled` | Signed integer in units of 1/10000 |
| `single-bits`, `double-bits`, `date-bits` | Hexadecimal bits without `0x`; DATE bits must be valid |
| `guid` | UUID spelling |
| `text:N` | Text limited to N UTF-16 code units |
| `binary:N` | Complete hexadecimal byte pairs, limited to N decoded bytes |
| `decimal:P:S` | `COEFFICIENT:SCALE`, matching declared precision and exact retained scale |

Text and Binary dimensions are explicitly 1 through 255 in the current creation
profile. Decimal precision is 1 through 28 and scale is zero through precision.
There is no inferred capacity, numeric coercion, truncation or Decimal rescaling,
even for empty or all-NULL input. UTF-16 code units differ from Unicode scalar
characters and encoded payload bytes. See the [native declaration and encoding
rules](../../tests/reports/creation/ACE16_DATABASE_CREATION.md#explicit-variable-declarations).

In typed mode, exactly lowercase `null` means stored NULL. Prefix a literal with
`value:` to bypass that keyword; `value:null` is literal Text `null`, and
`value:value:null` is literal Text `value:null`. Empty Text/Binary cells or
`value:` supply present-empty values. Decimal zero retains its supplied scale.
There is no omission/default marker or expression evaluation. The typed schema
uses the existing exact write value spellings; it does not enable engineering
editing or expand existing-file write admission.

Creation rejects `--password`; it does not generate an encrypted file. The required
publication guarantee must be supported by the platform and filesystem.
Unsupported backends refuse before creating a file, with no durability downgrade.
See [durable publication](DURABLE_OUTPUT.md).

Input parsing and native construction complete before any file is staged. The
completion report is allocated before publication and emitted only after the
platform durability operation acknowledges. The report names that operation. Errors distinguish no known
publication, uncertain visibility and a published file with incomplete terminal
work. A completion-output error retains the fact that the file was published.
Do not automatically repeat creation or remove a final file to resolve those
errors.

`--work-bytes`, `--work-rows` and `--work-steps` bound cumulative work. Their
defaults are 256 MiB, 100,000 rows and 100,000,000 steps. Increasing these
allowances does not raise the column, row, input-file or physical-fit ceilings.

This fresh construction profile does not authorize normal updates to imported
ACE databases, certify arbitrary files as macro-free, or establish native Access
creation/save compatibility. Existing [normal write limits](../access/writes/NATIVE_WRITES.md)
remain separate.

## Explicit Increment primary

An empty table can explicitly declare its first Int32 primary field as Increment
AutoNumber. One through sixteen constructible scalar fields are admitted:

```sh
tablecraft create fresh.accdb --table Items --column Id --column Score \
  --primary-key Id PrimaryId --incrementing Id
```

An initial ID-only table uses the same operation:

```sh
tablecraft create initial.accdb --table Table1 --field ID int32 \
  --primary-key ID PrimaryKey --incrementing ID
```

Companions may use the existing fixed, Short Text, Short Binary or Decimal
creation declarations. No initial rows, defaults or field policies are admitted
in this Increment profile. This constructs a fresh file; it does not add a table
to an existing blank database or widen generated INSERT eligibility.
Later omitted-ID INSERT uses actual native metadata, checked LAST arithmetic and
the normal journal service. Supplied IDs/NULL are never turned into omissions.
Random generation and reseeding remain unsupported. See the
[creation contract](../access/writes/INCREMENTING_CREATION.md).
