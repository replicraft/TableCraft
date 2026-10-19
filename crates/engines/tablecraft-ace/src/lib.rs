//! Independent bounded Jet/ACE reads and offline edit plans. No active content.
#![deny(missing_docs)]
mod catalog;
mod queries;
pub use queries::{
    QueryAttribute, SavedQueryDefinition, SavedQueryRow, read_saved_query_definition,
};
mod constraints;
mod error;
mod format;
mod header;
mod limits;
mod preservation;
mod properties;
mod relationships;
mod table;
mod transaction;
pub use catalog::{CatalogInspection, NativeObjectOrigin, inspect_catalog};
pub use constraints::{
    ConstraintReport, ConstraintViolation, FieldCheck, RelationshipCheck, RelationshipCheckStatus,
    RuleCheckStatus, check_table_constraints,
};
pub use error::InspectError;
pub use format::profiles::ReadProfile;
pub use header::{FileFormat, Header, inspect_header};
pub use preservation::{
    DependencyCoverage, NativeImage, ObjectDependency, ObjectPayloadData, ObjectPayloads,
    OpaquePayload, PayloadPresence,
};
#[cfg(feature = "ace16-write-preview")]
pub use table::Ace16WritePreview;
#[cfg(feature = "ace16-write-preview")]
pub use table::FixedScalarUpdate;
pub use table::{AttachmentColumn, StoredAttachment, read_attachment_column};
pub use table::{AttachmentEncoding, DecodedAttachment, decode_attachment};
pub use table::{ColumnFeatures, TableFeatures, inspect_table_features};
pub use table::{
    ComplexColumnLinks, ComplexParentLink, ComplexReferenceViolation, read_complex_column_links,
};
pub use table::{ComplexColumnSchema, ComplexTableSchema, inspect_complex_schema};
pub use table::{EditError, Int32Update, NativeEditPlan, WriteBlocker};
pub use table::{IndexCheck, IndexCheckStatus, TableCheck, check_table};
pub use table::{IndexColumn, IndexInspection, NativeIndex, inspect_table_indexes};
pub use table::{IndexConstraintViolation, IndexConstraints};
pub use table::{
    Int32IndexSelection, Int64IndexSelection, read_table_index, read_table_index_range,
    read_table_int64_index_range,
};
pub use table::{MultivalueColumn, read_multivalue_column};
pub use table::{NativeColumn, TableSchema, TableSnapshot, inspect_table_schema, read_table};
pub use table::{
    StoredCalculation, StoredInt32Calculation, read_stored_calculation,
    read_stored_int32_calculation,
};
pub use table::{StoredVersion, VersionHistoryColumn, read_version_history_column};
pub use transaction::OfflineReadTransaction;
#[cfg(feature = "journal")]
pub use transaction::{NativeRecoveryError, PreparedNativeEdit, recover_native_edit};

pub use properties::{
    NativeProperty, PropertyGroup, PropertyGroupKind, TableProperties, inspect_table_properties,
};

pub use relationships::{NativeRelationship, inspect_relationships};
