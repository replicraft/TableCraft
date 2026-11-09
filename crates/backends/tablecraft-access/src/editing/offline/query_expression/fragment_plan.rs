//! One staging owner for two explicitly distinct, frozen request languages.
use super::super::Cause;
use tablecraft_ace::{
    NativeImage, RecoverableNativeQueryExpressionEditsPlan, RecoverableNativeQueryExpressionPlan,
};
use tablecraft_core::limits::ResourceBudget;
use tablecraft_io::journal::DurableStorage;

pub(in crate::editing::offline) enum QueryExpressionPlan {
    Fitting(RecoverableNativeQueryExpressionPlan),
    Fragments(RecoverableNativeQueryExpressionEditsPlan),
}

impl QueryExpressionPlan {
    pub(super) fn query(&self) -> &str {
        match self {
            Self::Fitting(plan) => plan.query(),
            Self::Fragments(plan) => plan.query(),
        }
    }

    pub(super) fn native_id(&self) -> i32 {
        match self {
            Self::Fitting(plan) => plan.native_id(),
            Self::Fragments(plan) => plan.native_id(),
        }
    }

    pub(super) fn changed(&self) -> bool {
        match self {
            Self::Fitting(plan) => plan.changed(),
            Self::Fragments(plan) => plan.changed(),
        }
    }

    pub(super) fn before_bytes(&self) -> &[u8] {
        match self {
            Self::Fitting(plan) => plan.before_bytes(),
            Self::Fragments(plan) => plan.before_bytes(),
        }
    }

    pub(super) fn requalify(
        self,
        image: NativeImage,
        native: &mut ResourceBudget,
        proof: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<Self, Cause> {
        match self {
            Self::Fitting(plan) => plan
                .requalify_with_budgets(image, native, proof, cancelled)
                .map(Self::Fitting)
                .map_err(Cause::QueryExpressionQualification),
            Self::Fragments(plan) => plan
                .requalify_with_budgets(image, native, proof, cancelled)
                .map(Self::Fragments)
                .map_err(Cause::QueryExpressionEditsIntent),
        }
    }

    pub(in crate::editing::offline) fn encode_intent(
        &self,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<Vec<u8>, Cause> {
        match self {
            Self::Fitting(plan) => plan
                .intent()
                .encode(budget, cancelled)
                .map_err(Cause::QueryExpressionIntent),
            Self::Fragments(plan) => plan
                .intent()
                .encode(budget, cancelled)
                .map_err(Cause::QueryExpressionEditsIntent),
        }
    }

    pub(in crate::editing::offline) fn commit<D: DurableStorage, J: DurableStorage>(
        self,
        target: &mut D,
        journal: &mut J,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<(), Cause> {
        match self {
            Self::Fitting(plan) => plan
                .prepare(target, journal, budget, cancelled)
                .map_err(Cause::Prepare)?
                .commit(budget, cancelled)
                .map_err(Cause::Commit),
            Self::Fragments(plan) => plan
                .prepare(target, journal, budget, cancelled)
                .map_err(Cause::Prepare)?
                .commit(budget, cancelled)
                .map_err(Cause::Commit),
        }
    }
}
