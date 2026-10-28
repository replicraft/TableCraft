# Desktop database file associations

MDB/ACCDB file-manager registration is reserved for the future desktop UI.
The current CLI release remains a portable command-line archive: it does not
install handlers, launch a terminal on double-click, or include a placeholder
desktop application. No OS registrations or default applications have changed.

## Prepared contracts

[`file_associations.py`](../../scripts/packaging/file_associations.py) prepares
platform registration data without installing it, starting an application or
writing registry/desktop settings. Its executable argument is an explicit future
GUI artifact path, with no default to the current `tablecraft` CLI. These are
tested registration renderers, not an implemented desktop file-opening host.

| Platform | Prepared data | Future integration |
| --- | --- | --- |
| Windows | Per-user application capabilities, MDB/ACCDB OpenWithProgids and a quoted executable command | The desktop installer registers its actual installed GUI executable; uninstall removes only its owned values. Do not overwrite UserChoice or extension defaults. |
| Linux | Desktop entry with `Terminal=false`, literal executable arguments and MDB/ACCDB MIME declarations | Install under the appropriate XDG data directories and refresh MIME/desktop caches. Leave existing default-app choices intact. |
| macOS | MDB/ACCDB document declarations with Viewer role and Alternate rank | Merge into the real desktop app's Info.plist and implement Finder open-document event handling in that host. CLI argv alone cannot receive Finder events. |

The proposed Windows/Linux desktop entry contract passes one file as `-- PATH`.
The future GUI host must implement that receiver before these records are
installed. Paths remain data, never shell fragments. The macOS metadata uses
documented legacy extension matching while canonical foreign MDB/ACCDB UTIs
remain unverified; it does not invent competing imported type identities.

## Acceptance before shipping

Connect registration only when a working desktop artifact exists. Test paths
with spaces, Unicode, quotes and metacharacters, cold and running-app opens,
multiple documents, encrypted/malformed databases and launch failures. Verify
Explorer, Finder and supported Linux desktops, including coexistence with an
existing Access/default handler and ownership-safe uninstall or relocation.

Opening a document must reach the shared native backend through the UI's normal
inert open workflow. It must not execute startup macros, VBA, stored queries,
external providers or project code merely because the OS delivered a filename.
Passwords belong in the application's prompt, never an installed command.

The current renderer tests cover metadata and quoting only. They do not establish
native registration, a GUI receiver, default preservation at runtime or actual
file-manager launches. The existing six-platform CLI packaging gate remains
separate from this future desktop integration.

## Platform references

- [Windows file types](https://learn.microsoft.com/en-us/windows/win32/shell/fa-file-types)
- [Windows default application registration](https://learn.microsoft.com/en-us/windows/win32/shell/vista-managing-defaults)
- [Desktop Entry executable arguments](https://specifications.freedesktop.org/desktop-entry/latest/exec-variables.html)
- [MIME default applications](https://specifications.freedesktop.org/mime-apps/1.0/default.html)
- [Apple document-type keys](https://developer.apple.com/library/archive/documentation/General/Reference/InfoPlistKeyReference/Articles/CoreFoundationKeys.html)
- [Apple foreign type declarations](https://developer.apple.com/documentation/uniformtypeidentifiers/defining-file-and-data-types-for-your-app)
