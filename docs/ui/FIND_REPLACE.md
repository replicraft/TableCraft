# Find and reviewed replacement

Home Find and Replace share the modeless native-table search window. It follows
the actual Microsoft Find and Replace reference recorded in
[visual references](VISUAL_REFERENCES.md). Query-result searching remains
unsupported; selected query cells do not acquire native edit identities.

## Find a match first

Find Next searches the complete native source through runtime services. A result
binds the exact database revision, table, original search request, field, native
record ordinal, UTF-16 occurrence span and active filter/sort view. The checked
page publication selects that exact result. The UI retains the original request
separately from the next search's continuation position.

The Replace tab adds Replace With. Replace prepares only the currently selected
verified occurrence. Selecting another record or field, changing search settings,
changing the source or altering the view invalidates that authority. A cancelled
or superseded response cannot open an edit in a new Find session. Replace does
not operate on an arbitrary selected cell.

Find What and Replace With each have a 64 KiB UTF-8 UI limit and a separate scalar
layout ceiling. An oversized insertion is rejected whole before layout, leaving
the prior input visible with a diagnosis. Replacement text is literal: an empty
replacement removes the matched text, and the text `NULL` is not SQL NULL.

## Review before saving

Runtime reacquires the raw native value and verifies the search occurrence and
source-bound view. It constructs the complete replacement input. The UI never
splices a formatted display string or converts a UTF-16 span into guessed byte
positions.

The current runtime profile supports qualified Text and invariant integer
carriers through existing typed edit inputs. Formatted numeric/date replacement,
lookup-dependent values and unsupported native controls refuse explicitly.
Native comparison still requires a qualified database sort policy; replacement
does not bypass unknown-collation refusal.

An accepted replacement closes the modeless search window and opens the ordinary
cell-edit draft with the runtime's complete initial input. Review and Apply are
still explicit. Preparing a replacement does not review it, write database bytes
or start a save worker. Cancelling the draft preserves the file. Normal native
validation, constraints, preservation and recovery restrictions still apply.
Reopening Find after closing that draft starts a new search session.

Replace All is visibly unavailable. There is no per-cell loop that can leave a
silently partially replaced database. A future implementation needs a separately
qualified atomic batch service.

## Preview and qualification

The `replace` and `replace-error` scenes show synthetic controls and explicit
synthetic notices. Focused input carets advance through bounded export frames
rather than waiting for animation to stop. These scenes do not manufacture a
verified native match or prepare a
write. Mock persistence stays unavailable. The actual search, occurrence check,
review and save behavior requires the native runtime and its qualified source
profile.

Light/dark themes and sectioned translations share the existing `find.ftl`
catalogs. These controls are screenshot-guided approximations. No native Access
or Windows interaction qualification is claimed by the headless preview.
