# Original template scalar profile replay

The [accepted report](results/template-scalar-profiles-2026-10-25.json) records
seven successful CLI queries, 14 independently modeled rows and 44 compared stored
cells. Python's standard-library ZIP and XML readers derive expected cells from
unchanged `data.xml` parts; only TableCraft executes SQL. This qualifies the
engineering stored-data paths exercised here, not Access-created template
instantiation or native Access application behavior.

The replay uses the immutable CLI compiled from
`1286af1ea0ea39c0d350c4d64bb235fb8132f196`, SHA-256
`398b9c4f4b9e41521c2fd70750b5f946d5fcfa38133f3f66304f11c92ea91808`.
It predates the exact XML lexical-name correction. The Chinese field-name case
already succeeded in the initial audit and remains a regression control.

## Inputs and provenance

These seven original, repository-owned synthetic ZIP/XML packages were authored
in the storage-read audit using the package/table/List Definition structures in
[compare_template_packages.py](compare_template_packages.py),
[compare_template_tables.py](compare_template_tables.py) and
[compare_template_lists.py](compare_template_lists.py). The historical per-variant
construction steps were not retained as a byte-reproducible generator. The
hash-locked original files are the replay inputs; this runner never regenerates
or changes them. No third-party fixture bytes or additional binaries are added.

Files remain under ignored `.cache/storage-read-audit-20261025/`. Their complete
hashes, raw `data.xml` hashes, actual stdout and expected cells are retained in the
report.

| Original input | SHA-256 | Projection |
| --- | --- | --- |
| `typed-utf8.accdt` | `95adcfbebc6e75ac46a83b3be4a39082cd64c1c2a504e93c4038b79f321ce55d` | ID, Full Name, Tiny, Large |
| `typed-utf16.accdt` | `871d72e6bb2dc40518c6aa6491d2d24546d48f808a6ce2eb85dbb852e915937c` | ID, Full Name, Tiny, Large |
| `typed-local-named-type.accdt` | `3a9759e2565159852fdd59ee6767bbe2e0b12f30d3f10dc2009db3cbbfe55e27` | ID, Full Name, Tiny, Large |
| `list-note.accdt` | `2ad73278023bed67c7471a806b58ed5184d0982606f2b4acab11a47308e1e344` | ID, Notes, Full Name |
| `list-single-choice.accdt` | `76de18416ff4297bf7b3ef11f4bb005195ddd54114eac0a9360f9fd277299f90` | ID, Notes, Full Name |
| `list-single-choice-valid-default.accdt` | `4e56add6c11ece32b61b70d78471107c00b43dd40270346d28029d43068dc67c` | ID, Notes, Full Name |
| `unicode-ncname.accdt` | `cd6349cafd5a1c670507ed2604671f715a8e4a0542b36fb7bee8cc16bf179bd6` | 编号 |

## Independent expectations and observations

The model reads each package's table name from `metadata.xml` and projected cells
from `data.xml`. Known integer fields use Python exact integers; present empty
Text elements become empty strings, and XML references decode through ElementTree.
Rows sort by the first integer field, matching the explicit SQL `ORDER BY`.
The model never uses declared record counts, CLI results, defaults or options as
its value oracle. CSV decoding handles TSV transport before comparing ordinary
scalar display strings.

All three typed inputs preserve `-128`/`127`,
`-9223372036854775808`/`9223372036854775807`, leading/trailing spaces and the Unicode
string ` A&🦀 `, plus a genuinely empty second-row String. All three List inputs
project actual `Notes` values `Data & <inert>` and both Full Name strings.
Choices, defaults and navigation metadata do not substitute or constrain these
stored values. This includes the exploratory Choice package whose default is not
an option; the replay does not qualify interactive default or membership behavior.

The report pins 12 immutable inputs/tools before and after the replay: the CLI,
seven original packages, the new runner and three original authoring-basis
helpers. Thirty scoped template/CLI Rust files from the compiled revision are independently read with
`git show` at the frozen revision, with hashes checked again after execution.
These revision-based hashes do not claim that the moving live source tree was
frozen or that every Rust source or test was validated by these seven commands.

The earlier audit reports remain unchanged and separate. Their UTF-16, local
named-type and Choice refusals describe the older CLI. None of those failures is
counted as a passing check here; the fresh report contains seven actual successes.

## Reproduction

```sh
python3 docs/testing/compare_template_scalar_profiles.py \
  --cli .cache/tablecraft-bin/japanese-string-cli-20261025 \
  --directory .cache/storage-read-audit-20261025 \
  --report docs/testing/results/template-scalar-profiles-2026-10-25.json
```

The accepted report already exists. Use a fresh report filename when repeating
this command; the runner refuses overwrites, unknown CLI bytes or changed
originals. Every subprocess has a 120-second timeout. No Cargo build, network,
provider opening, template instantiation or application code execution is needed.
The runner was syntax checked with Python and passed its first seven-command
replay without corrections. Its report preserves disagreements if any occur in a
future run.
