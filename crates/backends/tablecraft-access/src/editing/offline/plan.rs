//! Private action composition; row and schema operations retain distinct intents.
#[cfg(feature = "ace16-create")]
use super::database_options::{
    intent::Intent as DatabaseOptionsIntent, plan::Plan as DatabaseOptionsPlan,
};
#[cfg(feature = "ace16-create")]
use super::field_addition::plan::Plan as FieldAdditionPlan;
use super::{Cause, QueryChange, SchemaChange, query_expression::QueryExpressionPlan};
use crate::editing::journal::NativeJournalInsertPlan;
#[cfg(not(feature = "ace16-create"))]
use tablecraft_ace::DeclaredFieldAdditionIntentError;
#[cfg(feature = "ace16-create")]
use tablecraft_ace::{
    AddTablePlan, FirstTablePlan, NativeAddColumnIntent, NativeAddTableIntent,
    NativeDeclaredFieldAdditionIntent, NativeDefaultZeroAddColumnIntent,
    NativeFieldAdditionAfterOffHistoryIntent, NativeFieldAdditionAfterOffIntent,
    NativeFieldAdditionIntent, NativeFirstTableIntent, NativeIndexCreateIntent,
    NativePopulatedAddTableIntent, NativeRepeatedDefaultZeroAddColumnIntent,
    NativeRetainedTypedFieldAdditionIntent, PopulatedAddTablePlan, RecoverableAddColumnPlan,
    RecoverableFieldDropPlan, RecoverableFieldRenamePlan, RecoverableNativeIndexCreatePlan,
};
use tablecraft_ace::{
    NativeClockInsertIntent, NativeCompleteInsertIntent, NativeCurrentFieldAdditionIntent,
    NativeDeleteIntent, NativeEofInsertIntent, NativeFieldDropIntent, NativeFieldRenameIntent,
    NativeInsertIntent, NativeOmittedInsertIntent, NativeQueryExpressionEditsIntent,
    NativeQueryExpressionIntent, NativeTableDropIntent, NativeTableRenameIntent,
    NativeUpdateIntent, NativeUpdateProfile, PreparedNativeUpdate, RecoverableNativeDeletePlan,
    RecoverableNativeEditPlan, RecoverableNativeTableRenamePlan, RecoverableTableDropPlan,
};
use tablecraft_core::limits::ResourceBudget;
use tablecraft_io::journal::{DurableStorage, ResizableDurableStorage};

#[cfg(feature = "ace16-create")]
const FIELD_RENAME_FAMILY: &[u8; 4] = b"TCFR";
#[cfg(feature = "ace16-create")]
const FIELD_DROP_FAMILY: &[u8; 4] = b"TCFD";
const TABLE_DROP_FAMILY: &[u8; 4] = b"TCTD";
const POPULATED_TABLE_FAMILY: &[u8; 4] = b"TCPT";
const INDEX_CREATION_FAMILY: &[u8; 4] = b"TCIC";

#[cfg(all(
    test,
    not(feature = "ace16-create"),
    any(target_os = "macos", target_os = "linux", windows)
))]
mod non_tail_tests;

#[cfg(all(test, any(target_os = "macos", target_os = "linux", windows)))]
mod keyless_tests;

#[cfg(all(test, not(feature = "ace16-create")))]
mod index_creation_tests;

#[cfg(all(test, not(feature = "ace16-create")))]
mod declared_addition_tests;

#[cfg(all(test, not(feature = "ace16-create")))]
mod populated_tests;

pub(super) enum MutationPlan {
    #[cfg(feature = "ace16-create")]
    IndexCreation(RecoverableNativeIndexCreatePlan),
    #[cfg(feature = "ace16-create")]
    DatabaseOptions(DatabaseOptionsPlan),
    QueryExpression(QueryExpressionPlan),
    #[cfg(feature = "ace16-create")]
    AddColumn(RecoverableAddColumnPlan),
    #[cfg(feature = "ace16-create")]
    FieldAddition(FieldAdditionPlan),
    #[cfg(feature = "ace16-create")]
    FieldRename(RecoverableFieldRenamePlan),
    #[cfg(feature = "ace16-create")]
    FieldDrop(RecoverableFieldDropPlan),
    TableRename(RecoverableNativeTableRenamePlan),
    TableDrop(RecoverableTableDropPlan),
    Update(RecoverableNativeEditPlan),
    Delete(RecoverableNativeDeletePlan),
    Insert(NativeJournalInsertPlan),
    #[cfg(feature = "ace16-create")]
    FirstTable(FirstTablePlan),
    #[cfg(feature = "ace16-create")]
    AddTable(AddTablePlan),
    #[cfg(feature = "ace16-create")]
    PopulatedTable(PopulatedAddTablePlan),
}

impl MutationPlan {
    pub(super) fn explicit_intent_limit(&self) -> Option<u64> {
        match self {
            #[cfg(feature = "ace16-create")]
            Self::PopulatedTable(_) => {
                Some(NativePopulatedAddTableIntent::MAX_ENCODED_BYTES as u64)
            }
            _ => None,
        }
    }

    pub(super) fn encode_intent(
        &self,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<Vec<u8>, Cause> {
        match self {
            #[cfg(feature = "ace16-create")]
            Self::IndexCreation(plan) => plan
                .intent()
                .encode(budget, cancelled)
                .map_err(Cause::IndexCreationIntent),
            #[cfg(feature = "ace16-create")]
            Self::PopulatedTable(plan) => plan
                .encode_intent(budget, cancelled)
                .map_err(Cause::PopulatedTableIntent),
            #[cfg(feature = "ace16-create")]
            Self::DatabaseOptions(plan) => plan.encode_intent(budget, cancelled),
            Self::QueryExpression(plan) => plan.encode_intent(budget, cancelled),
            #[cfg(feature = "ace16-create")]
            Self::AddColumn(plan) => plan
                .intent()
                .encode(budget, cancelled)
                .map_err(Cause::AddColumnIntent),
            #[cfg(feature = "ace16-create")]
            Self::FieldAddition(plan) => plan.encode_intent(budget, cancelled),
            #[cfg(feature = "ace16-create")]
            Self::FieldDrop(plan) => plan
                .intent()
                .encode(budget, cancelled)
                .map_err(Cause::FieldDropIntent),
            #[cfg(feature = "ace16-create")]
            Self::FieldRename(plan) => plan
                .intent()
                .encode(budget, cancelled)
                .map_err(Cause::FieldRenameIntent),
            Self::TableDrop(plan) => plan
                .intent()
                .encode(budget, cancelled)
                .map_err(Cause::TableDropIntent),
            Self::TableRename(plan) => plan
                .intent()
                .encode(budget, cancelled)
                .map_err(Cause::TableRenameIntent),
            Self::Update(plan) => plan
                .intent()
                .encode(budget, cancelled)
                .map_err(Cause::Intent),
            Self::Delete(plan) => plan
                .intent()
                .encode(budget, cancelled)
                .map_err(Cause::Intent),
            Self::Insert(plan) => plan.encode_intent(budget, cancelled).map_err(Cause::Intent),
            #[cfg(feature = "ace16-create")]
            Self::FirstTable(plan) => plan
                .encode_intent(budget, cancelled)
                .map_err(Cause::FirstTableIntent),
            #[cfg(feature = "ace16-create")]
            Self::AddTable(plan) => plan
                .encode_intent(budget, cancelled)
                .map_err(Cause::AddTableIntent),
        }
    }

    pub(super) fn commit<D: ResizableDurableStorage, J: DurableStorage>(
        self,
        target: &mut D,
        journal: &mut J,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<(), Cause> {
        let prepared = match self {
            #[cfg(feature = "ace16-create")]
            Self::IndexCreation(plan) => {
                return plan
                    .prepare_retained(target, journal, budget, cancelled)
                    .map_err(Cause::Prepare)?
                    .commit(budget, cancelled)
                    .map_err(Cause::Commit);
            }
            #[cfg(feature = "ace16-create")]
            Self::PopulatedTable(plan) => {
                return plan
                    .prepare_populated(target, journal, budget, cancelled)
                    .map_err(Cause::Prepare)?
                    .commit(budget, cancelled)
                    .map_err(Cause::Commit);
            }
            #[cfg(feature = "ace16-create")]
            Self::DatabaseOptions(plan) => {
                return plan.commit(target, journal, budget, cancelled);
            }
            Self::QueryExpression(plan) => {
                return plan.commit(target, journal, budget, cancelled);
            }
            #[cfg(feature = "ace16-create")]
            Self::AddColumn(plan) => {
                let prepared = plan
                    .prepare(target, journal, budget, cancelled)
                    .map_err(Cause::Prepare)?;
                return prepared.commit(budget, cancelled).map_err(Cause::Commit);
            }
            #[cfg(feature = "ace16-create")]
            Self::FieldAddition(plan) => {
                return plan.commit(target, journal, budget, cancelled);
            }
            #[cfg(feature = "ace16-create")]
            Self::FieldDrop(plan) => {
                if plan.intent().is_retained()
                    || plan.intent().is_retained_typed()
                    || plan.intent().is_retained_non_tail()
                {
                    return plan
                        .prepare_retained(target, journal, budget, cancelled)
                        .map_err(Cause::Prepare)?
                        .commit(budget, cancelled)
                        .map_err(Cause::Commit);
                }
                let prepared = plan
                    .prepare(target, journal, budget, cancelled)
                    .map_err(Cause::Prepare)?;
                return prepared.commit(budget, cancelled).map_err(Cause::Commit);
            }
            #[cfg(feature = "ace16-create")]
            Self::FieldRename(plan) => {
                if plan.intent().is_retained_schema() || plan.intent().is_retained_typed() {
                    return plan
                        .prepare_retained(target, journal, budget, cancelled)
                        .map_err(Cause::Prepare)?
                        .commit(budget, cancelled)
                        .map_err(Cause::Commit);
                }
                let prepared = plan
                    .prepare(target, journal, budget, cancelled)
                    .map_err(Cause::Prepare)?;
                return prepared.commit(budget, cancelled).map_err(Cause::Commit);
            }
            Self::TableDrop(plan) => {
                if plan.intent().is_retained() {
                    let prepared = plan
                        .prepare_retained(target, journal, budget, cancelled)
                        .map_err(Cause::Prepare)?;
                    return prepared.commit(budget, cancelled).map_err(Cause::Commit);
                }
                let prepared = plan
                    .prepare(target, journal, budget, cancelled)
                    .map_err(Cause::Prepare)?;
                return prepared.commit(budget, cancelled).map_err(Cause::Commit);
            }
            Self::TableRename(plan) => {
                let prepared = plan
                    .prepare(target, journal, budget, cancelled)
                    .map_err(Cause::Prepare)?;
                return prepared.commit(budget, cancelled).map_err(Cause::Commit);
            }
            Self::Update(plan) => {
                if matches!(
                    plan.intent().profile(),
                    NativeUpdateProfile::Ace16RetainedEmptyRows
                        | NativeUpdateProfile::Ace16RetainedRows
                        | NativeUpdateProfile::Ace16RetainedOpaqueRows
                        | NativeUpdateProfile::Ace16RetainedCurrencyRows
                        | NativeUpdateProfile::Ace16RetainedBooleanRows
                        | NativeUpdateProfile::Ace16RetainedFixedScalarRows
                        | NativeUpdateProfile::Ace16RetainedWideValueRows
                        | NativeUpdateProfile::Ace16RetainedBooleanMaterialization
                ) {
                    plan.prepare_retained(target, journal, budget, cancelled)
                        .map(PreparedNativeUpdate::Resizable)
                } else {
                    plan.prepare_update(target, journal, budget, cancelled)
                }
            }
            Self::Delete(plan) => {
                if plan.intent().is_retained() {
                    plan.prepare_retained(target, journal, budget, cancelled)
                        .map(PreparedNativeUpdate::Resizable)
                } else {
                    plan.prepare(target, journal, budget, cancelled)
                        .map(PreparedNativeUpdate::Fixed)
                }
            }
            Self::Insert(plan) => plan.prepare_insert(target, journal, budget, cancelled),
            #[cfg(feature = "ace16-create")]
            Self::FirstTable(plan) => {
                let prepared = if plan.is_native() {
                    plan.prepare_native(target, journal, budget, cancelled)
                } else {
                    plan.prepare(target, journal, budget, cancelled)
                }
                .map_err(Cause::Prepare)?;
                return prepared.commit(budget, cancelled).map_err(Cause::Commit);
            }
            #[cfg(feature = "ace16-create")]
            Self::AddTable(plan) => {
                let prepared = if plan.is_native() {
                    plan.prepare_native(target, journal, budget, cancelled)
                } else {
                    plan.prepare(target, journal, budget, cancelled)
                }
                .map_err(Cause::Prepare)?;
                return prepared.commit(budget, cancelled).map_err(Cause::Commit);
            }
        }
        .map_err(Cause::Prepare)?;

        prepared
            .commit(budget, cancelled)
            .map(|_| ())
            .map_err(Cause::Commit)
    }
}

pub(super) enum MutationIntent {
    #[cfg(feature = "ace16-create")]
    IndexCreation(NativeIndexCreateIntent),
    #[cfg(feature = "ace16-create")]
    DatabaseOptions(DatabaseOptionsIntent),
    QueryExpression(NativeQueryExpressionIntent),
    QueryExpressionEdits(NativeQueryExpressionEditsIntent),
    #[cfg(feature = "ace16-create")]
    AddColumn(NativeAddColumnIntent),
    #[cfg(feature = "ace16-create")]
    DefaultZeroAddColumn(NativeDefaultZeroAddColumnIntent),
    #[cfg(feature = "ace16-create")]
    RepeatedDefaultZeroAddColumn(NativeRepeatedDefaultZeroAddColumnIntent),
    #[cfg(feature = "ace16-create")]
    FieldAddition(NativeFieldAdditionIntent),
    #[cfg(feature = "ace16-create")]
    FieldAdditionAfterOff(NativeFieldAdditionAfterOffIntent),
    #[cfg(feature = "ace16-create")]
    FieldAdditionAfterOffHistory(NativeFieldAdditionAfterOffHistoryIntent),
    #[cfg(feature = "ace16-create")]
    CurrentFieldAddition(NativeCurrentFieldAdditionIntent),
    RetainedFieldAddition(NativeCurrentFieldAdditionIntent),
    #[cfg(feature = "ace16-create")]
    RetainedTypedFieldAddition(NativeRetainedTypedFieldAdditionIntent),
    #[cfg(feature = "ace16-create")]
    DeclaredFieldAddition(NativeDeclaredFieldAdditionIntent),
    #[cfg(feature = "ace16-create")]
    FieldRename(NativeFieldRenameIntent),
    RetainedFieldRename(NativeFieldRenameIntent),
    #[cfg(feature = "ace16-create")]
    FieldDrop(NativeFieldDropIntent),
    RetainedFieldDrop(NativeFieldDropIntent),
    TableRename(NativeTableRenameIntent),
    TableDrop(NativeTableDropIntent),
    RetainedTableDrop(NativeTableDropIntent),
    Update(NativeUpdateIntent),
    RetainedUpdate(NativeUpdateIntent),
    Delete(NativeDeleteIntent),
    RetainedDelete(NativeDeleteIntent),
    Insert(NativeInsertIntent),
    EofInsert(NativeEofInsertIntent),
    CompleteInsert(NativeCompleteInsertIntent),
    OmittedInsert(NativeOmittedInsertIntent),
    RetainedInsert(NativeOmittedInsertIntent),
    ClockInsert(NativeClockInsertIntent),
    #[cfg(feature = "ace16-create")]
    FirstTable(NativeFirstTableIntent),
    #[cfg(feature = "ace16-create")]
    AddTable(NativeAddTableIntent),
    #[cfg(feature = "ace16-create")]
    PopulatedTable(NativePopulatedAddTableIntent),
}

impl MutationIntent {
    pub(super) fn database_options_change(&self) -> Option<super::DatabaseOptionsChange> {
        match self {
            #[cfg(feature = "ace16-create")]
            Self::DatabaseOptions(_) => Some(super::DatabaseOptionsChange::NameAutoCorrectDisabled),
            _ => None,
        }
    }

    pub(super) fn schema_change(&self) -> Option<SchemaChange> {
        match self {
            #[cfg(feature = "ace16-create")]
            Self::IndexCreation(_) => Some(SchemaChange::IndexCreated),
            #[cfg(feature = "ace16-create")]
            Self::AddColumn(_)
            | Self::DefaultZeroAddColumn(_)
            | Self::RepeatedDefaultZeroAddColumn(_)
            | Self::FieldAddition(_)
            | Self::FieldAdditionAfterOff(_)
            | Self::FieldAdditionAfterOffHistory(_)
            | Self::CurrentFieldAddition(_) => Some(SchemaChange::ColumnAdded),
            Self::RetainedFieldAddition(_) => Some(SchemaChange::ColumnAdded),
            #[cfg(feature = "ace16-create")]
            Self::RetainedTypedFieldAddition(_) => Some(SchemaChange::ColumnAdded),
            #[cfg(feature = "ace16-create")]
            Self::DeclaredFieldAddition(_) => Some(SchemaChange::ColumnAdded),
            #[cfg(feature = "ace16-create")]
            Self::FieldDrop(_) => Some(SchemaChange::ColumnDropped),
            Self::RetainedFieldDrop(_) => Some(SchemaChange::ColumnDropped),
            #[cfg(feature = "ace16-create")]
            Self::FieldRename(intent) => {
                (intent.column() != intent.new_name()).then_some(SchemaChange::ColumnRenamed)
            }
            Self::RetainedFieldRename(intent) => {
                (intent.column() != intent.new_name()).then_some(SchemaChange::ColumnRenamed)
            }
            Self::TableDrop(_) | Self::RetainedTableDrop(_) => Some(SchemaChange::TableDropped),
            Self::TableRename(intent) => {
                (intent.table() != intent.new_name()).then_some(SchemaChange::TableRenamed)
            }
            #[cfg(feature = "ace16-create")]
            Self::FirstTable(intent) => Some(SchemaChange::FirstTableCreated {
                column_count: intent.column_count(),
            }),
            #[cfg(feature = "ace16-create")]
            Self::PopulatedTable(intent) => Some(SchemaChange::TableAdded {
                column_count: intent.column_count(),
            }),
            #[cfg(feature = "ace16-create")]
            Self::AddTable(intent) => Some(SchemaChange::TableAdded {
                column_count: intent.column_count(),
            }),
            _ => None,
        }
    }

    pub(super) fn query_change(&self) -> Option<QueryChange> {
        match self {
            Self::QueryExpressionEdits(intent) => intent
                .edits()
                .iter()
                .any(|edit| {
                    let request = edit.as_request();
                    request.expected != request.replacement
                })
                .then_some(QueryChange::ExpressionSaved),
            Self::QueryExpression(intent) => (intent.expected_expression()
                != intent.replacement_expression())
            .then_some(QueryChange::ExpressionSaved),
            _ => None,
        }
    }

    pub(super) fn decode(
        raw: &[u8],
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<Self, Cause> {
        if raw.starts_with(INDEX_CREATION_FAMILY) {
            #[cfg(feature = "ace16-create")]
            return NativeIndexCreateIntent::decode(raw, budget, cancelled)
                .map(Self::IndexCreation)
                .map_err(Cause::IndexCreationIntent);
            #[cfg(not(feature = "ace16-create"))]
            return Err(Cause::UnsupportedSource);
        }
        if raw.starts_with(POPULATED_TABLE_FAMILY) {
            #[cfg(feature = "ace16-create")]
            return NativePopulatedAddTableIntent::decode(raw, budget, cancelled)
                .map(Self::PopulatedTable)
                .map_err(Cause::PopulatedTableIntent);
            #[cfg(not(feature = "ace16-create"))]
            return Err(Cause::UnsupportedSource);
        }
        if raw.starts_with(b"TCND0003") {
            return NativeDeleteIntent::decode_retained(raw, budget, cancelled)
                .map(Self::RetainedDelete)
                .map_err(Cause::Intent);
        }
        if raw.starts_with(b"TCNI0024")
            || raw.starts_with(b"TCNI0017")
            || raw.starts_with(b"TCNI0018")
            || raw.starts_with(b"TCNI0019")
            || raw.starts_with(b"TCNI0020")
            || raw.starts_with(b"TCNI0021")
            || raw.starts_with(b"TCNI0022")
            || raw.starts_with(b"TCNI0023")
        {
            return NativeUpdateIntent::decode(raw, budget, cancelled)
                .map(Self::RetainedUpdate)
                .map_err(Cause::Intent);
        }
        if raw.starts_with(b"TCNA0010") {
            return NativeOmittedInsertIntent::decode_retained_keyless(raw, budget, cancelled)
                .map(Self::RetainedInsert)
                .map_err(Cause::Intent);
        }
        if raw.starts_with(b"TCNA0009") {
            return NativeOmittedInsertIntent::decode_retained_opaque(raw, budget, cancelled)
                .map(Self::RetainedInsert)
                .map_err(Cause::Intent);
        }
        #[cfg(feature = "ace16-create")]
        if raw.starts_with(b"TCNA0008") {
            return NativeOmittedInsertIntent::decode_retained(raw, budget, cancelled)
                .map(Self::RetainedInsert)
                .map_err(Cause::Intent);
        }
        if raw.starts_with(b"TCAC0013") {
            #[cfg(feature = "ace16-create")]
            return NativeDeclaredFieldAdditionIntent::decode(raw, budget, cancelled)
                .map(Self::DeclaredFieldAddition)
                .map_err(Cause::DeclaredFieldAdditionIntent);
            #[cfg(not(feature = "ace16-create"))]
            return Err(Cause::DeclaredFieldAdditionIntent(
                DeclaredFieldAdditionIntentError::UnsupportedSource,
            ));
        }
        if raw.starts_with(b"TCAC0012") {
            #[cfg(feature = "ace16-create")]
            {
                return NativeRetainedTypedFieldAdditionIntent::decode(raw, budget, cancelled)
                    .map(Self::RetainedTypedFieldAddition)
                    .map_err(Cause::RetainedTypedFieldAdditionIntent);
            }
            #[cfg(not(feature = "ace16-create"))]
            {
                return Err(Cause::RetainedTypedFieldAdditionIntent(
                    tablecraft_ace::RetainedTypedFieldAdditionIntentError::UnsupportedSource,
                ));
            }
        }
        if raw.starts_with(b"TCFD0005") {
            return NativeFieldDropIntent::decode_retained_non_tail(raw, budget, cancelled)
                .map(Self::RetainedFieldDrop)
                .map_err(Cause::FieldDropIntent);
        }
        if raw.starts_with(b"TCFD0004") {
            return NativeFieldDropIntent::decode_retained_typed(raw, budget, cancelled)
                .map(Self::RetainedFieldDrop)
                .map_err(Cause::FieldDropIntent);
        }
        if raw.starts_with(b"TCFD0003") {
            return NativeFieldDropIntent::decode_retained(raw, budget, cancelled)
                .map(Self::RetainedFieldDrop)
                .map_err(Cause::FieldDropIntent);
        }
        if raw.starts_with(b"TCFR0007") {
            return NativeFieldRenameIntent::decode_retained_typed(raw, budget, cancelled)
                .map(Self::RetainedFieldRename)
                .map_err(Cause::FieldRenameIntent);
        }
        if raw.starts_with(b"TCFR0006") {
            return NativeFieldRenameIntent::decode_retained_schema(raw, budget, cancelled)
                .map(Self::RetainedFieldRename)
                .map_err(Cause::FieldRenameIntent);
        }
        // Closed framing selection occurs before decode; a failed decoder is
        // never retried as another action or promoted from its record prefix.
        #[cfg(feature = "ace16-create")]
        if raw.starts_with(b"TCFD0002") {
            return NativeFieldDropIntent::decode_current_tail(raw, budget, cancelled)
                .map(Self::FieldDrop)
                .map_err(Cause::FieldDropIntent);
        }
        #[cfg(feature = "ace16-create")]
        if raw.starts_with(FIELD_DROP_FAMILY) {
            return NativeFieldDropIntent::decode(raw, budget, cancelled)
                .map(Self::FieldDrop)
                .map_err(Cause::FieldDropIntent);
        }
        #[cfg(feature = "ace16-create")]
        if raw.starts_with(b"TCDO") {
            return DatabaseOptionsIntent::decode(raw, budget, cancelled)
                .map(Self::DatabaseOptions);
        }
        if raw.starts_with(b"TCQE0002") {
            return NativeQueryExpressionEditsIntent::decode(raw, budget, cancelled)
                .map(Self::QueryExpressionEdits)
                .map_err(Cause::QueryExpressionEditsIntent);
        }
        if raw.starts_with(b"TCQE") {
            return NativeQueryExpressionIntent::decode(raw, budget, cancelled)
                .map(Self::QueryExpression)
                .map_err(Cause::QueryExpressionIntent);
        }
        if raw.starts_with(b"TCAC0010") || raw.starts_with(b"TCAC0011") {
            return NativeCurrentFieldAdditionIntent::decode_retained(raw, budget, cancelled)
                .map(Self::RetainedFieldAddition)
                .map_err(Cause::CurrentFieldAdditionIntent);
        }

        #[cfg(feature = "ace16-create")]
        if raw.starts_with(b"TCAC0009") {
            return NativeCurrentFieldAdditionIntent::decode_short_text(raw, budget, cancelled)
                .map(Self::CurrentFieldAddition)
                .map_err(Cause::CurrentFieldAdditionIntent);
        }
        #[cfg(feature = "ace16-create")]
        if raw.starts_with(b"TCAC0008") {
            return NativeCurrentFieldAdditionIntent::decode(raw, budget, cancelled)
                .map(Self::CurrentFieldAddition)
                .map_err(Cause::CurrentFieldAdditionIntent);
        }
        #[cfg(feature = "ace16-create")]
        if raw.starts_with(b"TCAC0007") {
            return NativeFieldAdditionAfterOffHistoryIntent::decode(raw, budget, cancelled)
                .map(Self::FieldAdditionAfterOffHistory)
                .map_err(Cause::FieldAdditionAfterOffHistoryIntent);
        }
        #[cfg(feature = "ace16-create")]
        if raw.starts_with(b"TCAC0006") {
            return NativeFieldAdditionAfterOffIntent::decode(raw, budget, cancelled)
                .map(Self::FieldAdditionAfterOff)
                .map_err(Cause::FieldAdditionAfterOffIntent);
        }
        #[cfg(feature = "ace16-create")]
        if raw.starts_with(b"TCAC0005") {
            return NativeFieldAdditionIntent::decode(raw, budget, cancelled)
                .map(Self::FieldAddition)
                .map_err(Cause::FieldAdditionIntent);
        }
        #[cfg(feature = "ace16-create")]
        if raw.starts_with(b"TCAC0004") {
            return NativeRepeatedDefaultZeroAddColumnIntent::decode(raw, budget, cancelled)
                .map(Self::RepeatedDefaultZeroAddColumn)
                .map_err(Cause::RepeatedDefaultZeroAddColumnIntent);
        }
        #[cfg(feature = "ace16-create")]
        if raw.starts_with(b"TCAC0003") {
            return NativeDefaultZeroAddColumnIntent::decode(raw, budget, cancelled)
                .map(Self::DefaultZeroAddColumn)
                .map_err(Cause::DefaultZeroAddColumnIntent);
        }
        #[cfg(feature = "ace16-create")]
        if raw.starts_with(b"TCAC") {
            return NativeAddColumnIntent::decode(raw, budget, cancelled)
                .map(Self::AddColumn)
                .map_err(Cause::AddColumnIntent);
        }
        #[cfg(feature = "ace16-create")]
        if raw.starts_with(FIELD_RENAME_FAMILY) {
            return NativeFieldRenameIntent::decode(raw, budget, cancelled)
                .map(Self::FieldRename)
                .map_err(Cause::FieldRenameIntent);
        }
        if raw.starts_with(b"TCTD0003") {
            return NativeTableDropIntent::decode_retained(raw, budget, cancelled)
                .map(Self::RetainedTableDrop)
                .map_err(Cause::TableDropIntent);
        }
        if raw.starts_with(b"TCTD0002") {
            return NativeTableDropIntent::decode_current(raw, budget, cancelled)
                .map(Self::TableDrop)
                .map_err(Cause::TableDropIntent);
        }
        if raw.starts_with(TABLE_DROP_FAMILY) {
            return NativeTableDropIntent::decode(raw, budget, cancelled)
                .map(Self::TableDrop)
                .map_err(Cause::TableDropIntent);
        }
        if raw.starts_with(b"TCRN") {
            return NativeTableRenameIntent::decode(raw, budget, cancelled)
                .map(Self::TableRename)
                .map_err(Cause::TableRenameIntent);
        }

        #[cfg(feature = "ace16-create")]
        if raw.starts_with(b"TCAT0007") {
            return NativeAddTableIntent::decode_native(raw, budget, cancelled)
                .map(Self::AddTable)
                .map_err(Cause::AddTableIntent);
        }
        #[cfg(feature = "ace16-create")]
        if raw.starts_with(b"TCAT0006") {
            return NativeAddTableIntent::decode_retained(raw, budget, cancelled)
                .map(Self::AddTable)
                .map_err(Cause::AddTableIntent);
        }
        #[cfg(feature = "ace16-create")]
        if raw.starts_with(b"TCAT") {
            return NativeAddTableIntent::decode(raw, budget, cancelled)
                .map(Self::AddTable)
                .map_err(Cause::AddTableIntent);
        }

        #[cfg(feature = "ace16-create")]
        if raw.starts_with(b"TCNT0007") {
            return NativeFirstTableIntent::decode_native(raw, budget, cancelled)
                .map(Self::FirstTable)
                .map_err(Cause::FirstTableIntent);
        }
        #[cfg(feature = "ace16-create")]
        if raw.starts_with(b"TCNT") {
            return NativeFirstTableIntent::decode(raw, budget, cancelled)
                .map(Self::FirstTable)
                .map_err(Cause::FirstTableIntent);
        }

        let decoded = if raw.starts_with(b"TCND0002") {
            NativeDeleteIntent::decode_current(raw, budget, cancelled).map(Self::Delete)
        } else if raw.starts_with(b"TCND") {
            NativeDeleteIntent::decode(raw, budget, cancelled).map(Self::Delete)
        } else if raw.starts_with(b"TCNA0005") {
            NativeClockInsertIntent::decode(raw, budget, cancelled).map(Self::ClockInsert)
        } else if raw.starts_with(b"TCNA0004") {
            NativeCompleteInsertIntent::decode(raw, budget, cancelled).map(Self::CompleteInsert)
        } else if raw.starts_with(b"TCNA0003") {
            NativeEofInsertIntent::decode(raw, budget, cancelled).map(Self::EofInsert)
        } else if raw.starts_with(b"TCNA0007") {
            NativeOmittedInsertIntent::decode_current(raw, budget, cancelled)
                .map(Self::OmittedInsert)
        } else if raw.starts_with(b"TCNA0002") || raw.starts_with(b"TCNA0006") {
            NativeOmittedInsertIntent::decode(raw, budget, cancelled).map(Self::OmittedInsert)
        } else if raw.starts_with(b"TCNA") {
            NativeInsertIntent::decode(raw, budget, cancelled).map(Self::Insert)
        } else {
            NativeUpdateIntent::decode(raw, budget, cancelled).map(Self::Update)
        };
        decoded.map_err(Cause::Intent)
    }
}
