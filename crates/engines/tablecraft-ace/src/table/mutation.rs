#[cfg(any(feature = "ace16-write-preview", feature = "journal"))]
mod ace_profile;
#[cfg(all(feature = "journal", feature = "ace16-create"))]
mod add_field;
#[cfg(feature = "journal")]
mod field_addition_error;
#[cfg(feature = "journal")]
pub use field_addition_error::CurrentFieldAdditionError;
#[cfg(feature = "ace16-write-preview")]
pub(in crate::table) mod append;
mod apply;
#[cfg(any(feature = "journal", feature = "ace16-write-preview"))]
mod autonumber_counter;
#[cfg(all(feature = "journal", feature = "ace16-create"))]
pub(crate) mod catalog_property;
#[cfg(all(test, feature = "ace16-write-preview"))]
mod create_index_tests;
#[cfg(all(feature = "journal", feature = "ace16-create"))]
mod current_row_updates;
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
pub(super) mod fixed_records;
mod gates;
#[cfg(any(feature = "ace16-write-preview", feature = "journal"))]
mod guards;
#[cfg(feature = "ace16-write-preview")]
mod index_append;
#[cfg(feature = "ace16-write-preview")]
pub(in crate::table) mod index_create;
#[cfg(feature = "journal")]
mod index_ownership;
#[cfg(feature = "journal")]
pub(super) mod normal_eof;
#[cfg(all(feature = "journal", feature = "ace16-create"))]
mod retained_opaque_row_updates;
#[cfg(all(feature = "journal", feature = "ace16-create"))]
mod retained_row_updates;
#[cfg(all(feature = "journal", feature = "ace16-create"))]
mod retained_currency_row_updates;
#[cfg(feature = "ace16-write-preview")]
pub use index_create::{Ace16IndexCreatePreview, SecondaryIndexCreate};
#[cfg(all(feature = "ace16-create", feature = "journal"))]
pub(crate) mod current_schema;
#[cfg(all(feature = "ace16-create", feature = "journal"))]
mod drop_field;
#[cfg(feature = "journal")]
pub(crate) mod drop_table;
#[cfg(feature = "ace16-write-preview")]
mod insert;
#[cfg(feature = "ace16-write-preview")]
mod long_value;
#[cfg(any(feature = "ace16-write-preview", feature = "journal"))]
pub(in crate::table) mod lval_ownership;
#[cfg(any(feature = "ace16-write-preview", feature = "journal"))]
pub(in crate::table) mod map_authority;
#[cfg(feature = "journal")]
pub(crate) mod normal_before_change;
#[cfg(feature = "journal")]
mod normal_fitted;
#[cfg(any(
    feature = "ace16-write-preview",
    all(feature = "journal", feature = "ace16-create")
))]
mod owned_lval;
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
#[cfg(all(feature = "ace16-create", feature = "journal"))]
mod rename_field;
#[cfg(feature = "journal")]
mod rename_table;
#[cfg(any(feature = "ace16-write-preview", feature = "journal"))]
mod requests;
mod rows;
#[cfg(all(feature = "ace16-create", feature = "journal"))]
mod saved_field;
#[cfg(feature = "journal")]
pub(in crate::table) mod schema_source;
#[cfg(feature = "journal")]
pub use drop_table::{
    NativeTableDropIdentity, NativeTableDropInspection, NativeTableDropPlan, TableDropError,
};
#[cfg(feature = "journal")]
pub use rename_table::{NativeTableRenameIdentity, NativeTableRenamePlan, TableRenameError};

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
#[path = "mutation/normal_nullable_fixed_int32_sources/anchors.rs"]
mod normal_nullable_fixed_int32_anchors;

#[cfg(all(test, feature = "journal"))]
mod normal_fixed_sql_sources;

#[cfg(all(test, feature = "journal"))]
mod normal_ordinary_sources;

#[cfg(feature = "journal")]
pub(in crate::table) mod ordinary_content;

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
pub(crate) mod insert_defaults;

#[cfg(feature = "journal")]
mod normal_actions;

#[cfg(any(feature = "journal", feature = "ace16-write-preview"))]
mod storage_scope;

#[cfg(any(feature = "journal", feature = "ace16-write-preview"))]
pub(in crate::table) mod map_changes;

#[cfg(feature = "journal")]
pub(crate) mod query_expression;
#[cfg(feature = "journal")]
pub(crate) mod query_expression_edit;
#[cfg(feature = "journal")]
mod query_expression_source;

#[cfg(all(feature = "ace16-create", feature = "journal"))]
pub use add_field::{
    CurrentFieldAdditionSource, NativeCurrentFieldAdditionCandidate,
    NativeCurrentShortTextFieldAdditionCandidate,
};
#[cfg(all(feature = "ace16-create", feature = "journal"))]
pub use drop_field::{
    FieldDropError, NativeCurrentTailFieldDropCandidate, NativeFieldDropCandidate,
    NativeFieldDropIdentity, SavedFieldDropSourcePurpose,
};
#[cfg(all(feature = "ace16-create", feature = "journal"))]
pub use rename_field::{
    FieldRenameError, FittedFieldRenameSource, NativeCurrentFieldRenameCandidate,
    NativeFieldRenameCandidate, NativeFieldRenameIdentity, NativeFittedFieldRenameCandidate,
    NativePopulatedFieldRenameCandidate, NativeRetainedFieldRenameCandidate,
};

#[cfg(all(test, feature = "journal", feature = "ace16-create"))]
mod historical_fixed_sources;

#[cfg(all(feature = "journal", feature = "ace16-create"))]
mod historical_fixed;

#[cfg(all(test, feature = "journal", feature = "ace16-create"))]
mod historical_materialization_sources;

#[cfg(all(feature = "journal", feature = "ace16-create"))]
mod historical_materialization;

#[cfg(all(test, feature = "journal", feature = "ace16-create"))]
mod historical_mixed_materialization_sources;

#[cfg(all(test, feature = "journal", feature = "ace16-create"))]
mod historical_forwarded_materialization_sources;

#[cfg(any(feature = "journal", feature = "ace16-write-preview"))]
mod value_limits;

#[cfg(all(feature = "journal", feature = "ace16-create"))]
mod historical_text_updates;
