# Native Python scripting

Selected direction, not an implemented provider/API. Python is the primary and
initially only general-purpose language for new `.tcraft` projects. Rust owns
application services, database semantics, forms/reports and designers. Visual
actions and shared definitions are language independent; scripts attach to typed
events/commands rather than becoming the only representation of an application.
The earlier TypeScript/JavaScript-default proposal is superseded.

`application/tablecraft-python` will own module loading, Python/Rust carrier
conversion, bounded host calls, exceptions, worker lifecycle and the versioned
Python-facing API. Do not create it until working behavior justifies the boundary.
CPython through PyO3 is the selected direction, preferably in a helper process
outside the UI. The future scripting CLI path must load this service explicitly;
normal query/inspect builds remain independent of Python and network runtimes.

## Deployment and trust requirements

- Ship a supported CPython runtime; choose exact version/ABI, distributions,
  licenses, update policy and target artifacts with the first executable proof.
  A system Python installation must not be an implicit project prerequisite.
- Prefer `uv` for explicitly managed per-project environments and locked
  dependencies. Keep the interpreter/environment outside the portable document;
  include source, dependency declarations/lock and supported OS/architecture/Python
  constraints. Native extensions require available, verified platform artifacts.
  An embedded lockfile does not make every package portable or installed.
- Opening, inspecting, packing or receiving a project never automatically runs
  scripts, imports modules, installs packages or executes package build hooks.
  Dependency installation and changed executable content require explicit trust
  decisions tied to the artifact/content, separately from database access.
- A helper process contains crashes and enables termination/resource policy; it
  is not a security sandbox. Untrusted code requires qualified OS isolation and
  scoped filesystem/network/process/database permissions. Until that exists,
  disable unsupported untrusted execution rather than claim Python is sandboxed.
- Expose command capabilities, not unrestricted engine internals. Preserve exact
  Decimal/Currency/Int64/100ns date carriers across IPC without silent float
  conversion. Specify Python integer bounds, exception causes, cancellation,
  transaction ownership, recursion and cleanup before enabling event handlers.
- Web Python is a reduced capability profile. Pyodide is a candidate to qualify;
  desktop extensions, files, networking and threads are not automatically portable.
  A server Python backend is a separate deployment. RustPython is an evaluated
  alternative, not the default CPython replacement or a second installed provider.

MDB/ACCDB keep Access-native automation and preserved VBA, with VBA execution
deferred. Python behavior requires a native TableCraft project or explicit external
project metadata; it must never be disguised as executable Access functionality.
An application can later use an Access backend through qualified adapters, but
Python does not weaken native locking/write/validation gates.

JS/TypeScript, Lua and PHP embedded hosts are not selected. Future PHP/TypeScript
code generation uses the shared application model and does not dictate the native
language. Python ecosystem packages such as pandas, openpyxl and SQLAlchemy are
optional project dependencies, not bundled requirements or shortcuts around
TableCraft validation/transactions. No new dependency is installed by this decision.

References: [PyO3 embedding](https://pyo3.rs/main/python-from-rust),
[uv project environments and locks](https://docs.astral.sh/uv/concepts/projects/layout/),
[Pyodide platform constraints](https://pyodide.org/en/stable/usage/wasm-constraints.html).
