//! Inert result-local bindings never become native parent rows or edit targets.
use crate::{CellKind, Data};

/// A contents selection within one actually published query result.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct QueryContentsTarget {
    pub result_id: u64,
    pub binding: usize,
}

/// Output coordinates associated with a runtime-issued child metadata slot.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct QueryCollectionBinding {
    pub row: usize,
    pub column: usize,
    pub slot: usize,
}

/// Presentation facts only. The host retains the real runtime result capability.
pub struct QueryContentsBindings {
    result_id: u64,
    entries: Vec<QueryCollectionBinding>,
    owned_bytes: usize,
}

impl QueryContentsBindings {
    /// Accept ordered sparse output facts; data publication checks actual cells.
    /// This constructs no native or runtime inspection authority.
    pub fn new(
        result_id: u64,
        entries: Vec<QueryCollectionBinding>,
        owned_bytes: usize,
    ) -> Option<Self> {
        if result_id == 0
            || entries.is_empty()
            || entries
                .iter()
                .enumerate()
                .any(|(position, entry)| entry.slot != position)
            || entries
                .windows(2)
                .any(|pair| (pair[0].row, pair[0].column) >= (pair[1].row, pair[1].column))
        {
            return None;
        }
        Some(Self {
            result_id,
            entries,
            owned_bytes,
        })
    }

    /// Exact issued result generation; never a native database identity.
    pub fn result_id(&self) -> u64 {
        self.result_id
    }

    pub(in crate::workspace) fn retained_bytes(&self) -> Option<usize> {
        self.entries
            .capacity()
            .checked_mul(size_of::<QueryCollectionBinding>())?
            .checked_add(self.owned_bytes)
    }

    pub(in crate::workspace) fn valid_for(&self, data: &Data) -> bool {
        data.source_revision.is_none()
            && data.row_ordinals.is_none()
            && self.entries.iter().all(|entry| {
                data.columns.get(entry.column).is_some()
                    && data
                        .rows
                        .get(entry.row)
                        .and_then(|row| row.get(entry.column))
                        .is_some_and(|cell| matches!(cell.kind, CellKind::Attachments { .. }))
            })
    }

    pub(in crate::workspace) fn entry(
        &self,
        target: &QueryContentsTarget,
    ) -> Option<QueryCollectionBinding> {
        if target.result_id != self.result_id {
            return None;
        }
        self.entries
            .get(target.binding)
            .copied()
            .filter(|entry| entry.slot == target.binding)
    }
}

impl Data {
    pub(crate) fn query_contents_target(
        &self,
        row: usize,
        column: usize,
    ) -> Option<QueryContentsTarget> {
        if self.source_revision.is_some() {
            return None;
        }
        let bindings = self.query_contents.as_ref()?;
        let position = bindings
            .entries
            .binary_search_by_key(&(row, column), |entry| (entry.row, entry.column))
            .ok()?;
        Some(QueryContentsTarget {
            result_id: bindings.result_id,
            binding: bindings.entries[position].slot,
        })
    }
}

impl crate::State {
    /// Inspect only a cell bound to the actual current query-result publication.
    pub fn can_view_query_contents(&self, target: &QueryContentsTarget) -> bool {
        if self.view != crate::workspace::DocumentView::Query
            || self.backstage_visible()
            || self.zoom_dialog_open()
            || self.edit_dialog_open()
            || self.parameter_prompt_open()
            || if self.saved_query.active {
                self.saved_query.selection.row_selected()
            } else {
                self.query_selection.row_selected()
            }
        {
            return false;
        }
        let Some(data) = self.query_contents_data() else {
            return false;
        };
        data.source_revision.is_none()
            && data.query_contents.as_ref().is_some_and(|bindings| {
                bindings.entry(target).is_some_and(|entry| {
                    data.rows
                        .get(entry.row)
                        .and_then(|row| row.get(entry.column))
                        .is_some_and(|cell| matches!(cell.kind, CellKind::Attachments { .. }))
                })
            })
    }

    /// The inert owner of the currently published query result, if any.
    pub fn query_contents_result_id(&self) -> Option<u64> {
        Some(
            self.query_contents_data()?
                .query_contents
                .as_ref()?
                .result_id(),
        )
    }

    pub(in crate::workspace) fn query_contents_data(&self) -> Option<&Data> {
        if self.saved_query.active {
            self.saved_query.data.as_ref()
        } else {
            self.query_data.as_ref()
        }
    }
}

#[cfg(test)]
mod tests;
