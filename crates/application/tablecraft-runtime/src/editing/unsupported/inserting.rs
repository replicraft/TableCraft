//! Unsupported hosts retain display facts without native insertion authority.
use crate::editing::{EditError, InsertInput, InsertRetryWitness, InsertTarget};
use crate::{Error, Response, document::Document};
use std::{path::PathBuf, sync::Arc};

pub(in crate::editing) struct State {
    pub(in crate::editing) path: PathBuf,
    pub(in crate::editing) target: InsertTarget,
    pub(in crate::editing) retry: Arc<InsertRetryWitness>,
}

impl Document {
    pub(crate) fn begin_insert(
        &mut self,
        _target: &InsertTarget,
        _cancelled: &dyn Fn() -> bool,
    ) -> Result<Response, Error> {
        Err(EditError::UnsupportedPlatform.into())
    }

    pub(crate) fn review_insert(
        &mut self,
        _draft_id: u64,
        _inputs: &[InsertInput],
        _cancelled: &dyn Fn() -> bool,
    ) -> Result<Response, Error> {
        Err(EditError::UnsupportedPlatform.into())
    }
}
