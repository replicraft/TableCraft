# Leading-BOM catalog names

A pre-fix CLI reproduction created a fresh database with the table name U+FEFF
and returned success, but inspection failed with `invalid schema name`. The
retained working-tree executable and malformed file are preserved with their
hashes; this reproduction is not a fresh current-source build qualification.

The cause is the distinction between catalog Text and raw definition names.
Catalog readers recognize leading `FF FE` as a compression marker. Writing a
leading U+FEFF table name as ordinary UTF-16 therefore changes its interpretation.
Column-definition names use raw UTF-16 and do not have this marker behavior.

The correction binds a stored-text plan before image allocation. Leading
U+FEFF names use `FF FE 00` followed by unchanged UTF-16 units when every unit has
a nonzero low byte. A zero low byte in that case receives the precise
`TextEncoding` failure; the inspected representation has no reversible encoding
for it. This does not establish that Access cannot store such a name. Raw column
names, including U+FEFF followed by U+0100, remain allowed. Nonleading BOM units
retain the ordinary raw path. No spelling normalization or silent replacement
is introduced.

Four independently authored originals cover U+FEFF alone, mixed Unicode,
the full 64-unit table-name boundary and a raw column-name control. Their native
and SDK source qualification was committed as `dd000ad6` before the repaired
producer ran. Its first standalone `ace16-create` suite passed all fourteen
tests, including eight new complete-image comparisons through both APIs,
populated read-back and preallocation `TextEncoding` refusal; all eleven prior
creator tests remained passing. Eight CLI tests passed, including complete
source-file comparisons and unsupported-encoding refusal without publication.
All-target/all-feature strict checks passed for ACE, Access and the CLI. A fresh
working-tree CLI build then created the leading-BOM file, inspected its catalog
and read its exact TSV header successfully. The file matched the independent
table-bom source completely. An unsupported name returned exit 2 without a file.
This process check used the actual uncommitted repair build, not a committed
source export. These sources and tests confer no
imported-file write or native Access save permission.

Minimal creation compilation, workspace formatting and all 6,885 local
documentation targets also pass. The [acceptance record](results/ace16-catalog-name-encoding-2026-10-31.json)
retains the failed reproduction and separate validation phases.
