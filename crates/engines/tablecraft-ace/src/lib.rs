//! Independent bounded Jet/ACE reads and offline edit plans. No active content.
#![deny(missing_docs)]
mod catalog;
mod collation;
pub use collation::{
    GeneralSortFamily, GeneralTextSearch, NativeSortOrder, NativeTextKey,
    compare_french_legacy_text, compare_french_legacy_utf16, compare_general_like_units,
    compare_general_text, compare_general_utf16, encode_text_index_key,
    encode_utf16_text_index_key, find_general_text, find_general_utf16, general_like_literal_step,
    general_like_unit_step, inspect_database_sort_order, is_general_like_unit,
};
mod queries;
pub use queries::{
    QueryAttribute, SavedQueryDefinition, SavedQueryRow, read_saved_query_definition,
};
mod constraints;
mod encryption;
mod error;
mod format;
mod header;
mod limits;
mod source;
pub use source::{
    AgileSource, EncryptedSource, LongTextReadLimits, LongValueReadLimits, LongValueReadSource,
    NativeReadSource,
};
mod preservation;
mod properties;
mod relationships;
mod table;
#[cfg(test)]
mod test_support;
mod transaction;
pub use catalog::{
    CatalogInspection, NativeAccessLinkTarget, NativeLinkText, NativeLinkedTableDefinition,
    NativeObjectOrigin, NativeTableStorage, inspect_catalog, read_linked_table_definition,
};
pub use constraints::{
    ConstraintReport, ConstraintViolation, FieldCheck, RelationshipCheck, RelationshipCheckStatus,
    RuleCheckStatus, check_table_constraints,
};
pub use encryption::{
    AgileEncryption, AgileHashAlgorithm, AgilePageDecoder, AgileParameters, EncryptionCopy,
    EncryptionInspection, EncryptionRewrite, OfficeEncryptionFamily, OfficeEncryptionInfo,
    check_encryption_add, check_encryption_rewrite, inspect_encryption, read_agile_encryption,
    rewrite_encryption, rewrite_encryption_to,
};
pub use error::InspectError;
pub use format::profiles::ReadProfile;
pub use header::{FileFormat, Header, inspect_header};
pub use preservation::{
    DependencyCoverage, NativeImage, ObjectDependency, ObjectPayloadData, ObjectPayloads,
    OpaquePayload, PayloadPresence,
};
#[cfg(feature = "ace16-write-preview")]
pub use table::{Ace16DeletePreview, Ace16WritePreview, RowDelete, TableRowDeletes};
pub use table::{
    AttachmentColumn, AttachmentMetadataColumn, StoredAttachment, read_attachment_column,
    read_attachment_column_with_limits, read_attachment_metadata,
    read_attachment_metadata_with_limits,
};
pub use table::{
    AttachmentDecodeLimits, AttachmentDecoder, AttachmentEncoding, DecodedAttachment,
    decode_attachment, decode_attachment_with_limits,
};
pub use table::{
    AttachmentMemberDeclaration, MultivalueColumn, MultivalueDeclaration,
    inspect_attachment_declarations, inspect_attachment_members, inspect_multivalue_columns,
    inspect_multivalue_declarations, read_multivalue_column, read_multivalue_column_with_limits,
};
pub use table::{
    BinaryIndexSelection, GeneralEqualityPrefix, GuidIndexSelection, TextIndexSelection,
    read_table_binary_index_range, read_table_guid_index, read_table_text_index_range,
};
pub use table::{
    BooleanIndexSelection, ByteIndexSelection, CurrencyIndexSelection, DateTimeIndexSelection,
    DecimalIndexSelection, Int16IndexSelection, Int32IndexSelection, Int64IndexSelection,
    read_table_index, read_table_index_range, read_table_int64_index_range,
};
pub use table::{
    CalculationDeclaration, StoredCalculation, StoredInt32Calculation,
    inspect_calculation_declaration, read_stored_calculation, read_stored_int32_calculation,
};
pub use table::{ColumnFeatures, TableFeatures, inspect_table_features};
pub use table::{
    ComplexColumnLinks, ComplexParentLink, ComplexReferenceViolation, read_complex_column_links,
    read_complex_column_links_with_limits,
};
pub use table::{ComplexColumnSchema, ComplexTableSchema, inspect_complex_schema};
pub use table::{EditError, Int32Update, NativeEditPlan, WriteBlocker};
#[cfg(feature = "ace16-write-preview")]
pub use table::{FixedColumnUpdates, FixedScalarUpdate, SlotScalarUpdate};
pub use table::{
    ForeignIndexInspection, ForeignIndexLink, ForeignIndexReference, inspect_foreign_index_links,
};
pub use table::{IndexCheck, IndexCheckStatus, TableCheck, check_table};
pub use table::{IndexColumn, IndexInspection, NativeIndex, inspect_table_indexes};
pub use table::{IndexConstraintViolation, IndexConstraints};
pub use table::{
    MultivaluePageCell, MultivaluePageValue, MultivalueTablePage, read_multivalue_table_page,
    read_multivalue_table_page_with_request,
};
pub use table::{NativeColumn, TableSchema, TableSnapshot, inspect_table_schema, read_table};
pub use table::{StoredTablePage, read_stored_table_page};
pub use table::{
    StoredVersion, VersionHistoryColumn, VersionHistoryMemberDeclaration,
    inspect_version_history_declarations, read_version_history_column,
    read_version_history_column_with_limits,
};
pub use table::{
    TableBatch, TableCursor, TablePage, TablePageRequest, TableSnapshotLimits, read_table_batched,
    read_table_page, read_table_projection_batched,
};
pub use transaction::OfflineReadTransaction;
#[cfg(feature = "journal")]
pub use transaction::{NativeRecoveryError, PreparedNativeEdit, recover_native_edit};

pub use properties::{
    NativeProperty, PropertyGroup, PropertyGroupKind, TableProperties, inspect_table_properties,
};

pub use relationships::{NativeRelationship, inspect_relationships};

pub use table::{
    CollectionPageRequest, StoredCollectionColumn, StoredCollectionItem, StoredCollectionPageCell,
    StoredCollectionPageValue, StoredCollectionTablePage, read_stored_collection_table_page,
    read_stored_collection_table_page_with_request,
};

pub use table::read_table_projection;

pub use table::{
    IndexProjection, IndexProjectionSelection, IndexReadRequest, read_table_index_in_table_order,
    read_table_index_projection, read_table_index_with_request,
};

pub use table::{
    LongValueCheck, LongValueCheckStatus, LongValueCounts, LongValueMapReferences,
    TableLongValueCheck, check_table_long_values,
};
