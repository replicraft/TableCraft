//! Explicit host requests and password ownership.
#[cfg(feature = "ace16-create")]
use crate::CreationReport;
#[cfg(all(feature = "offline-edit", feature = "ace16-create"))]
use crate::{
    AddedColumnReopen, CreatedTableReopen, DatabaseOptionsReopen, FieldDroppedReopen,
    FieldRenamedReopen,
};
use crate::{
    ContentsTarget, FieldFilterOperator, FieldFilterTarget, FilterComparison, FilterDefinition,
    FilterHandle, FilterSelectionTarget, FindRequest, InspectedQuery, ParameterInput,
    QueryDesignTarget, SavedQueryTarget, SortSpec, SortTarget, TotalSpec, TotalsTarget,
};
#[cfg(feature = "offline-edit")]
use crate::{
    DeleteTarget, EditInput, EditTarget, FindPosition, InsertInput, InsertRetryWitness,
    InsertTarget, QueryExpressionReopen, TableRenameReopen,
};
use std::{path::PathBuf, sync::Arc};
use tablecraft_builders::query::QueryDesignDraft;
#[cfg(all(feature = "offline-edit", feature = "ace16-create"))]
use tablecraft_builders::table::TableDraftSnapshot;
use zeroize::Zeroize;

/// Explicit host requests. Queued and consumed passwords are erased on drop.
pub enum Command {
    #[cfg(all(feature = "offline-edit", feature = "ace16-create"))]
    InspectDatabaseOptions {
        document_revision: u64,
    },
    #[cfg(all(feature = "offline-edit", feature = "ace16-create"))]
    ReviewDisableNameAutoCorrect {
        document_revision: u64,
    },
    #[cfg(all(feature = "offline-edit", feature = "ace16-create"))]
    OpenDatabaseOptionsSaved {
        witness: Arc<DatabaseOptionsReopen>,
    },
    #[cfg(feature = "offline-edit")]
    ReviewTableRename {
        document_revision: u64,
        table: String,
        new_name: String,
    },
    #[cfg(feature = "offline-edit")]
    InspectQuerySqlDraft {
        target: SavedQueryTarget,
    },
    #[cfg(feature = "offline-edit")]
    ReviewQuerySql {
        target: SavedQueryTarget,
        original_sql: String,
        proposed_sql: String,
    },
    #[cfg(feature = "offline-edit")]
    ReviewQueryLiteralExpression {
        target: SavedQueryTarget,
        expected: String,
        replacement: String,
    },
    #[cfg(feature = "offline-edit")]
    OpenQueryExpressionSaved {
        witness: Arc<QueryExpressionReopen>,
    },
    #[cfg(feature = "offline-edit")]
    OpenTableRenamed {
        witness: Arc<TableRenameReopen>,
    },
    /// Read-only guarded reopening; hosts separately decide whether retry is safe.
    #[cfg(feature = "offline-edit")]
    OpenInsertOriginal {
        witness: Arc<InsertRetryWitness>,
    },
    #[cfg(all(feature = "offline-edit", feature = "ace16-create"))]
    ReviewFieldDrop {
        document_revision: u64,
        table: String,
        column: String,
        native_number: usize,
    },
    #[cfg(all(feature = "offline-edit", feature = "ace16-create"))]
    OpenFieldDropped {
        witness: Arc<FieldDroppedReopen>,
    },
    #[cfg(all(feature = "offline-edit", feature = "ace16-create"))]
    InspectFieldDropSource {
        document_revision: u64,
        table: String,
        column: String,
        native_number: usize,
    },
    #[cfg(all(feature = "offline-edit", feature = "ace16-create"))]
    ReviewFieldRename {
        document_revision: u64,
        table: String,
        column: String,
        native_number: usize,
        new_name: String,
    },
    #[cfg(all(feature = "offline-edit", feature = "ace16-create"))]
    OpenFieldRenamed {
        witness: Arc<FieldRenamedReopen>,
    },
    #[cfg(all(feature = "offline-edit", feature = "ace16-create"))]
    InspectFieldRenameSource {
        document_revision: u64,
        table: String,
        column: String,
        native_number: usize,
    },
    #[cfg(all(feature = "offline-edit", feature = "ace16-create"))]
    InspectFieldAdditionAvailability {
        document_revision: u64,
        table: String,
    },
    #[cfg(all(feature = "offline-edit", feature = "ace16-create"))]
    ReviewAddField {
        document_revision: u64,
        table: String,
        column: String,
        declaration: tablecraft_builders::table::FieldAdditionDeclaration,
    },
    #[cfg(all(feature = "offline-edit", feature = "ace16-create"))]
    InspectNumberColumnAvailability {
        document_revision: u64,
        table: String,
    },
    #[cfg(all(feature = "offline-edit", feature = "ace16-create"))]
    ReviewAddNumberColumn {
        document_revision: u64,
        table: String,
        column: String,
    },
    #[cfg(all(feature = "offline-edit", feature = "ace16-create"))]
    ReviewAddInt32Column {
        document_revision: u64,
        table: String,
        column: String,
    },
    #[cfg(all(feature = "offline-edit", feature = "ace16-create"))]
    OpenColumnAdded {
        witness: Arc<AddedColumnReopen>,
    },
    #[cfg(all(feature = "offline-edit", feature = "ace16-create"))]
    ReviewTableCreation {
        document_revision: u64,
        snapshot: TableDraftSnapshot,
    },
    #[cfg(all(feature = "offline-edit", feature = "ace16-create"))]
    OpenTableCreated {
        witness: Arc<CreatedTableReopen>,
    },
    /// Reopen only an acknowledged creation whose fresh snapshot still matches
    /// the canonical blank image. This does not attest physical file identity.
    #[cfg(feature = "ace16-create")]
    OpenCreated {
        report: Arc<CreationReport>,
    },
    #[cfg(feature = "offline-edit")]
    ReplaceFindMatch {
        request: FindRequest,
        found: FindPosition,
        replacement: String,
        filter: Option<(FilterHandle, bool)>,
    },
    DescribeQueryDesign {
        target: QueryDesignTarget,
        table: String,
    },
    BuildQueryDesign {
        target: QueryDesignTarget,
        draft: QueryDesignDraft,
    },
    ImportQueryDesign {
        target: QueryDesignTarget,
        sql: String,
    },
    DescribeTotals {
        target: TotalsTarget,
    },
    Totals {
        target: TotalsTarget,
        specs: Vec<TotalSpec>,
        base: Option<FilterHandle>,
    },
    Open {
        path: PathBuf,
        password: String,
    },
    #[cfg(feature = "offline-edit")]
    BeginEdit {
        target: EditTarget,
    },
    #[cfg(feature = "offline-edit")]
    ReviewEdit {
        draft_id: u64,
        input: EditInput,
    },
    #[cfg(feature = "offline-edit")]
    BeginInsert {
        target: InsertTarget,
    },
    #[cfg(feature = "offline-edit")]
    ReviewInsert {
        draft_id: u64,
        inputs: Vec<InsertInput>,
    },
    #[cfg(feature = "offline-edit")]
    ReviewDelete {
        target: DeleteTarget,
    },
    SortView {
        target: SortTarget,
        sort: Option<SortSpec>,
        base: Option<FilterHandle>,
        enabled: bool,
    },
    ClearFilter {
        handle: FilterHandle,
    },
    ReapplyFilter {
        definition: FilterDefinition,
        document_revision: u64,
        enabled: bool,
        offset: u32,
    },
    DescribeFieldFilter {
        target: FieldFilterTarget,
    },
    ApplyFieldFilter {
        target: FieldFilterTarget,
        operator: FieldFilterOperator,
        inputs: Vec<String>,
        base: Option<FilterHandle>,
    },
    FilterSelection {
        target: FilterSelectionTarget,
        comparison: FilterComparison,
        base: Option<FilterHandle>,
    },
    FilterPage {
        handle: FilterHandle,
        offset: u32,
        enabled: bool,
    },
    FindFiltered {
        handle: FilterHandle,
        request: FindRequest,
        enabled: bool,
    },
    FindNext {
        request: FindRequest,
    },
    Page {
        table: String,
        offset: u32,
    },
    Describe {
        table: String,
    },
    InspectCollection {
        target: ContentsTarget,
    },
    Query {
        sql: String,
    },
    InspectQuery {
        document_revision: u64,
        sql: String,
    },
    RunQuery {
        target: Arc<InspectedQuery>,
        parameters: Vec<ParameterInput>,
    },
    #[cfg(feature = "offline-edit")]
    ReviewQueryInsert {
        target: Arc<InspectedQuery>,
        parameters: Vec<ParameterInput>,
    },
    #[cfg(feature = "offline-edit")]
    ReviewQueryDelete {
        target: Arc<InspectedQuery>,
        parameters: Vec<ParameterInput>,
    },
    #[cfg(feature = "offline-edit")]
    ReviewQueryUpdate {
        target: Arc<InspectedQuery>,
        parameters: Vec<ParameterInput>,
    },
    InspectSavedQuery {
        target: SavedQueryTarget,
    },
    RunSavedQuery {
        target: SavedQueryTarget,
        parameters: Vec<ParameterInput>,
    },
}

impl Drop for Command {
    fn drop(&mut self) {
        if let Self::Open { password, .. } = self {
            password.zeroize();
        }
    }
}
