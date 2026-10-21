# Native ACE text comparison capture

This engineering recipe prepares original inputs and captures native DAO query
results on Windows. It is not a production dependency, a TableCraft comparison,
an Access application harness or evidence that native comparisons have passed.
The Windows/DAO run remains **unrun**. Portable integrity tests and PowerShell
parser checks are separate evidence.

## Run

Use Windows with Python 3.11+ and a native ACE DAO installation. Match the
PowerShell process bitness to that installation. Start with ACE 16; the runner
records the actual registered DLL build and engine version rather than assuming
the `DAO.DBEngine.120` ProgID identifies a particular release.

From the repository root, choose a fresh directory outside tracked files:

```powershell
python docs/testing/native_text_oracle.py prepare C:\temp\tablecraft-native-text
powershell -NoProfile -File docs/testing/Capture-NativeText.ps1 -Directory C:\temp\tablecraft-native-text
python docs/testing/native_text_oracle.py check C:\temp\tablecraft-native-text
```

The request's exact SHA-256 is checked before executing its SQL. Both preparation
and capture refuse existing output directories. An interrupted run leaves its
partial evidence intact; use a new directory for a retry. There is no automatic
download, dependency installation, existing application open or startup execution.
The runner creates local files through DAO, then reads each closed baseline
through read-only snapshots and verifies unchanged whole-file hashes afterward.

## Inputs and observations

The original recipe has 37 Short Text and 46 Long Text values and 38 query cases
executed against both an unindexed baseline and a separate copy with a Short Text
index. Index presence is an observed difference, not a forced optimizer plan.
Inputs cover NULL versus empty, case, accents/combining marks, expansions,
punctuation, ignored/control characters, Turkish I, kana, supplementary pairs,
trailing spaces, 255-unit boundaries and differing suffixes beyond 1,024 units.

Queries capture stored values, every pair's equality/less/greater results,
ascending/descending sorts with ID tie breakers, DISTINCT, grouping, equijoins,
eleven LIKE patterns and an explicitly typed Text parameter. LIKE patterns are
recorded literally under DAO's actual SQL mode; the recipe does not infer ANSI
mode from the filename or assert that `%`/`_` act as wildcards.

`capture/results.json` records exact UTF-16 units, typed Boolean/integer/NULL
cells, column names, database/field DAO CollatingOrder values, engine/DLL/Windows/
PowerShell versions, process bitness, file sizes/hashes and the original masked
header prefixes. Database files retain the complete physical descriptors for
later `tablecraft inspect` and format comparisons. No normalization, UTF-8
replacement, localized formatting or truncated physical key supplies an expected
SQL result. Query COM failures record HRESULT/DAO numbers without partial rows;
other harness failures abort instead of masquerading as native query failures.

DAO `dbVersion120` requests an ACCDB format; running on ACE 16 does **not** prove
that the generated file has the ACE 16 layout header. Retain the actual files and
inspect their headers. Further native-authored ACE 16/Jet 4/ACE 2007 profiles,
mixed field descriptors, other locales and Access UI/build comparisons remain
additional qualification work. This recipe requests General locale and does not
claim to force either physical General weight version.

## Interpreting evidence

The portable checker requires the exact versioned request/runner hashes, both file phases,
every query outcome, complete pair/ID coverage, original stored values, typed
cells, error codes and matching file hashes. A failed stored-value round trip
invalidates the capture. Other native query errors remain counted as errors,
not successes or skipped compatibility tests. `Evidence complete` means that the
capture envelope is complete; it does not mean its SQL is supported by TableCraft.

Keep generated files outside Git. A report must be reviewed with its creating
command, source revision, recorded build, original request and hashed database
files before it becomes native evidence. Then implement/compare the appropriate
TableCraft full-string policy across each operation. DISTINCT representative
values and grouping equivalence need semantic comparison rather than assuming
one arbitrarily chosen case spelling. No native query collation is enabled by
this recipe. See [the full-string boundary](../NATIVE_TEXT.md#full-string-sql-qualification-boundary).

## Replay against TableCraft

After a complete capture, use the portable replay tool with an explicit current
CLI policy. For example, from the repository root:

```sh
python3 docs/testing/replay_native_text.py /path/to/tablecraft-native-text \
  --cli target/release/tablecraft --collation ordinal --like-mode ansi89 \
  > /path/to/native-text-replay.json
```

The current ordinal policy is an explicit comparison baseline, not native Access
locale support. Choose the LIKE mode deliberately; the tool does not infer it
from the database or reinterpret the captured literal SQL. No new runtime policy
is introduced. Native capture/replay remains unrun locally.

Replay first validates the complete capture, then executes all 38 original SQL
cases against both database phases, including the typed Text parameter. It
compares CSV-decoded TSV display cells with the captured typed cells, preserving
NULL/empty/text distinctions, exact Unicode/control escapes and full Long Text
suffixes. Ordered cases retain their complete row order. DISTINCT compares
unordered multisets; a different representative spelling is accepted only when
the captured pair-equality matrix forms an equivalence partition over the original
inputs and native DISTINCT contains exactly one representative of every class.
Operation-specific DISTINCT behavior or missing equality evidence leaves a
representative difference unresolved rather than inventing an equivalence rule.

Every case remains in the JSON report, with candidate return code and output/error
hashes when available. Native errors remain unresolved even when TableCraft also
fails. Candidate errors, partial stdout on failure, timeouts, malformed transport,
column differences and value/order differences are separate outcomes. Each
process has a 30-second timeout and successful output has a 1 MiB comparison
ceiling. Capture/request/database files and the CLI executable are hashed before
and after replay; a changed protected input invalidates the run. The tool writes
only stdout and refuses incomplete evidence before launching the CLI.

Exit status zero requires every case to match, including any justified DISTINCT
representative equivalence. Nonzero reports include unresolved native errors;
they are not silently skipped. Even an all-matched report qualifies only the
recorded original probes, policy, input descriptors and native build—not all
Access locales, field profiles, LIKE modes or query semantics. Portable synthetic
tests exercise replay mechanics and never count as native SQL comparisons.

## References

Microsoft documents the creation locale/format in
[DBEngine.CreateDatabase](https://learn.microsoft.com/en-us/office/client-developer/access/desktop-database-reference/dbengine-createdatabase-method-dao),
read-only snapshots in
[Database.OpenRecordset](https://learn.microsoft.com/en-us/office/client-developer/access/desktop-database-reference/database-openrecordset-method-dao)
and field sort metadata in
[Field.CollatingOrder](https://learn.microsoft.com/en-us/office/client-developer/access/desktop-database-reference/field-collatingorder-property-dao).
These API contracts justify the capture mechanism, not guessed comparison weights.
