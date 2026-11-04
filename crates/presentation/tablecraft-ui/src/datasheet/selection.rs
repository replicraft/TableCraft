//! Cell coordinates belong to one owned Data publication, not widget memory.
use super::Data;
use egui::{Id, Key, Ui};

#[derive(Default)]
pub(crate) struct Selection {
    position: Option<(usize, usize)>,
    focus: Option<Id>,
    surrender_focus: Option<Id>,
    focus_pending: bool,
    scroll_pending: Option<usize>,
    reveal_pending: Option<egui::Rect>,
}

impl Selection {
    pub(crate) fn reset(&mut self, data: &Data) {
        let surrender_focus = self.focus.or(self.surrender_focus);
        *self = Self::default();
        self.surrender_focus = surrender_focus;
        if !data.columns.is_empty() && data.rows.first().is_some_and(|row| !row.is_empty()) {
            self.position = Some((0, 0));
        }
    }

    pub(crate) fn clear(&mut self) {
        let surrender_focus = self.focus.or(self.surrender_focus);
        *self = Self::default();
        self.surrender_focus = surrender_focus;
    }

    pub(crate) fn text<'a>(&self, data: &'a Data) -> Option<&'a str> {
        let (row, column) = self.position?;
        data.rows
            .get(row)?
            .get(column)
            .map(|cell| cell.text.as_str())
    }

    pub(super) fn prepare(&mut self, ui: &mut Ui, data: &Data) {
        if let Some(id) = self.surrender_focus.take() {
            ui.memory_mut(|memory| {
                if memory.focused() == Some(id) {
                    memory.surrender_focus(id);
                }
            });
        }
        if self.text(data).is_none() {
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
        let mut consumed_arrow = false;
        ui.input_mut(|input| {
            input.events.retain(|event| {
                let egui::Event::Key {
                    key,
                    pressed: true,
                    modifiers,
                    ..
                } = event
                else {
                    return true;
                };
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
        if before != (row, column) {
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
        self.position == Some((row, column))
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
            if self.position != Some((row, column)) {
                ui.ctx().request_repaint();
            }
            self.position = Some((row, column));
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
        }
    }
}
