# Desktop 0.1.0 preparation checklist

This is a preparation checklist, not a release announcement. The existing
[release workflow](../../.github/workflows/release.yml) and
[release contract](../development/RELEASING.md) package the CLI only. No desktop
artifact, installer or file association is qualified by those jobs.

A desktop preview can ship a useful bounded Access browser/query workspace
without claiming the full [ACE 16 compatibility target](../ROADMAP.md). Existing
restricted offline writes remain separately qualified capabilities, not general
Access editing.

## Remaining gates

- [x] **Implement and headlessly verify Ribbon focus navigation.** Verify F10 entry, tab arrows,
  Down, Ctrl+Left/Right, Tab/Shift+Tab and layered Escape using the actual
  workspace. Include German/Japanese overflow, contextual-tab removal, temporary
  strips and child popups; navigation must preserve document state and not
  dispatch commands. These focus journeys pass; native qualification and focus
  after output-only temporary commands remain open. Keep unimplemented KeyTips
  explicit; the retained Home capture supplies no command-letter assignments.
  See [Ribbon coverage](../ui/RIBBON.md).
- [ ] **Record the release's supported browser profile.** Match the
  [desktop guide](../ui/DESKTOP.md), [compatibility matrix](../COMPATIBILITY.md)
  and release notes to the actual build. Retain snapshot/work limits, inert
  opening, explicit query execution, unsupported-feature errors and the
  stable/offline source requirement. Do not imply live shared-file editing,
  VBA execution or full format compatibility.
- [ ] **Repeat the real-file read/query journey.** Use registered original
  ACE 16 ACCDB and Jet 4 MDB fixtures with multiple pages. Verify opening the
  first table, exact final-page navigation, sorting/filtering, Find highlighting,
  NULL/stored-field presentation, SQL result provenance, schema visits,
  cancellation/errors and object close/reopen. Check source hashes before/after.
  Preserve independent-fixture evidence separately from native Access comparison.
  Existing observations are in [the presentation record](../ui/PRESENTATION_PREVIEW.md).
- [ ] **Keep write limits visible and tested.** Identify the compiled platform
  capabilities and qualified UPDATE/INSERT/DELETE profiles in the build.
  Preserve explicit review/apply, consuming source-bound receipts, journal and
  recovery ownership, cancellation outcomes and unsupported-operation refusal.
  Query results must not acquire native edit identity. Keep engineering previews
  outside normal saves; do not advertise arbitrary edits or database creation.
  See [runtime editing contracts](../application/RUNTIME.md) and
  [native write limits](../access/writes/NATIVE_WRITES.md).
- [ ] **Add a separate desktop artifact/CI contract.** Choose and document the
  actual supported OS/architecture set and artifact names; do not inherit the
  CLI's six-platform qualification. Build/package the real GUI with its runtime
  dependencies, icon and platform metadata. Verify extraction, architecture,
  version, hashes and launches of the packaged artifact. Decide and disclose
  signing/notarization status; checksums alone are not publisher signatures.
- [ ] **Verify desktop notices.** Review the actual desktop dependency closure,
  bundled font OFLs/provenance and embedded artwork. Include the project license
  and consolidated third-party notices in desktop artifacts; existing CLI
  notices are not evidence of desktop coverage. Preserve notices in About.
- [ ] **Qualify native platform behavior when authorized.** Native window checks
  are currently suspended. Headless tests and macOS offscreen frames do not
  qualify Windows/Linux launch, window controls, DPI, input methods, accessibility
  or file-manager integration. Keep those gates open and claims scoped until
  actual evidence is recorded; do not resume native checks automatically.
- [ ] **Gate file associations on working packaged receivers.** Prepared
  [registration metadata](../architecture/FILE_ASSOCIATIONS.md) is not an
  installed handler. Before including registration, verify cold/running-app file
  opens, inert password/error handling, paths with spaces/Unicode and ownership-safe
  coexistence/uninstall on each claimed platform.
- [ ] **Prepare desktop-specific release notes and asset verification.** Explain
  preview scope, platform limitations and restricted writes; distinguish synthetic,
  headless, native-host and Access comparison evidence. Verify the declared complete
  desktop asset set before any later publication. This checklist authorizes no
  tag, push, workflow dispatch or release publication.
