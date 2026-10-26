mod attachment;
mod calculated;
pub use attachment::{
    AttachmentDecodeLimits, AttachmentDecoder, AttachmentEncoding, DecodedAttachment,
    decode_attachment, decode_attachment_with_limits,
};
mod complex;
pub use calculated::{
    CalculationDeclaration, StoredCalculation, StoredInt32Calculation, StoredTablePage,
    inspect_calculation_declaration, read_stored_calculation, read_stored_int32_calculation,
    read_stored_table_page,
};
mod features;
pub use complex::{
    AttachmentColumn, AttachmentMemberDeclaration, AttachmentMetadataColumn, StoredAttachment,
    inspect_attachment_declarations, inspect_attachment_members, read_attachment_column,
    read_attachment_column_with_limits, read_attachment_metadata,
    read_attachment_metadata_with_limits,
};
pub use complex::{
    ComplexColumnLinks, ComplexParentLink, ComplexReferenceViolation, read_complex_column_links,
    read_complex_column_links_with_limits,
};
pub use complex::{ComplexColumnSchema, ComplexTableSchema, inspect_complex_schema};
pub use complex::{
    MultivalueColumn, MultivalueDeclaration, inspect_multivalue_columns,
    inspect_multivalue_declarations, read_multivalue_column, read_multivalue_column_with_limits,
};
pub use complex::{
    MultivaluePageCell, MultivaluePageValue, MultivalueTablePage, read_multivalue_table_page,
    read_multivalue_table_page_with_request,
};
pub use complex::{
    StoredVersion, VersionHistoryColumn, VersionHistoryMemberDeclaration,
    inspect_version_history_declarations, read_version_history_column,
    read_version_history_column_with_limits,
};
mod mutation;
mod schema;
pub use features::{ColumnFeatures, TableFeatures, inspect_table_features};
pub use indexes::{IndexCheck, IndexCheckStatus, TableCheck, check_table};
#[cfg(feature = "ace16-write-preview")]
pub use mutation::{Ace16DeletePreview, Ace16WritePreview, RowDelete, TableRowDeletes};
pub use mutation::{EditError, Int32Update, NativeEditPlan, WriteBlocker};
#[cfg(feature = "ace16-write-preview")]
pub use mutation::{FixedColumnUpdates, FixedScalarUpdate};
pub use schema::{NativeColumn, TableSchema, inspect_table_schema};
mod page;
mod record;
mod record_layout;
mod security_identifiers;
pub use page::{
    TableBatch, TableCursor, TablePage, TablePageRequest, TableSnapshotLimits, read_table_batched,
    read_table_page, read_table_projection_batched,
};
pub(crate) use security_identifiers::remask;
mod snapshot;
pub use snapshot::{TableSnapshot, read_table};
mod decimal;
mod extended_datetime;
mod indexes;
pub use indexes::{
    BinaryIndexSelection, GeneralEqualityPrefix, GuidIndexSelection, TextIndexSelection,
    read_table_binary_index_range, read_table_guid_index, read_table_text_index_range,
};
pub use indexes::{IndexConstraintViolation, IndexConstraints};
mod long_text;
mod long_value;
mod row_layout;
pub use indexes::{
    BooleanIndexSelection, ByteIndexSelection, CurrencyIndexSelection, DateTimeIndexSelection,
    DecimalIndexSelection, Int16IndexSelection, Int32IndexSelection, Int64IndexSelection,
    read_table_index, read_table_index_range, read_table_int64_index_range,
};
pub use indexes::{
    ForeignIndexInspection, ForeignIndexLink, ForeignIndexReference, inspect_foreign_index_links,
};
pub use indexes::{IndexColumn, IndexInspection, NativeIndex, inspect_table_indexes};

pub(crate) use record::decode_selected as decode_projected_record;
pub(crate) use snapshot::read_selected;

pub use complex::{
    CollectionPageRequest, StoredCollectionColumn, StoredCollectionItem, StoredCollectionPageCell,
    StoredCollectionPageValue, StoredCollectionTablePage, read_stored_collection_table_page,
    read_stored_collection_table_page_with_request,
};

pub use snapshot::read_table_projection;

pub use indexes::{
    IndexProjection, IndexProjectionSelection, IndexReadRequest, read_table_index_in_table_order,
    read_table_index_projection, read_table_index_with_request,
};
