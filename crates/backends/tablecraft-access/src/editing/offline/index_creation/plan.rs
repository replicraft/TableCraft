//! Actual old/fresh index originals share V3 staging, never a decoder fallback.
use super::super::Cause;
use tablecraft_ace::{
    NativeImage, RecoverableNativeFreshIndexCreatePlan, RecoverableNativeIndexCreatePlan,
};
use tablecraft_core::limits::ResourceBudget;
use tablecraft_io::journal::DurableStorage;
use tablecraft_io::journal::ResizableDurableStorage;

pub(in crate::editing::offline) enum Plan {
    Original(RecoverableNativeIndexCreatePlan),
    Fresh(RecoverableNativeFreshIndexCreatePlan),
}

impl Plan {
    pub(super) fn table(&self) -> &str {
        match self {
            Self::Original(plan) => plan.table(),
            Self::Fresh(plan) => plan.table(),
        }
    }

    pub(super) fn column(&self) -> &str {
        match self {
            Self::Original(plan) => plan.column(),
            Self::Fresh(plan) => plan.column(),
        }
    }

    pub(super) fn index_name(&self) -> &str {
        match self {
            Self::Original(plan) => plan.index_name(),
            Self::Fresh(plan) => plan.index_name(),
        }
    }

    pub(super) fn indexed_rows(&self) -> usize {
        match self {
            Self::Original(plan) => plan.indexed_rows(),
            Self::Fresh(plan) => plan.indexed_rows(),
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
                .map_err(Cause::IndexCreationIntent),
            Self::Fresh(plan) => plan
                .requalify_with_budgets(image, native, proof, cancelled)
                .map(Self::Fresh)
                .map_err(Cause::FreshIndexCreationIntent),
        }
    }

    pub(in crate::editing::offline) fn encode_intent(
        &self,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<Vec<u8>, Cause> {
        match self {
            Self::Original(plan) => plan
                .intent()
                .encode(budget, cancelled)
                .map_err(Cause::IndexCreationIntent),
            Self::Fresh(plan) => plan
                .intent()
                .encode(budget, cancelled)
                .map_err(Cause::FreshIndexCreationIntent),
        }
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
            Self::Fresh(plan) => plan.prepare_retained(target, journal, budget, cancelled),
        }
        .map_err(Cause::Prepare)?;
        prepared.commit(budget, cancelled).map_err(Cause::Commit)
    }
}
