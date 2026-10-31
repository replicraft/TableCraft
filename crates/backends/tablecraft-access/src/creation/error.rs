use tablecraft_ace::CreateDatabaseError;
use tablecraft_io::publication::PublicationError;
use thiserror::Error;

/// Native input refusal or typed fresh-publication failure; never a retry grant.
#[derive(Debug, Error)]
pub enum CreationError {
    /// The closed native creation profile rejected its explicit typed inputs.
    #[error(transparent)]
    Create(#[from] CreateDatabaseError),
    /// Inspect the retained publication fact before any manual next action.
    #[error(transparent)]
    Publication(#[from] PublicationError),
}
