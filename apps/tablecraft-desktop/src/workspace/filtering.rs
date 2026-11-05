//! Own opaque filter sessions; widgets receive only inert source witnesses.
use super::{Desktop, PendingOperation};
use crate::view_models;
use tablecraft_runtime::{
    self as runtime, Command, FilterDefinition, FilterHandle, FilteredData as RuntimeFilteredData,
};
use tablecraft_ui::{
    Data, FilterComparison, FilterPublication, FilterRestore, FilterSelectionTarget, FilterTarget,
    RowOrdinals,
};

pub(super) struct RestoreFilter {
    definition: FilterDefinition,
    enabled: bool,
    offset: u32,
}

impl Desktop {
    pub(super) fn retained_filter_after_save(&self, table: &str) -> Option<RestoreFilter> {
        let target = self.state.active_filter_target()?;
        if target.table != table {
            return None;
        }
        let handle = self.filter_handle(&target)?;
        Some(RestoreFilter {
            definition: handle.definition(),
            enabled: self.state.table_filter_enabled(),
            offset: self.state.data.as_ref()?.offset,
        })
    }

    pub(super) fn restore_filter_after_save(&mut self, table: &str) -> bool {
        let Some(retained) = self.filter_restore.take() else {
            return false;
        };
        let Some(database) = &self.state.database else {
            return false;
        };
        if retained.definition.table() != table {
            self.state.error = Some(self.state.localizer.text("filtering-stale"));
            return true;
        }
        let restore = FilterRestore {
            document_revision: database.document_revision,
            table: table.to_owned(),
            enabled: retained.enabled,
            offset: retained.offset,
        };
        let request_id = self.worker.request(Command::ReapplyFilter {
            definition: retained.definition,
            document_revision: database.document_revision,
            enabled: retained.enabled,
            offset: retained.offset,
        });
        self.generation = request_id;
        if !self.state.begin_filter_restore(request_id, restore) {
            self.worker.cancel();
            self.state.error = Some(self.state.localizer.text("filtering-stale"));
            return true;
        }
        self.pending = PendingOperation::FilterRestore { request_id };
        self.state.busy = true;
        true
    }

    pub(super) fn filter_selection(
        &mut self,
        target: FilterSelectionTarget,
        comparison: FilterComparison,
    ) {
        if self.editing.has_save() || self.state.busy || !self.state.can_filter_selection(&target) {
            return;
        }
        let base = match target.filter_id {
            Some(filter_id) => {
                let expected = FilterTarget {
                    document_revision: target.document_revision,
                    table: target.table.clone(),
                    filter_id,
                };
                let Some(handle) = self.filter_handle(&expected) else {
                    self.state.error = Some(self.state.localizer.text("filtering-stale"));
                    return;
                };
                Some(handle)
            }
            None => None,
        };
        let command = Command::FilterSelection {
            target: runtime::FilterSelectionTarget {
                document_revision: target.document_revision,
                table: target.table.clone(),
                parent_ordinal: target.parent_ordinal,
                column: target.column.clone(),
                filter_id: target.filter_id,
            },
            comparison: match comparison {
                FilterComparison::Equal => runtime::FilterComparison::Equal,
                FilterComparison::NotEqual => runtime::FilterComparison::NotEqual,
            },
            base,
        };
        let request_id = self.worker.request(command);
        self.generation = request_id;
        self.state
            .begin_filter_selection(request_id, target.clone(), comparison);
        self.pending = PendingOperation::FilterSelection { request_id };
        self.state.busy = true;
    }

    pub(super) fn filter_page(&mut self, target: FilterTarget, offset: u32, enabled: bool) {
        if self.editing.has_save()
            || self.state.busy
            || !self.state.can_filter_page(&target, offset, enabled)
        {
            return;
        }
        let Some(handle) = self.filter_handle(&target) else {
            self.state.error = Some(self.state.localizer.text("filtering-stale"));
            return;
        };
        let request_id = self.worker.request(Command::FilterPage {
            handle,
            offset,
            enabled,
        });
        self.generation = request_id;
        self.state
            .begin_filter_page(request_id, target, offset, enabled);
        self.pending = PendingOperation::FilterPage { request_id };
        self.state.busy = true;
    }

    pub(super) fn clear_table_filter(&mut self, target: FilterTarget) {
        if self.editing.has_save()
            || self.state.busy
            || !self.state.can_clear_filter(&target)
            || self.filter_handle(&target).is_none()
        {
            return;
        }
        let request_id = self.worker.request(Command::Page {
            table: target.table.clone(),
            offset: 0,
        });
        self.generation = request_id;
        self.state.begin_clear_filter(request_id, target.clone());
        self.pending = PendingOperation::ClearFilter { request_id, target };
        self.state.busy = true;
    }

    pub(super) fn filter_handle(&self, expected: &FilterTarget) -> Option<FilterHandle> {
        self.filter
            .as_ref()
            .filter(|handle| ui_target(handle.target()) == *expected)
            .cloned()
    }

    pub(super) fn publish_filtered(&mut self, generation: u64, result: RuntimeFilteredData) {
        if matches!(self.pending, PendingOperation::FindPage { .. }) {
            self.publish_filtered_found(generation, result);
            return;
        }
        let request_id = match &self.pending {
            PendingOperation::FilterSelection { request_id }
            | PendingOperation::FilterPage { request_id }
            | PendingOperation::FilterRestore { request_id } => *request_id,
            _ => return,
        };
        if request_id != generation || !self.state.accepts_filter_reply(request_id) {
            return;
        }
        let (handle, publication) = match filtered_publication(result) {
            Ok(publication) => publication,
            Err(_) => {
                self.state.fail_filter(
                    request_id,
                    self.state.localizer.text("filtering-invalid-result"),
                );
                return;
            }
        };
        let accepted = self.state.set_filtered_data(request_id, publication);
        if accepted {
            // Keep the old capability until the complete new publication passes
            // UI source/request checks. Failed or cancelled candidates drop.
            self.filter = Some(handle);
        }
    }
}

pub(super) struct InvalidFilterPublication;

pub(super) fn filtered_publication(
    result: RuntimeFilteredData,
) -> Result<(FilterHandle, FilterPublication), InvalidFilterPublication> {
    if result.handle.target() != &result.target
        || result.handle.matched_rows() != result.matched_rows
        || result.handle.source_rows() != result.source_rows
        || result
            .source_ordinals
            .iter()
            .enumerate()
            .any(|(index, &ordinal)| {
                let visible = u32::try_from(index)
                    .ok()
                    .and_then(|index| result.offset.checked_add(index));
                visible.is_none()
                    || result
                        .handle
                        .display_offset_for_ordinal(result.enabled, ordinal)
                        != visible
            })
    {
        return Err(InvalidFilterPublication);
    }
    let ordinals =
        RowOrdinals::new(result.source_ordinals).map_err(|_| InvalidFilterPublication)?;
    let target = ui_target(&result.target);
    let data = Data {
        source_revision: Some(target.document_revision),
        table: target.table.clone(),
        offset: result.offset,
        columns: view_models::columns(result.columns),
        rows: view_models::rows(result.rows),
        has_more: result.has_more,
        row_ordinals: Some(ordinals),
    };
    Ok((
        result.handle,
        FilterPublication {
            target,
            enabled: result.enabled,
            matched_rows: result.matched_rows,
            source_rows: result.source_rows,
            data,
        },
    ))
}

fn ui_target(target: &runtime::FilterTarget) -> FilterTarget {
    FilterTarget {
        document_revision: target.document_revision,
        table: target.table.clone(),
        filter_id: target.filter_id,
    }
}

#[cfg(test)]
mod tests;
