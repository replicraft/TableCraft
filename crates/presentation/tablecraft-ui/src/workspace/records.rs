//! Visible record positions remain distinct from native snapshot ordinals.
use super::ribbon::{CommandSize, Icon, command};
use super::{Action, Data, DocumentView, State};
use crate::localization::tr;
use egui::{Popup, accesskit::HasPopup};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RecordTarget {
    pub document_revision: u64,
    pub table: String,
    pub parent_ordinal: u32,
    pub column: String,
}

#[derive(Default)]
pub(super) struct RecordNavigation {
    pending: Option<Pending>,
    input: String,
    error: Option<String>,
}

#[derive(Debug, PartialEq, Eq)]
struct RecordPositions {
    current: u32,
    first: Option<u32>,
    previous: Option<u32>,
    next: Option<u32>,
    last: Option<u32>,
}

struct Pending {
    request_id: u64,
    target: RecordTarget,
}

impl State {
    fn record_source_matches(&self, target: &RecordTarget) -> bool {
        self.view == DocumentView::Table
            && !self.backstage_visible()
            && !self.zoom_dialog_open()
            && !self.edit_dialog_open()
            && !self.saved_query_prompt_open()
            && self.contents_request_id().is_none()
            && self
                .database
                .as_ref()
                .is_some_and(|database| database.document_revision == target.document_revision)
            && self.data.as_ref().is_some_and(|data| {
                data.source_revision == Some(target.document_revision)
                    && data.table == target.table
                    && data.validate_row_ordinals().is_ok()
                    && data.column_index(&target.column).is_some()
            })
    }

    /// Host preflight before replacing any latest-wins read request.
    pub fn can_navigate_record(&self, target: &RecordTarget) -> bool {
        !self.busy
            && self.records.pending.is_none()
            && self.record_source_matches(target)
            && self
                .data
                .as_ref()
                .is_some_and(|data| data.row_ordinals.is_none())
    }

    /// Bind a request accepted by the host without discarding the current page.
    pub fn begin_record_navigation(&mut self, target: RecordTarget, request_id: u64) {
        if self.can_navigate_record(&target) {
            self.records.pending = Some(Pending { request_id, target });
            self.records.error = None;
        }
    }

    /// Publish only a matching native page that contains the requested record.
    pub fn set_record_data(&mut self, request_id: u64, target: RecordTarget, data: Data) -> bool {
        let accepts =
            self.records.pending.as_ref().is_some_and(|pending| {
                pending.request_id == request_id && pending.target == target
            });
        if !accepts || !self.record_source_matches(&target) {
            return false;
        }
        self.records.pending = None;
        if data.source_revision != Some(target.document_revision)
            || data.table != target.table
            || data.row_ordinals.is_some()
            || data.validate_row_ordinals().is_err()
        {
            self.records.error = Some(self.localizer.text("datasheet-wrong-source"));
            return false;
        }
        if data.offset != target.parent_ordinal {
            self.records.error = Some(self.localizer.text("datasheet-wrong-offset"));
            return false;
        }
        if data.column_index(&target.column).is_none() {
            self.records.error = Some(self.localizer.text("datasheet-missing-column"));
            return false;
        }
        let position = data.row_index(target.parent_ordinal).and_then(|row| {
            let column = data.column_index(&target.column)?;
            data.rows.get(row)?.get(column)?;
            Some((row, column))
        });
        let Some((row, column)) = position else {
            self.records.error = Some(if !data.has_more {
                self.localizer.text("datasheet-missing-record")
            } else {
                self.localizer.text("datasheet-incomplete-page")
            });
            return false;
        };

        self.set_table_data(data);
        if let Some(data) = &self.data {
            self.table_selection.select(data, row, column);
        }
        true
    }

    pub fn fail_record_navigation(&mut self, request_id: u64, message: String) {
        if self
            .records
            .pending
            .as_ref()
            .is_some_and(|pending| pending.request_id == request_id)
        {
            self.records.pending = None;
            self.records.error = Some(message);
        }
    }

    pub(super) fn reset_record_navigation(&mut self) {
        self.records = RecordNavigation::default();
    }

    fn selected_record_target(&self) -> Option<RecordTarget> {
        let data = self.data.as_ref()?;
        let (row, column) = self.table_selection.position()?;
        let target = RecordTarget {
            document_revision: data.source_revision?,
            table: data.table.clone(),
            parent_ordinal: data.parent_ordinal(row)?,
            column: data.columns.get(column)?.name.clone(),
        };
        self.record_source_matches(&target).then_some(target)
    }

    fn request_record(&mut self, target: RecordTarget, actions: &mut Vec<Action>) {
        if !self.can_navigate_record(&target) {
            return;
        }
        let data = self.data.as_ref().expect("admitted native source");
        let position = data.row_index(target.parent_ordinal).and_then(|row| {
            let column = data.column_index(&target.column)?;
            data.rows.get(row)?.get(column)?;
            Some((row, column))
        });
        if let Some((row, column)) = position {
            self.table_selection.select(data, row, column);
            self.records.error = None;
        } else {
            actions.push(Action::NavigateRecord { target });
        }
    }

    fn request_display_record(
        &mut self,
        current: RecordTarget,
        display_ordinal: u32,
        actions: &mut Vec<Action>,
    ) {
        if self.busy || self.records.pending.is_some() || !self.record_source_matches(&current) {
            return;
        }
        let data = self.data.as_ref().expect("admitted native page");
        if data.row_ordinals.is_none() {
            self.request_record(
                RecordTarget {
                    parent_ordinal: display_ordinal,
                    ..current
                },
                actions,
            );
            return;
        }

        let position = display_ordinal.checked_sub(data.offset).and_then(|row| {
            let row = usize::try_from(row).ok()?;
            let column = data.column_index(&current.column)?;
            data.rows.get(row)?.get(column)?;
            Some((row, column))
        });
        if let Some((row, column)) = position {
            self.table_selection.select(data, row, column);
            self.records.error = None;
        } else {
            self.records.error = Some(self.localizer.text("datasheet-filter-page-unavailable"));
        }
    }

    fn record_positions(&self) -> Option<RecordPositions> {
        let data = self.data.as_ref()?;
        let (row, _) = self.table_selection.position()?;
        let current = data.display_ordinal(row)?;
        let mapped = data.row_ordinals.is_some();
        let first = (current > 0 && (!mapped || data.offset == 0)).then_some(0);
        let previous = current.checked_sub(1).filter(|_| !mapped || row > 0);
        let next = current
            .checked_add(1)
            .filter(|_| row + 1 < data.rows.len() || (!mapped && data.has_more));
        let last = if data.has_more {
            None
        } else {
            data.rows
                .len()
                .checked_sub(1)
                .and_then(|row| data.display_ordinal(row))
        };
        Some(RecordPositions {
            current,
            first,
            previous,
            next,
            last: last.filter(|last| *last != current),
        })
    }

    pub(super) fn can_select_record(&self) -> bool {
        if self.busy
            || self.backstage_visible()
            || self.edit_dialog_open()
            || self.contents_request_id().is_some()
            || self.saved_query_prompt_open()
        {
            return false;
        }
        let Some(data) = self.visible_data() else {
            return false;
        };
        match self.view {
            DocumentView::Table => {
                self.records.pending.is_none()
                    && data.validate_row_ordinals().is_ok()
                    && self.database.as_ref().is_some_and(|database| {
                        data.source_revision == Some(database.document_revision)
                    })
                    && self.table_selection.can_copy(data)
            }
            DocumentView::Query if self.saved_query.active => {
                self.saved_query.selection.can_copy(data)
            }
            DocumentView::Query => self.query_selection.can_copy(data),
            DocumentView::Design => false,
        }
    }

    pub(super) fn select_current_record(&mut self, context: &egui::Context) -> bool {
        if !self.can_select_record() {
            return false;
        }
        match self.view {
            DocumentView::Table => self
                .table_selection
                .select_record(self.data.as_ref().expect("visible table page"), context),
            DocumentView::Query if self.saved_query.active => {
                self.saved_query.selection.select_record(
                    self.saved_query
                        .data
                        .as_ref()
                        .expect("visible saved result"),
                    context,
                )
            }
            DocumentView::Query => self.query_selection.select_record(
                self.query_data.as_ref().expect("visible query result"),
                context,
            ),
            DocumentView::Design => false,
        }
    }

    pub(super) fn record_menu(&mut self, ui: &mut egui::Ui, actions: &mut Vec<Action>) {
        let current = self.selected_record_target();
        let enabled = current.as_ref().is_some_and(|target| {
            !self.busy && self.records.pending.is_none() && self.record_source_matches(target)
        });
        let response = command(
            ui,
            Icon::GoTo,
            &tr(ui, "ribbon-go-to"),
            enabled,
            CommandSize::Small,
        );
        let popup_id = Popup::default_response_id(&response);
        if !response.enabled() {
            Popup::close_id(ui.ctx(), popup_id);
            return;
        }

        let current = current.expect("enabled native record");
        let RecordPositions {
            first,
            previous,
            next,
            last,
            ..
        } = self.record_positions().expect("selected position");
        let mut destination = None;
        Popup::menu(&response).show(|ui| {
            for (key, target) in [
                ("datasheet-first", first),
                ("datasheet-previous", previous),
                ("datasheet-next", next),
                ("datasheet-last", last),
            ] {
                if ui
                    .add_enabled(target.is_some(), egui::Button::new(tr(ui, key)))
                    .clicked()
                {
                    destination = target;
                    ui.close();
                }
            }
            ui.separator();
            if let Some(ordinal) = self.record_number_input(ui, true, "ribbon_record_number") {
                destination = Some(ordinal);
                ui.close();
            }
            if let Some(error) = &self.records.error {
                ui.label(error);
            }
        });
        let expanded = Popup::is_id_open(ui.ctx(), popup_id);
        ui.ctx().accesskit_node_builder(response.id, |node| {
            node.set_has_popup(HasPopup::Menu);
            node.set_expanded(expanded);
        });
        if let Some(display_ordinal) = destination {
            self.request_display_record(current, display_ordinal, actions);
        }
    }

    fn record_number_input(
        &mut self,
        ui: &mut egui::Ui,
        enabled: bool,
        id: &'static str,
    ) -> Option<u32> {
        let label = ui.label(tr(ui, "datasheet-go-record"));

        let input = ui
            .add_enabled(
                enabled,
                egui::TextEdit::singleline(&mut self.records.input)
                    .id(egui::Id::new(id))
                    .desired_width(80.0),
            )
            .labelled_by(label.id);
        // Single-line TextEdit releases focus on Return before this
        // response is inspected. It still owns that submission frame.
        let enter = (input.has_focus() || input.lost_focus())
            && ui.input_mut(|input| {
                let mut enter = false;
                input.events.retain(|event| {
                    let plain_enter = matches!(
                        event,
                        egui::Event::Key {
                            key: egui::Key::Enter,
                            pressed: true,
                            modifiers,
                            ..
                        } if modifiers.is_none()
                    );
                    enter |= plain_enter;
                    !plain_enter
                });
                enter
            });
        if ui
            .add_enabled(enabled, egui::Button::new(tr(ui, "datasheet-go")))
            .clicked()
            || enabled && enter
        {
            match parse_record(&self.records.input) {
                Some(ordinal) => return Some(ordinal),
                None => {
                    self.records.error = Some(self.localizer.text("datasheet-record-input-help"));
                }
            }
        }
        None
    }

    pub(super) fn record_navigator(&mut self, ui: &mut egui::Ui, actions: &mut Vec<Action>) {
        let target = self.current_insert_target();
        if command(
            ui,
            Icon::Add,
            &tr(ui, "insertion-title"),
            target.is_some(),
            CommandSize::Small,
        )
        .clicked()
            && let Some(target) = target
        {
            actions.push(Action::BeginInsert { target });
        }
        let Some(current) = self.selected_record_target() else {
            return;
        };
        let idle = !self.busy && self.records.pending.is_none();
        let RecordPositions {
            current: position,
            first,
            previous,
            next,
            ..
        } = self.record_positions().expect("selected position");
        let mut destination = None;

        ui.horizontal(|ui| {
            ui.label(tr(ui, "datasheet-record"));
            if ui
                .add_enabled(
                    idle && first.is_some(),
                    egui::Button::new(tr(ui, "datasheet-first")),
                )
                .clicked()
            {
                destination = first;
            }
            if ui
                .add_enabled(
                    idle && previous.is_some(),
                    egui::Button::new(tr(ui, "datasheet-previous")),
                )
                .clicked()
            {
                destination = previous;
            }
            ui.label((u64::from(position) + 1).to_string());
            if ui
                .add_enabled(
                    idle && next.is_some(),
                    egui::Button::new(tr(ui, "datasheet-next")),
                )
                .clicked()
            {
                destination = next;
            }
            ui.separator();
            if let Some(ordinal) = self.record_number_input(ui, idle, "record_number") {
                destination = Some(ordinal);
            }
            if self.records.pending.is_some() {
                ui.spinner();
            }
        });
        if let Some(error) = &self.records.error {
            ui.label(error);
        }
        if let Some(display_ordinal) = destination {
            self.request_display_record(current, display_ordinal, actions);
        }
    }
}

fn parse_record(input: &str) -> Option<u32> {
    let number = input.trim().parse::<u64>().ok()?;
    u32::try_from(number.checked_sub(1)?).ok()
}

#[cfg(test)]
mod tests;
