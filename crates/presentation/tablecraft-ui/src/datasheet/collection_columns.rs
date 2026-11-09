//! Bounded display-only sizing for initial collection metadata columns.
use crate::Data;
use egui::{TextStyle, Ui};

const INITIAL_MIN_WIDTH: f32 = 128.0;
const INITIAL_MAX_WIDTH: f32 = 320.0;
const SAMPLE_COLUMNS: usize = 32;
const SAMPLE_ROWS: usize = 8;
const SAMPLE_CHARACTERS: usize = 128;
const TEXT_MARGIN: f32 = super::CELL_TEXT_PADDING * 2.0 + 4.0;

/// Only initial widths change. Values, schema, ordering and later user resizing
/// remain untouched; the last column keeps its independent remainder policy.
pub(super) fn initial_widths(ui: &Ui, data: &Data) -> Vec<f32> {
    let mut widths = vec![INITIAL_MIN_WIDTH; data.columns.len()];
    // The layout API consumes this FontId; keep the original style or loop-local descriptor
    // available for the next measurement.
    let font = ui.style().text_styles[&TextStyle::Body].clone();
    let color = ui.visuals().text_color();
    let sampled_columns = data.columns.len().saturating_sub(1).min(SAMPLE_COLUMNS);

    for (index, width) in widths.iter_mut().enumerate().take(sampled_columns) {
        let labels = std::iter::once(data.columns[index].label()).chain(
            data.rows
                .iter()
                .take(SAMPLE_ROWS)
                .filter_map(|row| row.get(index))
                .map(|cell| cell.text.as_str()),
        );
        for label in labels {
            let prefix: String = label.chars().take(SAMPLE_CHARACTERS).collect();
            // The layout API consumes this FontId; keep the original style or loop-local
            // descriptor available for the next measurement.
            let galley = ui.painter().layout_no_wrap(prefix, font.clone(), color);
            *width = width
                .max(galley.size().x + TEXT_MARGIN)
                .clamp(INITIAL_MIN_WIDTH, INITIAL_MAX_WIDTH);
        }
    }
    widths
}
