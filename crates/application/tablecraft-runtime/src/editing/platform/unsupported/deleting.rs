//! Unsupported hosts retain display facts without native deletion authority.
use crate::editing::{DeleteTarget, EditError};
use crate::{Error, Response, document::Document};
use std::path::PathBuf;

pub(in crate::editing) struct State {
    pub(in crate::editing) path: PathBuf,
    pub(in crate::editing) target: DeleteTarget,
    pub(in crate::editing) refresh_ordinal: u32,
}

impl Document {
    pub(crate) fn review_delete(
        &mut self,
        _target: &DeleteTarget,
        _cancelled: &dyn Fn() -> bool,
    ) -> Result<Response, Error> {
        Err(EditError::UnsupportedPlatform.into())
    }
}
