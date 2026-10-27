mod apply;
#[cfg(feature = "ace16-write-preview")]
mod delete;
mod error;
mod field;
#[cfg(feature = "ace16-write-preview")]
mod fixed;
mod gates;
#[cfg(feature = "ace16-write-preview")]
mod long_value;
#[cfg(feature = "ace16-write-preview")]
pub(in crate::table) mod map_authority;
#[cfg(feature = "ace16-write-preview")]
mod page_update;
mod patch;
mod physical;
mod plan;
mod preservation;
#[cfg(feature = "ace16-write-preview")]
mod preview;
mod rows;
#[cfg(feature = "ace16-write-preview")]
mod slot;
#[cfg(feature = "ace16-write-preview")]
mod split;
#[cfg(feature = "ace16-write-preview")]
mod variable_update;
#[cfg(feature = "ace16-write-preview")]
pub use slot::SlotScalarUpdate;

#[cfg(feature = "ace16-write-preview")]
pub use delete::{Ace16DeletePreview, RowDelete, TableRowDeletes};
pub use error::{EditError, WriteBlocker};
#[cfg(feature = "ace16-write-preview")]
pub use fixed::{FixedColumnUpdates, FixedScalarUpdate};
pub use plan::{Int32Update, NativeEditPlan};
#[cfg(feature = "ace16-write-preview")]
pub use preview::Ace16WritePreview;
