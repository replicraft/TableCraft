//! Own opaque filter sessions; widgets receive only inert source witnesses.
mod field;
use super::{Desktop, PendingOperation};
use crate::view_models;
use crate::workspace::request_read;
use tablecraft_runtime::{
    self as runtime, Command, FilterDefinition, FilterHandle, FilteredData as RuntimeFilteredData,
};
use tablecraft_ui::{
    Data, FilterComparison, FilterPublication, FilterRestore, FilterSelectionTarget, FilterTarget,
    RowOrdinals, SortSpec, SortTarget,
};

#[derive(Clone)]
pub(super) struct RestoreFilter {
    definition: FilterDefinition,
    enabled: bool,
    offset: u32,
    sort: Option<SortSpec>,
    filter_count: u32,
}

impl RestoreFilter {
    pub(super) fn capture(handle: &FilterHandle, enabled: bool, offset: u32) -> Self {
        Self {
            definition: handle.definition(),
            enabled,
            offset,
            sort: handle.sort().map(|sort| SortSpec {
                // NO_CLONE: Retained filter restore preferences must survive invalidation of the
                // source- bound FilterHandle; the sort column is an independent owned
                // inert preference.
                column: sort.column.clone(),
                descending: sort.descending,
            }),
            filter_count: handle.filter_count(),
        }
    }

    pub(super) fn table(&self) -> &str {
        self.definition.table()
    }
}

impl Desktop {
    pub(super) fn reapply_object_filter(&mut self, retained: RestoreFilter) {
        let table = retained.table().to_owned();
        let Some(database) = &self.state.database else {
            return;
        };
        let restore = FilterRestore {
            document_revision: database.document_revision,
            table: table.to_owned(),
            enabled: retained.enabled,
            offset: retained.offset,
            sort: retained.sort,
            filter_count: retained.filter_count,
        };
        let Some(request_id) = request_read(
            Command::ReapplyFilter {
                definition: retained.definition,
                document_revision: database.document_revision,
                enabled: retained.enabled,
                offset: retained.offset,
            },
            &self.worker,
            &mut self.state,
        ) else {
            self.read_request_rejected();
            return;
        };
        self.generation = request_id;
        if !self.state.begin_filter_restore(request_id, restore) {
            self.worker.cancel();
            self.state.error = Some(self.state.localizer.text("filtering-stale"));
            return;
        }
        self.pending = PendingOperation::FilterRestore { request_id };
        self.state.busy = true;
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
                let Some(handle) =
                    self.filter_handle(target.document_revision, &target.table, filter_id)
                else {
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
                // NO_CLONE: The outgoing runtime filter-selection command and UI retained selection
                // request each own the source table identity through asynchronous
                // completion.
                table: target.table.clone(),
                parent_ordinal: target.parent_ordinal,
                // NO_CLONE: The runtime command and UI pending selection each own the selected
                // native column; dropping either owner must not invalidate the other.
                column: target.column.clone(),
                filter_id: target.filter_id,
                enabled: target.enabled,
            },
            comparison: match comparison {
                FilterComparison::Equal => runtime::FilterComparison::Equal,
                FilterComparison::NotEqual => runtime::FilterComparison::NotEqual,
            },
            base,
        };
        let Some(request_id) = request_read(command, &self.worker, &mut self.state) else {
            self.read_request_rejected();
            return;
        };
        self.generation = request_id;
        self.state
            .begin_filter_selection(request_id, target, comparison);
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
        let Some(handle) =
            self.filter_handle(target.document_revision, &target.table, target.filter_id)
        else {
            self.state.error = Some(self.state.localizer.text("filtering-stale"));
            return;
        };
        let Some(request_id) = request_read(
            Command::FilterPage {
                handle,
                offset,
                enabled,
            },
            &self.worker,
            &mut self.state,
        ) else {
            self.read_request_rejected();
            return;
        };
        self.generation = request_id;
        self.state
            .begin_filter_page(request_id, target, offset, enabled);
        self.pending = PendingOperation::FilterPage { request_id };
        self.state.busy = true;
    }

    pub(super) fn clear_table_filter(&mut self, target: FilterTarget) {
        if self.editing.has_save() || self.state.busy || !self.state.can_clear_filter(&target) {
            return;
        }
        let Some(handle) =
            self.filter_handle(target.document_revision, &target.table, target.filter_id)
        else {
            return;
        };
        let Some(request_id) = request_read(
            Command::ClearFilter { handle },
            &self.worker,
            &mut self.state,
        ) else {
            self.read_request_rejected();
            return;
        };
        self.generation = request_id;
        self.state.begin_clear_filter(request_id, target);
        self.pending = PendingOperation::ClearFilter { request_id };
        self.state.busy = true;
    }

    pub(super) fn sort_view(&mut self, target: SortTarget, sort: Option<SortSpec>, enabled: bool) {
        if self.editing.has_save()
            || self.state.busy
            || !self.state.can_sort_view(&target, sort.as_ref(), enabled)
        {
            return;
        }
        let base = if let Some(filter_id) = target.filter_id {
            let Some(handle) =
                self.filter_handle(target.document_revision, &target.table, filter_id)
            else {
                self.state.error = Some(self.state.localizer.text("filtering-stale"));
                return;
            };
            Some(handle)
        } else {
            None
        };
        let Some(request_id) = request_read(
            Command::SortView {
                target: runtime::SortTarget {
                    document_revision: target.document_revision,
                    // NO_CLONE: Runtime SortView and shared pending sort state both retain the
                    // exact table; the request must remain valid after dispatch.
                    table: target.table.clone(),
                    filter_id: target.filter_id,
                },
                sort: sort.as_ref().map(|sort| runtime::SortSpec {
                    // NO_CLONE: Runtime sort specification and UI pending sort independently own
                    // the column; both are checked against eventual publication.
                    column: sort.column.clone(),
                    descending: sort.descending,
                }),
                base,
                enabled,
            },
            &self.worker,
            &mut self.state,
        ) else {
            self.read_request_rejected();
            return;
        };
        self.generation = request_id;
        if !self
            .state
            .begin_sort_view(request_id, target, sort, enabled)
        {
            self.worker.cancel();
            return;
        }
        self.pending = PendingOperation::SortView { request_id };
        self.state.busy = true;
    }

    pub(super) fn filter_handle(
        &self,
        document_revision: u64,
        table: &str,
        filter_id: u64,
    ) -> Option<FilterHandle> {
        self.filter
            .as_ref()
            .filter(|handle| {
                // NO_CLONE: Host and read worker share the immutable source-bound filter snapshot;
                // FilterHandle::clone shares Arc storage rather than deep-copying rows.
                let target = handle.target();
                target.document_revision == document_revision
                    && target.table == table
                    && target.filter_id == filter_id
            })
            .cloned()
    }

    pub(super) fn publish_filtered(&mut self, generation: u64, result: RuntimeFilteredData) {
        if matches!(self.pending, PendingOperation::FindPage { .. }) {
            self.publish_filtered_found(generation, result);
            return;
        }
        let request_id = match &self.pending {
            PendingOperation::FilterSelection { request_id }
            | PendingOperation::FieldFilterApply { request_id }
            | PendingOperation::FilterPage { request_id }
            | PendingOperation::FilterRestore { request_id }
            | PendingOperation::SortView { request_id }
            | PendingOperation::ClearFilter { request_id } => *request_id,
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
            self.complete_object_filter_restore(&handle.target().table);
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
        || result.handle.sort() != result.sort.as_ref()
        || result.handle.filter_count() != result.filter_count
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
    let ordinals = if result.sort.is_some() {
        RowOrdinals::from_display_order(result.source_ordinals)
    } else {
        RowOrdinals::new(result.source_ordinals)
    }
    .map_err(|_| InvalidFilterPublication)?;
    let target = ui_target(result.target);
    let data = Data {
        source_revision: Some(target.document_revision),
        // NO_CLONE: Published filter target and Data.table both retain the table name in separate
        // owned publication fields for downstream source validation.
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
            sort: result.sort.map(|sort| SortSpec {
                column: sort.column,
                descending: sort.descending,
            }),
            filter_count: result.filter_count,
            enabled: result.enabled,
            matched_rows: result.matched_rows,
            source_rows: result.source_rows,
            data,
        },
    ))
}

fn ui_target(target: runtime::FilterTarget) -> FilterTarget {
    FilterTarget {
        document_revision: target.document_revision,
        table: target.table,
        filter_id: target.filter_id,
    }
}

#[cfg(test)]
mod tests;
