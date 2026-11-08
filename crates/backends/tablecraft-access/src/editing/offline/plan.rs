//! Private action composition; row and schema operations retain distinct intents.
use super::{Cause, QueryChange, SchemaChange};
use crate::editing::journal::NativeJournalInsertPlan;
#[cfg(feature = "ace16-create")]
use tablecraft_ace::{
    AddTablePlan, FirstTablePlan, NativeAddColumnIntent, NativeAddTableIntent,
    NativeFirstTableIntent, RecoverableAddColumnPlan,
};
use tablecraft_ace::{
    NativeClockInsertIntent, NativeCompleteInsertIntent, NativeDeleteIntent, NativeEofInsertIntent,
    NativeInsertIntent, NativeOmittedInsertIntent, NativeQueryExpressionIntent,
    NativeTableRenameIntent, NativeUpdateIntent, PreparedNativeUpdate, RecoverableNativeDeletePlan,
    RecoverableNativeEditPlan, RecoverableNativeQueryExpressionPlan,
    RecoverableNativeTableRenamePlan,
};
use tablecraft_core::limits::ResourceBudget;
use tablecraft_io::journal::{DurableStorage, ResizableDurableStorage};

pub(super) enum MutationPlan {
    QueryExpression(RecoverableNativeQueryExpressionPlan),
    #[cfg(feature = "ace16-create")]
    AddColumn(RecoverableAddColumnPlan),
    TableRename(RecoverableNativeTableRenamePlan),
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
            Self::QueryExpression(plan) => plan
                .intent()
                .encode(budget, cancelled)
                .map_err(Cause::QueryExpressionIntent),
            #[cfg(feature = "ace16-create")]
            Self::AddColumn(plan) => plan
                .intent()
                .encode(budget, cancelled)
                .map_err(Cause::AddColumnIntent),
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
            Self::QueryExpression(plan) => {
                let prepared = plan
                    .prepare(target, journal, budget, cancelled)
                    .map_err(Cause::Prepare)?;
                return prepared.commit(budget, cancelled).map_err(Cause::Commit);
            }
            #[cfg(feature = "ace16-create")]
            Self::AddColumn(plan) => {
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
    QueryExpression(NativeQueryExpressionIntent),
    #[cfg(feature = "ace16-create")]
    AddColumn(NativeAddColumnIntent),
    TableRename(NativeTableRenameIntent),
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
    pub(super) fn schema_change(&self) -> Option<SchemaChange> {
        match self {
            #[cfg(feature = "ace16-create")]
            Self::AddColumn(_) => Some(SchemaChange::ColumnAdded),
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
        if raw.starts_with(b"TCQE") {
            return NativeQueryExpressionIntent::decode(raw, budget, cancelled)
                .map(Self::QueryExpression)
                .map_err(Cause::QueryExpressionIntent);
        }
        #[cfg(feature = "ace16-create")]
        if raw.starts_with(b"TCAC") {
            return NativeAddColumnIntent::decode(raw, budget, cancelled)
                .map(Self::AddColumn)
                .map_err(Cause::AddColumnIntent);
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

        let decoded = if raw.starts_with(b"TCND") {
            NativeDeleteIntent::decode(raw, budget, cancelled).map(Self::Delete)
        } else if raw.starts_with(b"TCNA0005") {
            NativeClockInsertIntent::decode(raw, budget, cancelled).map(Self::ClockInsert)
        } else if raw.starts_with(b"TCNA0004") {
            NativeCompleteInsertIntent::decode(raw, budget, cancelled).map(Self::CompleteInsert)
        } else if raw.starts_with(b"TCNA0003") {
            NativeEofInsertIntent::decode(raw, budget, cancelled).map(Self::EofInsert)
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
