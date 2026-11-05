//! Revision-bound read-only collection dialogs; no database or payload decoding.
use super::ribbon::Icon;
use super::{Action, Data, DocumentView, State};
use crate::localization::{for_context, message, tr};
use crate::{CellKind, datasheet};

const EMPTY_CONTENTS_HEIGHT: f32 = 48.0;
const MODAL_VERTICAL_RESERVE: f32 = 100.0;
const MIN_BODY_HEIGHT: f32 = 120.0;
const MAX_CHILD_HEIGHT: f32 = 440.0;
const ATTACHMENT_HEADING_ICON_SIZE: f32 = 16.0;

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
            column: self.columns.get(column)?.name.clone(),
            parent_ordinal: self.parent_ordinal(row)?,
        })
    }
}

impl State {
    pub(super) fn refresh_contents_display(&mut self) {
        if let Some(dialog) = &mut self.contents
            && let ContentsResult::Ready { data, .. } = &mut dialog.result
        {
            self.format_cache.apply_data(data);
        }
    }

    /// Only actual collection cells in the current native page are actionable.
    pub fn can_view_contents(&self, target: &ContentsTarget) -> bool {
        if self.zoom_dialog_open()
            || self.edit_dialog_open()
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
        let Some(row) = data.row_index(target.parent_ordinal) else {
            return false;
        };
        let Some(column) = data.column_index(&target.column) else {
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
        if data.row_ordinals.is_some() {
            self.fail_contents(
                request_id,
                target,
                self.localizer.text("datasheet-row-identity-invalid"),
            );
            return;
        }
        // Child rows are observations, never a new native parent capability.
        data.source_revision = None;
        self.prepare_display(&mut data);
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
            ui.spacing_mut().scroll = egui::style::ScrollStyle::solid();
            ui.set_width((ctx.content_rect().width() - 80.0).clamp(320.0, 880.0));
            ui.horizontal(|ui| {
                let attachments = matches!(
                    &dialog.result,
                    ContentsResult::Ready {
                        kind: CollectionKind::Attachments,
                        ..
                    }
                );
                let title = if attachments {
                    ui.add(Icon::Attachment.image(ui, ATTACHMENT_HEADING_ICON_SIZE));
                    "collections-attachments-title"
                } else {
                    "collections-title"
                };
                ui.heading(tr(ui, title));
                close = ui
                    .scope_builder(
                        egui::UiBuilder::new()
                            .id(ui.make_persistent_id(("contents_close", request_id))),
                        |ui| ui.button(tr(ui, "backstage-close")),
                    )
                    .inner
                    .clicked();
            });
            let body_height =
                (ctx.content_rect().height() - MODAL_VERTICAL_RESERVE).max(MIN_BODY_HEIGHT);
            egui::ScrollArea::vertical()
                .id_salt("collection_body")
                .auto_shrink([false, true])
                // Measure against the full bound after a shorter Loading state;
                // auto-shrink still contracts the finished viewport to its content.
                .min_scrolled_height(body_height)
                .max_height(body_height)
                .show(ui, |ui| {
                    let body_top = ui.cursor().top();
                    ui.label(message(
                        ui,
                        "collections-target",
                        &[
                            ("table", dialog.target.table.clone()),
                            ("column", dialog.target.column.clone()),
                            (
                                "parent",
                                (u64::from(dialog.target.parent_ordinal) + 1).to_string(),
                            ),
                        ],
                    ));
                    ui.separator();

                    match &dialog.result {
                        ContentsResult::Loading => {
                            ui.horizontal(|ui| {
                                ui.spinner();
                                ui.label(tr(ui, "collections-reading"));
                            });
                        }
                        ContentsResult::Failed(message) => {
                            ui.label(message);
                        }
                        ContentsResult::Ready {
                            kind,
                            assigned,
                            data,
                        } => {
                            ui.label(crate::cells::membership(
                                &for_context(ui.ctx()),
                                *kind,
                                *assigned,
                                data.rows.len(),
                            ));
                            if *kind == CollectionKind::Attachments {
                                ui.small(tr(ui, "collections-attachment-help"));
                            }
                            if *kind == CollectionKind::VersionHistory {
                                ui.small(tr(ui, "collections-history-help"));
                            }
                            ui.add_space(4.0);
                            let height = if data.rows.is_empty() {
                                EMPTY_CONTENTS_HEIGHT
                            } else {
                                (body_height - (ui.cursor().top() - body_top))
                                    .clamp(EMPTY_CONTENTS_HEIGHT, MAX_CHILD_HEIGHT)
                            };
                            let size = egui::vec2(ui.available_width(), height);
                            // Keep the child viewport below modal controls and membership
                            // metadata. Empty collections retain their declared headers.
                            ui.allocate_ui_with_layout(
                                size,
                                egui::Layout::top_down(egui::Align::LEFT),
                                |ui| {
                                    ui.set_clip_rect(ui.clip_rect().intersect(ui.max_rect()));
                                    ui.push_id(
                                        (
                                            dialog.target.document_revision,
                                            &dialog.target.table,
                                            &dialog.target.column,
                                        ),
                                        |ui| {
                                            datasheet::show_collection(
                                                ui,
                                                data,
                                                &mut dialog.selection,
                                            );
                                        },
                                    );
                                },
                            );
                        }
                    }
                });
        });
        if close || response.should_close() {
            self.close_contents();
            actions.push(Action::CloseContents { request_id });
        }
    }
}

#[cfg(test)]
mod tests;
