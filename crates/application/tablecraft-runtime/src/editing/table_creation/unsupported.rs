//! Unsupported hosts retain reopen facts without table-creation authority.
use super::CreatedTableReopen;
use crate::{Error, Response, document::Document, editing::EditError};
use std::sync::Arc;
use tablecraft_builders::table::TableDraftSnapshot;

pub(in crate::editing) struct State {
    pub(in crate::editing) id: u64,
    pub(in crate::editing) reopen: Arc<CreatedTableReopen>,
}

impl Document {
    pub(crate) fn review_table_creation(
        &mut self,
        _document_revision: u64,
        _snapshot: &TableDraftSnapshot,
        _cancelled: &dyn Fn() -> bool,
    ) -> Result<Response, Error> {
        Err(EditError::UnsupportedPlatform.into())
    }
}
