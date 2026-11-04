# Desktop UI

The explicit desktop kickoff starts with native Access browsing and read queries.
Initial desktop/presentation code and a shared application-runtime read worker
are present. Native startup, catalog/table/query results and picker cancellation
were observed on macOS with a temporary copy of the public ACE 2007 example.
Field inspection is metadata-only; explicit collection dialogs retain stored
membership and inert attachment metadata. File/Open uses the retained Access
Backstage layout reference. Complete interaction and cross-platform visual
acceptance remain pending. The desktop bridges UI actions; `tablecraft-runtime`
owns database sessions and commands. A restricted reviewed offline cell-edit
workflow now uses runtime-issued capabilities and retained save outcomes;
its draft/review/apply/refresh and validation/cancel paths were observed on macOS
with an independent synthetic Jet 4 database.
This does not change backend compatibility claims or resume SQLite, project
infrastructure or VBA execution.

- [Native desktop kickoff, ownership and release gates](DESKTOP.md)
- [Observed Access visual references and current corrections](VISUAL_REFERENCES.md)
- [Current implementation status](../IMPLEMENTATION.md)
- [Access behavioral parity](../decisions/0003_ACCESS_BEHAVIORAL_PARITY.md)

[Documentation index](../README.md)
