//! Native offline leases currently have no implementation on this platform.
use super::{EditError, EditInput, EditTarget};
use crate::{Error, Response, document::Document};

impl Document {
    pub(crate) fn begin_edit(
        &mut self,
        _target: &EditTarget,
        _cancelled: &dyn Fn() -> bool,
    ) -> Result<Response, Error> {
        Err(EditError::UnsupportedPlatform.into())
    }

    pub(crate) fn review_edit(
        &mut self,
        _draft_id: u64,
        _input: &EditInput,
        _cancelled: &dyn Fn() -> bool,
    ) -> Result<Response, Error> {
        Err(EditError::UnsupportedPlatform.into())
    }
}
