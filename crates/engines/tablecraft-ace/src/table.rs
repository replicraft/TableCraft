#[cfg(feature = "journal")]
pub(crate) use mutation::fixed_records::FixedCarriers;
#[cfg(feature = "journal")]
pub(crate) use mutation::normal_eof::OrdinaryVariablePlanKind;
#[cfg(feature = "journal")]
pub(crate) use mutation::normal_insert::OrdinaryInsertPlanKind;
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
    AttachmentColumn, AttachmentMemberDeclaration, AttachmentMetadataColumn, AttachmentReader,
    ParentAttachments, StoredAttachment, inspect_attachment_declarations,
    inspect_attachment_members, read_attachment_column, read_attachment_column_with_limits,
    read_attachment_metadata, read_attachment_metadata_with_limits,
};
pub use complex::{
    ComplexColumnLinks, ComplexParentLink, ComplexReferenceViolation, read_complex_column_links,
    read_complex_column_links_with_limits,
};
pub use complex::{ComplexColumnSchema, ComplexTableSchema, inspect_complex_schema};
pub use complex::{
    MultivalueColumn, MultivalueDeclaration, MultivalueReader, ParentMultivalue,
    inspect_multivalue_columns, inspect_multivalue_declarations, read_multivalue_column,
    read_multivalue_column_with_limits,
};
pub use complex::{
    MultivaluePageCell, MultivaluePageValue, MultivalueTablePage, read_multivalue_table_page,
    read_multivalue_table_page_with_request,
};
pub use complex::{
    ParentVersionHistory, StoredVersion, VersionHistoryColumn, VersionHistoryMemberDeclaration,
    VersionHistoryReader, inspect_version_history_declarations, read_version_history_column,
    read_version_history_column_with_limits,
};
pub(crate) mod mutation;
mod schema;
mod short_text;
pub use features::{ColumnFeatures, TableFeatures, inspect_table_features};
pub use indexes::{IndexCheck, IndexCheckStatus, TableCheck, check_table};
pub use indexes::{IndexedTableSnapshot, read_table_index_with_references};
#[cfg(feature = "ace16-write-preview")]
pub use mutation::{
    Ace16AutoNumberInsertPreview, Ace16IncrementingInsertBinding, AutoNumberRowInsert,
    IncrementingAutoNumber,
};
#[cfg(feature = "ace16-write-preview")]
pub use mutation::{Ace16BeforeChangePreview, BeforeChangePreviewError};
#[cfg(feature = "ace16-write-preview")]
pub use mutation::{Ace16DeletePreview, Ace16WritePreview};
#[cfg(feature = "ace16-write-preview")]
pub use mutation::{Ace16IndexCreatePreview, SecondaryIndexCreate};
#[cfg(any(feature = "ace16-write-preview", feature = "journal"))]
pub use mutation::{
    BeforeChangeAssignment, BeforeChangeFieldPreparation, BeforeChangeFinalRow, BeforeEvent,
    BeforeEventGuardError, BeforeEventGuardPreparation, BeforeEventGuardSelector,
    BeforeEventIntents, BeforeGuardProgramRevision, BeforeGuardRejection,
    BeforeGuardSelectionError, BeforeGuardTrace, BeforeGuardTraceKind, BeforeGuardUnsupported,
    FixedScalarUpdate,
};
pub use mutation::{EditError, Int32Update, NativeEditPlan, WriteBlocker};
#[cfg(all(feature = "ace16-create", feature = "journal"))]
pub use mutation::{FieldDropError, NativeFieldDropCandidate, NativeFieldDropIdentity};
#[cfg(all(feature = "ace16-create", feature = "journal"))]
pub use mutation::{
    FieldRenameError, FittedFieldRenameSource, NativeFieldRenameCandidate,
    NativeFieldRenameIdentity, NativeFittedFieldRenameCandidate,
    NativePopulatedFieldRenameCandidate,
};
#[cfg(any(feature = "ace16-write-preview", feature = "journal"))]
pub use mutation::{FittedColumnUpdates, FixedColumnUpdates, SlotScalarUpdate};
#[cfg(feature = "journal")]
pub use mutation::{
    NativeTableDropIdentity, NativeTableDropInspection, NativeTableDropPlan, TableDropError,
};
#[cfg(feature = "journal")]
pub use mutation::{NativeTableRenameIdentity, NativeTableRenamePlan, TableRenameError};
#[cfg(any(feature = "journal", feature = "ace16-write-preview"))]
pub use mutation::{RowDelete, TableRowDeletes};
pub use schema::{NativeColumn, TableSchema, inspect_table_schema};
mod page;
mod record;
mod record_layout;
pub(crate) mod security_identifiers;
pub use indexes::{
    read_stored_table_index_in_table_order, read_stored_table_index_with_references,
    read_stored_table_index_with_request,
};
pub use page::{
    TableBatch, TableCursor, TablePage, TablePageRequest, TableSnapshotLimits, read_table_batched,
    read_table_page, read_table_projection_batched, read_table_projection_page,
};
pub(crate) use security_identifiers::remask;
mod row_reference;
pub use row_reference::NativeRowReference;
mod snapshot;
pub use snapshot::{TableSnapshot, read_table};
pub(crate) mod decimal;
mod extended_datetime;
mod indexes;
pub use indexes::{
    BinaryIndexSelection, GeneralEqualityPrefix, GuidIndexSelection, TextIndexSelection,
    read_table_binary_index_range, read_table_guid_index, read_table_text_index_range,
};
pub use indexes::{IndexConstraintViolation, IndexConstraints};
mod long_text;
mod long_value;
pub use long_value::{
    LongValueCheck, LongValueCheckStatus, LongValueCounts, LongValueMapReferences,
    TableLongValueCheck, check_table_long_values,
};
mod row_layout;
pub use indexes::{
    BooleanIndexSelection, ByteIndexSelection, CurrencyIndexSelection, DateTimeIndexSelection,
    DecimalIndexSelection, Int16IndexSelection, Int32IndexSelection, Int64IndexSelection,
    read_table_index, read_table_index_range, read_table_int64_index_range,
};
pub use indexes::{
    ForeignIndexInspection, ForeignIndexLink, ForeignIndexReference, inspect_foreign_index_links,
};
pub use indexes::{
    IndexColumn, IndexHistoricalCounter, IndexInspection, NativeIndex, inspect_table_index_counter,
    inspect_table_indexes,
};

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

#[cfg(any(
    feature = "ace16-write-preview",
    feature = "ace16-create",
    feature = "journal"
))]
pub(crate) mod scalar_encoding;

#[cfg(any(feature = "journal", feature = "ace16-write-preview"))]
pub use mutation::{InsertFieldValue, OmittedRowInsert, RowInsert};
