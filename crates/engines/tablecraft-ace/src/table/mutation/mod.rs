#[cfg(feature = "ace16-write-preview")]
pub(in crate::table) mod append;
mod apply;
#[cfg(all(test, feature = "ace16-write-preview"))]
mod create_index_tests;
#[cfg(feature = "ace16-write-preview")]
mod delete;
mod error;
mod field;
#[cfg(feature = "ace16-write-preview")]
pub(in crate::table) mod fitted;
#[cfg(feature = "ace16-write-preview")]
pub(in crate::table) mod fixed;
mod gates;
#[cfg(feature = "ace16-write-preview")]
mod guards;
#[cfg(feature = "ace16-write-preview")]
mod index_append;
#[cfg(feature = "ace16-write-preview")]
mod insert;
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

#[cfg(all(test, feature = "ace16-write-preview"))]
mod mixed_fitted_tests;
#[cfg(feature = "ace16-write-preview")]
mod slot;
#[cfg(feature = "ace16-write-preview")]
mod split;
#[cfg(feature = "ace16-write-preview")]
mod split_slot;
#[cfg(feature = "ace16-write-preview")]
mod variable_split;
#[cfg(feature = "ace16-write-preview")]
pub(in crate::table) mod variable_update;
#[cfg(feature = "ace16-write-preview")]
pub use insert::{
    Ace16AutoNumberInsertPreview, Ace16IncrementingInsertBinding, AutoNumberRowInsert,
    IncrementingAutoNumber, InsertFieldValue, OmittedRowInsert, RowInsert,
};
#[cfg(feature = "ace16-write-preview")]
pub use slot::SlotScalarUpdate;
#[cfg(feature = "ace16-write-preview")]
pub(in crate::table) use variable_update::map_audit::CertifiedData;

#[cfg(feature = "ace16-write-preview")]
pub use delete::{Ace16DeletePreview, RowDelete, TableRowDeletes};
pub use error::{EditError, WriteBlocker};
#[cfg(feature = "ace16-write-preview")]
pub use fitted::FittedColumnUpdates;
#[cfg(feature = "ace16-write-preview")]
pub use fixed::{FixedColumnUpdates, FixedScalarUpdate};
#[cfg(feature = "ace16-write-preview")]
pub use guards::{
    BeforeEvent, BeforeEventGuardError, BeforeEventGuardPreparation, BeforeEventGuardSelector,
    BeforeEventIntents, BeforeGuardProgramRevision, BeforeGuardRejection,
    BeforeGuardSelectionError, BeforeGuardTrace, BeforeGuardTraceKind, BeforeGuardUnsupported,
};
pub use plan::{Int32Update, NativeEditPlan};
#[cfg(feature = "ace16-write-preview")]
pub use preview::Ace16WritePreview;
