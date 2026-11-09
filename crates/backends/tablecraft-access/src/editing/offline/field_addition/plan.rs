//! Closed typed/first/repeated column request composition, never decoder fallback.
use super::super::Cause;
use tablecraft_ace::{
    NativeImage, RecoverableDefaultZeroAddColumnPlan, RecoverableFieldAdditionAfterOffPlan,
    RecoverableFieldAdditionPlan, RecoverableRepeatedDefaultZeroAddColumnPlan,
};
use tablecraft_core::limits::ResourceBudget;
use tablecraft_io::journal::{DurableStorage, ResizableDurableStorage};

pub(in crate::editing::offline) enum Plan {
    First(RecoverableDefaultZeroAddColumnPlan),
    Repeated(RecoverableRepeatedDefaultZeroAddColumnPlan),
    Typed(RecoverableFieldAdditionPlan),
    AfterOff(RecoverableFieldAdditionAfterOffPlan),
}

impl Plan {
    pub(super) fn before_bytes(&self) -> &[u8] {
        match self {
            Self::First(plan) => plan.before_bytes(),
            Self::Repeated(plan) => plan.before_bytes(),
            Self::Typed(plan) => plan.before_bytes(),
            Self::AfterOff(plan) => plan.before_bytes(),
        }
    }

    pub(super) fn table(&self) -> &str {
        match self {
            Self::First(plan) => plan.table(),
            Self::Repeated(plan) => plan.table(),
            Self::Typed(plan) => plan.table(),
            Self::AfterOff(plan) => plan.table(),
        }
    }

    pub(super) fn column(&self) -> &str {
        match self {
            Self::First(plan) => plan.column(),
            Self::Repeated(plan) => plan.column(),
            Self::Typed(plan) => plan.column(),
            Self::AfterOff(plan) => plan.column(),
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
            Self::First(plan) => plan
                .requalify_with_budgets(fresh, native, proof, cancelled)
                .map(Self::First)
                .map_err(Cause::DefaultZeroAddColumnIntent),
            Self::Repeated(plan) => plan
                .requalify_with_budgets(fresh, native, proof, cancelled)
                .map(Self::Repeated)
                .map_err(Cause::RepeatedDefaultZeroAddColumnIntent),
            Self::Typed(plan) => plan
                .requalify_with_budgets(fresh, native, proof, cancelled)
                .map(Self::Typed)
                .map_err(Cause::FieldAdditionIntent),
            Self::AfterOff(plan) => plan
                .requalify_with_budgets(fresh, native, proof, cancelled)
                .map(Self::AfterOff)
                .map_err(Cause::FieldAdditionAfterOffIntent),
        }
    }

    pub(in crate::editing::offline) fn encode_intent(
        &self,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<Vec<u8>, Cause> {
        match self {
            Self::First(plan) => plan
                .intent()
                .encode(budget, cancelled)
                .map_err(Cause::DefaultZeroAddColumnIntent),
            Self::Repeated(plan) => plan
                .intent()
                .encode(budget, cancelled)
                .map_err(Cause::RepeatedDefaultZeroAddColumnIntent),
            Self::Typed(plan) => plan
                .intent()
                .encode(budget, cancelled)
                .map_err(Cause::FieldAdditionIntent),
            Self::AfterOff(plan) => plan
                .intent()
                .encode(budget, cancelled)
                .map_err(Cause::FieldAdditionAfterOffIntent),
        }
    }

    pub(in crate::editing::offline) fn commit<D: ResizableDurableStorage, J: DurableStorage>(
        self,
        target: &mut D,
        journal: &mut J,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<(), Cause> {
        // All profiles remain checked V2 operations even when their current
        // source and replacement happen to have equal lengths.
        let prepared = match self {
            Self::First(plan) => plan.prepare(target, journal, budget, cancelled),
            Self::Repeated(plan) => plan.prepare(target, journal, budget, cancelled),
            Self::Typed(plan) => plan.prepare(target, journal, budget, cancelled),
            Self::AfterOff(plan) => plan.prepare(target, journal, budget, cancelled),
        }
        .map_err(Cause::Prepare)?;
        prepared.commit(budget, cancelled).map_err(Cause::Commit)
    }
}
