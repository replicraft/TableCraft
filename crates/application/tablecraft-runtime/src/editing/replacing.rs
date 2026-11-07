//! A checked occurrence becomes an ordinary edit draft, never an implicit save.
#[cfg(not(any(target_os = "macos", target_os = "linux", windows)))]
use crate::{
    Error as RuntimeError, FilterHandle, FindPosition, FindRequest, Response, document::Document,
};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum ReplaceError {
    #[error("the original Find occurrence no longer matches its source and options")]
    StaleMatch,
    #[error("replacement is not supported for this carrier or regional format")]
    UnsupportedField,
    #[error("replacement in a lookup or unsupported display control is not available")]
    UnsupportedControl,
    #[error("replacing a stored NULL requires an explicit typed edit")]
    NullMatch,
}

#[cfg(any(target_os = "macos", target_os = "linux", windows))]
mod native;

#[cfg(not(any(target_os = "macos", target_os = "linux", windows)))]
impl Document {
    pub(crate) fn replace_find_match(
        &mut self,
        _request: &FindRequest,
        _found: &FindPosition,
        _replacement: &str,
        _filter: Option<(&FilterHandle, bool)>,
        _cancelled: &dyn Fn() -> bool,
    ) -> Result<Response, RuntimeError> {
        Err(super::EditError::UnsupportedPlatform.into())
    }
}
