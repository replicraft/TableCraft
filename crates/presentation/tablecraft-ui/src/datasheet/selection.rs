//! Cell coordinates belong to one owned Data publication, not widget memory.
use super::{Data, clipboard};
use egui::{Id, Key, Ui};

#[derive(Default)]
pub(crate) struct Selection {
    position: Option<(usize, usize)>,
    row_selected: bool,
    copy_error: Option<clipboard::CopyError>,
    focus: Option<Id>,
    surrender_focus: Option<Id>,
    focus_pending: bool,
    pane_focus_witness: Option<PaneFocusWitness>,
    scroll_pending: Option<usize>,
    reveal_pending: Option<egui::Rect>,
}

/// Comparison only; this identifier is never requested as a future target.
struct PaneFocusWitness(Option<Id>);

impl Selection {
    pub(crate) fn reset(&mut self, data: &Data) {
        let surrender_focus = self.focus.or(self.surrender_focus);
        *self = Self::default();
        self.surrender_focus = surrender_focus;
        if !data.columns.is_empty() && data.rows.first().is_some_and(|row| !row.is_empty()) {
            self.position = Some((0, 0));
        }
    }

    pub(crate) fn position(&self) -> Option<(usize, usize)> {
        self.position
    }

    /// Explicit record navigation focuses and reveals the selected loaded cell.
    pub(crate) fn select(&mut self, data: &Data, row: usize, column: usize) {
        if data.rows.get(row).and_then(|row| row.get(column)).is_some() {
            self.position = Some((row, column));
            self.row_selected = false;
            self.copy_error = None;
            self.focus_pending = true;
            self.pane_focus_witness = None;
            self.scroll_pending = Some(row);
        }
    }

    /// Explicit Select record preserves the current native row and field.
    pub(crate) fn select_record(&mut self, data: &Data, context: &egui::Context) -> bool {
        let Some((row, column)) = self.position else {
            return false;
        };
        if data
            .rows
            .get(row)
            .and_then(|values| values.get(column))
            .is_none()
        {
            return false;
        }
        self.row_selected = true;
        self.copy_error = None;
        self.focus_selected(context);
        true
    }

    /// Pane entry reveals the existing selection without changing cell/record mode.
    pub(crate) fn focus_selected(&mut self, context: &egui::Context) {
        if let Some((row, _)) = self.position {
            self.focus_pending = true;
            self.pane_focus_witness =
                Some(PaneFocusWitness(context.memory(|memory| memory.focused())));
            self.scroll_pending = Some(row);
        }
    }

    pub(crate) fn clear(&mut self) {
        let surrender_focus = self.focus.or(self.surrender_focus);
        *self = Self::default();
        self.surrender_focus = surrender_focus;
    }

    pub(crate) fn text<'a>(&self, data: &'a Data) -> Option<&'a str> {
        if self.row_selected {
            return None;
        }
        let (row, column) = self.position?;
        data.rows
            .get(row)?
            .get(column)
            .map(|cell| cell.text.as_str())
    }

    pub(crate) fn row_selected(&self) -> bool {
        self.row_selected
    }

    pub(crate) fn can_copy(&self, data: &Data) -> bool {
        let Some((row, column)) = self.position else {
            return false;
        };
        data.rows.get(row).is_some_and(|values| {
            if self.row_selected {
                !values.is_empty() && values.len() == data.columns.len()
            } else {
                values.get(column).is_some()
            }
        })
    }

    pub(crate) fn copy(&mut self, data: &Data, ctx: &egui::Context) -> bool {
        let Some((row, column)) = self.position else {
            return false;
        };
        self.copy_position(data, ctx, row, column, self.row_selected)
    }

    fn copy_position(
        &mut self,
        data: &Data,
        ctx: &egui::Context,
        row: usize,
        column: usize,
        record: bool,
    ) -> bool {
        if record {
            match clipboard::row_text(data, row) {
                Ok(text) => ctx.copy_text(text),
                Err(error) => {
                    self.copy_error = Some(error);
                    ctx.request_repaint();
                    return false;
                }
            }
        } else if let Some(value) = data.rows.get(row).and_then(|values| values.get(column)) {
            super::copy_text(ctx, &value.text);
        } else {
            return false;
        }
        self.copy_error = None;
        true
    }

    pub(super) fn copy_error(&self, ctx: &egui::Context) -> Option<String> {
        let localizer = crate::localization::for_context(ctx);
        self.copy_error.map(|error| error.message(&localizer))
    }

    pub(super) fn observe_row(
        &mut self,
        ui: &Ui,
        data: &Data,
        row: usize,
        response: &egui::Response,
    ) {
        if ui.is_enabled() && self.row_selected && self.row() == Some(row) && self.focus_pending {
            response.request_focus();
            self.focus = Some(response.id);
            self.focus_pending = false;
            self.pane_focus_witness = None;
        }
        if !ui.is_enabled()
            || !response.clicked() && !response.secondary_clicked() && !response.gained_focus()
        {
            return;
        }
        let column = self.position.map_or(0, |(_, column)| column);
        if data
            .rows
            .get(row)
            .is_none_or(|values| column >= values.len())
        {
            return;
        }
        self.position = Some((row, column));
        self.row_selected = true;
        if response.clicked() || response.secondary_clicked() {
            self.copy_error = None;
        }
        self.focus = Some(response.id);
        response.request_focus();
        ui.ctx().request_repaint();
    }

    pub(super) fn prepare(&mut self, ui: &mut Ui, data: &Data) {
        if self
            .pane_focus_witness
            .as_ref()
            .is_some_and(|witness| ui.memory(|memory| memory.focused()) != witness.0)
        {
            self.focus_pending = false;
            self.pane_focus_witness = None;
            self.scroll_pending = None;
            self.reveal_pending = None;
        }
        if let Some(id) = self.surrender_focus.take() {
            ui.memory_mut(|memory| {
                if memory.focused() == Some(id) {
                    memory.surrender_focus(id);
                }
            });
        }
        if self.position.is_none_or(|(row, column)| {
            data.rows.get(row).and_then(|row| row.get(column)).is_none()
        }) {
            self.reset(data);
        }
        if !ui.is_enabled()
            || self.focus.is_none()
            || ui.memory(|memory| memory.focused()) != self.focus
        {
            return;
        }
        let Some((mut row, mut column)) = self.position else {
            return;
        };
        let before = (row, column);
        let was_row = self.row_selected;
        let mut row_selected = was_row;
        // Capture the last Copy event's exact selection in event order,
        // then serialize outside the input lock.
        let mut copy = None;
        let mut consumed_arrow = false;
        ui.input_mut(|input| {
            input.events.retain(|event| {
                if matches!(event, egui::Event::Copy) {
                    copy = Some((row, column, row_selected));
                    return false;
                }
                let egui::Event::Key {
                    key,
                    pressed: true,
                    modifiers,
                    ..
                } = event
                else {
                    return true;
                };
                if row_selected && *key == Key::Enter && modifiers.is_none() {
                    return false;
                }
                if *key == Key::Space && *modifiers == egui::Modifiers::SHIFT {
                    row_selected = !row_selected;
                    if !row_selected {
                        column = 0;
                    }
                    return false;
                }
                if !modifiers.is_none() {
                    return true;
                }
                match key {
                    Key::ArrowUp => row = row.saturating_sub(1),
                    Key::ArrowDown => {
                        row = row.saturating_add(1).min(data.rows.len().saturating_sub(1))
                    }
                    Key::ArrowLeft => column = column.saturating_sub(1),
                    Key::ArrowRight => column = column.saturating_add(1),
                    _ => return true,
                }
                consumed_arrow = true;
                row_selected = false;
                column = column.min(
                    data.columns
                        .len()
                        .min(data.rows[row].len())
                        .saturating_sub(1),
                );
                false
            });
        });
        if consumed_arrow {
            // egui computes cardinal focus navigation before UI code runs.
            // Cancel that already-computed move only for arrows consumed by
            // the currently focused grid, leaving Tab and Escape untouched.
            ui.memory_mut(|memory| memory.move_focus(egui::FocusDirection::None));
        }
        self.row_selected = row_selected;
        if was_row != row_selected || consumed_arrow {
            self.copy_error = None;
            self.focus_pending = true;
            ui.ctx().request_repaint();
        }
        if let Some((row, column, record)) = copy {
            self.copy_position(data, ui.ctx(), row, column, record);
        }
        if before != (row, column) || was_row && !self.row_selected {
            self.position = Some((row, column));
            self.focus_pending = true;
            self.scroll_pending = Some(row);
            ui.ctx().request_repaint();
        }
    }

    #[cfg(test)]
    pub(super) fn focus_id(&self) -> Option<Id> {
        self.focus
    }

    pub(super) fn row(&self) -> Option<usize> {
        self.position.map(|(row, _)| row)
    }

    pub(super) fn column(&self) -> Option<usize> {
        self.position.map(|(_, column)| column)
    }

    pub(super) fn selected(&self, row: usize, column: usize) -> bool {
        !self.row_selected && self.position == Some((row, column))
    }

    pub(super) fn scroll_to_row(&mut self) -> Option<usize> {
        self.scroll_pending.take()
    }

    pub(super) fn take_reveal(&mut self) -> Option<egui::Rect> {
        self.reveal_pending.take()
    }

    pub(super) fn observe(
        &mut self,
        ui: &Ui,
        row: usize,
        column: usize,
        response: &egui::Response,
    ) {
        if !ui.is_enabled() {
            return;
        }
        if response.clicked()
            || response.secondary_clicked()
            || response.gained_focus() && !self.focus_pending
        {
            if self.position != Some((row, column)) || self.row_selected {
                ui.ctx().request_repaint();
            }
            self.position = Some((row, column));
            self.row_selected = false;
            if response.clicked() || response.secondary_clicked() {
                self.copy_error = None;
            }
            self.focus = Some(response.id);
            if response.clicked() || response.secondary_clicked() {
                response.request_focus();
            }
        }
        if self.selected(row, column) && self.focus_pending {
            response.request_focus();
            self.reveal_pending = Some(response.rect);
            self.focus = Some(response.id);
            self.focus_pending = false;
            self.pane_focus_witness = None;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Cell;

    #[test]
    fn failed_record_copy_publishes_no_clipboard_prefix() {
        let data = Data {
            row_ordinals: None,
            source_revision: None,
            table: "Result".into(),
            offset: 0,
            columns: vec!["Value".into()],
            rows: vec![vec![Cell::scalar("x".repeat(1_048_576))]],
            has_more: false,
        };
        let mut selection = Selection {
            position: Some((0, 0)),
            row_selected: true,
            ..Default::default()
        };
        let context = egui::Context::default();
        let mut output = context.run_ui(egui::RawInput::default(), |ui| {
            assert!(!selection.copy(&data, ui.ctx()));
        });
        assert!(selection.copy_error(&context).is_some());
        assert!(
            !output
                .platform_output
                .commands
                .iter()
                .any(|command| { matches!(command, egui::OutputCommand::CopyText(_)) })
        );
        output.textures_delta.clear();
    }
}
