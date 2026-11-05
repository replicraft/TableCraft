//! Native selection filtering retains physical row identities and runtime values.
mod controls;
mod field;
mod sorting;

pub use field::{
    FieldFilterDescription, FieldFilterOperator, FieldFilterPatternMode, FieldFilterTarget,
};
pub use sorting::{SortSpec, SortTarget};

use super::{Data, DocumentView, State};
use crate::CellKind;
use tablecraft_display::Options;

const MAX_FILTER_FIELDS: u32 = 16;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FilterTarget {
    pub document_revision: u64,
    pub table: String,
    pub filter_id: u64,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FilterSelectionTarget {
    pub document_revision: u64,
    pub table: String,
    pub parent_ordinal: u32,
    pub column: String,
    pub filter_id: Option<u64>,
    pub enabled: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FilterComparison {
    Equal,
    NotEqual,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FilterRestore {
    pub document_revision: u64,
    pub table: String,
    pub enabled: bool,
    pub offset: u32,
    pub sort: Option<SortSpec>,
    pub filter_count: u32,
}

pub struct FilterPublication {
    pub target: FilterTarget,
    pub sort: Option<SortSpec>,
    pub filter_count: u32,
    pub enabled: bool,
    pub matched_rows: u32,
    pub source_rows: u32,
    pub data: Data,
}

#[derive(Default)]
pub(super) struct TableFilter {
    field: Option<Box<field::Session>>,
    view: Option<View>,
    pending: Option<Pending>,
    error: Option<Failure>,
}

struct Failure {
    document_revision: u64,
    table: String,
    source: FailureSource,
    message: String,
}

enum FailureSource {
    View { filter_id: Option<u64> },
    FreshCatalog,
}

struct View {
    target: FilterTarget,
    sort: Option<SortSpec>,
    filter_count: u32,
    enabled: bool,
    matched_rows: u32,
    source_rows: u32,
}

struct Pending {
    request_id: u64,
    publication: u64,
    formatting: Options,
    operation: Operation,
    column: Option<String>,
}

enum Operation {
    Field {
        target: FieldFilterTarget,
        operator: FieldFilterOperator,
    },
    Selection {
        target: FilterSelectionTarget,
        comparison: FilterComparison,
    },
    Page {
        target: FilterTarget,
        offset: u32,
        enabled: bool,
    },
    Clear {
        target: FilterTarget,
    },
    Restore {
        target: FilterRestore,
    },
    Sort {
        target: SortTarget,
        sort: Option<SortSpec>,
        enabled: bool,
    },
}

impl Operation {
    fn source(&self) -> (u64, &str) {
        match self {
            Self::Field { target, .. } => (target.document_revision, &target.table),
            Self::Selection { target, .. } => (target.document_revision, &target.table),
            Self::Restore { target } => (target.document_revision, &target.table),
            Self::Sort { target, .. } => (target.document_revision, &target.table),
            Self::Page { target, .. } | Self::Clear { target } => {
                (target.document_revision, &target.table)
            }
        }
    }
}

impl TableFilter {
    pub(super) fn retained_bytes(&self) -> Option<usize> {
        let mut count = super::objects::memory::Counter::new(size_of::<Self>());
        if let Some(view) = &self.view {
            count.string(&view.target.table);
            if let Some(sort) = &view.sort {
                count.string(&sort.column);
            }
        }
        if let Some(error) = &self.error {
            count.string(&error.table);
            count.string(&error.message);
        }
        if let Some(field) = &self.field {
            count.add(field.retained_bytes()?);
        }
        count.finish()
    }
}

impl State {
    fn filter_source_current(&self, revision: u64, table: &str) -> bool {
        self.database
            .as_ref()
            .is_some_and(|database| database.document_revision == revision)
            && self.data.as_ref().is_some_and(|data| {
                data.source_revision == Some(revision)
                    && data.table == table
                    && data.validate_row_ordinals().is_ok()
            })
    }

    fn filter_controls_ready(&self) -> bool {
        self.view == DocumentView::Table
            && !self.busy
            && !self.backstage_visible()
            && !self.zoom_dialog_open()
            && !self.edit_dialog_open()
            && self.contents_request_id().is_none()
            && !self.saved_query_prompt_open()
            && self.table_filter.pending.is_none()
    }

    pub fn active_filter_target(&self) -> Option<FilterTarget> {
        let view = self.table_filter.view.as_ref()?;
        (self.filter_source_current(view.target.document_revision, &view.target.table)
            && self
                .data
                .as_ref()
                .is_some_and(|data| data.row_ordinals.is_some()))
        .then(|| view.target.clone())
    }

    pub fn table_filter_enabled(&self) -> bool {
        self.active_filter_target().is_some()
            && self
                .table_filter
                .view
                .as_ref()
                .is_some_and(|view| view.enabled && view.filter_count > 0)
    }

    pub(super) fn filtered_record_count(&self) -> Option<u32> {
        self.active_filter_target()?;
        let view = self.table_filter.view.as_ref()?;
        Some(if view.enabled {
            view.matched_rows
        } else {
            view.source_rows
        })
    }

    pub(super) fn table_filter_pending(&self) -> bool {
        self.table_filter.pending.is_some()
    }

    /// A retained view can be republished by another source-bound read, such as
    /// Find, only without changing its membership identity or cardinalities.
    pub(super) fn accepts_filtered_view(&self, publication: &FilterPublication) -> bool {
        let Some(view) = &self.table_filter.view else {
            return false;
        };
        self.active_filter_target().as_ref() == Some(&publication.target)
            && view.enabled == publication.enabled
            && view.sort == publication.sort
            && view.filter_count == publication.filter_count
            && view.matched_rows == publication.matched_rows
            && view.source_rows == publication.source_rows
            && valid_publication(publication)
    }

    pub(super) fn install_filtered_view(&mut self, publication: FilterPublication) -> bool {
        let FilterPublication {
            target,
            sort,
            filter_count,
            enabled,
            matched_rows,
            source_rows,
            data,
        } = publication;
        if !self.set_table_data(data) {
            return false;
        }
        self.table_filter = TableFilter {
            field: None,
            view: Some(View {
                target,
                sort,
                filter_count,
                enabled,
                matched_rows,
                source_rows,
            }),
            pending: None,
            error: None,
        };
        true
    }

    pub fn current_filter_selection(&self) -> Option<FilterSelectionTarget> {
        let data = self.data.as_ref()?;
        let (row, column) = self.table_selection.position()?;
        let cell = data.rows.get(row)?.get(column)?;
        if !matches!(
            cell.kind,
            CellKind::Scalar {
                stored_calculation: false,
                ..
            }
        ) {
            return None;
        }
        let target = FilterSelectionTarget {
            document_revision: data.source_revision?,
            table: data.table.clone(),
            parent_ordinal: data.parent_ordinal(row)?,
            column: data.columns.get(column)?.name.clone(),
            filter_id: self.active_filter_target().map(|target| target.filter_id),
            enabled: self.table_filter_enabled(),
        };
        self.can_filter_selection(&target).then_some(target)
    }

    pub fn can_filter_selection(&self, target: &FilterSelectionTarget) -> bool {
        if !self.filter_controls_ready()
            || self.table_selection.row_selected()
            || !self.filter_source_current(target.document_revision, &target.table)
        {
            return false;
        }
        let Some(data) = &self.data else {
            return false;
        };
        let Some((row, column)) = self.table_selection.position() else {
            return false;
        };
        let current_filter = self.active_filter_target().map(|target| target.filter_id);
        let mapped_source_known =
            data.row_ordinals.is_none() || self.active_filter_target().is_some();
        mapped_source_known
            && target.filter_id == current_filter
            && target.enabled == self.table_filter_enabled()
            && data.parent_ordinal(row) == Some(target.parent_ordinal)
            && data
                .columns
                .get(column)
                .is_some_and(|field| field.name == target.column)
            && data
                .rows
                .get(row)
                .and_then(|row| row.get(column))
                .is_some_and(|cell| {
                    matches!(
                        cell.kind,
                        CellKind::Scalar {
                            stored_calculation: false,
                            ..
                        }
                    )
                })
    }

    pub fn can_filter_page(&self, target: &FilterTarget, offset: u32, enabled: bool) -> bool {
        if !self.filter_controls_ready() || self.active_filter_target().as_ref() != Some(target) {
            return false;
        }
        let view = self
            .table_filter
            .view
            .as_ref()
            .expect("retained filter view");
        if enabled && view.filter_count == 0 {
            return false;
        }
        let count = if enabled {
            view.matched_rows
        } else {
            view.source_rows
        };
        offset < count || offset == 0 && count == 0
    }

    pub fn can_clear_filter(&self, target: &FilterTarget) -> bool {
        self.filter_controls_ready()
            && self.active_filter_target().as_ref() == Some(target)
            && self.table_filter_count() > 0
    }

    fn begin_filter_operation(&mut self, request_id: u64, operation: Operation) {
        self.table_filter.pending = Some(Pending {
            request_id,
            publication: self.focus_publication,
            formatting: self.formatting.clone(),
            operation,
            column: self.data.as_ref().and_then(|data| {
                let (_, column) = self.table_selection.position()?;
                Some(data.columns.get(column)?.name.clone())
            }),
        });
        self.table_filter.error = None;
    }

    pub fn begin_filter_selection(
        &mut self,
        request_id: u64,
        target: FilterSelectionTarget,
        comparison: FilterComparison,
    ) -> bool {
        if !self.can_filter_selection(&target) {
            return false;
        }
        self.begin_filter_operation(request_id, Operation::Selection { target, comparison });
        true
    }

    pub fn begin_filter_page(
        &mut self,
        request_id: u64,
        target: FilterTarget,
        offset: u32,
        enabled: bool,
    ) -> bool {
        if !self.can_filter_page(&target, offset, enabled) {
            return false;
        }
        self.begin_filter_operation(
            request_id,
            Operation::Page {
                target,
                offset,
                enabled,
            },
        );
        true
    }

    pub fn begin_clear_filter(&mut self, request_id: u64, target: FilterTarget) -> bool {
        if !self.can_clear_filter(&target) {
            return false;
        }
        self.begin_filter_operation(request_id, Operation::Clear { target });
        true
    }

    /// A save acknowledgement can restore an opaque runtime definition only
    /// against the freshly opened catalog, before its first table publication.
    pub fn begin_filter_restore(&mut self, request_id: u64, target: FilterRestore) -> bool {
        let source = self.database.as_ref().is_some_and(|database| {
            database.document_revision == target.document_revision
                && database
                    .tables
                    .iter()
                    .any(|table| table.name == target.table)
        });
        if !source
            || self.data.is_some()
            || self.busy
            || self.backstage_visible()
            || self.zoom_dialog_open()
            || self.edit_dialog_open()
            || self.contents_request_id().is_some()
            || self.saved_query_prompt_open()
            || self.table_filter.pending.is_some()
        {
            return false;
        }
        self.select_view(DocumentView::Table);
        self.begin_filter_operation(request_id, Operation::Restore { target });
        true
    }

    pub fn accepts_filter_reply(&self, request_id: u64) -> bool {
        let Some(pending) = &self.table_filter.pending else {
            return false;
        };
        let (revision, table) = pending.operation.source();
        let source_current = match &pending.operation {
            Operation::Restore { target } => {
                self.data.is_none()
                    && self.database.as_ref().is_some_and(|database| {
                        database.document_revision == target.document_revision
                            && database
                                .tables
                                .iter()
                                .any(|table| table.name == target.table)
                    })
            }
            _ => self.filter_source_current(revision, table),
        };
        pending.request_id == request_id
            && pending.publication == self.focus_publication
            && pending.formatting == self.formatting
            && self.view == DocumentView::Table
            && !self.backstage_visible()
            && !self.zoom_dialog_open()
            && (!self.edit_dialog_open() || self.field_reply_current(request_id))
            && self.contents_request_id().is_none()
            && !self.saved_query_prompt_open()
            && source_current
    }

    pub fn set_filtered_data(&mut self, request_id: u64, publication: FilterPublication) -> bool {
        if !self.accepts_filter_reply(request_id) {
            return false;
        }
        let pending = self
            .table_filter
            .pending
            .as_ref()
            .expect("accepted filter request");
        let expected = match &pending.operation {
            Operation::Field { target, operator } => {
                self.field_reply_current(request_id)
                    && self
                        .table_filter
                        .field
                        .as_ref()
                        .is_some_and(|field| field.operator() == *operator)
                    && publication.target.document_revision == target.document_revision
                    && publication.target.table == target.table
                    && publication.enabled
                    && publication.data.offset == 0
                    && publication.sort == self.table_sort()
                    && publication.filter_count > 0
                    && self
                        .table_filter
                        .view
                        .as_ref()
                        .is_none_or(|view| publication.source_rows == view.source_rows)
                    && (target.enabled || publication.filter_count == 1)
            }
            Operation::Selection { target, .. } => {
                publication.target.document_revision == target.document_revision
                    && publication.target.table == target.table
                    && publication.enabled
                    && publication.data.offset == 0
                    && publication.sort == self.table_sort()
                    && publication.filter_count > 0
                    && self
                        .table_filter
                        .view
                        .as_ref()
                        .is_none_or(|view| publication.source_rows == view.source_rows)
                    && (target.enabled || publication.filter_count == 1)
            }
            Operation::Page {
                target,
                offset,
                enabled,
            } => {
                publication.target == *target
                    && self.table_filter.view.as_ref().is_some_and(|view| {
                        publication.matched_rows == view.matched_rows
                            && publication.source_rows == view.source_rows
                            && publication.filter_count == view.filter_count
                            && publication.sort == view.sort
                    })
                    && publication.data.offset == *offset
                    && publication.enabled == *enabled
            }
            Operation::Restore { target } => {
                publication.target.document_revision == target.document_revision
                    && publication.target.table == target.table
                    && publication.enabled == target.enabled
                    && publication.data.offset <= target.offset
                    && publication.sort == target.sort
                    && publication.filter_count == target.filter_count
            }
            Operation::Clear { target } => {
                publication.target.document_revision == target.document_revision
                    && publication.target.table == target.table
                    && publication.filter_count == 0
                    && self
                        .table_filter
                        .view
                        .as_ref()
                        .is_some_and(|view| publication.source_rows == view.source_rows)
                    && !publication.enabled
                    && publication.sort == self.table_sort()
                    && publication.data.offset == 0
            }
            Operation::Sort {
                target,
                sort,
                enabled,
            } => {
                publication.target.document_revision == target.document_revision
                    && publication.target.table == target.table
                    && publication.sort == *sort
                    && publication.enabled == *enabled
                    && publication.filter_count == self.table_filter_count()
                    && publication.data.offset == 0
                    && self.table_filter.view.as_ref().is_none_or(|view| {
                        publication.matched_rows == view.matched_rows
                            && publication.source_rows == view.source_rows
                    })
            }
        };
        if !expected || !valid_publication(&publication) {
            self.fail_filter(request_id, self.localizer.text("filtering-invalid-result"));
            return false;
        }

        let column = pending.column.clone();
        if !self.install_filtered_view(publication) {
            let message = self
                .error
                .take()
                .unwrap_or_else(|| self.localizer.text("filtering-invalid-result"));
            self.fail_filter(request_id, message);
            return false;
        }
        if let Some(data) = &self.data
            && let Some(column) = column
                .as_deref()
                .and_then(|column| data.column_index(column))
            && !data.rows.is_empty()
        {
            self.table_selection.select(data, 0, column);
        }
        true
    }

    pub fn fail_filter(&mut self, request_id: u64, message: String) {
        if self.field_reply_current(request_id) {
            self.fail_field_filter(request_id, message);
            return;
        }
        if !self.accepts_filter_reply(request_id) {
            return;
        }
        let pending = self
            .table_filter
            .pending
            .as_ref()
            .expect("accepted filter request");
        let (document_revision, table) = pending.operation.source();
        let failure = Failure {
            document_revision,
            table: table.to_owned(),
            source: if matches!(pending.operation, Operation::Restore { .. }) {
                FailureSource::FreshCatalog
            } else {
                FailureSource::View {
                    filter_id: self.active_filter_target().map(|target| target.filter_id),
                }
            },
            message,
        };
        self.table_filter.pending = None;
        self.table_filter.error = Some(failure);
    }

    pub(super) fn filter_error_message(&self) -> Option<&str> {
        let failure = self.table_filter.error.as_ref()?;
        let current = match failure.source {
            FailureSource::View { filter_id } => {
                self.filter_source_current(failure.document_revision, &failure.table)
                    && self.active_filter_target().map(|target| target.filter_id) == filter_id
            }
            FailureSource::FreshCatalog => {
                self.data.is_none()
                    && self.database.as_ref().is_some_and(|database| {
                        database.document_revision == failure.document_revision
                            && database
                                .tables
                                .iter()
                                .any(|table| table.name == failure.table)
                    })
            }
        };
        current.then_some(failure.message.as_str())
    }

    pub(super) fn validate_table_filter(&mut self) {
        let stale = self
            .table_filter
            .pending
            .as_ref()
            .is_some_and(|pending| !self.accepts_filter_reply(pending.request_id));
        if stale {
            self.table_filter.pending = None;
        }
    }

    pub(super) fn reset_table_filter(&mut self) {
        self.table_filter = TableFilter::default();
    }
}

fn valid_page_counts(data: &Data, count: u32) -> bool {
    let Ok(length) = u32::try_from(data.rows.len()) else {
        return false;
    };
    data.offset.checked_add(length).is_some_and(|end| {
        end <= count && data.has_more == (end < count) && (!data.rows.is_empty() || count == 0)
    })
}

fn valid_publication(publication: &FilterPublication) -> bool {
    let data = &publication.data;
    let count = if publication.enabled {
        publication.matched_rows
    } else {
        publication.source_rows
    };
    publication.target.filter_id != 0
        && publication.filter_count <= MAX_FILTER_FIELDS
        && (publication.filter_count > 0
            || (!publication.enabled && publication.matched_rows == publication.source_rows))
        && publication
            .sort
            .as_ref()
            .is_none_or(|sort| data.columns.iter().any(|column| column.name == sort.column))
        && publication.matched_rows <= publication.source_rows
        && data.source_revision == Some(publication.target.document_revision)
        && data.table == publication.target.table
        && data.validate_row_ordinals().is_ok()
        && data.row_ordinals.as_ref().is_some_and(|ordinals| {
            (publication.sort.is_some() || ordinals.is_source_ordered())
                && ordinals
                    .as_slice()
                    .iter()
                    .all(|ordinal| *ordinal < publication.source_rows)
        })
        && !data.columns.is_empty()
        && data.rows.iter().all(|row| row.len() == data.columns.len())
        && valid_page_counts(data, count)
}

#[cfg(test)]
mod tests;
