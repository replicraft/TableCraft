//! Unsupported hosts cannot turn a Find occurrence into an edit.
use crate::{
    Error as RuntimeError, FilterHandle, FindPosition, FindRequest, Response, document::Document,
};

impl Document {
    pub(crate) fn replace_find_match(
        &mut self,
        _request: &FindRequest,
        _found: &FindPosition,
        _replacement: &str,
        _filter: Option<(&FilterHandle, bool)>,
        _cancelled: &dyn Fn() -> bool,
    ) -> Result<Response, RuntimeError> {
        Err(crate::editing::EditError::UnsupportedPlatform.into())
    }
}
