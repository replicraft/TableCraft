# Database-defined Access ribbons

Status: future compatibility requirements, reviewed 18 October 2026. No Ribbon XML
reader, renderer or callback host is implemented. TableCraft's future application
toolbar and an imported application's custom ribbon are separate responsibilities.

VBA callbacks are deferred with VBA execution. Preserve and identify their bindings;
report dependent actions/dynamic properties as unavailable. Non-VBA commands and
Access macros remain separate future work. See [VBA scope](../architecture/VBA_COMPATIBILITY.md).

## Preservation and execution

Native object codecs must preserve USysRibbons definitions, original XML,
database/form/report associations and unknown extensions. Discovery and preview
must not invoke callbacks, load external resources or activate providers. Bound
XML input size, depth and expanded content; disable external entity resolution.

The planned runtime resolves ribbon associations and commands; presentation
renders supported controls. Automation supplies non-VBA action/macro contracts;
VBA execution remains deferred. Query or native storage code must not import
the UI or runtime to preserve a ribbon.

Qualify supported XML namespaces, built-in command identifiers, custom controls,
visibility/enabled state, callback signatures, invalidation and form/report
activation. Record unsupported controls and missing callbacks explicitly. A static
preview must show which dynamic properties were not evaluated. Imported hidden
commands do not grant or revoke security capabilities.

Reading, preserving, rendering and executing callbacks are separately reported
capabilities. Preserve unsupported definitions through unrelated edits, or reject
the affected save if preservation cannot be guaranteed. Never replace unknown XML
with a simplified rendering and call it a lossless save.

## Acceptance evidence

Compare database and form/report ribbon selection, supported built-in commands,
non-VBA/macro callbacks, dynamic state and error reporting with native Access.
VBA callback execution comparisons are deferred; test unavailable-action reporting
and original-binding preservation instead.
Include unknown elements, malformed/oversized XML, denied callbacks and an
inspection test proving zero execution/network activity. Reopen saved copies in
Access and verify definitions and associations. No such comparison has run yet;
use the [application corpus](../testing/ACCESS_APPLICATIONS.md).

## References

Microsoft's [custom ribbon guide](https://support.microsoft.com/en-us/access/create-a-custom-ribbon-in-access)
documents USysRibbons and database/form/report association. Execution follows
[VBA compatibility](../architecture/VBA_COMPATIBILITY.md) and
[startup/session contracts](ACCESS_APP_STARTUP.md).
