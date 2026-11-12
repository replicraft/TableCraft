//! Exact ordinary/unique DROP originals share storage mechanics, never admission.
use super::super::Cause;
use tablecraft_ace::{
    NativeImage, RecoverableNativeIndexDropPlan, RecoverableNativeUniqueLongIndexDropPlan,
};
use tablecraft_core::limits::ResourceBudget;
use tablecraft_io::journal::{DurableStorage, ResizableDurableStorage};

pub(in crate::editing::offline) enum Plan {
    Original(RecoverableNativeIndexDropPlan),
    UniqueLong(RecoverableNativeUniqueLongIndexDropPlan),
}

impl Plan {
    pub(super) fn table(&self) -> &str {
        match self {
            Self::Original(plan) => plan.table(),
            Self::UniqueLong(plan) => plan.table(),
        }
    }

    pub(super) fn index_name(&self) -> &str {
        match self {
            Self::Original(plan) => plan.index_name(),
            Self::UniqueLong(plan) => plan.index_name(),
        }
    }

    pub(super) fn retired_pages(&self) -> usize {
        match self {
            Self::Original(plan) => plan.retired_pages(),
            Self::UniqueLong(plan) => plan.retired_pages(),
        }
    }

    pub(super) fn requalify_with_budgets(
        self,
        image: NativeImage,
        native: &mut ResourceBudget,
        proof: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<Self, Cause> {
        match self {
            Self::Original(plan) => plan
                .requalify_with_budgets(image, native, proof, cancelled)
                .map(Self::Original)
                .map_err(Cause::IndexDropIntent),
            Self::UniqueLong(plan) => plan
                .requalify_with_budgets(image, native, proof, cancelled)
                .map(Self::UniqueLong)
                .map_err(Cause::IndexDropIntent),
        }
    }

    pub(in crate::editing::offline) fn encode_intent(
        &self,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<Vec<u8>, Cause> {
        match self {
            Self::Original(plan) => plan.intent().encode(budget, cancelled),
            Self::UniqueLong(plan) => plan.intent().encode(budget, cancelled),
        }
        .map_err(Cause::IndexDropIntent)
    }

    pub(in crate::editing::offline) fn commit<D: ResizableDurableStorage, J: DurableStorage>(
        self,
        target: &mut D,
        journal: &mut J,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<(), Cause> {
        let prepared = match self {
            Self::Original(plan) => plan.prepare_retained(target, journal, budget, cancelled),
            Self::UniqueLong(plan) => plan.prepare_retained(target, journal, budget, cancelled),
        }
        .map_err(Cause::Prepare)?;
        prepared.commit(budget, cancelled).map_err(Cause::Commit)
    }
}
