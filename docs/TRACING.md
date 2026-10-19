# Structured operation diagnostics

CLI `-v` / `--verbose` enables debug diagnostics; repeat for trace-level I/O detail.
The application installs tracing-subscriber only for explicitly verbose command
execution. Normal commands stay quiet. Help/argument errors retain clap behavior.
Diagnostics go to stderr; stdout remains the buffered command result. Monotonic
uptime and span close busy/idle timing help locate expensive phases. No collector,
file, environment filter or network destination is configured.

Libraries depend only on tracing and leave subscriber selection to their host.
Manual operation spans cover SQL parsing, binding/execution, native catalog/table
reads, native saved-query reconstruction, SQLite snapshot/scan/optional worker queries
and low-level read-at-offset work. Events expose result row/column counts,
cache hit state and unsuccessful command completion. They record no SQL, parameters,
row values, source paths, object names or Debug representations of arguments.
Typed user-facing errors remain the executable boundary's responsibility.
Do not introduce attribute instrumentation that implicitly records arguments.

```sh
cargo run --locked -p tablecraft-cli -- -v query run "SELECT 1 AS N"
cargo run --locked -p tablecraft-cli -- -vv inspect path/to/copy.accdb --catalog
```

CLI integration tests verify normal/verbose stdout identity, expected phase names,
quiet default stderr, debug/trace filtering, payload omission and unchanged failure
status/no partial stdout. These tests exercise the real subscriber/engine path.
Trace-level output can be verbose; enable it for a bounded investigation. Logging
is diagnostic work, not part of resource-budget accounting or correctness checks.
It does not replace query/native cancellation and structural limits.
