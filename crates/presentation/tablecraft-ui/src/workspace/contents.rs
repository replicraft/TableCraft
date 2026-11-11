//! Revision-bound read-only collection dialogs; no database or payload decoding.
use super::ribbon::Icon;
use super::{Action, Data, DocumentView, QueryContentsTarget, State};
use crate::localization::{for_context, tr};
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

#[derive(PartialEq, Eq)]
enum ContentsOwner {
    Native(ContentsTarget),
    Query(QueryContentsTarget),
}

pub(super) struct ContentsDialog {
    owner: ContentsOwner,
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
        if !matches!(
            cell.kind,
            CellKind::Multivalue { .. }
                | CellKind::Attachments { .. }
                | CellKind::VersionHistory { .. }
        ) {
            return None;
        }
        Some(ContentsTarget {
            document_revision: self.source_revision?,
            // NO_CLONE: Construct a typed owned target whose table/field identity is retained
            // separately from the borrowed data/catalog/query snapshot. The source
            // observation remains available after command dispatch.
            table: self.table.clone(),
            // NO_CLONE: Construct a typed owned target whose table/field identity is retained
            // separately from the borrowed data/catalog/query snapshot. The source
            // observation remains available after command dispatch.
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
            .is_some_and(|cell| {
                matches!(
                    cell.kind,
                    CellKind::Multivalue { .. }
                        | CellKind::Attachments { .. }
                        | CellKind::VersionHistory { .. }
                )
            })
    }

    /// Bind the dialog to the actual host request, after target admission.
    pub fn begin_contents(&mut self, target: ContentsTarget, request_id: u64) {
        if self.can_view_contents(&target) {
            self.reset_record_navigation();
            self.contents = Some(ContentsDialog {
                owner: ContentsOwner::Native(target),
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
        target: &ContentsTarget,
        kind: CollectionKind,
        assigned: bool,
        mut data: Data,
    ) {
        if !self.accepts_contents(request_id, target) {
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

    pub fn fail_contents(&mut self, request_id: u64, target: &ContentsTarget, message: String) {
        if !self.accepts_contents(request_id, target) {
            return;
        }
        if let Some(dialog) = &mut self.contents {
            dialog.result = ContentsResult::Failed(message);
        }
    }

    fn accepts_contents(&self, request_id: u64, target: &ContentsTarget) -> bool {
        self.can_view_contents(target)
            && self.contents.as_ref().is_some_and(|dialog| {
                dialog.request_id == request_id
                    && matches!(&dialog.owner, ContentsOwner::Native(current) if current == target)
            })
    }

    pub(super) fn selected_contents_text(&self) -> Option<&str> {
        let dialog = self.contents.as_ref()?;
        if !self.can_view_contents_owner(&dialog.owner) {
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
        if !self.can_view_contents_owner(&dialog.owner) {
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

    fn can_view_contents_owner(&self, owner: &ContentsOwner) -> bool {
        match owner {
            ContentsOwner::Native(target) => self.can_view_contents(target),
            ContentsOwner::Query(target) => self.can_view_query_contents(target),
        }
    }

    fn contents_owner_label(&self, owner: &ContentsOwner) -> Option<String> {
        let mut args = crate::FluentArgs::new();
        let key = match owner {
            ContentsOwner::Native(target) => {
                args.set("table", target.table.as_str());
                args.set("column", target.column.as_str());
                args.set("parent", (u64::from(target.parent_ordinal) + 1).to_string());
                "collections-target"
            }
            ContentsOwner::Query(target) => {
                let data = self.query_contents_data()?;
                let entry = data.query_contents.as_ref()?.entry(target)?;
                args.set("column", data.columns.get(entry.column)?.name.as_str());
                args.set("row", (entry.row as u128 + 1).to_string());
                "collections-query-target"
            }
        };
        Some(self.localizer.format(key, &args))
    }

    fn contents_width_scope(&self, owner: &ContentsOwner) -> Option<egui::Id> {
        match owner {
            ContentsOwner::Native(target) => Some(egui::Id::new((
                target.document_revision,
                &target.table,
                &target.column,
            ))),
            ContentsOwner::Query(target) => {
                let data = self.query_contents_data()?;
                let entry = data.query_contents.as_ref()?.entry(target)?;
                // All parents of one projected field share widths; another field
                // or successful result has an independent presentation scope.
                Some(egui::Id::new((
                    "query_contents",
                    target.result_id,
                    entry.column,
                )))
            }
        }
    }

    /// Bind only the actual current query result's inert token after host lookup.
    pub fn begin_query_contents(&mut self, target: QueryContentsTarget, request_id: u64) {
        if self.can_view_query_contents(&target) {
            self.reset_record_navigation();
            self.contents = Some(ContentsDialog {
                owner: ContentsOwner::Query(target),
                request_id,
                result: ContentsResult::Loading,
                selection: datasheet::Selection::default(),
            });
        }
    }

    /// Publish metadata-only child rows without native source or edit identity.
    pub fn set_query_contents(
        &mut self,
        request_id: u64,
        target: &QueryContentsTarget,
        mut data: Data,
    ) {
        if !self.accepts_query_contents(request_id, target) {
            return;
        }
        if data.source_revision.is_some()
            || data.row_ordinals.is_some()
            || data.query_contents.is_some()
        {
            self.fail_query_contents(
                request_id,
                target,
                self.localizer.text("datasheet-row-identity-invalid"),
            );
            return;
        }
        self.prepare_display(&mut data);
        if !self.admit_object_data(&data) {
            return;
        }
        let membership = self.query_contents_data().and_then(|parent| {
            let entry = parent.query_contents.as_ref()?.entry(target)?;
            match parent.rows.get(entry.row)?.get(entry.column)?.kind {
                CellKind::Attachments { assigned, .. } => {
                    Some((CollectionKind::Attachments, assigned))
                }
                CellKind::VersionHistory { assigned, .. } => {
                    Some((CollectionKind::VersionHistory, assigned))
                }
                _ => None,
            }
        });
        if let Some((kind, assigned)) = membership
            && let Some(dialog) = &mut self.contents
        {
            dialog.selection.reset(&data);
            dialog.result = ContentsResult::Ready {
                kind,
                assigned,
                data,
            };
        }
    }

    pub fn fail_query_contents(
        &mut self,
        request_id: u64,
        target: &QueryContentsTarget,
        message: String,
    ) {
        if self.accepts_query_contents(request_id, target)
            && let Some(dialog) = &mut self.contents
        {
            dialog.result = ContentsResult::Failed(message);
        }
    }

    fn accepts_query_contents(&self, request_id: u64, target: &QueryContentsTarget) -> bool {
        self.can_view_query_contents(target)
            && self.contents.as_ref().is_some_and(|dialog| {
                dialog.request_id == request_id
                    && matches!(&dialog.owner, ContentsOwner::Query(current) if current == target)
            })
    }

    pub(in crate::workspace) fn contents_retained_bytes(&self) -> Option<usize> {
        let Some(dialog) = &self.contents else {
            return Some(0);
        };
        let mut bytes = size_of::<ContentsDialog>();
        if let ContentsOwner::Native(target) = &dialog.owner {
            bytes = bytes
                .checked_add(target.table.capacity())?
                .checked_add(target.column.capacity())?;
        }
        match &dialog.result {
            ContentsResult::Loading => Some(bytes),
            ContentsResult::Failed(message) => bytes.checked_add(message.capacity()),
            ContentsResult::Ready { data, .. } => {
                bytes.checked_add(super::objects::memory::data(data)?)
            }
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
        if !self.can_view_contents_owner(&dialog.owner) {
            self.close_contents();
            actions.push(Action::CloseContents { request_id });
            return;
        }
        let Some(target_label) = self.contents_owner_label(&dialog.owner) else {
            return;
        };
        let Some(width_scope) = self.contents_width_scope(&dialog.owner) else {
            return;
        };
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
                    ui.label(&target_label);
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
                                    ui.push_id(width_scope, |ui| {
                                        datasheet::show_collection(ui, data, &mut dialog.selection);
                                    });
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
