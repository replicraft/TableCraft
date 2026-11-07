//! Paint an exact visible native Find occurrence without changing the text galley.
mod direction;
use super::preview::Preview;
use crate::{Cell, CellKind};
use egui::{Galley, Pos2, Rect, Ui};
use std::ops::Range;

/// Coordinates are resolved afresh from a current source-bound Find receipt.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Occurrence {
    pub row: usize,
    pub column: usize,
    pub start_utf16: u32,
    pub end_utf16: u32,
    pub formatted: bool,
}

impl Occurrence {
    pub(super) fn visible_range(&self, cell: &Cell, preview: &Preview<'_>) -> Option<Range<usize>> {
        if !matches!(cell.kind, CellKind::Scalar { is_null: false, .. })
            || cell.checkbox_value().is_some()
            || !self.formatted && cell.presentation.is_some()
        {
            return None;
        }
        let prefix = if preview.shortened {
            preview.text.strip_suffix('…')?
        } else {
            preview.text.as_ref()
        };
        let bytes = utf8_range(prefix, self.start_utf16, self.end_utf16)?;
        let start = prefix[..bytes.start].chars().count();
        let length = prefix[bytes].chars().count();

        Some(start..start + length)
    }
}

/// Inspect only the already bounded original prefix. Never clamp an endpoint or
/// count the synthetic ellipsis as source text, including a crossing occurrence.
fn utf8_range(prefix: &str, start: u32, end: u32) -> Option<Range<usize>> {
    if start >= end {
        return None;
    }
    let mut utf16 = 0;
    let mut start_byte = None;
    for (byte, character) in prefix.char_indices() {
        if utf16 == start {
            start_byte = Some(byte);
        }
        if utf16 == end {
            return Some(start_byte?..byte);
        }
        utf16 += character.len_utf16() as u32;
        if utf16 > end {
            return None;
        }
    }
    if utf16 != end {
        return None;
    }
    Some(start_byte?..prefix.len())
}

fn rectangles(galley: &Galley, range: Range<usize>) -> Vec<Rect> {
    // Public glyphs expose no logical cluster indices. Repeated RTL characters
    // can compare equal after reordering, so character equality is not enough.
    if !direction::unambiguous(&galley.job.text) {
        return Vec::new();
    }

    // epaint 0.36.2 emits one glyph per scalar, including zero-advance cluster
    // continuations. Count alone does not prove logical order: refuse reordered
    // runs or substitutions rather than painting a different logical occurrence.
    let mut logical = galley.job.text.chars();
    let mut character = 0;
    let mut rectangles = Vec::new();
    for (row_index, row) in galley.rows.iter().enumerate() {
        let elided = galley.elided && row_index + 1 == galley.rows.len();
        let visible_glyphs = row.glyphs.len().saturating_sub(usize::from(elided));
        for (index, glyph) in row.glyphs.iter().take(visible_glyphs).enumerate() {
            if logical.next() != Some(glyph.chr) {
                return Vec::new();
            }
            // A ligature's positive glyph may own later zero-advance source
            // continuations. Public geometry supplies no caret positions inside
            // that glyph: paint only complete groups, never a guessed fraction.
            if glyph.advance_width > 0.0 {
                let continuations = row.glyphs[index + 1..visible_glyphs]
                    .iter()
                    .take_while(|next| next.advance_width == 0.0)
                    .count();
                let group_end = character + 1 + continuations;
                let overlaps = range.start < group_end && character < range.end;
                if overlaps && (range.start > character || group_end > range.end) {
                    return Vec::new();
                }
                if overlaps {
                    rectangles.push(Rect::from_min_size(
                        row.pos + egui::vec2(glyph.pos.x, 0.0),
                        egui::vec2(glyph.advance_width, row.size.y),
                    ));
                }
            }
            character += 1;
        }
        if row.ends_with_newline {
            if logical.next() != Some('\n') {
                return Vec::new();
            }
            character += 1;
        }
    }
    rectangles
}

pub(super) fn paint(ui: &Ui, cell: Rect, position: Pos2, galley: &Galley, range: Range<usize>) {
    let painter = ui.painter().with_clip_rect(ui.clip_rect().intersect(cell));
    let fill = crate::theme::palette(ui.ctx()).active_header;
    for rectangle in rectangles(galley, range) {
        painter.rect_filled(rectangle.translate(position.to_vec2()), 0.0, fill);
    }
}

#[cfg(test)]
mod tests;
