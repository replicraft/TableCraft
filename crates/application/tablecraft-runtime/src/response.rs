//! Completed host task responses and inert presentation facts.
#[cfg(all(feature = "offline-edit", feature = "ace16-create"))]
use crate::{
    AddColumnReview, DatabaseOptionsInspection, DatabaseOptionsReview, FieldAdditionAvailability,
    FieldAdditionReview, FieldDropReview, FieldDropSourceInspection, FieldRenameReview,
    FieldRenameSourceInspection, NumberColumnAvailability, ReviewedAddColumn,
    ReviewedDatabaseOptions, ReviewedFieldAddition, ReviewedFieldDrop, ReviewedFieldRename,
    ReviewedNumberColumn, ReviewedTableCreation,
};
use crate::{
    Cell, CollectionKind, Column, ContentsTarget, DiscoveredQuery, Field, FieldFilterDescription,
    FilteredData, FindPosition, FindRequest, InspectedQuery, ParameterSpec, QueryDesignTarget,
    SavedQuery, SavedQueryTarget, Table, TotalsData, TotalsDescription,
};
#[cfg(feature = "offline-edit")]
use crate::{
    DeleteReview, EditDraft, EditReview, InsertDraft, InsertReview, QueryDeleteReview,
    QueryExpressionReview, QueryInsertReview, QuerySqlDraft, QuerySqlReview, QueryUpdateReview,
    ReviewedDelete, ReviewedEdit, ReviewedInsert, ReviewedQueryDelete, ReviewedQueryExpression,
    ReviewedQueryInsert, ReviewedQuerySql, ReviewedQueryUpdate, ReviewedTableDrop,
    ReviewedTableRename, TableDropReview, TableDropSourceInspection, TableRenameReview,
};
use std::{path::PathBuf, sync::Arc};
use tablecraft_builders::query::{BuiltQueryDesign, DesignTableMetadata};

pub enum Response {
    #[cfg(feature = "offline-edit")]
    TableDropSourceInspected {
        inspection: TableDropSourceInspection,
    },
    #[cfg(feature = "offline-edit")]
    TableDropReviewed {
        review: TableDropReview,
        capability: ReviewedTableDrop,
    },
    #[cfg(all(feature = "offline-edit", feature = "ace16-create"))]
    DatabaseOptionsInspected {
        inspection: DatabaseOptionsInspection,
    },
    #[cfg(all(feature = "offline-edit", feature = "ace16-create"))]
    DatabaseOptionsReviewed {
        review: DatabaseOptionsReview,
        capability: ReviewedDatabaseOptions,
    },
    #[cfg(feature = "offline-edit")]
    QuerySqlDraft {
        draft: QuerySqlDraft,
    },
    #[cfg(feature = "offline-edit")]
    QuerySqlReviewed {
        review: QuerySqlReview,
        capability: ReviewedQuerySql,
    },
    #[cfg(feature = "offline-edit")]
    QueryExpressionReviewed {
        review: QueryExpressionReview,
        capability: ReviewedQueryExpression,
    },
    #[cfg(feature = "offline-edit")]
    TableRenameReviewed {
        review: TableRenameReview,
        capability: ReviewedTableRename,
    },
    #[cfg(all(feature = "offline-edit", feature = "ace16-create"))]
    FieldDropReviewed {
        review: FieldDropReview,
        capability: ReviewedFieldDrop,
    },
    #[cfg(all(feature = "offline-edit", feature = "ace16-create"))]
    FieldDropSourceInspected {
        inspection: FieldDropSourceInspection,
    },
    #[cfg(all(feature = "offline-edit", feature = "ace16-create"))]
    FieldRenameReviewed {
        review: FieldRenameReview,
        capability: ReviewedFieldRename,
    },
    #[cfg(all(feature = "offline-edit", feature = "ace16-create"))]
    FieldRenameSourceInspected {
        inspection: FieldRenameSourceInspection,
    },
    #[cfg(all(feature = "offline-edit", feature = "ace16-create"))]
    FieldAdditionAvailability {
        availability: FieldAdditionAvailability,
    },
    #[cfg(all(feature = "offline-edit", feature = "ace16-create"))]
    FieldAdditionReviewed {
        review: FieldAdditionReview,
        capability: ReviewedFieldAddition,
    },
    #[cfg(all(feature = "offline-edit", feature = "ace16-create"))]
    NumberColumnAvailability {
        availability: NumberColumnAvailability,
    },
    #[cfg(all(feature = "offline-edit", feature = "ace16-create"))]
    NumberColumnReviewed {
        review: AddColumnReview,
        capability: ReviewedNumberColumn,
    },
    #[cfg(all(feature = "offline-edit", feature = "ace16-create"))]
    AddColumnReviewed {
        review: AddColumnReview,
        capability: ReviewedAddColumn,
    },
    #[cfg(all(feature = "offline-edit", feature = "ace16-create"))]
    TableCreationReviewed {
        capability: ReviewedTableCreation,
    },
    QueryDesignTable {
        target: QueryDesignTarget,
        metadata: DesignTableMetadata,
    },
    QueryDesignBuilt {
        target: QueryDesignTarget,
        built: BuiltQueryDesign,
        metadata: Vec<DesignTableMetadata>,
    },
    TotalsDescription(TotalsDescription),
    Totals(TotalsData),
    FieldFilterDescription(FieldFilterDescription),
    QueryParametersDiscovered {
        target: Arc<DiscoveredQuery>,
        parameters: Vec<ParameterSpec>,
    },
    QueryParameters {
        target: Arc<InspectedQuery>,
        parameters: Vec<ParameterSpec>,
    },
    #[cfg(feature = "offline-edit")]
    QueryInsertReviewed {
        review: QueryInsertReview,
        capability: ReviewedQueryInsert,
    },
    #[cfg(feature = "offline-edit")]
    QueryDeleteReviewed {
        review: QueryDeleteReview,
        capability: ReviewedQueryDelete,
    },
    #[cfg(feature = "offline-edit")]
    QueryUpdateReviewed {
        review: QueryUpdateReview,
        capability: ReviewedQueryUpdate,
    },
    SavedQuery {
        target: SavedQueryTarget,
        parameters: Vec<ParameterSpec>,
    },
    #[cfg(feature = "offline-edit")]
    EditDraft(EditDraft),
    #[cfg(feature = "offline-edit")]
    EditReviewed {
        review: EditReview,
        capability: ReviewedEdit,
    },
    #[cfg(feature = "offline-edit")]
    InsertDraft(InsertDraft),
    #[cfg(feature = "offline-edit")]
    InsertReviewed {
        review: InsertReview,
        capability: ReviewedInsert,
    },
    #[cfg(feature = "offline-edit")]
    DeleteReviewed {
        review: DeleteReview,
        capability: ReviewedDelete,
    },
    FilteredData(FilteredData),
    Found {
        request: FindRequest,
        position: Option<FindPosition>,
    },
    /// Read-only owned metadata for an issued query result slot; no native row identity.
    QueryContents {
        /// The inspected slot within the submitted issued result owner.
        binding: usize,
        /// Exact child member declarations, including deliberately uninspected members.
        columns: Vec<Column>,
        /// Complete metadata rows; uninspected cells remain distinct from SQL NULL.
        rows: Vec<Vec<Cell>>,
    },
    /// A complete explicit read of one native parent's stored collection. There are no
    /// executable attachment actions or borrowed native values in this response.
    Contents {
        target: ContentsTarget,
        kind: CollectionKind,
        assigned: bool,
        columns: Vec<Column>,
        rows: Vec<Vec<Cell>>,
    },
    /// Inert declarations. Row totals are unverified; field reader nullability
    /// does not establish native Required/default/validation properties.
    Details {
        table: String,
        declared_rows: u32,
        fields: Vec<Field>,
    },
    Opened {
        document_revision: u64,
        /// Absolute path captured by the successful open operation.
        source_path: PathBuf,
        /// Exact captured native source admission, tied to document_revision.
        /// This is not a draft or permission to bypass fresh review/save checks.
        table_creation_available: bool,
        /// Canonical blank admission for manual first-table Design. Additional
        /// tables do not gain this capability from ordinary creation admission.
        table_design_available: bool,
        title: String,
        format: String,
        tables: Vec<Table>,
        queries: Vec<SavedQuery>,
    },
    Data {
        /// Issued read-only child metadata, present only for structured query outputs.
        query_contents: Option<crate::QueryResultHandle>,
        source_revision: Option<u64>,
        table: String,
        offset: u32,
        columns: Vec<Column>,
        rows: Vec<Vec<Cell>>,
        has_more: bool,
    },
}
