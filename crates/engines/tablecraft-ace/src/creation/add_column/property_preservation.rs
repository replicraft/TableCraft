//! Shared physical property-storage and complement proof; source policy stays local.
mod initial;
mod repeated;
pub(super) use initial::candidate as initial;
pub(super) use repeated::candidate as repeated;

use crate::InspectError;
use thiserror::Error;

#[derive(Debug, Error)]
pub(super) enum PropertyPreservationError {
    #[error(transparent)]
    Inspect(#[from] InspectError),
    #[error("property storage preservation check failed")]
    Preservation,
}
