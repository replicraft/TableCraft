# Access behavioral parity and paused native projects

Accepted direction, 26 October 2026. This supersedes the implementation priority
of [native projects and scripting](0001_PROJECT_FORMAT_AND_EXECUTION.md), while
retaining those plans for a future explicit decision to resume them.

## Immediate objective

TableCraft targets faithful cross-platform Microsoft Access behavior on native
Access files. ACE 16 remains primary; existing MDB and older ACE coverage remains
subject to the documented compatibility profiles. Current support and independent
engineering evidence do not establish complete Access parity.

SQLite, `.tcraft`, `.tcraftproj/`, native-project Python automation, project
conversion and native-project modernization are paused. They are outside the
active backend and pre-UI acceptance path. Existing working SQLite functionality
is retained; ordinary repository-integrity checks do not resume its development.
Do not build generalized project infrastructure to satisfy those deferred plans.
Native Access shared-file coordination remains active work; native-project P2P
collaboration remains deferred.

The backend-first goal continues. This decision sets the behavioral contract for
that backend and its eventual UI; it does not start UI implementation.

## Observable compatibility

The corresponding Access behavior governs defaults, restrictions, documented
quirks, command availability and resulting state. Equivalent error meaning and
failure behavior matter as well as successful results. Preserve query evaluation,
NULL/empty distinctions, rounding, dialect modes and saved-query semantics under
their owning contracts and evidence.

Shared application services must enforce these policies for GUI, CLI and future
agent hosts. A different host cannot bypass a file restriction, required
validation, confirmation or event. Engineering previews retain their separate,
explicitly unqualified contracts and do not become production Access operations.

The future application compatibility profile must distinguish:

- Actual stored format and capabilities.
- The selected target Access application version.
- Database-specific SQL, locale, options and behavior settings.

File magic or an ACCDB extension alone cannot identify a ribbon release or the
complete application policy. Existing storage profiles are working format
contracts, not an implemented application-profile API. Add no placeholder types
or packages merely to represent this decision.

## Future presentation and interaction

Access-compatible hosts must reproduce familiar ribbon groups, contextual
commands, Navigation Pane organization, object views, property sheets, dialogs,
wizards, shortcuts, focus behavior and context menus. Required action-query
warnings and confirmation sequences remain part of the workflow.

Datasheets must follow Access dirty-record, save, cell/record undo, validation,
navigation, lookup, multivalue, deletion and concurrency-conflict semantics.
Imported forms retain sections, coordinates, sizes, control properties, focus
order, subforms, events and navigation behavior. Reports retain print layout and
pagination. No automatic responsive redesign or silent SQL rewriting is allowed.

Platform window decorations may differ. Selawik remains the selected UI font.
Rendering speed, smooth scrolling, high DPI, accessibility, dark mode and optional
diagnostics may improve while preserving observable workflows. Diagnostics should
remain optional and accessible without scattering new commands through familiar
Access ribbon groups. Shared rendering choices must support faithful controls.

## Implementation and qualification

After the native backend milestones, organize UI work around these parallel
compatibility workstreams:

| Workstream | Scope |
| --- | --- |
| Application shell | Ribbon, Navigation Pane, tabs, contextual commands, menus, Backstage, dialogs and options |
| Tables and queries | Datasheets, Table Design, Relationships, Query Design, SQL View and results |
| Forms and reports | Native controls/properties, sections, layouts, runtime, print preview and wizards |
| External Data and Database Tools | Corresponding import/export/link, maintenance and wizard workflows |
| Application behavior | Native macros, startup, custom ribbon definitions, options and event sequencing |
| Qualification | Differential workflow and persisted-state comparisons against actual Access |

Qualification begins with the first UI components. Record interaction traces for
mouse/keyboard actions, command states, focus, dialogs, visible editing state and
persisted results; screenshots supplement semantic assertions. Record the actual
Access version/build and fixture provenance. Qualify other platforms against the
same contracts. Windows comparisons remain user-deferred, not passing evidence.

VBA execution, editor/debugger, Windows/COM/Wine integration and `vba-rs` hosting
remain deferred under [VBA compatibility](../architecture/VBA_COMPATIBILITY.md).
Preserving VBA and detecting dependent features remain required. Required Access
data macros stay in scope. These gaps must remain visible in full-parity claims.

Modern native-project behavior may be reconsidered when that work is explicitly
resumed. It must not redefine native Access behavior in the meantime.
