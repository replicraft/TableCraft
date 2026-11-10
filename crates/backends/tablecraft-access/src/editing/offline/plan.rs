//! Private action composition; row and schema operations retain distinct intents.
#[cfg(feature = "ace16-create")]
use super::database_options::{
    intent::Intent as DatabaseOptionsIntent, plan::Plan as DatabaseOptionsPlan,
};
#[cfg(feature = "ace16-create")]
use super::field_addition::plan::Plan as FieldAdditionPlan;
use super::{Cause, QueryChange, SchemaChange, query_expression::QueryExpressionPlan};
use crate::editing::journal::NativeJournalInsertPlan;
#[cfg(feature = "ace16-create")]
use tablecraft_ace::{
    AddTablePlan, FirstTablePlan, NativeAddColumnIntent, NativeAddTableIntent,
    NativeCurrentFieldAdditionIntent, NativeDefaultZeroAddColumnIntent,
    NativeFieldAdditionAfterOffHistoryIntent, NativeFieldAdditionAfterOffIntent,
    NativeFieldAdditionIntent, NativeFieldDropIntent, NativeFieldRenameIntent,
    NativeFirstTableIntent, NativeRepeatedDefaultZeroAddColumnIntent, RecoverableAddColumnPlan,
    RecoverableFieldDropPlan, RecoverableFieldRenamePlan,
};
use tablecraft_ace::{
    NativeClockInsertIntent, NativeCompleteInsertIntent, NativeDeleteIntent, NativeEofInsertIntent,
    NativeInsertIntent, NativeOmittedInsertIntent, NativeQueryExpressionEditsIntent,
    NativeQueryExpressionIntent, NativeTableDropIntent, NativeTableRenameIntent,
    NativeUpdateIntent, PreparedNativeUpdate, RecoverableNativeDeletePlan,
    RecoverableNativeEditPlan, RecoverableNativeTableRenamePlan, RecoverableTableDropPlan,
};
use tablecraft_core::limits::ResourceBudget;
use tablecraft_io::journal::{DurableStorage, ResizableDurableStorage};

#[cfg(feature = "ace16-create")]
const FIELD_RENAME_FAMILY: &[u8; 4] = b"TCFR";
#[cfg(feature = "ace16-create")]
const FIELD_DROP_FAMILY: &[u8; 4] = b"TCFD";
const TABLE_DROP_FAMILY: &[u8; 4] = b"TCTD";

pub(super) enum MutationPlan {
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
}

impl MutationPlan {
    pub(super) fn encode_intent(
        &self,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<Vec<u8>, Cause> {
        match self {
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
                let prepared = plan
                    .prepare(target, journal, budget, cancelled)
                    .map_err(Cause::Prepare)?;
                return prepared.commit(budget, cancelled).map_err(Cause::Commit);
            }
            #[cfg(feature = "ace16-create")]
            Self::FieldRename(plan) => {
                let prepared = plan
                    .prepare(target, journal, budget, cancelled)
                    .map_err(Cause::Prepare)?;
                return prepared.commit(budget, cancelled).map_err(Cause::Commit);
            }
            Self::TableDrop(plan) => {
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
            Self::Update(plan) => plan.prepare_update(target, journal, budget, cancelled),
            Self::Delete(plan) => plan
                .prepare(target, journal, budget, cancelled)
                .map(PreparedNativeUpdate::Fixed),
            Self::Insert(plan) => plan.prepare_insert(target, journal, budget, cancelled),
            #[cfg(feature = "ace16-create")]
            Self::FirstTable(plan) => {
                let prepared = plan
                    .prepare(target, journal, budget, cancelled)
                    .map_err(Cause::Prepare)?;
                return prepared.commit(budget, cancelled).map_err(Cause::Commit);
            }
            #[cfg(feature = "ace16-create")]
            Self::AddTable(plan) => {
                let prepared = plan
                    .prepare(target, journal, budget, cancelled)
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
    #[cfg(feature = "ace16-create")]
    FieldRename(NativeFieldRenameIntent),
    #[cfg(feature = "ace16-create")]
    FieldDrop(NativeFieldDropIntent),
    TableRename(NativeTableRenameIntent),
    TableDrop(NativeTableDropIntent),
    Update(NativeUpdateIntent),
    Delete(NativeDeleteIntent),
    Insert(NativeInsertIntent),
    EofInsert(NativeEofInsertIntent),
    CompleteInsert(NativeCompleteInsertIntent),
    OmittedInsert(NativeOmittedInsertIntent),
    ClockInsert(NativeClockInsertIntent),
    #[cfg(feature = "ace16-create")]
    FirstTable(NativeFirstTableIntent),
    #[cfg(feature = "ace16-create")]
    AddTable(NativeAddTableIntent),
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
            Self::AddColumn(_)
            | Self::DefaultZeroAddColumn(_)
            | Self::RepeatedDefaultZeroAddColumn(_)
            | Self::FieldAddition(_)
            | Self::FieldAdditionAfterOff(_)
            | Self::FieldAdditionAfterOffHistory(_)
            | Self::CurrentFieldAddition(_) => Some(SchemaChange::ColumnAdded),
            #[cfg(feature = "ace16-create")]
            Self::FieldDrop(_) => Some(SchemaChange::ColumnDropped),
            #[cfg(feature = "ace16-create")]
            Self::FieldRename(intent) => {
                (intent.column() != intent.new_name()).then_some(SchemaChange::ColumnRenamed)
            }
            Self::TableDrop(_) => Some(SchemaChange::TableDropped),
            Self::TableRename(intent) => {
                (intent.table() != intent.new_name()).then_some(SchemaChange::TableRenamed)
            }
            #[cfg(feature = "ace16-create")]
            Self::FirstTable(intent) => Some(SchemaChange::FirstTableCreated {
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
        if raw.starts_with(b"TCAT") {
            return NativeAddTableIntent::decode(raw, budget, cancelled)
                .map(Self::AddTable)
                .map_err(Cause::AddTableIntent);
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
