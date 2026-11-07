# TableCraft command icons

Original colored vector illustrations authored for TableCraft. The command
metaphors and compact density are informed by the retained Access Home/Design
references; the geometry is original and does not copy Microsoft glyph paths,
product logos, or PaintCraft artwork. PaintCraft was consulted only for its
embedded theme-variant SVG rendering and display-density approach.

Each icon has a 32-unit canvas, a light/dark color variant, and corresponding
neutral disabled variants. Enabled rendering preserves the source colors;
disabled commands use neutral artwork and muted text. The Ribbon preserves full
artwork opacity for legibility; other controls retain their host disabled styling. Dark variants use brighter outlines and saturated semantic accents so
small command glyphs remain distinct from the dark Ribbon background. Record
navigation has dedicated solid silhouettes; Datasheet and Design view glyphs
have separate geometry from table editing. Files are self-contained vector shapes with no text/font dependency,
external references, filters, scripts, or linked images. Commands sharing a
concept may share geometric building blocks; distinct command meanings have
separate compositions.

Run `python3 assets/ui/icons/tablecraft/generate.py` from any working directory
to reproduce the variants and hash manifest. The geometry and semantic palette
are defined together in that owning generator.

These original repository assets use the repository license. The complete local
file/hash inventory is in PROVENANCE.json. The older Fluent subset and its MIT
notice remain retained source material but are not embedded by the active icon
module.

For native packaging, the headless `tablecraft-ui` example
`export_application_icons` rasterizes `application-light.svg` directly at nine
sizes from 16 through 1024 pixels and emits PNG, ICO and ICNS files. Set
`TABLECRAFT_ICON_DIR` to a fresh directory with an existing parent, then run
`cargo run --offline --locked -p tablecraft-ui --example export_application_icons`.
The generated images are build outputs; they do not change this source inventory
or install application icons. Four decode/container tests pass, and Apple's
`iconutil` independently extracts the generated ICNS. Offscreen review covered
16/256-pixel PNGs and the extracted 1024-pixel image on 7 November 2026.
