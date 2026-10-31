#[cfg(any(feature = "ace16-write-preview", feature = "journal"))]
mod ace_profile;
#[cfg(feature = "ace16-write-preview")]
pub(in crate::table) mod append;
mod apply;
#[cfg(all(test, feature = "ace16-write-preview"))]
mod create_index_tests;
mod data_ownership;
#[cfg(feature = "ace16-write-preview")]
mod delete;
mod error;
mod field;
#[cfg(feature = "ace16-write-preview")]
pub(in crate::table) mod fitted;
#[cfg(feature = "ace16-write-preview")]
pub(in crate::table) mod fixed;
mod gates;
#[cfg(any(feature = "ace16-write-preview", feature = "journal"))]
mod guards;
#[cfg(feature = "ace16-write-preview")]
mod index_append;
#[cfg(feature = "ace16-write-preview")]
pub(in crate::table) mod index_create;
#[cfg(feature = "ace16-write-preview")]
pub use index_create::{Ace16IndexCreatePreview, SecondaryIndexCreate};
#[cfg(feature = "ace16-write-preview")]
mod insert;
#[cfg(feature = "ace16-write-preview")]
mod long_value;
#[cfg(any(feature = "ace16-write-preview", feature = "journal"))]
mod lval_ownership;
#[cfg(any(feature = "ace16-write-preview", feature = "journal"))]
pub(in crate::table) mod map_authority;
#[cfg(feature = "journal")]
pub(crate) mod normal_before_change;
#[cfg(feature = "ace16-write-preview")]
mod page_update;
mod patch;
mod physical;
mod plan;
mod preservation;
#[cfg(any(feature = "ace16-write-preview", feature = "journal"))]
mod preserved_source;
#[cfg(feature = "ace16-write-preview")]
mod preview;
#[cfg(any(feature = "ace16-write-preview", feature = "journal"))]
mod record_roots;
#[cfg(any(feature = "ace16-write-preview", feature = "journal"))]
mod requests;
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
pub use guards::{Ace16BeforeChangePreview, BeforeChangePreviewError};
#[cfg(any(feature = "ace16-write-preview", feature = "journal"))]
pub use guards::{
    BeforeChangeAssignment, BeforeChangeFieldPreparation, BeforeChangeFinalRow, BeforeEvent,
    BeforeEventGuardError, BeforeEventGuardPreparation, BeforeEventGuardSelector,
    BeforeEventIntents, BeforeGuardProgramRevision, BeforeGuardRejection,
    BeforeGuardSelectionError, BeforeGuardTrace, BeforeGuardTraceKind, BeforeGuardUnsupported,
};
pub use plan::{Int32Update, NativeEditPlan};
#[cfg(feature = "ace16-write-preview")]
pub use preview::Ace16WritePreview;
#[cfg(feature = "ace16-write-preview")]
pub use requests::FixedColumnUpdates;
#[cfg(any(feature = "ace16-write-preview", feature = "journal"))]
pub use requests::FixedScalarUpdate;

#[cfg(all(test, feature = "ace16-write-preview"))]
mod creation_sources;

#[cfg(all(test, feature = "ace16-write-preview"))]
mod creation_v2_sources;

#[cfg(all(test, feature = "ace16-write-preview"))]
mod creation_fixed_sources;

#[cfg(all(test, feature = "ace16-write-preview"))]
mod creation_fixed_float_sources;

#[cfg(all(test, feature = "ace16-write-preview"))]
mod creation_variable_sources;

#[cfg(all(test, feature = "ace16-write-preview"))]
mod creation_catalog_name_sources;

#[cfg(all(test, feature = "ace16-write-preview"))]
mod normal_before_change_sources;

#[cfg(all(test, feature = "ace16-write-preview"))]
mod creation_variable_capacity_sources;

#[cfg(all(test, feature = "ace16-write-preview"))]
mod creation_multiple_sources;
