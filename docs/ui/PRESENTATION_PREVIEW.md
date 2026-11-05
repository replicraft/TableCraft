# Presentation preview

Current UI work focuses on screenshot-guided visual polish with synthetic data,
without expanding database behavior. The preview reuses the shared workspace
renderer and existing view/action model. Its separate desktop host never creates
a database worker, file dialog or save worker. Synthetic responses cannot open,
query or modify a real file. Preview-only future Ribbon commands remain disabled.

```sh
cargo run -p tablecraft-desktop -- --mock
cargo run -p tablecraft-desktop -- --mock --mock-view design
cargo run -p tablecraft-desktop -- --mock --mock-language ja --mock-dark
```

Other scenes cover Options, ad-hoc SQL, saved queries, attachments, multivalues, stored
history, edit drafts, reviewed edits, recovery and File/Open. They review existing
presentation states; simulated validation/save outcomes are not engine evidence.
The normal optional database path conflicts with mock mode.

File → Options selects Light or Dark and the interface language. These choices
apply to the current session. Selawik remains the primary font; bundled regional
Noto Sans CJK fallbacks supply Japanese, Korean and Simplified Chinese glyphs.
Font sources, hashes and OFL notices live in `assets/ui/fonts/`.

Fluent (`fluent-bundle`, from fluent-rs) handles bundled resources. Each locale
has separate `shell`, `ribbon`, `backstage`, `datasheet`, `query`, `saved-query`,
`design`, `editing`, `collections` and `settings` `.ftl` files under
`assets/ui/locales/`. Languages are English, Spanish, German, Russian, Japanese
(`ja`; `jp` is accepted as an input alias), Korean, Simplified Chinese (`zh-CN`)
and Brazilian Portuguese (`pt-BR`). Catalog tests check syntax, exact message and
variable parity, formatting and fallback diagnostics. Translations are initial
engineering translations pending native-speaker review.

User/database names, SQL, stored expression text, cell contents and raw service
error details remain literal. UI language does not select database comparison,
wildcard, time-zone or regional value-format policies. Missing/invalid messages
have a diagnosed English fallback; counters retain no message arguments.

The actual retained [Access captures](VISUAL_REFERENCES.md) govern light
Ribbon/datasheet/Table Design/Backstage proportions. No Access captures exist for
dark mode, query parameter forms or edit/collection dialogs. Those surfaces remain
visual adaptations, not verified replicas. Offscreen render evidence is separate
from native platform interaction and Windows Access behavior.

## Render review

```sh
TABLECRAFT_PREVIEW_DIR=.cache/ui-previews \
TABLECRAFT_PREVIEW_LOCALE=ja \
TABLECRAFT_PREVIEW_THEME=dark \
TABLECRAFT_PREVIEW_SCENE=options \
cargo run -p tablecraft-ui --example render_preview
```

Omit `TABLECRAFT_PREVIEW_SCENE` to export all twelve scenes. Artifacts are PNGs at
1280 × 820 logical pixels, 1× scale, using Windows keyboard conventions on the
local renderer. The development-only renderer requires a GPU adapter; it does not
open an OS window. Keep generated images in ignored local output directories.

Set `TABLECRAFT_PREVIEW_WIDTH` and `TABLECRAFT_PREVIEW_HEIGHT` to review another
viewport, for example the desktop minimum of 900 × 580. Use a different output
directory to keep those artifacts separate from the default-size review.

## Local verification — 4 November 2026

The coordinated presentation/host batch passes 70 shared UI tests and 23 desktop
tests, including actual theme/language selection and localized mock startup.
All eight locales × both themes × twelve scenes draw successfully with zero
localization diagnostics. Complete catalog message/variable parity, exceptional
fallback behavior, literal argument preservation and CJK glyph coverage pass.
Strict Clippy, formatting, documentation links and dependency-boundary checks pass;
the engineering scripts' 60 tests also pass. No full engine-suite rerun was needed
for this presentation-only change.

The local macOS offscreen renderer exported 192 default-size PNGs at 1280 × 820,
1× scale with Windows keyboard conventions. Representative light/dark, CJK,
long German/Russian labels, collection and edit/review surfaces were opened and
inspected. Five additional Russian scenes were exported at the desktop minimum
900 × 580; compact datasheet, history and review renders were inspected. The
review corrected field-grid width/gutters, long Ribbon wrapping, input boundaries
and collection-column widths. Overflowing Ribbon and record controls remain
horizontally scrollable rather than clipping inaccessible commands.

These are local mock/render observations, not Windows execution, screen-reader
qualification or native Access behavioral comparisons. Translations and surfaces
without retained Access captures still require the separate reviews noted above.
