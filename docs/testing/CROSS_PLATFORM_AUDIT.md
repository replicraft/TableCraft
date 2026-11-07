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

## Renewed source and host review

The follow-up review covers Windows/macOS/Linux on x86-64 and ARM64. It revisits
native publication, locking and recovery; shared carriers, binary codecs and
query semantics; runtime workers, desktop paths/dialogs/fonts; and repository
and release tooling. Existing working changes were included in checks without
being treated as native operating-system qualification.

Additional corrections:

- Cargo metadata, Git document inventories, UTF-8 artifacts and release
  manifests now select UTF-8 explicitly rather than inheriting a Windows legacy
  code page. Current diagnostic and dictionary generators use the same explicit
  decoding. Diagnostic catalog output uses UTF-8/LF, matching its recorded hash
  on Windows as well as Unix. Historical hash-pinned oracle scripts and receipts
  remain unchanged.
- Read/save workers contain panics from host repaint callbacks after publishing
  their result. The worker continues processing, and retained save resources
  remain available for explicit release and acknowledgement. This catches only
  notification failures and does not replay operations.
- Portable CLI journal preflight, output-failure and finalization tests are now
  enabled on Windows too. Their synthetic Unix receipt cases exercise formatting;
  they do not claim that Windows supplies a Unix directory barrier.
- Fresh headless tests exposed a deadlock in pending field-property accessibility
  labels: translation attempted to re-enter the context inside `widget_info`.
  Labels are now resolved before entering that callback. The literal editor also
  preserves egui's multiline accessibility role instead of replacing it with a
  generic single-line annotation.

The source review found no new host-endian wire encoding or architecture-only
SIMD dependency in the reviewed shared code. Existing `wide` and AES consumers
have fallback implementations; native zerocopy layouts retain explicit endian
and alignment contracts. This establishes no additional file-format admission.
Floating transcendental/financial calculations use Rust floating operations whose
precision may vary by platform/toolchain. Portable execution does not promise
bit-identical transcendental results or native Access numerical equivalence.

`tempfile` remains appropriate for ordinary temporary files and content-only
copies. It does not replace the required publisher's captured-parent identity,
file/directory synchronization, typed uncertain outcomes or platform-specific
retirement protocol. Windows local-NTFS restrictions and retained retired-journal
policy remain intentional. No weaker fallback was introduced.

Fresh macOS ARM64 library/binary batches pass 1,004 tests: 139 IO, 95 Access,
108 CLI, 140 runtime, 420 shared UI and 102 desktop. Twelve focused native schema/
initial-insertion recovery tests also pass. This is not the entire workspace
regression suite. Repository tooling passes 81 top-level Python tests, 32 archive
tests and eight association tests; all 881 formatter roots, the 20-package
dependency guard and 7,911 documentation targets pass. Logs use the ignored
`.cache/platform-audit-current-*` prefix. Initial property-render failures and
the corrected byte-literal lint finding remain separate failed evidence.
Strict all-target/all-feature Clippy passes for the complete 20-package workspace
on macOS ARM64. The runtime also passes all 91 tests with optional capabilities
disabled. Windows x64 and ARM64 pass all-target/all-feature checks of the 18
packages other than CLI/SQLite; the existing local MSVC C SDK limitation remains
an explicit exclusion, not a successful Windows CLI build.
Intel macOS and both Linux architectures pass complete workspace
all-target/all-feature cross-checks. Linux's bundled SQLite C compilation uses the retained Zig wrapper
and a repository-local writable cache. Initial attempts failed because Zig's
default global cache was outside the sandbox; their logs remain separate from
the corrected passes.

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

## Explicit oracle exporter portability follow-up

Seven ignored, explicitly invoked ACE fixture exporters previously attempted
`File::open(directory).sync_all()` unconditionally after finishing their files.
Windows ordinary directory opens do not supply the Unix directory-handle route,
so export would fail after writing its artifacts. They now share testkit's
`support::ace::artifacts::finish_export_directory`: Unix retains its directory
sync, while Windows reports `NotSynchronized` after directory-kind validation.
Every exporter retains its existing exclusive artifact creation and individual
file flushes. This is content-only artifact completion on Windows, not a weaker
fallback in database publication or journal recovery. Native execution and the
focused helper test remain separate from this source correction.

## Current-tree verification follow-up

The renewed audit includes the current working tree and keeps the earlier
completed batches above as historical evidence. CI and release configuration
still name six Windows/macOS/Linux x86-64 and ARM64 targets. This source review
found no additional release-script portability defect beyond the corrections
already recorded. It confirms configured coverage, not successful native jobs.
The exporter correction above applies only to explicitly invoked engineering
artifacts; normal database publication and journal guarantees remain unchanged.
The Windows rename review also checks the retained-parent use against Microsoft's
[`FILE_RENAME_INFO` contract](https://learn.microsoft.com/en-us/windows/win32/api/winbase/ns-winbase-file_rename_info):
a relative target name may resolve against the supplied directory handle. This
API-contract check is separate from native filesystem or power-loss execution.

| Current check | Result / scope |
| --- | --- |
| Repository Python guards | 82 top-level tests pass locally on macOS ARM64 |
| Local `scripts/tests` | 55 tests pass, including archive, association and locally present desktop-notice tooling |
| Dependency boundaries | Pass for all 20 workspace packages |
| Documentation links | 7,923 local targets pass |
| Strict workspace lint | Complete 20-package workspace, all targets/features, passes on macOS ARM64 |
| Intel macOS / Linux x64 / Linux ARM64 | Complete workspace, all targets/features, passes cross-checks |
| Windows x64 / ARM64 | All targets/features pass for the 18 packages other than CLI/SQLite; their local MSVC C-SDK exclusion remains explicit |
| IO / Access / runtime / builders | All selected library/binary tests pass: 139 / 95 / 142 / 27 respectively |
| Shared UI / desktop | Final post-correction library/binary batch passes all 425 / 107 tests; the affected Design filter separately passes all 24 |
| Fixture export helper | One focused test passes on macOS ARM64 |
| Current Number native codecs/defaults/recovery | Nine focused tests pass; this does not qualify native Access behavior |
| Formatting | All 882 Cargo target roots pass in four bounded batches |
| Native Windows/Linux filesystem and GUI execution | Not performed by this local headless audit |

The initial current-tree UI batch passed 424 tests and failed one compact-layout
case. Its unchanged assertion detected the Korean Decimal literal input extending
below the footer. The corrected property-height measurement includes actual
input/combo-box padding; all 24 affected Design tests subsequently pass. The
final complete shared-UI/desktop batch also passes. Both Windows architectures
then pass a final all-target/all-feature UI/desktop compilation refresh.

The selected final library/binary and focused native batches total 945 passing
tests across eight packages; repeated focused reruns are not added to that total.
This is not the entire workspace test suite.

The initial workspace lint also caught a redundant test-only `Vec`; the corrected
strict workspace run passes. Failed and corrected logs are retained separately
under ignored `.cache/platform-review-*20261107.log` paths. These batches include
pending Number work but do not turn this platform audit into a native Access
compatibility claim or a completed full-workspace regression run.

The local desktop-notice tooling includes retained user working files; its
presence in a local test run does not add it to the tracked CI test globs or
establish a desktop release. The release workflow still builds and publishes
only the CLI when explicitly triggered under its existing release contract.
This audit dispatches no workflow and publishes no artifacts. The local Windows
CLI/SQLite C-SDK exclusion, desktop release preparation, minimum-OS/runtime-library
qualification, installed file associations, window controls, dialogs, clipboard
and screen-reader behavior retain their previously documented evidence limits.
