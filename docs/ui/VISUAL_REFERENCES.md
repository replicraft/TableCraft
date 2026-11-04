# Access visual references

The initial shared desktop presentation is being corrected against three retained
screenshots under private `docs/temp/Access Sample/`. They are local reference
material, not copied assets or a public fixture requirement:

| Private reference basename | Visible state | Useful observations |
| --- | --- | --- |
| `Bildschirmfoto_2026-10-28_12-07-35.png` | Table1 datasheet, Home ribbon | Light title chrome, white command ribbon, compact object navigation, separate object tab and dense gray-header grid; blue selected row and yellow active field header. |
| `Bildschirmfoto_2026-10-28_12-07-50.png` | Table1 Design view | Field Name / Data Type / Description grid above a separate lower Field Properties area; contextual Table Design ribbon. |
| `Bildschirmfoto_2026-10-28_12-09-31.png` | File / Account Backstage | Full-page Backstage with left navigation and content sections, distinct from an object document or small popup. |

These are Windows Access captures inside VirtualBox. The title describes an
Access 2016 database while the Account screen identifies Microsoft 365 Apps,
version 2610, build 20527.20088. Do not infer a particular application's UI release
from the database header or title alone. The screenshots include OS/VM chrome;
that chrome does not belong in TableCraft's shared workspace. Account details
are partly obscured and are not product content to reproduce.

## Immediate presentation correction

Use a compact light-gray application title area, white command surface and red
accent rather than a large burgundy banner. Reduce broad button padding and
heading emphasis. Move the functional table/query document switch above its
content, separate from file-opening commands. Keep actual table names visible.
The left object pane follows the reference's compact search and Tables grouping.

The datasheet uses smaller rows and headers, narrower row numbers, gray column
headers and fine grid separators while preserving virtual rows, resizing,
keyboard focus and complete cell copying. Reference pixel sizes are proportions
from a captured display, not platform-independent logical measurements. Current
colors and spacing are approximations requiring visual comparison at the target
scale; they are not measured pixel-exact matches.

The active datasheet field uses the observed yellow header, blue current row and
white active cell with a red outline. Home exposes actual View and Clipboard
groups: Datasheet/Table Design target the current table object, and Copy uses the
retained selected cell. Selection and read-only commands do not establish record
editing, rectangular selection or query-result updateability.
The sample's checked Yes/No fields now render from typed Boolean values plus
their inspected native checkbox declarations. A Boolean type alone does not
override a stored text-box choice or stand in for missing field metadata.

Only working commands remain interactive. File opening, passwords, cancellation,
table pages and read queries retain their current paths. No mock Create ribbon,
editable new-record row, Click to Add field, checkbox editing, query designer or
Microsoft account service is introduced to fill the screenshot layout. The
File/Open presentation uses the Backstage reference's full-page structure and
narrow left navigation. That capture shows Account, not Open: the implemented
Open form is TableCraft's working file-selection surface, not a measured copy of
Access's Open page. Only working Open/About sections are exposed; account/cloud,
Save, Print and other unsupported commands are not simulated.

The read-only schema inspector now follows the observed Design-view organization,
with a central fields grid and lower selected-field pane. Native rendering and
selection were observed for the public example on macOS. It exposes inspected
facts only, rather than the screenshot's complete editable property set. A generic right-side field
inspector is not established by these references. Native type codes, supported
read carriers and uninspected properties must remain distinguishable; a familiar
property-sheet shape does not justify inventing Required/default/validation facts.

## Review each new surface against the captures

Use the actual local images while implementing the corresponding surface, not
only this written description. Compare ribbon grouping and density, object/tab
placement, grid spacing, active/selected states and the Design pane arrangement.
Observe the rendered TableCraft window at a known logical size and scale; record
which surface was inspected and which platform remains unverified. Retain exact
native measurements in future form/report models rather than deriving them from
screenshot pixels or a layout library's preferred units.

A library or replacement icon set cannot redefine the reference appearance.
Choose any Fluent assets individually for working actions; their Windows 11
styling is not evidence of a match to these Access captures. Preserve labels,
keyboard/focus behavior and native embedded application images. Keep the source
captures private and out of CI requirements.

## Verification limits

These references establish observed layout, not complete interaction, keyboard,
accessibility, query or database semantics. Selawik is the selected bundled font;
the captures do not prove every displayed font uses it. Retain native window
management and fallback glyphs, then compare actual rendered TableCraft windows
at representative scales. Headless tests and successful builds alone do not
establish screenshot fidelity or native Access behavioral compatibility.

[Desktop preview](DESKTOP.md) · [UI index](README.md)
