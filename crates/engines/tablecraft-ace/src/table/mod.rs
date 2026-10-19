mod attachment;
mod calculated;
pub use attachment::{AttachmentEncoding, DecodedAttachment, decode_attachment};
mod complex;
pub use calculated::{
    StoredCalculation, StoredInt32Calculation, read_stored_calculation,
    read_stored_int32_calculation,
};
mod features;
pub use complex::{AttachmentColumn, StoredAttachment, read_attachment_column};
pub use complex::{
    ComplexColumnLinks, ComplexParentLink, ComplexReferenceViolation, read_complex_column_links,
};
pub use complex::{ComplexColumnSchema, ComplexTableSchema, inspect_complex_schema};
pub use complex::{MultivalueColumn, read_multivalue_column};
pub use complex::{StoredVersion, VersionHistoryColumn, read_version_history_column};
mod mutation;
mod schema;
pub use features::{ColumnFeatures, TableFeatures, inspect_table_features};
pub use indexes::{IndexCheck, IndexCheckStatus, TableCheck, check_table};
#[cfg(feature = "ace16-write-preview")]
pub use mutation::Ace16WritePreview;
#[cfg(feature = "ace16-write-preview")]
pub use mutation::FixedScalarUpdate;
pub use mutation::{EditError, Int32Update, NativeEditPlan, WriteBlocker};
pub use schema::{NativeColumn, TableSchema, inspect_table_schema};
mod record;
mod snapshot;
pub use snapshot::{TableSnapshot, read_table};
mod decimal;
mod extended_datetime;
mod indexes;
pub use indexes::{IndexConstraintViolation, IndexConstraints};
mod long_text;
mod long_value;
mod row_layout;
pub use indexes::{IndexColumn, IndexInspection, NativeIndex, inspect_table_indexes};
pub use indexes::{
    Int32IndexSelection, Int64IndexSelection, read_table_index, read_table_index_range,
    read_table_int64_index_range,
};

pub(crate) use record::decode_selected as decode_projected_record;
pub(crate) use snapshot::read_selected;
