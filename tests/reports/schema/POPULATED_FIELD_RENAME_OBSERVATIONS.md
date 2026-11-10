# Populated field rename cursor observations

The [9 November 2026 record](../../results/schema/populated-field-rename-sdk-read-2026-11-09.json)
checks synthetic, independently authored NumberFirst images containing dense
generated IDs and nullable Int32 companions. Three row sets are observed:
`[42]`, `[42, NULL, -7]` and `[NULL, 42]`. Full input/AFTER images were pinned
before the observer ran. Names cover growth (`TaxAmount`), shrink (`X`),
equal-width change (`Total`) and exact no-op (`Score`).

Jackcess 5.0.4 opened three baselines and twelve requested AFTER files read-only.
These are **twelve distinct images**, because three no-op copies have their
baseline hashes. Each open traversed both the table cursor and its primary-index
cursor. All sixty actual row visits matched exact IDs `1..N`, typed Int32 values
and NULL positions; declared counts also matched. No row/schema mismatch, failed
case or excluded observation occurred.

Column names, stored IDs, native numbers, fixed offsets, generation, field
properties, primary-index identity and explicit OFF settings were checked.
Every AFTER retains baseline metadata and rows except the requested name. All
observed file hashes remain unchanged. The small first-party observer disables
expression evaluation, refuses linked targets and invokes no database writer.
Its runner reuses the existing SDK execution/metadata parser; older observers and
captured pins remain unchanged.

This phase records **zero SDK database writes and no Microsoft Access execution**.
It does not replace whole-image preservation, recovery or host acceptance checks.
The [empty-source damaged controls](FITTED_FIELD_RENAME_OBSERVATIONS.md), including
the stale-descriptor shrink blind spot, remain separate evidence; cursor success
does not establish those stronger invariants. No private corpus or database binary
is published. Commands use sanitized local-resource placeholders, and the record
retains actual source/tool/JAR/image and stdout/stderr hashes.
