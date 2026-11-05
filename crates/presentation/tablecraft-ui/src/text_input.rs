//! Reject oversized insertions whole before TextEdit can truncate or lay them out.
use egui::text_edit::TextEditState;
use egui::{Event, Id, ImeEvent, Key, Ui};

// Scalar layout and UTF-8 publication limits are separate units. UTF-8 admission
// can reject an insertion below this scalar ceiling; neither accepts a prefix.
pub(crate) const MAX_EDITOR_SCALARS: usize = 64 * 1024;

pub(crate) fn reject_oversized_events(ui: &mut Ui, id: Id, text: &str, max_bytes: usize) -> bool {
    if !ui.memory(|memory| memory.has_focus(id)) {
        return false;
    }
    let mut chars = text.chars().count();
    let mut bytes = text.len();
    let range = TextEditState::load(ui.ctx(), id)
        .and_then(|state| state.cursor.char_range())
        .map(|range| range.as_sorted_char_range());
    let (mut selected_chars, mut selected_bytes) = range.map_or((0, 0), |range| {
        let start = usize::from(range.start).min(chars);
        let end = usize::from(range.end).min(chars);
        let byte_start = text
            .char_indices()
            .nth(start)
            .map_or(text.len(), |(index, _)| index);
        let byte_end = text
            .char_indices()
            .nth(end)
            .map_or(text.len(), |(index, _)| index);
        (
            end.saturating_sub(start),
            byte_end.saturating_sub(byte_start),
        )
    });
    let mut rejected = false;
    ui.input_mut(|input| {
        input.events.retain(|event| {
            let (inserted, composition) = match event {
                Event::Text(text) if text.is_empty() || text == "\n" || text == "\r" => {
                    return true;
                }
                Event::Paste(text) if text.is_empty() => return true,
                Event::Text(text) | Event::Paste(text) | Event::Ime(ImeEvent::Commit(text)) => {
                    (text, false)
                }
                Event::Ime(ImeEvent::Preedit { text, .. }) => (text, true),
                Event::Key {
                    key: Key::A,
                    pressed: true,
                    modifiers,
                    ..
                } if modifiers.command => {
                    selected_chars = chars;
                    selected_bytes = bytes;
                    return true;
                }
                Event::Key {
                    key:
                        Key::ArrowLeft
                        | Key::ArrowRight
                        | Key::ArrowUp
                        | Key::ArrowDown
                        | Key::Home
                        | Key::End
                        | Key::PageUp
                        | Key::PageDown,
                    pressed: true,
                    ..
                }
                | Event::PointerButton { pressed: true, .. }
                | Event::Touch { .. }
                | Event::Ime(_) => {
                    // These events can replace the old selection before an
                    // insertion in this same frame. Its new extent is unknown:
                    // credit no deletion rather than accepting a partial paste.
                    selected_chars = 0;
                    selected_bytes = 0;
                    return true;
                }
                Event::Cut => {
                    chars = chars.saturating_sub(selected_chars);
                    bytes = bytes.saturating_sub(selected_bytes);
                    selected_chars = 0;
                    selected_bytes = 0;
                    return true;
                }
                _ => return true,
            };
            if inserted.len() > max_bytes {
                rejected = true;
                return false;
            }
            let incoming_chars = inserted.chars().count();
            let next_chars = chars
                .saturating_sub(selected_chars)
                .saturating_add(incoming_chars);
            let next_bytes = bytes
                .saturating_sub(selected_bytes)
                .saturating_add(inserted.len());
            if next_chars > MAX_EDITOR_SCALARS || next_bytes > max_bytes {
                rejected = true;
                return false;
            }
            chars = next_chars;
            bytes = next_bytes;
            selected_chars = if composition { incoming_chars } else { 0 };
            selected_bytes = if composition { inserted.len() } else { 0 };
            true
        });
    });
    rejected
}
