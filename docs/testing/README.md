# Testing and compatibility evidence

Engineering material is organized by feature under [`tests/`](../../tests/README.md):

- [Access tools](../../tests/access/README.md): query, row, schema, index,
  encryption, recovery, storage, creation and reading tools.
- [Reports](../../tests/reports/README.md): human-readable findings and limits.
- [Captured results](../../tests/results/README.md): original verification records.

Rust tests stay with their Cargo packages, and shared Rust test machinery belongs
in `tablecraft-testkit`. Use `python3 tests/run.py CURRENT_TOOL --help` to inspect
an engineering tool without launching its reference application.

Historical observations keep their original source paths and hashes. They are
not fresh current-tool or native Access verification. New executions must produce
new receipts; there is no path-mapping or hash-substitution compatibility layer.
