//! Independent bounded Jet/ACE reads and offline edit plans. No active content.
#![deny(missing_docs)]

#[cfg(all(test, feature = "ace16-write-preview"))]
extern crate self as tablecraft_ace;

mod catalog;
mod collation;
pub use collation::{
    FrenchTextSearch, GeneralSortFamily, GeneralTextSearch, NativeSortOrder, NativeTextKey,
    compare_french_legacy_like_units, compare_french_legacy_text, compare_french_legacy_utf16,
    compare_general_like_units, compare_general_text, compare_general_utf16, encode_text_index_key,
    encode_utf16_text_index_key, find_french_legacy_text, find_french_legacy_utf16,
    find_general_text, find_general_utf16, french_legacy_like_literal_step,
    french_legacy_like_unit_step, general_like_literal_step, general_like_unit_step,
    inspect_database_sort_order, is_french_legacy_like_unit, is_general_like_unit,
};
mod queries;
#[cfg(feature = "journal")]
pub use queries::{
    NativeQueryExpressionEditsPlan, OwnedQueryFragmentExpressionEdit, QueryExpressionEditsError,
    QueryFragmentExpressionEdit,
};
#[cfg(feature = "journal")]
pub use queries::{NativeQueryExpressionPlan, QueryExpressionError};
pub use queries::{
    QueryAttribute, SavedQueryDefinition, SavedQueryRow, read_saved_query_definition,
};
mod constraints;
#[cfg(feature = "ace16-create")]
mod creation;
#[cfg(feature = "ace16-create")]
pub use creation::{
    AddColumnCandidate, AddColumnError, CreateColumn, CreateDatabaseError, CreateTable,
    CreatedDatabase, FirstTableDefinition, FirstTableError, FirstTablePlan, create_ace16_database,
    create_ace16_database_tables, create_ace16_int32_database, create_blank_ace16_database,
    first_table_source_available,
};
#[cfg(all(feature = "ace16-create", feature = "journal"))]
pub use creation::{
    AddTableError, AddTablePlan, AddTableSourceCapabilities, CreatedAddColumnCandidate,
    CreatedAddColumnError, DatabaseOptionsError, DatabaseOptionsSourceError,
    DatabaseOptionsSourcePurpose, FieldHistoryDatabaseOptionsCandidate,
    FieldHistoryDatabaseOptionsError, NativeDatabaseOptionsPlan, add_table_source_available,
    add_table_source_capabilities,
};
#[cfg(all(feature = "ace16-create", feature = "journal"))]
pub use creation::{
    CreatedDefaultZeroAddColumnCandidate, CreatedDefaultZeroAddColumnError,
    CreatedDefaultZeroColumnRequired, CreatedFieldAdditionSourcePurpose,
    FieldAdditionAfterOffCandidate, FieldAdditionAfterOffError,
    FieldAdditionAfterOffHistoryCandidate, FieldAdditionAfterOffHistoryError,
    FieldAdditionCandidate, FieldAdditionError, FieldAdditionSourceAvailability,
    FieldAdditionSourceError, FieldAdditionSourcePurpose, FieldAdditionTarget,
    RepeatedDefaultZeroAddColumnCandidate,
};
mod encryption;
mod error;
mod format;
mod header;
#[cfg(any(feature = "ace16-create", feature = "journal"))]
mod identifier;
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
    check_encryption_add, check_encryption_rewrite, encryption_rewrite_budget, inspect_encryption,
    read_agile_encryption, rewrite_encryption, rewrite_encryption_to,
};
#[cfg(all(feature = "parallel-encryption", not(target_family = "wasm")))]
pub use encryption::{ParallelEncryption, rewrite_encryption_parallel_to};
pub use error::InspectError;
pub use format::profiles::ReadProfile;
pub use header::{FileFormat, Header, inspect_header};
pub use preservation::{
    DataMacroEnvelope, DataMacroProgram, DataMacroXml, DataMacroXmlNamespace, DependencyCoverage,
    FixedUpdateImageLimits, MacroCoverageChild, MacroProgramCoverage, MacroProgramDiagnostic,
    MacroProgramNode, MacroProgramNodeKind, MacroProgramProfile, MacroProgramText,
    MacroProgramTextKind, MacroPropertyInventory, MacroPropertyObservation, NativeImage,
    NativeImageSource, ObjectDependency, ObjectPayloadData, ObjectPayloads, OpaquePayload,
    PayloadPresence, TableExtra, TableExtraCoverage, TableExtraGroup, TableExtraProperty,
    TableMacroCoverage, TableMacroInventory, XmlByteSpan,
};
#[cfg(feature = "ace16-write-preview")]
pub use table::{
    Ace16AutoNumberInsertPreview, Ace16IncrementingInsertBinding, AutoNumberRowInsert,
    IncrementingAutoNumber,
};
#[cfg(feature = "ace16-write-preview")]
pub use table::{Ace16BeforeChangePreview, BeforeChangePreviewError};
#[cfg(feature = "ace16-write-preview")]
pub use table::{Ace16DeletePreview, Ace16WritePreview};
#[cfg(feature = "ace16-write-preview")]
pub use table::{Ace16IndexCreatePreview, SecondaryIndexCreate};
pub use table::{
    AttachmentColumn, AttachmentMetadataColumn, AttachmentReader, ParentAttachments,
    StoredAttachment, read_attachment_column, read_attachment_column_with_limits,
    read_attachment_metadata, read_attachment_metadata_with_limits,
};
pub use table::{
    AttachmentDecodeLimits, AttachmentDecoder, AttachmentEncoding, DecodedAttachment,
    decode_attachment, decode_attachment_with_limits,
};
pub use table::{
    AttachmentMemberDeclaration, MultivalueColumn, MultivalueDeclaration, MultivalueReader,
    ParentMultivalue, inspect_attachment_declarations, inspect_attachment_members,
    inspect_multivalue_columns, inspect_multivalue_declarations, read_multivalue_column,
    read_multivalue_column_with_limits,
};
#[cfg(any(feature = "ace16-write-preview", feature = "journal"))]
pub use table::{
    BeforeChangeAssignment, BeforeChangeFieldPreparation, BeforeChangeFinalRow, BeforeEvent,
    BeforeEventGuardError, BeforeEventGuardPreparation, BeforeEventGuardSelector,
    BeforeEventIntents, BeforeGuardProgramRevision, BeforeGuardRejection,
    BeforeGuardSelectionError, BeforeGuardTrace, BeforeGuardTraceKind, BeforeGuardUnsupported,
    FixedScalarUpdate,
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
#[cfg(any(feature = "ace16-write-preview", feature = "journal"))]
pub use table::{FittedColumnUpdates, FixedColumnUpdates, SlotScalarUpdate};
pub use table::{
    ForeignIndexInspection, ForeignIndexLink, ForeignIndexReference, inspect_foreign_index_links,
};
pub use table::{IndexCheck, IndexCheckStatus, TableCheck, check_table};
pub use table::{
    IndexColumn, IndexHistoricalCounter, IndexInspection, NativeIndex, inspect_table_index_counter,
    inspect_table_indexes,
};
pub use table::{IndexConstraintViolation, IndexConstraints};
pub use table::{
    IndexedTableSnapshot, NativeColumn, NativeRowReference, TableSchema, TableSnapshot,
    inspect_table_schema, read_table,
};
pub use table::{
    MultivaluePageCell, MultivaluePageValue, MultivalueTablePage, read_multivalue_table_page,
    read_multivalue_table_page_with_request,
};
pub use table::{
    ParentVersionHistory, StoredVersion, VersionHistoryColumn, VersionHistoryMemberDeclaration,
    VersionHistoryReader, inspect_version_history_declarations, read_version_history_column,
    read_version_history_column_with_limits,
};
#[cfg(any(feature = "journal", feature = "ace16-write-preview"))]
pub use table::{RowDelete, TableRowDeletes};
pub use table::{StoredTablePage, read_stored_table_page};
pub use table::{
    TableBatch, TableCursor, TablePage, TablePageRequest, TableSnapshotLimits, read_table_batched,
    read_table_page, read_table_projection_batched, read_table_projection_page,
};
pub use transaction::OfflineReadTransaction;
#[cfg(feature = "journal")]
pub use transaction::{
    NativeFittedEditIntent, NativeFittedIntentGroup, NativeFittedUpdates, NativeFixedEditIntent,
    NativeFixedIntentGroup, NativeInt32EditIntent, NativeIntentError, NativeUpdateBody,
    NativeUpdateIntent, NativeUpdateProfile, RecoverableEditError, RecoverableNativeEditPlan,
};
#[cfg(feature = "journal")]
pub use transaction::{
    NativeQueryExpressionEditsIntent, PreparedQueryExpressionEdits,
    QualifiedQueryExpressionEditsRecovery, QueryExpressionEditsIntentError,
    RecoverableNativeQueryExpressionEditsPlan, qualify_native_query_expression_edits_recovery,
};
#[cfg(feature = "journal")]
pub use transaction::{
    NativeRecoveryError, PreparedNativeEdit, PreparedNativeUpdate, PreparedResizableNativeEdit,
    QualifiedNativeAllocationRecovery, qualify_native_allocation_recovery, recover_native_edit,
};
#[cfg(feature = "journal")]
pub use transaction::{
    NativeRecoveryQualificationError, QualifiedJet4Recovery, QualifiedNativeRecovery,
    qualify_jet4_int32_recovery, qualify_native_update_recovery,
};

pub use properties::{
    DatabaseNameAutoCorrect, DatabaseProperties, DatabaseQueryMode, NativeProperty, PropertyGroup,
    PropertyGroupKind, TableProperties, inspect_database_properties, inspect_table_properties,
};

pub use relationships::{NativeRelationship, inspect_relationships};

pub use table::{
    CollectionPageRequest, StoredCollectionColumn, StoredCollectionItem, StoredCollectionPageCell,
    StoredCollectionPageValue, StoredCollectionTablePage, read_stored_collection_table_page,
    read_stored_collection_table_page_with_request,
};

pub use table::read_table_projection;

pub use table::{
    IndexProjection, IndexProjectionSelection, IndexReadRequest,
    read_stored_table_index_in_table_order, read_stored_table_index_with_references,
    read_stored_table_index_with_request, read_table_index_in_table_order,
    read_table_index_projection, read_table_index_with_references, read_table_index_with_request,
};

pub use table::{
    LongValueCheck, LongValueCheckStatus, LongValueCounts, LongValueMapReferences,
    TableLongValueCheck, check_table_long_values,
};

#[cfg(feature = "journal")]
pub use transaction::{
    NativeDeleteIntent, QualifiedNativeDeleteRecovery, RecoverableNativeDeletePlan,
    qualify_native_delete_recovery,
};

#[cfg(any(feature = "journal", feature = "ace16-write-preview"))]
pub use table::{InsertFieldValue, OmittedRowInsert, RowInsert};

#[cfg(feature = "journal")]
pub use transaction::{
    NativeInsertIntent, QualifiedNativeInsertRecovery, RecoverableNativeInsertPlan,
    qualify_native_insert_recovery,
};

#[cfg(feature = "journal")]
pub use transaction::{
    NativeOmittedInsertIntent, QualifiedNativeOmittedInsertRecovery,
    RecoverableNativeOmittedInsertPlan, qualify_native_omitted_insert_recovery,
};

#[cfg(feature = "journal")]
pub use transaction::{
    NativeEofInsertIntent, QualifiedNativeEofInsertRecovery, RecoverableNativeEofInsertPlan,
    RecoverableNativeInsertOperation, qualify_native_eof_insert_recovery,
};

#[cfg(feature = "journal")]
pub use transaction::{
    NativeCompleteInsertIntent, QualifiedNativeCompleteInsertRecovery,
    RecoverableNativeCompleteInsertPlan, qualify_native_complete_insert_recovery,
};

#[cfg(feature = "journal")]
pub use transaction::{
    NativeClockInsertIntent, QualifiedNativeClockInsertRecovery, RecoverableNativeClockInsertPlan,
    qualify_native_clock_insert_recovery,
};

#[cfg(all(feature = "journal", feature = "ace16-create"))]
pub use transaction::{
    FirstTableIntentError, NativeFirstTableIntent, PreparedFirstTable, QualifiedFirstTableRecovery,
    qualify_native_first_table_recovery,
};

#[cfg(all(feature = "journal", feature = "ace16-create"))]
pub use transaction::{
    AddTableIntentError, NativeAddTableIntent, PreparedAddTable, QualifiedAddTableRecovery,
    qualify_native_add_table_recovery,
};

#[cfg(feature = "journal")]
pub use transaction::{
    NativeTableRenameIntent, PreparedTableRename, QualifiedTableRenameRecovery,
    RecoverableNativeTableRenamePlan, RecoverableTableRenameError, TableRenameIntentError,
    qualify_native_table_rename_recovery,
};

#[cfg(feature = "journal")]
pub use table::{NativeTableRenameIdentity, NativeTableRenamePlan, TableRenameError};

#[cfg(all(feature = "journal", feature = "ace16-create"))]
pub use transaction::{
    AddColumnIntentError, NativeAddColumnIntent, QualifiedAddColumnRecovery,
    RecoverableAddColumnPlan, qualify_native_add_column_recovery,
};

#[cfg(all(feature = "journal", feature = "ace16-create"))]
pub use transaction::{
    DefaultZeroAddColumnIntentError, NativeDefaultZeroAddColumnIntent,
    QualifiedDefaultZeroAddColumnRecovery, RecoverableDefaultZeroAddColumnPlan,
    qualify_native_default_zero_add_column_recovery,
};

#[cfg(all(feature = "journal", feature = "ace16-create"))]
pub use transaction::{
    NativeRepeatedDefaultZeroAddColumnIntent, QualifiedRepeatedDefaultZeroAddColumnRecovery,
    RecoverableRepeatedDefaultZeroAddColumnPlan, RepeatedDefaultZeroAddColumnIntentError,
    qualify_native_repeated_default_zero_add_column_recovery,
};

#[cfg(feature = "journal")]
pub use transaction::{
    NativeQueryExpressionIntent, PreparedQueryExpression, QualifiedQueryExpressionRecovery,
    QueryExpressionIntentError, QueryExpressionRecoveryError, RecoverableNativeQueryExpressionPlan,
    RecoverableQueryExpressionError, qualify_native_query_expression_recovery,
};

#[cfg(all(feature = "journal", feature = "ace16-create"))]
pub use transaction::{
    FieldHistoryOptionsIntentError, NativeFieldHistoryDatabaseOptionsIntent,
    QualifiedFieldHistoryDatabaseOptionsRecovery, RecoverableFieldHistoryDatabaseOptionsPlan,
    qualify_native_field_history_database_options_recovery,
};

#[cfg(all(feature = "journal", feature = "ace16-create"))]
pub use transaction::{
    DatabaseOptionsIntentError, NativeDatabaseOptionsIntent, QualifiedDatabaseOptionsRecovery,
    RecoverableNativeDatabaseOptionsPlan, qualify_native_database_options_recovery,
};

#[cfg(all(feature = "journal", feature = "ace16-create"))]
pub use transaction::{
    FieldAdditionIntentError, NativeFieldAdditionIntent, QualifiedFieldAdditionRecovery,
    RecoverableFieldAdditionPlan, qualify_native_field_addition_recovery,
};

#[cfg(all(feature = "ace16-create", feature = "journal"))]
pub use table::{FieldDropError, NativeFieldDropCandidate, NativeFieldDropIdentity};
#[cfg(all(feature = "ace16-create", feature = "journal"))]
pub use table::{FieldRenameError, NativeFieldRenameCandidate, NativeFieldRenameIdentity};

#[cfg(all(feature = "journal", feature = "ace16-create"))]
pub use transaction::{
    FieldRenameIntentError, NativeFieldRenameIntent, QualifiedFieldRenameRecovery,
    RecoverableFieldRenamePlan, qualify_native_field_rename_recovery,
};

#[cfg(all(feature = "journal", feature = "ace16-create"))]
pub use transaction::{
    FieldAdditionAfterOffIntentError, NativeFieldAdditionAfterOffIntent,
    QualifiedFieldAdditionAfterOffRecovery, RecoverableFieldAdditionAfterOffPlan,
    qualify_native_field_addition_after_off_recovery,
};

#[cfg(all(feature = "journal", feature = "ace16-create"))]
pub use transaction::{
    FieldAdditionAfterOffHistoryIntentError, NativeFieldAdditionAfterOffHistoryIntent,
    QualifiedFieldAdditionAfterOffHistoryRecovery, RecoverableFieldAdditionAfterOffHistoryPlan,
    qualify_native_field_addition_after_off_history_recovery,
};

#[cfg(all(feature = "journal", feature = "ace16-create"))]
pub use transaction::{
    FieldDropIntentError, NativeFieldDropIntent, QualifiedFieldDropRecovery,
    RecoverableFieldDropPlan, qualify_native_field_drop_recovery,
};
