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
cache hit state and unsuccessful command completion. Native projected reads emit
`native projected scan completed` for complete snapshots and
`native selected scan completed` for bounded ranges, with row and projected-column
counts. They record no SQL, parameters,
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

## Desktop read tasks

`tablecraft-desktop -v [DATABASE]` enables the same explicit stderr debug output;
`-vv` also enables trace-level native I/O. The default remains quiet. The desktop
owns its subscriber, and the shared runtime captures its caller's dispatcher when
starting the worker so task spans and nested backend spans share that subscriber.

An `application read task` span contains only the fixed operation `open`, `page`
or `query`, plus explicit metadata-only `describe`. Its completion event records
boolean `success` and `superseded` fields.
No command Debug value, error text, path, table name, SQL or password is recorded.
These events describe execution completion, not acceptance of a possibly stale
response by the GUI, and do not change cancellation or transactional outcomes.
A dedicated integration test captures all four command kinds on the background
thread and checks both operation visibility and payload omission. Its separate
process avoids tracing-core 0.1.36's single-dispatch callsite registration fast
path caching quiet parallel unit workers' sites as disabled. The real worker and
host diagnostics behavior are unchanged; the fixture does not install a global
subscriber or serialize the application.
