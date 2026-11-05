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
history, edit drafts, reviewed edits, recovery, File/Open, About, empty tables,
loading and read errors. They review existing
presentation states; simulated validation/save outcomes are not engine evidence.
The normal optional database path conflicts with mock mode.

File → Options selects Light or Dark and the interface language. These choices
apply to the current session. Selawik remains the primary font; bundled regional
Noto Sans CJK fallbacks supply Japanese, Korean and Simplified Chinese glyphs.
Font sources, hashes and OFL notices live in `assets/ui/fonts/`.

Fluent (`fluent-bundle`, from fluent-rs) handles bundled resources. Each locale
has separate `shell`, `ribbon`, `backstage`, `datasheet`, `query`, `saved-query`,
`design`, `editing`, `collections`, `navigation` and `settings` `.ftl` files under
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

Omit `TABLECRAFT_PREVIEW_SCENE` to export all 39 scenes. Artifacts are PNGs at
1280 × 820 logical pixels, 1× scale, using Windows keyboard conventions on the
local renderer. Animated pending scenes advance exactly three frames rather than waiting
for their spinners to become idle. The development-only renderer requires a GPU adapter; it does not
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

## Follow-on navigation and state review — 4 November 2026

That review covered sixteen scenes. Empty tables retain their
actual declared headers alongside the empty notice, with no invented new-record
row. Loading and read-error scenes preserve the last synthetic snapshot;
cancellation is local to the mock host. The default mock Table Design command
now uses the same existing ViewSchema intent as the native host and prepares its
synthetic declaration locally instead of requiring a preloaded schema.

Object navigation uses full-row hit regions and stable document/object widget
IDs, including distinct identities for a table and saved query with the same
name. Filtering earlier rows preserves focus; replacing the document discards
it. Its eight navigation messages now have their own `navigation.ftl` in every
language. Backstage has left-aligned Back/Open/About/Options SVG controls and
independent section scroll state.

The lower General property sheet is compact, full-width, left-aligned and
read-only. Its overflow scrollbar appears only when needed; actual scroll tests
reach Validation Text and the collapsed Native details. Plain Up/Down moves only
the focused current field and reveals it once, including across virtual rows.
Modified keys, property-text selection, replaced metadata and modal overlays
remain isolated. Ribbon commands similarly reveal only on gained focus; manual
scrolling is preserved. Disabled mock and background field controls expose no
Click/Focus actions and reject forced assistive activation.

The coordinated batch passes 78 shared UI tests and 23 desktop tests, including
all 256 language/theme/scene drawing combinations with zero localization
errors. The updated renderer exported all 256 default-size PNGs; representative
empty, loading, error, About, navigation and Design renders were opened for review. Strict Clippy, formatting, documentation links and dependency guards
pass. The 35 curated Fluent SVGs and retained MIT notice match their recorded
hashes; normalization preserves upstream geometry. These observations retain the
same local-render/native-platform limits as the preceding review.

## Query, collection and save lifecycle review — 5 November 2026

The preview now has 39 scenes. The additional cases use existing shared UI
transitions and synthetic responses; they add no database or task service.
Choose one with `--mock-view NAME` or `TABLECRAFT_PREVIEW_SCENE=NAME`:

| Area | Additional scene names |
| --- | --- |
| Queries | `query-running`, `query-error`, `saved-inspecting`, `saved-running`, `saved-error`, `saved-parameters` |
| Collections | `collection-loading`, `collection-error`, `collection-unassigned`, `collection-empty`, `collection-null`, `attachment-null`, `history-null` |
| Editing and saves | `edit-checking`, `edit-validating`, `edit-rejected`, `save-applying`, `save-cancelling`, `save-resolving`, `save-saved`, `save-cancelled`, `save-failed`, `recovery-diagnosis` |

Saved-query rerun/error scenes retain their previous complete sample result.
The parameter pane has an explicit overflow scrollbar; wheel tests reach its
last date input, NULL control and hint without changing bindings. Collection
scenes distinguish unassigned, assigned-empty and actual NULL members. Empty
collections retain declared headers. Initial metadata column sizing samples a
bounded display prefix while preserving complete values and user resizing.
Collection resizing is scoped to the document, parent table and field; other
parent rows of that field retain their widths without affecting unrelated fields.

Terminal save headings distinguish completed, cancelled and needs-attention
outcomes. The last heading does not imply rollback: the literal host message can
retain an established save outcome or unresolved resource-release facts.
Applying/cancelling/resolving scenes exercise existing presentation states only;
no real save, receipt or journal is created by the mock host.

The coordinated batch passes 86 shared UI tests and 23 desktop tests, strict
Clippy and formatting. All 624 scene/language/theme drawing combinations pass
with zero localization diagnostics. Local default-size macOS GPU renders were
reviewed for parameter scrolling, attachment MIME widths, empty headers and
terminal outcome presentation. These observations do not qualify native window
behavior, accessibility tools or Access compatibility. Query parameter,
collection, edit/save and dark surfaces still lack retained Access captures.
