# Create a fresh ACE database

`tablecraft create` constructs a fresh unencrypted ACE 2016 database containing
one table of ordinary Int32 columns. Existing files, directories and symlinks
are never overwritten.

The shared Access typed creation API is separate. This command continues to use
the Int32 convenience path: `--column` supplies names, and TSV does not gain
additional scalar kinds or a NULL token. See the [shared service](../access/writes/FRESH_DATABASE_CREATION.md#typed-scalar-preparation).

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
column arguments. Every data cell is a present decimal Int32 from -2147483648
through 2147483647, with an optional minus sign. NULL, plus-prefixed numbers,
expressions and surrounding whitespace are refused. A header-only file creates
an empty table. The file must be at most 64 KiB.

The current profile supports one through sixteen columns and at most 64 rows,
with an additional physical fit check. A wide schema can fit fewer rows; a
request that does not fit is refused before publication. Names are validated by
the native constructor and limited to 64 UTF-16 code units. Column order is the
written `--column` order. All columns are manual Int32 values: a name such as
`Id` does not create a primary key or AutoNumber.

```sh
tablecraft create people.accdb --table 'People Stats' \
  --column 'Person Id' --column 'Score Δ'
tablecraft create --help
```

Creation rejects `--password`; it does not generate an encrypted file. Required
captured-directory durability must be supported by the platform and filesystem.
Unsupported backends refuse before creating a file, with no durability downgrade.
See [durable publication](DURABLE_OUTPUT.md).

Input parsing and native construction complete before any file is staged. The
completion report is allocated before publication and emitted only after the
file and captured-directory barriers acknowledge. Errors distinguish no known
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
