# Deferred Windows and Wine compatibility research

Decision updated 18 October 2026: Windows API emulation, COM dispatch/bridging and
Wine integration are deferred with VBA execution. `tablecraft-wincompat` and
`tablecraft-wine` are removed from TableCraft's active implementation roadmap.
There are no provider crates, helpers, registry emulation, DLL loaders or Windows
binary execution in the current ten-package workspace.

The supplied Wine and standalone RustVBA proposals are research inputs, superseded
by this scope decision. They are not instructions to implement providers now.
If RustVBA matures, generic Windows/COM/Wine functionality belongs to that independent
project; TableCraft may later integrate its host API through the reserved
`tablecraft-vba-host` boundary. See [VBA scope](VBA_COMPATIBILITY.md).

## Work retained in TableCraft

Preserve original modules, references, Declare signatures, event bindings and
embedded object/control data through qualified unrelated edits. Future inert
analysis should flag known dependencies and report dynamic/opaque cases as unknown.
Discovery must not activate objects or load libraries. VBA-driven buttons, event
handlers and COM/DLL-dependent operations remain unavailable. No runtime dependency
tracing is planned while execution is deferred.

This deferral does not apply to normal OS integration: target-specific file handles,
byte-range locks, durability, accessibility and credential storage may still use
appropriate platform APIs. Low-level file operations belong to `tablecraft-io`;
Jet/ACE chooses native lock ranges/protocols. Such work does not implement VBA's
Windows API environment. Engineering-only `tablecraft-compat` remains the future
Access comparison boundary, distinct from product compatibility providers.

## Conditions for reconsidering providers

Wine is a possible future external-component provider, not a VBA runtime, Access
object model or security sandbox. It does not supply proprietary vendor DLLs or
promise ActiveX embedding in a native UI. Redistribution requires review of Wine's
[LGPL terms](https://github.com/wine-mirror/wine/blob/master/COPYING.LIB) and the
licenses of separately installed components.

A future helper needs a real IPC protocol for values, errors, object identity,
lifetimes, callbacks, ByRef and reentrancy; an arbitrary Windows DLL cannot simply
become a Rust function pointer. Qualify Windows/native and Wine helpers separately
by OS, CPU, 32/64-bit architecture and installed component. Unavailable or denied
calls must fail explicitly and must not fall back around a denial.

Before resuming, require corpus evidence of need, a supported RustVBA API, explicit
execution authority, tested isolation/resource boundaries and real per-component
comparisons. Preserve metadata regardless of execution support. Native shared-file
coordination and required table data-macro validation remain engine responsibilities;
neither is provided by Wine or deferred by this decision.

## Primary references

Reviewed on 18 October 2026 as specification inputs, not implementation evidence:

- [CreateObject](https://learn.microsoft.com/en-us/office/vba/language/reference/user-interface-help/createobject-function) and [GetObject](https://learn.microsoft.com/en-us/office/vba/language/reference/user-interface-help/getobject-function): activation/lookup inputs and distinct instance/file behavior.
- [IDispatch::Invoke](https://learn.microsoft.com/en-us/windows/win32/api/oaidl/nf-oaidl-idispatch-invoke): native Automation invocation context, arguments, member modes and errors.
- [FileSystemObject](https://learn.microsoft.com/en-us/office/vba/language/reference/user-interface-help/filesystemobject-object): file/folder/text-stream surface for subset qualification.
- [Declare](https://learn.microsoft.com/en-us/office/vba/language/reference/user-interface-help/declare-statement) and [64-bit VBA](https://learn.microsoft.com/en-us/office/vba/language/concepts/getting-started/64-bit-visual-basic-for-applications-overview): external signatures and pointer/handle architecture requirements.
- [Access database trust](https://support.microsoft.com/en-us/access/decide-whether-to-trust-a-database): background for distinguishing database access from enabled executable content.
