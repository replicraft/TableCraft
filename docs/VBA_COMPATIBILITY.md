# VBA preservation and deferred execution

Decision updated 18 October 2026: VBA execution, authoring, editor/debugger,
VBA-specific DAO/ADO interfaces, Windows API emulation, COM/Wine providers and
RustVBA integration are deferred. This supersedes the previous interpreter-first
TableCraft roadmap. Revisit execution when the independent RustVBA project has a
usable integration release; no release, repository or working API is assumed here.

Preserving existing VBA and detecting VBA-dependent features remain required.
Access table data macros are separate from VBA and remain in database compatibility
scope. None of these preservation, detection or macro capabilities is implemented
yet. Catalog module descriptors do not establish project extraction or safe saving.
The current CLI is read-only and executes no VBA or application startup content.

## Current scope and owners

| Owner | Scope |
| --- | --- |
| `tablecraft-ace` | Future native module/project storage and opaque preservation through supported unrelated database edits; no VBA interpreter |
| `tablecraft-model` and compatibility analysis consumers | Future inert dependency descriptions, event/callback bindings and explicit unknowns |
| `tablecraft-macros` | Retained future owner of Access visual/embedded macros and table data macros, independent of VBA |
| `tablecraft-automation` | Retained future owner of narrow macro actions, event dispatch and supplied application/database contracts independent of VBA |
| `tablecraft-runtime` | Future application sessions, forms/reports and supported non-VBA actions |
| `tablecraft-vba-host` | Reserved deferred boundary for a supported external RustVBA host API; no package, dependency or scaffolding now |

Remove `tablecraft-vba`, `tablecraft-wincompat` and `tablecraft-wine` from the active
TableCraft implementation map. RustVBA would own its language parser, Variant
runtime, interpreter/VM, standard library, debugging and optional Windows/COM/Wine
providers independently. TableCraft would supply Access-specific objects if that
integration is resumed. Do not build the standalone project inside this workspace
or add speculative RustVBA dependencies.

## Preserving existing projects

Before enabling native edits, retain standard modules, form/report class modules,
user classes, project references and original native metadata. Preserve source
bytes/encoding, module kind/scope, event bindings, reference identity/version,
compiled/cache structures and unknown streams. Keep original bytes separate from
any decoded or normalized view. A form/report without a class module must remain
distinct from one with an empty module; inspection must not create one.

Preservation means unchanged project/object content survives supported unrelated
database edits, with byte-level checks and reopen tests in actual Access. An edited
database need not be wholly byte-identical. If an operation cannot preserve affected
unknown content, reject it before modification. A user must be able to return a
database to an Access user without losing existing VBA.

MS-OVBA informs project storage; it does not specify every Access container or
prove a valid MDB/ACCDB writer. Opaque OLE scalar reading is not VBA project
preservation. ACCDE/compiled-only projects require a separate profile: preserve
what exists without inventing editable source. New/edited VBA source authoring,
recompilation, debugging and execution remain deferred.

## Detecting dependent features

Inert discovery should identify modules/references and known form/button/report,
startup, ribbon and query bindings that require VBA. Preserve the original binding
and show that the action cannot run in TableCraft. Never replace a missing handler
with successful-looking behavior. Dynamic names, opaque/compiled content and
unresolved references must be reported as unknown; no detected reference is not
proof of independence. Static detection must not activate COM, load a DLL or run
code. Runtime tracing of imported VBA is deferred with execution.

VBA-dependent buttons and event handlers will not execute. Do not claim a compatible
application launch or accept an update that requires an unavailable validation
handler. This restriction does not remove the independent form/report designers,
data binding or non-VBA application actions from the roadmap.

## Access macros and database writes

`tablecraft-macros` and `tablecraft-automation` remain future implementation owners.
Required table data macros and validation must execute with qualified semantics
within the affected transaction, including in a headless engine, or the write must
fail explicitly. The VBA deferral is never permission to skip these rules.
Unsupported dependencies of a macro also block the affected operation. Replication
must apply required validation without replaying user UI events or external effects.

Acceptance requires malformed/limit/cancellation checks, original-stream comparison,
module absence/unknown-reference cases, disabled-action reporting and safe unrelated
edit/save followed by Access reopen. Macro acceptance additionally needs rollback,
error and native side-effect comparisons. These are requirements, not passing tests.
Current implementation evidence is in [IMPLEMENTATION.md](IMPLEMENTATION.md).

## Future integration boundary

If RustVBA becomes ready, evaluate a released host API, version/feature compatibility,
licensing, bounded calls, cancellation, errors, transaction ownership and native
Access-specific tests. Its language and generic providers must know nothing about
TableCraft storage or UI. Query/expression libraries would consume a supplied narrow
callable contract rather than import a runtime. No such integration is authorized
by reserving the name. [Windows research](WINDOWS_COMPATIBILITY.md) is deferred too.

## Primary references

These references were reviewed on 18 October 2026 as inputs for future work, not
as proof of implemented compatibility:

- [MS-OVBA introduction](https://learn.microsoft.com/en-us/openspecs/office_file_formats/ms-ovba/b39ac32f-0ce1-4533-9297-2ff3ff62c9ec) and [VBA storage](https://learn.microsoft.com/en-us/openspecs/office_file_formats/ms-ovba/b693e0ba-489f-4ac8-ac9d-6387fb5779bb): project/module storage research; Access container behavior requires separate investigation.
- [MS-OVBA versioning](https://learn.microsoft.com/en-us/openspecs/office_file_formats/ms-ovba/4d20e95c-0c17-47d6-84b9-8da95d8ea129): interoperable project representations and version-dependent caches must be distinguished in writer research.
- [HasModule](https://learn.microsoft.com/en-us/office/vba/api/access.form.hasmodule): presence/absence of a form or report class module is meaningful, including effects of accessing module properties.
- [Form.BeforeUpdate](https://learn.microsoft.com/en-us/office/vba/api/access.form.beforeupdate-event) and [its binding property](https://learn.microsoft.com/en-us/office/vba/api/access.form.beforeupdate-property): cancellation and event binding are observable behavior.
- [DAO Workspace.BeginTrans](https://learn.microsoft.com/en-us/office/client-developer/access/desktop-database-reference/workspace-begintrans-method-dao): transaction scope must be modeled explicitly rather than inferred from UI events.
