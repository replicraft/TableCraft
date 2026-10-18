# Reusable import/export tasks and linked sources

Status: future requirements, reviewed 18 October 2026. No adapters, saved task
reader, job runner or import/export dialogs are implemented.

## Tasks and ownership

Treat reusable operations as a headless service with optional visual designer,
CLI and automation entry points. Connector/format modules own access; lifecycle
services own conversion/job orchestration; runtime supplies DoCmd integration.
Keep job configuration separate from secrets and platform UI.

Preserve native saved specifications and unknown metadata. Distinguish saved
import/export operations, text-format specifications used by transfer APIs and
linked-table definitions. Microsoft excludes linking operations from the saved
import/export workflow. TableCraft may offer reusable link-management tasks as a
separately labeled capability, without claiming native saved-spec compatibility.

Jobs need source/destination identities, mappings, encoding/delimiter/header and
locale policies, type conversion, keys, append/replace/new-table mode, filters,
overwrite policy and format options. Preview target changes and losses before
execution. Re-running is explicit; never retry writes or duplicate append rows
silently. Define cancellation, transaction scope, partial completion, row errors
and resume/idempotency only where actually supported.

Future DoCmd.RunSavedImportExport and transfer methods must resolve qualified
native names/options. Inspection cannot schedule jobs or activate embedded paths
and providers. A generic file dialog alone does not reproduce these methods.

## Independently qualified source profiles

| Family | Intended coverage |
| --- | --- |
| CSV / TSV / fixed-width | Early bounded streaming adapters; encoding, locale, malformed rows |
| XLSX / XLS / ODS | Sheets/ranges/types with explicit data and formatting fidelity limits |
| XML / JSON | Structured mappings/schema policies, not implicit serialization of every core value |
| SQL Server / PostgreSQL / ODBC / SharePoint | Actual server/driver authentication, types, writable links and transaction capabilities |
| PDF / HTML / print / labels | Report output owned by report/layout services |

Linked frontends need refresh/relink, offline errors and credential migration
without committed passwords or diagnostic leaks. Record provider/driver
differences and unsupported operations. SQLite conversion needs explicit mappings;
TableCraft's runtime supplies application behavior, not an arbitrary SQLite file.

Acceptance compares preserved specifications and authorized reruns with native
Access: filtered exports, changed paths, repeated append, conversion errors,
cancellation and partial writes. Linked tests need disposable server/driver
profiles. No production connections or task runs have been made. Record evidence
in the [corpus](../testing/ACCESS_APPLICATIONS.md).

## References

[Saved specifications](https://support.microsoft.com/en-us/access/save-the-details-of-an-import-or-export-operation-as-a-specification),
[DoCmd.RunSavedImportExport](https://learn.microsoft.com/en-us/office/vba/api/access.docmd.runsavedimportexport)
and [text import versus linking](https://support.microsoft.com/en-us/access/import-or-link-to-data-in-a-text-file)
define distinct upstream workflows. Each implemented adapter needs its own source
and fidelity evidence.
