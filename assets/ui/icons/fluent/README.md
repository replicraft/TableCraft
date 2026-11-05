# Fluent System Icons

Selected 24-unit regular SVG paths from Microsoft's
[Fluent UI System Icons](https://github.com/microsoft/fluentui-system-icons).
The exact upstream revision, source paths and original/local SHA-256 hashes are
in [PROVENANCE.json](PROVENANCE.json). The upstream [MIT license](LICENSE.txt)
accompanies this subset and must accompany redistributed builds using it.

The only SVG modification replaces the monochrome `#212121` fill with white.
Geometry is unchanged. TableCraft tints these white masks with its current
light/dark palette; multiplying a dark source fill by a light tint would not
produce a legible dark-theme icon. The SVG loader is embedded and has no file,
HTTP, SVG-text or system-font feature enabled for these assets.

These are command assets, not copies of the private Access screenshots or
replacements for images embedded in an Access database. Fluent geometry supports
TableCraft's ribbon; the actual Access captures still govern command grouping,
density and workflow. Disabled preview commands expose no backend action.
