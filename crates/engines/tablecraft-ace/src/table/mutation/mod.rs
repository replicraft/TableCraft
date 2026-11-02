#[cfg(any(feature = "ace16-write-preview", feature = "journal"))]
mod ace_profile;
#[cfg(feature = "ace16-write-preview")]
pub(in crate::table) mod append;
mod apply;
#[cfg(any(feature = "journal", feature = "ace16-write-preview"))]
mod autonumber_counter;
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
#[cfg(any(feature = "ace16-write-preview", feature = "journal"))]
mod fixed_records;
mod gates;
#[cfg(any(feature = "ace16-write-preview", feature = "journal"))]
mod guards;
#[cfg(feature = "ace16-write-preview")]
mod index_append;
#[cfg(feature = "ace16-write-preview")]
pub(in crate::table) mod index_create;
#[cfg(feature = "journal")]
pub(super) mod normal_eof;
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
#[cfg(feature = "journal")]
mod normal_fitted;
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
mod record_placement;
#[cfg(feature = "journal")]
mod record_protection;
#[cfg(any(feature = "ace16-write-preview", feature = "journal"))]
mod record_roots;
#[cfg(any(feature = "ace16-write-preview", feature = "journal"))]
mod record_serialization;
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
    IncrementingAutoNumber,
};
#[cfg(feature = "ace16-write-preview")]
pub(in crate::table) use variable_update::map_audit::CertifiedData;

#[cfg(feature = "ace16-write-preview")]
pub use delete::Ace16DeletePreview;
pub use error::{EditError, WriteBlocker};
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
#[cfg(any(feature = "ace16-write-preview", feature = "journal"))]
pub use requests::FixedScalarUpdate;
#[cfg(any(feature = "ace16-write-preview", feature = "journal"))]
pub use requests::{FittedColumnUpdates, FixedColumnUpdates, SlotScalarUpdate};

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

#[cfg(all(test, feature = "journal"))]
mod normal_fixed_scalar_sources;

#[cfg(all(test, feature = "journal"))]
mod normal_nullable_fixed_int32_sources;

#[cfg(all(test, feature = "journal"))]
#[path = "normal_nullable_fixed_int32_sources/anchors.rs"]
mod normal_nullable_fixed_int32_anchors;

#[cfg(all(test, feature = "journal"))]
mod normal_fixed_sql_sources;

#[cfg(all(test, feature = "journal"))]
mod normal_ordinary_sources;

#[cfg(feature = "journal")]
mod ordinary_content;

#[cfg(feature = "journal")]
mod normal_ordinary;

#[cfg(feature = "journal")]
mod no_required_programs;

#[cfg(feature = "journal")]
mod normal_delete;

#[cfg(any(feature = "journal", feature = "ace16-write-preview"))]
pub use requests::{RowDelete, TableRowDeletes};

#[cfg(any(feature = "journal", feature = "ace16-write-preview"))]
mod record_insertion;

#[cfg(feature = "journal")]
pub(super) mod normal_insert;

#[cfg(any(feature = "journal", feature = "ace16-write-preview"))]
pub use requests::{InsertFieldValue, OmittedRowInsert, RowInsert};

#[cfg(any(feature = "journal", feature = "ace16-write-preview"))]
mod insert_defaults;

#[cfg(feature = "journal")]
mod normal_actions;

#[cfg(any(feature = "journal", feature = "ace16-write-preview"))]
mod storage_scope;

#[cfg(any(feature = "journal", feature = "ace16-write-preview"))]
pub(in crate::table) mod map_changes;
