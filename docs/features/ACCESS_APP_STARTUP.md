# Access application startup and session state

Status: future compatibility requirements, reviewed 18 October 2026. There is no
application session, startup host or macro interpreter today. CLI query commands
can open explicit databases read-only but never run their startup content.

VBA startup/event execution is deferred. Preserve bindings and report dependent
launch actions as unavailable; non-VBA macros/session work remains in scope.
See [current VBA scope](../architecture/VBA_COMPATIBILITY.md).

## Ownership and behavior

The planned application runtime owns session lifecycle and active-object context;
macros own AutoExec/AutoKeys behavior; automation exposes supplied host contracts.
Native codecs must preserve startup metadata. See [VBA_COMPATIBILITY.md](../architecture/VBA_COMPATIBILITY.md)
for execution authorization and required database validation.

Preserve startup form selection, application title/icon, menu restrictions,
navigation options and bypass settings. Inspection must remain inert even when
an application contains AutoExec or form Open/Load handlers. Opening for inspection
and launching an application are distinct operations with distinct permissions.
Unsupported startup actions must be reported rather than silently skipped during
a claimed compatible launch. Record partial startup and cleanup after failures.

Qualify startup ordering, form Open/Load/Current events, cancellation, errors,
reentrancy and shutdown against each supported Access profile. Do not infer a
complete event order from the existence of an AutoExec macro. Imported menu
restrictions are application behavior, not a security boundary.

| State | Required scope |
| --- | --- |
| TempVars, current object, loaded Forms/Reports, focus and navigation | Application session; reset/dispose with that session |
| CurrentProject | Session view of project identity, metadata and connection context |
| Saved startup properties and object definitions | Database-persistent metadata with explicit save semantics |
| Collaboration operations | Only explicitly selected durable project data/design changes |

Do not replicate every variable, focus change or open form. Access database state,
local user preferences and replicated TableCraft state need separate lifetimes.
TempVars type restrictions, missing names and cleanup require observed contracts;
core expression states alone do not define this collection.

AutoKeys needs qualified key syntax, shortcut replacement, trust restrictions and
active-context dispatch. Document unavailable platform/browser shortcuts instead
of remapping them silently. A TableCraft inspection mode must not depend on an
imported AllowBypassKey value to avoid executing untrusted code.

## Acceptance evidence

Use application copies with startup form only, AutoExec, combined handlers,
AutoKeys overrides, disabled bypass, denied actions and deliberate failures.
Record event traces, session isolation, cancellation/cleanup and reopen behavior.
Test two independent sessions and verify that local state never leaks through P2P.
These are future tests; no startup application is qualified yet. Track results in
the [application corpus](../testing/ACCESS_APPLICATIONS.md).

## References

Microsoft documents [AutoExec and startup forms](https://support.microsoft.com/en-us/access/automate-startup-events-with-a-macro),
[startup bypass and AllowBypassKey](https://support.microsoft.com/en-us/access/bypass-startup-options-when-you-open-a-database),
[AutoKeys shortcuts](https://support.microsoft.com/en-us/access/run-an-access-macro-by-using-a-keyboard-shortcut)
and the [TempVars collection](https://learn.microsoft.com/en-us/office/vba/api/access.tempvars).
These establish research inputs, not TableCraft execution evidence.
