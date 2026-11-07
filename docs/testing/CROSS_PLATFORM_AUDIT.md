# Cross-platform audit — 7 November 2026

The active desktop/CLI target set is Windows MSVC, macOS and Linux GNU, each on
x86-64 and ARM64. Build portability, native execution, database compatibility
and crash/power-loss durability are separate evidence. No native app windows
were opened during this audit. SQLite functionality was checked without
expanding its paused scope; VBA execution remains deferred.

## Findings and corrections

| Finding | Correction |
| --- | --- |
| Desktop Replace remained hidden on Windows despite its implemented runtime route | Include Windows in the host capability and compile the existing complete replacement scenarios there |
| Creation scenarios and save helpers unnecessarily excluded Windows | Enable portable cases, move the shared runtime waiter out of the Unix-specific editing suite and compile their transitive helpers; preserve Unix-only symlink/identity cases |
| A creation test required a Unix parent-directory barrier on every platform | Assert the actual typed publication guarantee; NTFS write-through rename does not claim a Unix directory flush |
| Unix shared-file opening could wait on a FIFO before checking regular-file metadata | Use the existing nonblocking regular opener before trying either lock; preserve ordinary symlink semantics |
| Access's production feature forwarded a development helper feature through a cyclic dependency | Select Access test support explicitly in its dev-dependency; retain standalone and minimal-workspace checks |
| Whole-workspace formatting exceeded Windows process argument limits | Enumerate every actual Cargo target root and check bounded rustfmt batches, retaining editions, configurations and recursive modules |
| CI tested three native host combinations while release packaging listed six | Use six explicit native hosts, check the Rust host triple and retain optional-feature checks |
| The CLI ZIP manifest inherited host/time-dependent metadata | Apply the same fixed timestamp, platform and permission policy to every archive member |
| A pending new record disabled the custom Windows/Linux title bar | Keep minimize/maximize/close and title dragging enabled while separately disabling Open and retaining host draft guards |
| Desktop marked insertion review busy before the shared UI could register its pending request | Register the exact reviewed inputs first, then mark busy; all three connected insertion failures pass after the ordering correction |
| A journal-version test still classified the now-supported version 8 as unknown | Use an independent unknown version and retain separate version/profile mismatch coverage; production decoding is unchanged |
| Large headless builds accumulated unnecessary debug symbols and incremental products between completed phases | Keep assertions and incremental compilation, omit debug symbols in CI and prune only the completed phase's target-specific incremental cache |

The existing failed [CI run at `eef86e8e`](https://github.com/replicraft/TableCraft/actions/runs/42383594693)
was inspected directly. Linux/macOS stopped on missing testkit optional
dependencies; Windows stopped with OS error 206 before rustfmt started. These
failures are historical evidence, not successful runs of the corrected workflow.

The formatter uses a conservative 16,000 UTF-16-unit command budget, below the
[Windows process command-line limit](https://learn.microsoft.com/en-us/windows/win32/api/processthreadsapi/nf-processthreadsapi-createprocessw).
Its target inventory includes library, binary, integration-test, benchmark,
example and build-script roots reported for workspace members. It does not
drop large engine example inventories or disable recursive formatting.

## Verification scope

Local evidence is recorded in the ignored `.cache/platform-audit-*20261107*`
logs. Cross-target checks compile/type-check the selected Rust targets; they do
not execute another operating system's filesystem or GUI code.

| Environment / check | Result and exact scope |
| --- | --- |
| macOS ARM64 strict lint | Minimal workspace and full all-target/all-feature workspace pass; final focused changes checked separately |
| Intel macOS | Complete workspace, all targets/features, passes cross-check |
| Linux x86-64 GNU | Complete workspace, all targets/features, passes cross-check; bundled SQLite C uses a local Zig compiler |
| Linux ARM64 GNU | Complete workspace, all targets/features, passes cross-check with the same C-toolchain arrangement |
| Windows x86-64 MSVC | IO, runtime and desktop pass all-target/all-feature strict Clippy |
| Windows ARM64 MSVC | All targets/features pass for the 18 workspace packages other than CLI/SQLite; the complete desktop and shared UI test targets are included |
| Repository tooling | 73 top-level Python tests, 27 CLI/desktop archive tests and 8 association tests pass; workflow lint, 20-package dependency guard and documentation links pass |
| Formatting | All 881 Cargo target roots pass in four bounded rustfmt batches |

Native macOS ARM64 execution passes 139 IO tests, all 125 runtime tests,
342 shared UI tests, all 82 desktop tests, all 108 CLI unit tests and 10 builder
unit tests. All 19 Rust doctests pass. Sixteen isolated feature combinations pass
across runtime, Access, IO and testkit, including runtime without optional
capabilities, creation only, editing only and both together. These are completed
batches, not a claim that the entire native regression suite passed.

The initial all-feature workspace batch completed Access's unit/integration
suites (706 passed, 105 ignored), then reached ACE's unit target (1,204 passed,
258 ignored and one stale version assertion failed). The corrected version test
passes separately; production decoding was unchanged. Follow-up broad batches
completed the CLI/builder targets above and two ACE integration targets (11
passed) before intentional interruption in favor of affected-package tests.
The first desktop batch reported 79 passes and three real insertion-review
failures; after the host ordering fix, all 82 pass. Original failures, interrupted
runs and ignored cases remain in the logs; none are counted as a full-suite pass.

Preliminary builds encountered local disk pressure, and an initial Linux C
cross-tool invocation used an incompatible target spelling. Completed corrected
retries establish the table above. Local cleanup removed only old unlocked
incremental sessions, preserving binaries, libraries, metadata, fixtures and
reports. Normal compiler incrementality remained enabled.

The broader Windows cross-check excludes CLI/SQLite: the local machine lacks
MSVC C SDK headers for the existing bundled SQLite dependency. That local limitation is
not a Windows CLI pass or a reason to remove its existing dependency.

The source review covers all 20 workspace packages: filesystem and lock owners;
native engine, wire layouts and crypto; Access and retained SQLite adapters;
query/schema/model/core contracts; runtime workers and save capabilities; CLI,
shared display/builders/UI, desktop host, testkit, packaging and CI. Searches
for OS imports, native-endian wire conversions, hardcoded OS paths and platform
feature gates found the concrete issues above. Such searches do not prove that
every behavior is correct.

## Creation and recovery follow-up

Later connected checks pass all 130 runtime, 351 shared UI and 93 desktop tests
on macOS ARM64. The latter includes ordinary canonical-blank Open → explicit
Create → Table → reviewed same-file Save → fresh reopen, refusal for readable
noncanonical/populated sources, and pending first-table recovery followed by
explicit authoring. These scenarios compile for Windows x64 and ARM64 with all package
targets/features. They do not constitute native Windows filesystem execution.
The complete-byte engine availability predicate has six passing focused cases;
it never substitutes for fresh write admission. Current evidence lives in
ignored `.cache/gui-create-table-*20261107.log` files.

## Save-path and packaging follow-up

A second source review covered engine/adapter filesystem use, runtime/host
integration, dependencies, CI and release tooling. It found Windows coverage
exclusions that the earlier cross-checks could not expose: several portable
adapter creation/offline-save, runtime editing and desktop interaction suites
were never compiled there. Those suites now include Windows. Unix-only inode,
directory-retargeting and foreign-directory cleanup scenarios remain explicitly
scoped. Windows assertions retain its actual write-through retirement and intact
retired artifact rather than claiming Unix directory synchronization or removal.

Held-target observations also differ: Windows's exclusive sharing denies an
ordinary path read before lock acquisition, whereas Unix permits that read.
Shared testkit machinery checks the exact Windows sharing violation while the
lease is held, and consumers verify complete bytes after explicit release.
The shared/exclusive lock API now maps only Windows sharing/lock violations to
`LockBusy` at open time; access-denied and other errors retain their original
kind. Windows-only test cases cover both denial and reopening after handle close;
their native execution remains unrun locally.
This follows the distinct [Windows system error codes](https://learn.microsoft.com/en-us/windows/win32/debug/system-error-codes--0-499-).

CLI packaging now rejects dangling checksum symlinks, creates archive/checksum
paths exclusively and removes only outputs created by that invocation after a
later failure. Artifact-path tests separate optional symlink setup from universal
path validation, so missing Windows symlink privileges do not skip ordinary
coverage. The configured six native CI hosts remain unchanged. Native Windows/
Linux filesystem, GUI, installation and power-loss qualification still require
execution on those environments; none is established by cross-compilation.

Fresh follow-up evidence is in ignored `.cache/platform-followup-*20261107.log`
files. All six target triples pass all-target/all-feature checks of IO, Access,
runtime and desktop, including the shared UI dependency and newly enabled test
consumers. On macOS ARM64, the affected library/binary batch passes 857 tests:
95 Access adapter, 98 desktop, 139 IO, 136 runtime and 389 shared UI. This is not
the full workspace integration suite. Repository tooling passes 74 top-level
Python tests, 31 archive tests and eight association tests; all 881 formatter
roots, the 20-package dependency guard and 7,906 documentation targets pass.
Strict all-target/all-feature lint also passes for the affected five packages on
macOS ARM64 and for IO, Access, runtime and desktop on both Windows architectures.
The Windows lint results are cross-target evidence, not native test execution.

## Native CI and release boundaries

The CI matrix now names these native runner/host pairs:

| Runner | Required Rust host |
| --- | --- |
| `windows-2022` | `x86_64-pc-windows-msvc` |
| `windows-11-arm` | `aarch64-pc-windows-msvc` |
| `macos-15-intel` | `x86_64-apple-darwin` |
| `macos-15` | `aarch64-apple-darwin` |
| `ubuntu-22.04` | `x86_64-unknown-linux-gnu` |
| `ubuntu-22.04-arm` | `aarch64-unknown-linux-gnu` |

CI checks the actual host triple, installs Linux desktop build libraries and
runs headless checks. Isolated runtime checks cover no optional capabilities,
creation only, editing only and both together. Minimal workspace tests/lint
remain separate from optional IO, ACE, Access, runtime, expression, core and
SQLite coverage. Archive and association tests run on the native hosts.
The [workflow](../../.github/workflows/ci.yml) is configured coverage, not a record
that those six jobs passed. No push, workflow dispatch or release was performed
for this audit. The existing release workflow still publishes only CLI archives;
desktop archive tooling does not establish a published desktop release.

## Remaining qualification

- Actual Windows/Linux filesystem, sharing, ACL and retained-journal behavior
  needs native execution. Cross-compilation does not test those OS guarantees.
- Windows retains an intact retired private journal by policy; it never reports
  Unix-style durable deletion. Symlink and regular-file refusal cases retain
  their platform-specific semantics.
- The local macOS headless checks do not qualify Windows/Linux window controls,
  file dialogs, accessibility, installed file-open events, minimum OS versions or
  display servers.
- Native Access comparisons and physical crash/power-loss qualification remain
  separate. Passing synthetic fixtures does not establish complete ACE 16 parity.
- 32-bit, big-endian, other Unix and a web host are not covered by this six-target
  qualification. Existing explicit unsupported-platform refusals remain intact.

See [development commands](../development/DEVELOPMENT.md),
[IO guarantees](../foundation/IO.md), [compatibility](../COMPATIBILITY.md) and
[desktop scope](../ui/DESKTOP.md).
