mod apply;
#[cfg(feature = "ace16-write-preview")]
mod delete;
mod error;
mod field;
#[cfg(feature = "ace16-write-preview")]
mod fixed;
mod gates;
mod patch;
mod physical;
mod plan;
mod preservation;
#[cfg(feature = "ace16-write-preview")]
mod preview;
mod rows;

#[cfg(feature = "ace16-write-preview")]
pub use delete::{Ace16DeletePreview, RowDelete};
pub use error::{EditError, WriteBlocker};
#[cfg(feature = "ace16-write-preview")]
pub use fixed::{FixedColumnUpdates, FixedScalarUpdate};
pub use plan::{Int32Update, NativeEditPlan};
#[cfg(feature = "ace16-write-preview")]
pub use preview::Ace16WritePreview;
