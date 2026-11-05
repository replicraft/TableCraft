//! Parked presentation values have exactly one owning object.
use super::{ObjectKind, ObjectTab};
use crate::datasheet::Selection;
use crate::workspace::{
    Data, DocumentView, Schema, State, filtering::TableFilter, records::RecordNavigation,
    saved_query::SavedQueryState, totals::Totals,
};
use std::mem;

pub(super) struct Entry {
    pub(super) tab: ObjectTab,
    pub(super) session: Option<Session>,
    pub(super) offset: u32,
    pub(super) retained_bytes: Option<usize>,
}

impl Entry {
    pub(super) fn preferred_offset(&self) -> u32 {
        self.offset
    }
}

pub(super) enum Session {
    Table {
        data: Option<Data>,
        selection: Selection,
        schema: Option<Schema>,
        selected_field: usize,
        filter: TableFilter,
        totals: Totals,
        records: RecordNavigation,
        design: bool,
        error: Option<String>,
    },
    Saved {
        state: SavedQueryState,
        error: Option<String>,
    },
    Sql {
        data: Option<Data>,
        selection: Selection,
        error: Option<String>,
    },
}

impl Session {
    pub(super) fn empty(kind: &ObjectKind) -> Self {
        match kind {
            ObjectKind::Table { .. } => Self::Table {
                data: None,
                selection: Selection::default(),
                schema: None,
                selected_field: 0,
                filter: TableFilter::default(),
                totals: Totals::default(),
                records: RecordNavigation::default(),
                design: false,
                error: None,
            },
            ObjectKind::SavedQuery { .. } => Self::Saved {
                state: SavedQueryState::default(),
                error: None,
            },
            ObjectKind::Sql => Self::Sql {
                data: None,
                selection: Selection::default(),
                error: None,
            },
        }
    }

    pub(super) fn take(state: &mut State, kind: &ObjectKind) -> Self {
        match kind {
            ObjectKind::Table { .. } => Self::Table {
                data: state.data.take(),
                selection: mem::take(&mut state.table_selection),
                schema: state.schema.take(),
                selected_field: state.selected_field,
                filter: mem::take(&mut state.table_filter),
                totals: mem::take(&mut state.totals),
                records: mem::take(&mut state.records),
                design: state.view == DocumentView::Design,
                error: state.error.take(),
            },
            ObjectKind::SavedQuery { .. } => Self::Saved {
                state: mem::take(&mut state.saved_query),
                error: state.error.take(),
            },
            ObjectKind::Sql => Self::Sql {
                data: state.query_data.take(),
                selection: mem::take(&mut state.query_selection),
                error: state.error.take(),
            },
        }
    }

    pub(super) fn install(self, state: &mut State) {
        state.clear_object_slots();
        match self {
            Self::Table {
                data,
                selection,
                schema,
                selected_field,
                filter,
                totals,
                records,
                design,
                error,
            } => {
                state.data = data;
                state.table_selection = selection;
                state.schema = schema;
                state.selected_field = selected_field;
                state.table_filter = filter;
                state.totals = totals;
                state.records = records;
                state.view = if design {
                    DocumentView::Design
                } else {
                    DocumentView::Table
                };
                state.error = error;
            }
            Self::Saved {
                state: saved,
                error,
            } => {
                state.saved_query = saved;
                state.saved_query.active = true;
                state.view = DocumentView::Query;
                state.error = error;
            }
            Self::Sql {
                data,
                selection,
                error,
            } => {
                state.query_data = data;
                state.query_selection = selection;
                state.view = DocumentView::Query;
                state.error = error;
            }
        }
    }
}
