//! Closed sealed-plan composition behind the existing staged options owner.
use super::super::Cause;
use tablecraft_ace::{
    NativeImage, RecoverableFieldHistoryDatabaseOptionsPlan, RecoverableNativeDatabaseOptionsPlan,
};
use tablecraft_core::limits::ResourceBudget;
use tablecraft_io::journal::{
    DurableStorage, JournalError, PreparedResizableImage, ResizableDurableStorage,
};

pub(in crate::editing::offline) enum Plan {
    Creator(RecoverableNativeDatabaseOptionsPlan),
    FieldHistory(RecoverableFieldHistoryDatabaseOptionsPlan),
}

impl Plan {
    pub(super) fn before_bytes(&self) -> &[u8] {
        match self {
            Self::Creator(plan) => plan.before_bytes(),
            Self::FieldHistory(plan) => plan.before_bytes(),
        }
    }

    pub(super) fn requalify_with_budgets(
        self,
        fresh: NativeImage,
        native: &mut ResourceBudget,
        proof: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<Self, Cause> {
        match self {
            Self::Creator(plan) => plan
                .requalify_with_budgets(fresh, native, proof, cancelled)
                .map(Self::Creator)
                .map_err(Cause::DatabaseOptionsIntent),
            Self::FieldHistory(plan) => plan
                .requalify_with_budgets(fresh, native, proof, cancelled)
                .map(Self::FieldHistory)
                .map_err(Cause::FieldHistoryDatabaseOptionsIntent),
        }
    }

    pub(in crate::editing::offline) fn encode_intent(
        &self,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<Vec<u8>, Cause> {
        match self {
            Self::Creator(plan) => plan
                .intent()
                .encode(budget, cancelled)
                .map_err(Cause::DatabaseOptionsIntent),
            Self::FieldHistory(plan) => plan
                .intent()
                .encode(budget, cancelled)
                .map_err(Cause::FieldHistoryDatabaseOptionsIntent),
        }
    }

    pub(super) fn prepare<'a, D: ResizableDurableStorage, J: DurableStorage>(
        self,
        target: &'a mut D,
        journal: &'a mut J,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<PreparedResizableImage<'a, D, J>, JournalError> {
        match self {
            Self::Creator(plan) => plan.prepare(target, journal, budget, cancelled),
            Self::FieldHistory(plan) => plan.prepare(target, journal, budget, cancelled),
        }
    }

    pub(in crate::editing::offline) fn commit<D: ResizableDurableStorage, J: DurableStorage>(
        self,
        target: &mut D,
        journal: &mut J,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<(), Cause> {
        self.prepare(target, journal, budget, cancelled)
            .map_err(Cause::Prepare)?
            .commit(budget, cancelled)
            .map_err(Cause::Commit)
    }
}
