//! Revision-bound read-only collection dialogs; no database or payload decoding.
use super::{Action, Data, DocumentView, State};
use crate::{CellKind, datasheet};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ContentsTarget {
    pub document_revision: u64,
    pub table: String,
    pub column: String,
    pub parent_ordinal: u32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CollectionKind {
    Multivalue,
    Attachments,
    VersionHistory,
}

pub(super) struct ContentsDialog {
    target: ContentsTarget,
    request_id: u64,
    result: ContentsResult,
    selection: datasheet::Selection,
}

enum ContentsResult {
    Loading,
    Ready {
        kind: CollectionKind,
        assigned: bool,
        data: Data,
    },
    Failed(String),
}

impl Data {
    pub(crate) fn contents_target(&self, row: usize, column: usize) -> Option<ContentsTarget> {
        let cell = self.rows.get(row)?.get(column)?;
        if matches!(cell.kind, CellKind::Scalar { .. }) {
            return None;
        }
        Some(ContentsTarget {
            document_revision: self.source_revision?,
            table: self.table.clone(),
            column: self.columns.get(column)?.clone(),
            parent_ordinal: self.offset.checked_add(u32::try_from(row).ok()?)?,
        })
    }
}

impl State {
    /// Only actual collection cells in the current native page are actionable.
    pub fn can_view_contents(&self, target: &ContentsTarget) -> bool {
        if self.edit_dialog_open()
            || self.view != DocumentView::Table
            || self.table_selection.row_selected()
            || self.backstage_visible()
            || self.database.as_ref().map(|db| db.document_revision)
                != Some(target.document_revision)
        {
            return false;
        }
        let Some(data) = &self.data else {
            return false;
        };
        if data.source_revision != Some(target.document_revision) || data.table != target.table {
            return false;
        }
        let Some(row) = target
            .parent_ordinal
            .checked_sub(data.offset)
            .map(|row| row as usize)
        else {
            return false;
        };
        let Some(column) = data.columns.iter().position(|name| name == &target.column) else {
            return false;
        };
        data.rows
            .get(row)
            .and_then(|row| row.get(column))
            .is_some_and(|cell| !matches!(cell.kind, CellKind::Scalar { .. }))
    }

    /// Bind the dialog to the actual host request, after target admission.
    pub fn begin_contents(&mut self, target: ContentsTarget, request_id: u64) {
        if self.can_view_contents(&target) {
            self.reset_record_navigation();
            self.contents = Some(ContentsDialog {
                target,
                request_id,
                result: ContentsResult::Loading,
                selection: datasheet::Selection::default(),
            });
        }
    }

    pub fn contents_request_id(&self) -> Option<u64> {
        self.contents.as_ref().map(|dialog| dialog.request_id)
    }

    /// Closed, replaced, re-sourced or re-navigated dialogs ignore late replies.
    pub fn set_contents(
        &mut self,
        request_id: u64,
        target: ContentsTarget,
        kind: CollectionKind,
        assigned: bool,
        mut data: Data,
    ) {
        if !self.accepts_contents(request_id, &target) {
            return;
        }
        // Child rows are observations, never a new native parent capability.
        data.source_revision = None;
        if let Some(dialog) = &mut self.contents {
            dialog.selection.reset(&data);
            dialog.result = ContentsResult::Ready {
                kind,
                assigned,
                data,
            };
        }
    }

    pub fn fail_contents(&mut self, request_id: u64, target: ContentsTarget, message: String) {
        if !self.accepts_contents(request_id, &target) {
            return;
        }
        if let Some(dialog) = &mut self.contents {
            dialog.result = ContentsResult::Failed(message);
        }
    }

    fn accepts_contents(&self, request_id: u64, target: &ContentsTarget) -> bool {
        self.can_view_contents(target)
            && self
                .contents
                .as_ref()
                .is_some_and(|dialog| dialog.request_id == request_id && &dialog.target == target)
    }

    pub(super) fn selected_contents_text(&self) -> Option<&str> {
        let dialog = self.contents.as_ref()?;
        if !self.can_view_contents(&dialog.target) {
            return None;
        }
        let ContentsResult::Ready { data, .. } = &dialog.result else {
            return None;
        };
        dialog.selection.text(data)
    }

    pub(super) fn can_copy_contents_selection(&self) -> bool {
        let Some(dialog) = &self.contents else {
            return false;
        };
        if !self.can_view_contents(&dialog.target) {
            return false;
        }
        match &dialog.result {
            ContentsResult::Ready { data, .. } => dialog.selection.can_copy(data),
            _ => false,
        }
    }

    pub(super) fn copy_contents_selection(&mut self, ctx: &egui::Context) -> bool {
        if !self.can_copy_contents_selection() {
            return false;
        }
        let Some(dialog) = &mut self.contents else {
            return false;
        };
        match &dialog.result {
            ContentsResult::Ready { data, .. } => dialog.selection.copy(data, ctx),
            _ => false,
        }
    }

    pub fn close_contents(&mut self) {
        self.contents = None;
    }

    pub(super) fn contents_dialog(&mut self, ctx: &egui::Context, actions: &mut Vec<Action>) {
        let Some(dialog) = &self.contents else {
            return;
        };
        let request_id = dialog.request_id;
        if !self.can_view_contents(&dialog.target) {
            self.close_contents();
            actions.push(Action::CloseContents { request_id });
            return;
        }
        let dialog = self.contents.as_mut().expect("validated open dialog");
        let mut close = false;
        let response = egui::Modal::new(egui::Id::new("collection_contents")).show(ctx, |ui| {
            ui.set_width((ctx.content_rect().width() - 80.0).clamp(320.0, 880.0));
            ui.horizontal(|ui| {
                ui.heading("Collection contents");
                close = ui.button("Close").clicked();
            });
            ui.label(format!(
                "{} · {} · parent {}",
                dialog.target.table,
                dialog.target.column,
                u64::from(dialog.target.parent_ordinal) + 1,
            ));
            ui.separator();

            match &dialog.result {
                ContentsResult::Loading => {
                    ui.horizontal(|ui| {
                        ui.spinner();
                        ui.label("Reading stored contents…");
                    });
                }
                ContentsResult::Failed(message) => {
                    ui.label(message);
                }
                ContentsResult::Ready { kind, assigned, data } => {
                    ui.label(membership(*kind, *assigned, data.rows.len()));
                    if *kind == CollectionKind::Attachments {
                        ui.small("Stored metadata only. Attachment bytes remain encoded; nothing is opened or exported.");
                    }
                    if *kind == CollectionKind::VersionHistory {
                        ui.small("Stored order and original timestamp bits; current Memo text is separate.");
                    }
                    if !data.rows.is_empty() {
                        ui.add_space(4.0);
                        let height = (ctx.content_rect().height() - 260.0).clamp(160.0, 440.0);
                        let size = egui::vec2(ui.available_width(), height);
                        // The datasheet must receive its own bounded viewport
                        // starting below the modal labels, never the modal's
                        // full rectangle (which includes its heading and Close).
                        ui.allocate_ui_with_layout(
                            size,
                            egui::Layout::top_down(egui::Align::LEFT),
                            |ui| {
                                ui.set_clip_rect(ui.clip_rect().intersect(ui.max_rect()));
                                let _ = datasheet::show(ui, data, &mut dialog.selection);
                            },
                        );
                    }
                }
            }
        });
        if close || response.should_close() {
            self.close_contents();
            actions.push(Action::CloseContents { request_id });
        }
    }
}

fn membership(kind: CollectionKind, assigned: bool, count: usize) -> String {
    let name = match kind {
        CollectionKind::Multivalue => "multivalue",
        CollectionKind::Attachments => "attachment",
        CollectionKind::VersionHistory => "stored history",
    };
    crate::cells::membership(name, assigned, count)
}

#[cfg(test)]
mod tests;
