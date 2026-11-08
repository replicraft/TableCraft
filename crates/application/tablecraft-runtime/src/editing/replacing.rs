//! A checked occurrence becomes an ordinary edit draft, never an implicit save.
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
