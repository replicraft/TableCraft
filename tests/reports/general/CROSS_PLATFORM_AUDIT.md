# Cross-platform audit — 7 November 2026

The active desktop/CLI target set is Windows MSVC, macOS and Linux GNU, each on
x86-64 and ARM64. Build portability, native execution, database compatibility
and crash/power-loss durability are separate evidence. No native app windows
were opened during this audit. SQLite functionality was checked without
expanding its paused scope; VBA execution remains deferred.

## Latest working-tree review

The renewed three-way audit covers storage/publication and recovery, shared and
host source portability, and CI/release/association tooling. It includes the
current mixed-table creation and pending inline Design changes. No additional
platform defect was found in those source reviews. Existing working changes and
user-owned desktop packaging files were preserved; no native windows, remote
jobs, release or push were started.

Fresh all-target/all-feature compilation passes for ACE, runtime, shared UI and
desktop on Windows MSVC x64/ARM64, Intel macOS and Linux GNU x64/ARM64. macOS
ARM64 strict all-target/all-feature lint passes for ACE, Access, runtime, UI,
desktop and testkit. These affected-package checks are separate from the older
complete-workspace results below. Windows CLI/SQLite retain the documented local
MSVC C-SDK exclusion. A first lint attempt caught identical branches in pending
inline-input code; combining the condition preserves its original evaluation
order, and the corrected lint passes without suppressing the warning.

Repository verification passes 82 Python guard tests, 32 tracked archive tests,
eight association tests, both workflow lint checks, the 0.1.0 CLI release
contract, all 882 formatter roots, all 20 dependency boundaries and 7,971 local
documentation targets. Fresh macOS ARM64 execution passes all 139 IO, 152 runtime
and 119 desktop tests, plus six inline-field interaction tests and one foreground
type-menu clipping test. This is an affected-package batch, not a full-workspace
regression run. Fresh logs use
the ignored `.cache/platform-renewed-audit-corrected-*20261107.log` and
`.cache/platform-renewed-execution-*20261107.log` prefixes; the failed first lint
attempt is retained separately.

The remaining release gates are concrete native acceptance work: Windows
local-NTFS sharing/ACLs/publication and retained-journal recovery; Linux directory
barriers and recovery; installed dialogs, clipboard, accessibility, window
controls and file-open delivery; minimum OS/runtime-library support; and desktop
installation/signing. The release workflow still publishes only CLI archives.
Association metadata tests do not install a desktop file handler. Linux portal,
display-server and font/OpenGL availability also require installed execution.
Physical power-loss and native Access comparisons remain separate qualifications.

## Current conclusion

The reviewed implementation has Windows, macOS and Linux routes rather than a
Unix-only creation/save path. The six-target source and compilation audit is
complete within the recorded exclusions below. This is not yet an assurance that
every workflow works on all six native environments: Windows/Linux execution,
installation and physical durability remain release qualification work.

The source review includes repeated table creation and its consuming save/reopen
route. Shared services pass native paths as `Path`/`PathBuf`; codecs retain
explicit byte order. The common adapter lifecycle delegates file ownership,
publication and journal retirement to IO's platform implementation. Ordinary
temporary-file helpers do not replace those guarantees. No additional platform
fallback or file-format admission was introduced during this refresh.

The release-critical native checks still needed are:

- Run the configured native CI jobs, including the Windows CLI/SQLite build that
  cannot be completed with this machine's missing MSVC C SDK.
- Exercise create, reviewed update/insert/delete, repeated table addition,
  interrupted-save recovery, destination refusal and retained-journal release on
  Windows local NTFS and Linux.
- Exercise installed file-open events, Unicode paths, dialogs, clipboard,
  accessibility, Windows/Linux window controls and display scaling on each OS.
- Qualify minimum supported OS/runtime libraries and distribution installation;
  the tracked release workflow currently publishes CLI archives only.

These are separate from the deferred native Microsoft Access comparisons and
power-loss testing. Detailed findings and historical batches follow; their
counts describe the trees tested at those checkpoints.

An earlier headless refresh passed all 434 shared-UI tests and 111 desktop
tests, strict all-target/all-feature lint for both packages, and Windows x64 and
ARM64 all-target/all-feature checks for runtime and desktop. This includes the
current Navigation Pane and bundled-font working changes. The initial UI batch
passed 432 tests and exposed two test-selector failures: an empty-list assertion
used a mandatory-match query, and the new Queries heading made a global modal
button lookup ambiguous. The corrected selectors retain the same behavioral
assertions; the full rerun passes. Failed and accepted logs remain separate under
ignored `.cache/platform-refresh-*20261107.log` paths.

Repository checks also pass: 82 top-level Python tests, 55 local packaging/tooling
tests, all 882 formatter roots, all 20 dependency boundaries and 7,947 local
documentation targets. The local packaging test count includes preserved
user-owned notice working files and does not establish tracked desktop release
coverage. The earlier complete six-target workspace checks below retain their
recorded scope; this refresh does not rerun the complete backend regression suite
or execute Windows/Linux programs.

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

## Pending table-creation changes: renewed audit

The renewed source review includes the pending append-table engine, original
intent and recovery route, Access offline service, runtime review/save ownership,
and desktop Create → Table flow. Three parallel reviews covered native storage,
host integration, and tracked CI/release/association tooling. No additional
portability defect was found. This is a scoped source-review finding, not proof
that every application behavior works on another operating system.

The append-table codecs retain explicit little-endian fields and checked widths.
The save path uses the existing platform-specific journal and publication owners;
it does not introduce a Unix-only filesystem route. Windows held-target tests
observe the retained handle and check path contents after release. Unix inode and
symlink assertions remain explicitly platform-specific. Runtime and presentation
names do not replace native paths or determine query collation.

The publication review also checked the locked `tempfile` API against its
[upstream contract](https://docs.rs/tempfile/3.27.0/tempfile/struct.NamedTempFile.html#method.persist).
`persist` does not synchronize file contents or the containing directory, and
`persist_noclobber` does not guarantee atomic completion on every platform.
Ordinary temporary-file consumers can use that crate; required database
publication still needs its captured-parent identity, platform barriers and typed
uncertain outcomes. Windows retains its local-NTFS and retired-journal limits.

Fresh verification logs use the ignored `.cache/platform-final-*-20261107.log`
prefix. Results below distinguish current-tree checks from the historical
batches above. No native app windows, remote CI jobs or releases were started.

| Current working-tree check | Result / scope |
| --- | --- |
| Strict workspace lint | All 20 packages, all targets/features, pass on macOS ARM64 |
| Intel macOS / Linux x64 / Linux ARM64 cross-builds | Complete workspace, all targets/features, passes on each target |
| Windows x64 / ARM64 cross-builds | All targets/features pass for 18 packages; CLI/SQLite retain the local MSVC C-SDK exclusion |
| Isolated runtime features | Strict library lint passes without optional features, with creation alone, editing alone, and both together |
| Selected headless execution | 925 tests pass: IO 139, Access 99, runtime 149, desktop 111 and shared UI 427 |
| Repository and local packaging/tooling tests | 82 and 55 tests pass respectively |
| Formatting / dependency boundaries | All 882 formatter roots and all 20 package boundaries pass |
| Documentation links | 7,928 local targets pass |

The selected execution includes actual reviewed schema creation, row insertion,
second-table creation, fresh reopen and synthetic presentation scenarios. It is
not a full-workspace regression run or native Windows/Linux execution. Review
also corrected an unnecessary clone in a pending adapter test; strict lint passes
without suppressing the diagnostic. Existing user-owned desktop packaging and
notice working files were preserved.

The Windows checks exclude CLI/SQLite because the local cross-compilation
environment lacks the MSVC C SDK needed by bundled SQLite. Linux checks use the
retained local Zig C wrappers and writable cache. These checks do not run target
executables. Native Windows/Linux sharing, ACLs, journal retirement, dialogs,
window controls, installed associations and crash/power-loss behavior remain
unqualified locally. No new unsupported-platform fallback was introduced.

Completed target-specific incremental caches were removed between phases to
control local disk pressure. Incremental compilation remained enabled; native
host caches, dependencies, binaries, fixtures and logs were retained.

## Manual Design working-tree refresh

A further three-way source review covered native I/O, locking, creation and
journal publication; application/runtime/CLI/UI host integration; and tracked
CI, release, packaging and association tooling. It includes the pending manual
first-table definitions and TCNT6 recovery grammar. Those byte-oriented changes
continue through the existing platform-owned save path; they do not introduce a
Unix-only writer. Existing pending feature changes and user-owned desktop notice
files were preserved.

Fresh local checks distinguish current affected-package coverage from the
historical complete-workspace results above:

| Check | Result and boundary |
| --- | --- |
| macOS ARM64 execution | 139 I/O, 151 runtime and 454 shared-UI tests pass; desktop grows from 112 to 116 passing tests with the locale correction |
| Strict lint | ACE, Access, builders, runtime, UI and desktop: all targets/features pass |
| Windows x64 and ARM64 | Runtime, UI and desktop: all targets/features compile; target programs were not run |
| Intel macOS, Linux x64 and ARM64 | Runtime, UI and desktop: all targets/features compile; target programs were not run |
| Creation-only feature profiles | Native first-table tests and runtime library compilation pass |
| Tracked packaging and associations | 32 archive/package tests and eight association tests pass |
| Repository tooling | 82 Python tests pass; both workflows pass actionlint; CLI release contract validates version 0.1.0 |
| Repository structure | 882 formatter roots, 20 dependency boundaries and 7,957 local documentation targets pass before this record was added |

The execution and lint logs are retained under ignored
`.cache/gui-manual-design-final-*20261107.log` paths. Fresh I/O and
Intel-macOS/Linux compilation and repository-test logs use
`.cache/platform-final-refresh-*20261107.log`. Source review found no new storage
or packaging portability defect, so those areas did not receive speculative
changes. The host locale finding and its correction are recorded below.

### Regional-locale correction

The host review found one concrete display bug: `sys-locale` discovers preferred
UI languages, which do not necessarily identify regional formatting settings.
The desktop used that result for System locale dates, times and currency.
Live and synthetic hosts now call one host-owned regional discovery boundary:

- Windows reads `GetUserDefaultLocaleName` through a bounded UTF-16 buffer and a
  single documented FFI boundary. This is distinct from preferred UI languages.
- macOS uses the safe `NSLocale.currentLocale` wrapper from the already locked
  Foundation binding; no native windows or shell commands are involved.
- Linux uses the first nonempty `LC_ALL`, `LC_TIME`, then `LANG` setting. It does
  not consult `LANGUAGE` or `LC_MESSAGES`, mutate process-global locale state, or
  fall through after an explicitly selected but unavailable locale.

Identifier normalization preserves the existing visible unavailable-locale
fallback. C/POSIX and unsupported script/calendar modifiers remain unresolved.
The existing shared display API still has one regional locale: distinct Unix
`LC_MONETARY` settings and OS custom date/time patterns are not implemented.
This correction does not claim full category-specific regional parity and does
not alter translation choice, stored values, query collation or edit syntax.
Four injected-input tests cover category precedence, empty settings, explicit
unavailable selections, normalization and non-Unicode Unix environment values.

The unused `sys-locale` dependency is removed. Existing locked
`objc2-foundation` and `windows-sys` packages gain target-scoped desktop consumers;
no registry package or version is added. Unsafe code remains denied throughout
the desktop except its reviewed Windows locale module. The native storage FFI
boundary is unchanged.

Fresh correction checks use ignored `.cache/platform-regional-*20261107.log`
paths; desktop strict all-target/all-feature lint and all 116 tests pass. The
corrected desktop also passes all-target/all-feature compilation for Windows
x64/ARM64, Intel macOS and Linux x64/ARM64. Native macOS ARM64 is covered by
strict lint and execution. The broader pre-correction results above retain their
own scope.

Contracts: [Windows regional locale](https://learn.microsoft.com/en-us/windows/win32/api/winnls/nf-winnls-getuserdefaultlocalename),
[Unix category precedence](https://www.sourceware.org/glibc/manual/latest/html_node/Standard-Environment.html).

### Native acceptance still required

Cross-compilation does not qualify Windows sharing/ACL behavior, local-NTFS
publication and retained-journal release, Linux directory barriers, interrupted
save recovery or physical power-loss durability. Native CI is configured for all
six OS/architecture pairs; this local audit did not start or observe those remote
jobs. The Windows CLI/SQLite cross-build still needs an MSVC C SDK, as recorded
above. No SQLite functionality was expanded.

The tracked release workflow produces CLI archives. Desktop installation,
Finder/file-open event delivery, installed file associations, Windows/Linux
window-manager behavior, portal dialogs, clipboard, native accessibility,
minimum OS/runtime-library qualification, signing and notarization remain
separate release checks. Association metadata tests do not install a handler.
No native application windows were opened, and no release or push was performed.
