//! Record positions are native snapshot ordinals, never stored row identities.
use super::{Action, Data, DocumentView, State};

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

struct Pending {
    request_id: u64,
    target: RecordTarget,
}

impl State {
    fn record_source_matches(&self, target: &RecordTarget) -> bool {
        self.view == DocumentView::Table
            && !self.backstage_visible()
            && !self.edit_dialog_open()
            && self.contents_request_id().is_none()
            && self
                .database
                .as_ref()
                .is_some_and(|database| database.document_revision == target.document_revision)
            && self.data.as_ref().is_some_and(|data| {
                data.source_revision == Some(target.document_revision)
                    && data.table == target.table
                    && data.column_index(&target.column).is_some()
            })
    }

    /// Host preflight before replacing any latest-wins read request.
    pub fn can_navigate_record(&self, target: &RecordTarget) -> bool {
        !self.busy && self.records.pending.is_none() && self.record_source_matches(target)
    }

    /// Bind a request accepted by the host without discarding the current page.
    pub fn begin_record_navigation(&mut self, target: RecordTarget, request_id: u64) {
        if self.records.pending.is_none() && self.record_source_matches(&target) {
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
        if data.source_revision != Some(target.document_revision) || data.table != target.table {
            self.records.error = Some("The returned page belongs to another source.".into());
            return false;
        }
        if data.offset != target.parent_ordinal {
            self.records.error =
                Some("The returned page starts at another record position.".into());
            return false;
        }
        if data.column_index(&target.column).is_none() {
            self.records.error =
                Some("The requested column is absent from the returned page.".into());
            return false;
        }
        let position = target
            .parent_ordinal
            .checked_sub(data.offset)
            .and_then(|row| {
                let column = data.column_index(&target.column)?;
                data.rows.get(row as usize)?.get(column)?;
                Some((row as usize, column))
            });
        let Some((row, column)) = position else {
            self.records.error = Some(if !data.has_more {
                "There is no record at that position in this snapshot.".into()
            } else {
                "The returned page does not contain the requested record.".into()
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
            parent_ordinal: data.offset.checked_add(u32::try_from(row).ok()?)?,
            column: data.columns.get(column)?.name.clone(),
        };
        self.record_source_matches(&target).then_some(target)
    }

    fn request_record(&mut self, target: RecordTarget, actions: &mut Vec<Action>) {
        if !self.can_navigate_record(&target) {
            return;
        }
        let data = self.data.as_ref().expect("admitted native source");
        let position = target
            .parent_ordinal
            .checked_sub(data.offset)
            .and_then(|row| {
                let column = data.column_index(&target.column)?;
                data.rows.get(row as usize)?.get(column)?;
                Some((row as usize, column))
            });
        if let Some((row, column)) = position {
            self.table_selection.select(data, row, column);
            self.records.error = None;
        } else {
            actions.push(Action::NavigateRecord { target });
        }
    }

    pub(super) fn record_navigator(&mut self, ui: &mut egui::Ui, actions: &mut Vec<Action>) {
        let Some(current) = self.selected_record_target() else {
            return;
        };
        let idle = !self.busy && self.records.pending.is_none();
        let data = self.data.as_ref().expect("selected native page");
        let last_loaded = u64::from(data.offset) + data.rows.len() as u64;
        let next = current.parent_ordinal.checked_add(1);
        let next_exists = next.is_some_and(|next| data.has_more || u64::from(next) < last_loaded);
        let mut destination = None;

        ui.horizontal(|ui| {
            ui.label("Record:");
            if ui
                .add_enabled(
                    idle && current.parent_ordinal > 0,
                    egui::Button::new("First"),
                )
                .clicked()
            {
                destination = Some(0);
            }
            if ui
                .add_enabled(
                    idle && current.parent_ordinal > 0,
                    egui::Button::new("Previous"),
                )
                .clicked()
            {
                destination = current.parent_ordinal.checked_sub(1);
            }
            ui.label((u64::from(current.parent_ordinal) + 1).to_string());
            if ui
                .add_enabled(idle && next_exists, egui::Button::new("Next"))
                .clicked()
            {
                destination = next;
            }
            ui.separator();
            let label = ui.label("Go to record");

            let input = ui
                .add_enabled(
                    idle,
                    egui::TextEdit::singleline(&mut self.records.input)
                        .id(egui::Id::new("record_number"))
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
            if ui.add_enabled(idle, egui::Button::new("Go")).clicked() || idle && enter {
                match parse_record(&self.records.input) {
                    Some(ordinal) => destination = Some(ordinal),
                    None => {
                        self.records.error =
                            Some("Enter a whole record number from 1 to 4294967296.".into());
                    }
                }
            }
            if self.records.pending.is_some() {
                ui.spinner();
            }
        });
        if let Some(error) = &self.records.error {
            ui.label(error);
        }
        if let Some(parent_ordinal) = destination {
            self.request_record(
                RecordTarget {
                    parent_ordinal,
                    ..current
                },
                actions,
            );
        }
    }
}

fn parse_record(input: &str) -> Option<u32> {
    let number = input.trim().parse::<u64>().ok()?;
    u32::try_from(number.checked_sub(1)?).ok()
}

#[cfg(test)]
mod tests;
